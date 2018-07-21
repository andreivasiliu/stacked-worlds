Problem: Creating long chains reduces frame-rate to a crawl.

Solution: Record time-deltas and print them to stdout every frame.

Solution coolness-factor: Too low.

Solution #2: Metrics and graphs.

## Need metrics for
* duration of draw step
* duration of update step
* duration of update's physics step
* duration of update's physics' update step
* count of physical objects (across all rooms)
* count of drawable (or maybe all) entities

## Option 1: Store values, push all values to stream on perf_counter.update()

* perf_counter.register(Counter::Blah1)
* perf_counter.register(Counter::Blah2)
* perf_counter.set(Counter::Blah1, 10) - store/replace 10 in perf_counter for blah
  * counter has: Blah1: Some(10), Blah2: None
  * stream is empty
* perf_counter.update()
  * counter has: Blah1: None, Blah2: None
  * stream is: `[(time_stamp, Counters(Blah1: Some(10), Blah2: None))]`
* instead of 10, maybe store 10/1 so we have the average
  * another call to .set(15) changes this to: Some(10+15, 2)
  
Pros:
* Very quick to update

Cons:
* Information can be lost 
* Cannot be used by systems in parallel
  * Maybe retrieve with ReadExpect, but give write access via mutex?

## Option 2: Push time_stamps every time
* perf_counter.push(Counter::Blah1, 10)
  * stream is: `[(time_stamp, Counter::Blah1, 10)]`
* perf_counter.push(Counter::Blah2, 20)
  * stream is: `[(time_stamp, Counter::Blah1, 10), (time_stamp, Counter::Blah2, 20)]`
  
Cons:
* Have to query for time_stamps every time
* Values that update more than once per tick will have less storage time-wise

## Option 3: Decentralized
* Every system can have a perf_counter
* perf_counter.rs does not store references
* The draw system queries all perf_counters (it has to know all of them)

Cons:
* Have to get the time_stamp from somewhere
  * Perhaps from a common read-only resource
  * Or perhaps it is fed by a system into all the counters
  
## Option 4: Decentralized, centrally aggregated
* Instead of PerfCounters have PerfCounters<T>
* The PerfCounters structure only has values, not streams
* Have perfcount.rs know all of them, and every update, merge them
* After merging, attach a time_stamp and push them to a PerfCounterStream

Choices:
* Does PerfCounters have:
  * A list of values (indexed by `enum Counter` as `usize`)
    * Requires preparing the size in advance somehow
    * Or knowing the size of the enum at compile-time
  * A map of values (keyed by `enum Counter`)
    * Requires many lookups
* Multiple values set in the same tick:
  * Get lost, only the last survives
  * Get aggregated via a pre-set rule (e.g. avg or max or min)
  * Get aggregated into multiple fixed rules (e.g. avg and max and min)

## Other random issues/ideas:
* How do I make separate systems have write access in parallel?
  * Maybe parametrize a counter-set (CounterSet<PhysicsSystem>), register as a specs resource
    * The PhysicsSystem system will ask for WriteExpect(CounterSet<PhysicsSystem>)
    * Requires knowing about and regularly polling all counter-sets in perfcount.rs
    * Will be hard to add new counters
* Maybe use the system class as a way to have more of the same thing:
  * `perf_counter.push::<PhysicsSystem>(10)`
