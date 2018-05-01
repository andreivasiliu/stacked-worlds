extern crate specs;

use specs::prelude::{VecStorage, System, WriteStorage, Entities, Join};
use std::marker::PhantomData;

#[derive(Component, Debug, Serialize, Deserialize, Clone)]
#[storage(VecStorage)]
pub struct Animation<T>
where T: Sync + Send + 'static  {
    pub current: u32,
    pub limit: u32,

    #[serde(skip)]
    phantom: PhantomData<T>,
}

impl<T> Animation<T>
where T: Sync + Send + 'static {
    pub fn new(limit: u32) -> Self {
        Animation {
            current: 0,
            limit,
            phantom: PhantomData::default(),
        }
    }
}

pub struct UpdateAnimations;

impl <'a> System<'a> for UpdateAnimations {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Animation<RoomAnimation>>
    );

    fn run(&mut self, (entities, mut room_animations): Self::SystemData) {
        for (_entity, animation) in (&*entities, &mut room_animations).join() {
            if animation.current < animation.limit {
                animation.current += 1;
            } else {
                // remove animation from entity
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct RoomAnimation {}
