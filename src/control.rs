use specs::prelude::{System, Entities, ReadStorage, WriteStorage, Join};

use input::{PlayerController, Movement};
use physics::Acceleration;

pub struct ControlObjects;

impl <'a> System<'a> for ControlObjects {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, PlayerController>,
        WriteStorage<'a, Acceleration>,
    );

    fn run(&mut self, (entities, player_controller, mut accelerations): Self::SystemData) {
        for (_entity, player_controller, mut acceleration) in (&*entities, &player_controller, &mut accelerations).join() {
            let speed = 100000.0;

            let (x, y) = match player_controller.moving {
                Movement::Left => (-1.0 * speed, 0.0),
                Movement::Right => (1.0 * speed, 0.0),
                Movement::None => (0.0, 0.0),
            };

            acceleration.x = x;
            acceleration.y = y;
        }
    }
}
