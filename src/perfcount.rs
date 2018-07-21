/// Performance Counters
///
/// Measuring time:
///  * `perf_count.enter(Counter::Physics)` - stores a timestamp
///  * `perf_count.leave(Counter::Physics)` - saves the timestamp delta to the counter
///
/// Measuring occurrences:
///  * `perf_count.increment(Counter::ObjectsCreated)` - increases the counter's value by 1
///  * `perf_count.increment_by(Counter::ObjectsCreated, 10)` - increases the counter's value by 10
///
/// Measuring total count:
///  * `perf_count.set(Counter::ObjectsInWorld, 10)` - sets the counter's value to `10`
///
/// Each game update, the accumulated values are pushed to a stream, and all
/// counters are reset to None.
///
/// Note that since it's possible to have more update events than draw
/// events (but not vice-versa), some perf_counter updates will push None
/// values for that timestamp.

use std::marker::PhantomData;
use std::collections::{HashMap, VecDeque};
use std::collections::vec_deque::Iter;
use std::time::{Instant};
use std::time::Duration;

pub mod setup {
    use super::*;

    use specs::prelude::{World, System, WriteExpect};

    use physics::PhysicsSystem;
    use draw::DrawBalls;

    pub fn add_resources(world: &mut World) {
        world.add_resource(PerfCounterStream::new());

        world.add_resource(PerfCounters::<GlobalCounters>::new());
        world.add_resource(PerfCounters::<PhysicsSystem>::new());
        world.add_resource(PerfCounters::<DrawBalls>::new());
    }

    pub struct MergeCounters;

    impl <'a> System<'a> for MergeCounters {
        type SystemData = (
            WriteExpect<'a, PerfCounterStream>,
            WriteExpect<'a, PerfCounters<GlobalCounters>>,
            WriteExpect<'a, PerfCounters<PhysicsSystem>>,
            WriteExpect<'a, PerfCounters<DrawBalls<'static>>>,
        );

        fn run(&mut self, (mut perf_counter_stream, mut global_counters, mut physics_system,
            mut draw_balls)
        : Self::SystemData) {
            global_counters.add_from(&*physics_system);
            global_counters.add_from(&*draw_balls);

            // FIXME: Don't push counters if too soon
            perf_counter_stream.push_perf_counters(&global_counters);

            // println!("PhysicsSystem counters: {:?}", global_counters.values);

            // println!("Lines: {:?}", lines);

            global_counters.reset_all();
            physics_system.reset_all();
            draw_balls.reset_all();
        }
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Copy, Clone)]
pub enum Counter {
    WorldUpdateDuration,
    WorldDrawDuration,
    WorldInputDuration,
    PhysicsSystemDuration,
    ObjectsCreated,

    // CounterTypeCount,
}

/// Used as PerfCounters<GlobalCounters> when no other T is available.
pub struct GlobalCounters;

#[derive(Debug, Default)]
pub struct PerfCounters<T> {
    values: HashMap<Counter, Option<f64>>,
    enter_timestamp: HashMap<Counter, Option<Instant>>,
//    _values2: [f64; Counter::CounterTypeCount as usize],

    /// Used to allow multiple types of PerfCounters to be registered with
    /// specs as separate resources.
    _phantom_data: PhantomData<T>
}

impl<T> PerfCounters<T> {
    pub fn new() -> Self {
        PerfCounters {
            values: HashMap::new(),
            enter_timestamp: HashMap::new(),

            _phantom_data: PhantomData::default(),
        }
    }

    pub fn add_from<O>(&mut self, other: &PerfCounters<O>) {
        for (key, value) in other.values.iter() {
            if value.is_some() {
                self.values.insert(*key, *value);
            }
        }
    }

    pub fn reset_all(&mut self) {
        for value in self.values.values_mut() {
            *value = None;
        }
    }

    pub fn set(&mut self, counter: Counter, value: f64) {
        self.values.insert(counter, Some(value));
    }

    pub fn enter(&mut self, counter: Counter) {
        self.enter_timestamp.insert(counter, Some(Instant::now()));
    }

    pub fn leave(&mut self, counter: Counter) {
        let enter_timestamp = self.enter_timestamp.get(&counter)
            .and_then(|time_stamp| *time_stamp)
            .expect("PerfCounters::leave() used without PerfCoutners::enter()");

        let time_delta = Instant::now().duration_since(enter_timestamp);

        self.values.insert(counter, Some(time_delta.to_seconds_f64()));
    }
}

#[derive(Debug)]
pub struct GraphExtents {
    pub left: Instant,
    pub right: Instant,
    pub top: f64,
    pub bottom: f64,
}

impl GraphExtents {
    pub fn relative_to_extents<'a>(&self, line: ((&'a Instant, f64), (&'a Instant, f64))) -> ((f64, f64), (f64, f64)) {
        let height = self.top - self.bottom;
        let width = self.right.duration_since(self.left).to_seconds_f64();

        let ((instant1, value1), (instant2, value2)) = line;

        let instant1 = self.right.duration_since(*instant1).to_seconds_f64() / width;
        let instant2 = self.right.duration_since(*instant2).to_seconds_f64() / width;
        let value1 = (value1 - self.bottom) / height;
        let value2 = (value2 - self.bottom) / height;

        ((instant1, value1), (instant2, value2))
    }
}

// FIXME: Turns out, a stream means we clone hashmaps quite often
// Change from HashMap to a normal array
pub struct PerfCounterStream {
    counter_stream: VecDeque<(Instant, HashMap<Counter, Option<f64>>)>,
}

impl PerfCounterStream {
    fn new() -> Self {
        PerfCounterStream {
            // 10 seconds worth of counters at 60 FPS
            counter_stream: VecDeque::with_capacity(10 * 60 + 1),
        }
    }

    /// Return true if less than 1/60 seconds have passed since the last
    /// counters were pushed until `now`.
    ///
    /// Useful to prevent pushing too many counters at framerates higher
    /// than 60, and to ensure that the stream always has at least 10
    /// seconds worth of counters.
    fn too_recent(&self, now: Instant) -> bool {
        if let Some((instant, _)) = self.counter_stream.get(0) {
            now.duration_since(*instant) < Duration::from_millis(1000 / 60)
        } else {
            false
        }
    }

    fn push_perf_counters<T>(&mut self, perf_counters: &PerfCounters<T>) {
        let now = Instant::now();

        // Drop counters that are not at least 1/60 seconds older than the last set
        if self.too_recent(now) {
            return;
        }

        self.counter_stream.push_front((now, perf_counters.values.clone()));
        while self.counter_stream.len() >= 10 * 60 {
            self.counter_stream.pop_back();
        }
    }

    pub fn graph_extents(&self, counters: &[Counter], time_axis_duration: Duration) -> Option<GraphExtents> {
        let mut left: Option<Instant> = None;
        let mut right = None;
        let mut top = 0.0;
        let mut bottom = 0.0;

        assert!(time_axis_duration > Duration::new(0, 0));

        for (instant, counter_values) in self.counter_stream.iter() {
            if let Some(left) = left {
                if *instant < left {
                    break;
                }
            }

            if right.is_none() {
                right = Some(*instant);
                left = Some(*instant - time_axis_duration);
            }

            for counter in counters.iter() {
                if let Some(value) = counter_values.get(counter).and_then(|value| *value) {
                    if top < value {
                        top = value;
                    }
                    if bottom > value {
                        bottom = value;
                    }
                }
            }
        }

        if let (Some(left), Some(right)) = (left, right) {
            Some(GraphExtents { left, right, top, bottom })
        } else {
            None
        }
    }

    pub fn iter_lines_for_counter<'a>(&'a self, counter: Counter) -> GraphLineIterator<'a> {
        let mut stream_deque_iter = self.counter_stream.iter();

        let mut last = None;

        loop {
            if let Some((instant, counters)) = stream_deque_iter.next() {
                if let Some(value) = counters.get(&counter).and_then(|value| *value) {
                    last = Some((instant, value));
                    break;
                }
            } else {
                break;
            }
        }

        GraphLineIterator {
            stream_deque_iter,
            last,
            counter,
        }
    }
}

pub struct GraphLineIterator<'a> {
    stream_deque_iter: Iter<'a, (Instant, HashMap<Counter, Option<f64>>)>,
    last: Option<(&'a Instant, f64)>,
    counter: Counter,
}

// Fun fact: In Python, this iterator, plus the  iter_lines_for_counter
// function, would be probably be just five lines of code.
impl<'a> Iterator for GraphLineIterator<'a> {
    type Item = ((&'a Instant, f64), (&'a Instant, f64));

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let last = match self.last {
            None => return None,
            Some(last) => last,
        };

        loop {
            if let Some((instant, counters)) = self.stream_deque_iter.next() {
                if let Some(value) = counters.get(&self.counter).and_then(|value| *value) {
                    let next = (instant, value);
                    self.last = Some(next);

                    return Some((last, next));
                }
            } else {
                break;
            }
        }

        None
    }
}

pub trait DurationSeconds {
    fn to_seconds_f64(&self) -> f64;
}

impl DurationSeconds for Duration {
    fn to_seconds_f64(&self) -> f64 {
        let seconds = self.as_secs() as f64;
        let nanos = self.subsec_nanos() as f64 / 1_000_000_000.0;

        seconds + nanos
    }
}