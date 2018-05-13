use specs::prelude::{System, VecStorage, Entities, ReadExpect, ReadStorage, WriteStorage, Join};
use nalgebra::Vector2;

use input::{PlayerController, Movement};
use physics::{Force, CollisionSet};
use UpdateDeltaTime;
use draw::Position;
use physics::Aim;

#[derive(Component, Debug, Default, Serialize, Deserialize, Copy, Clone, PartialEq)]
#[storage(VecStorage)]
pub struct Jump {
    pub cooldown: f64,
}

pub struct ControlObjects;

impl <'a> System<'a> for ControlObjects {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, PlayerController>,
        ReadStorage<'a, CollisionSet>,
        WriteStorage<'a, Force>,
        WriteStorage<'a, Jump>,
    );

    fn run(&mut self, (entities, player_controller, collision_sets, mut forces, mut jumps): Self::SystemData) {
        let speed = 100000.0;

        for (_entity, mut force) in (&*entities, &mut forces).join() {
            force.x = 0.0;
            force.y = 0.0;
        }

        for (_entity, player_controller, mut force) in (&*entities, &player_controller, &mut forces).join() {
            let (x, y) = match player_controller.moving {
                Movement::Left => (-1.0 * speed, 0.0),
                Movement::Right => (1.0 * speed, 0.0),
                Movement::None => (0.0, 0.0),
            };

            force.x += x;
            force.y += y;
        }

        for (_entity, player_controller, mut jump, collision_set, mut force) in (&*entities, &player_controller, &mut jumps, &collision_sets, &mut forces).join() {
            if player_controller.jumping && collision_set.time_since_collision < 0.2 && jump.cooldown <= 0.0 {
                let jump_direction = -Vector2::new(collision_set.last_collision_normal.0,
                                                   collision_set.last_collision_normal.1).normalize();
                let jump_force = jump_direction * speed * 100.0;

                // FIXME: This needs to be an impulse, not a force
                force.x += jump_force.x;
                force.y += jump_force.y;

                jump.cooldown += 0.2;
            }
        }
    }
}

pub struct FireHook;

impl <'a> System<'a> for FireHook {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, PlayerController>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Aim>,
    );

    fn run(&mut self, (entities, mut player_controllers, positions, aims): Self::SystemData) {
        for (_entity, mut player_controller, position, aim) in (&*entities, &mut player_controllers, &positions, &aims).join() {
            if player_controller.hooking && !player_controller.hook_established {
                // Create grappling hook chain if possible
                let source = Vector2::new(position.x, position.y);
                let target = if let Some(point) = aim.aiming_at_point {
                    Vector2::new(point.0, point.1)
                } else {
                    continue
                };

                let chain_vector = target - source;
                let _direction = chain_vector.normalize();
                let link_count = (chain_vector / 10.0).norm().floor();
                let link_vector = chain_vector / link_count;

                println!("Count: {}, vector: {:?}", link_count, link_vector);
                player_controller.hook_established = true;
            } else if !player_controller.hooking && player_controller.hook_established {
                // Destroy grappling hook chain
                player_controller.hook_established = false;
            }
        }
    }
}

pub struct UpdateCooldowns;

impl <'a> System<'a> for UpdateCooldowns {
    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, UpdateDeltaTime>,
        WriteStorage<'a, Jump>,
    );

    fn run(&mut self, (entities, delta_time, mut jumps): Self::SystemData) {
        for (_entity, mut jump) in (&*entities, &mut jumps).join() {
            if jump.cooldown > 0.0 {
                jump.cooldown = (jump.cooldown - delta_time.dt).max(0.0);
            }
        }
    }
}
