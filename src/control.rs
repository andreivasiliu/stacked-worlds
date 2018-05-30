use specs::prelude::{System, VecStorage, DenseVecStorage, Entities, ReadExpect, ReadStorage, WriteStorage, Join};
use nalgebra::Vector2;

use UpdateDeltaTime;
use input::{PlayerController, Movement};
use physics::{Velocity, Force, Aim, CollisionSet, InRoom, RevoluteJoint};
use draw::{Position, Shape, ShapeClass};
use specs::LazyUpdate;
use specs::world::Index;
use saveload::DestroyEntity;
use specs::saveload::U64Marker;

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
        let jump_speed = 300.0;

        for (_entity, mut force) in (&*entities, &mut forces).join() {
            force.continuous = (0.0, 0.0);
            force.impulse = (0.0, 0.0);
        }

        for (_entity, player_controller, mut force) in (&*entities, &player_controller, &mut forces).join() {
            let (x, y) = match player_controller.moving {
                Movement::Left => (-1.0 * speed, 0.0),
                Movement::Right => (1.0 * speed, 0.0),
                Movement::None => (0.0, 0.0),
            };

            force.continuous = (force.continuous.0 + x, force.continuous.1 + y);
        }

        for (_entity, player_controller, mut jump, collision_set, mut force) in (&*entities, &player_controller, &mut jumps, &collision_sets, &mut forces).join() {
            if player_controller.jumping && collision_set.time_since_collision < 0.2 && jump.cooldown <= 0.0 {
                let jump_direction = -Vector2::new(collision_set.last_collision_normal.0,
                                                   collision_set.last_collision_normal.1).normalize();
                let jump_impulse = jump_direction * jump_speed;

                force.impulse = (
                    force.impulse.0 + jump_impulse.x,
                    force.impulse.1 + jump_impulse.y
                );

                jump.cooldown += 0.25;
            }
        }
    }
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(DenseVecStorage)]
pub struct ChainLink {
    // TODO: Maybe figure out how to move these to an Animation component
    pub creation_animation: f64,
    pub destruction_animation: f64,
    pub expire: bool,
    pub next_link: Option<Index>,
}

pub struct FireHook;

impl <'a> System<'a> for FireHook {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, PlayerController>,
        WriteStorage<'a, ChainLink>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Velocity>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, Aim>,
        ReadExpect<'a, LazyUpdate>,
    );

    fn run(&mut self, (entities, mut player_controllers, mut chain_links,
        positions, velocities, in_rooms, aims, lazy_update): Self::SystemData)
    {
        for (entity, mut player_controller, position, velocity, in_room, aim) in (&*entities, &mut player_controllers, &positions, &velocities, &in_rooms, &aims).join() {
            if player_controller.hooking && !player_controller.hook_established {
                // Create grappling hook chain if possible
                let source = Vector2::new(position.x, position.y);
                let (target, target_entity) = if let (Some(point), Some(entity)) = (aim.aiming_at_point, aim.aiming_at_entity) {
                    (Vector2::new(point.0, point.1), entity)
                } else {
                    continue
                };

                let chain_vector = target - source;
                let direction = chain_vector.normalize();
                let link_count = (chain_vector / 10.0).norm().floor();

                let mut linked_to_entity = target_entity.id();
                let mut next_link = None;
                let mut creation_animation = 0.1;

                for i in (2..=link_count as i32).rev() {
                    let chain_link_position = source + direction * 10.0 * (i as f64);

                    let new_entity = lazy_update.create_entity(&entities)
                        .with(Position { x: chain_link_position.x, y: chain_link_position.y })
                        .with(Shape { size: 3.0, class: ShapeClass::ChainLink })
                        .with(Velocity { .. *velocity })
                        .with(InRoom { .. *in_room })
                        .with(ChainLink { next_link, creation_animation, .. ChainLink::default() })
                        .with(RevoluteJoint { linked_to_entity, multibody_link: false })
                        .marked::<U64Marker>()
                        .build();

                    linked_to_entity = new_entity.id();
                    next_link = Some(linked_to_entity);
                    creation_animation += 0.02;
                }

                lazy_update.insert(entity, RevoluteJoint { linked_to_entity, multibody_link: false });
                lazy_update.insert(entity, ChainLink { next_link: Some(linked_to_entity), .. ChainLink::default() });

                player_controller.hook_established = true;
            } else if !player_controller.hooking && player_controller.hook_established {
                // Destroy grappling hook chain

                let mut some_next_entity = if let Some(chain_link) = chain_links.get(entity) {
                    chain_link.next_link
                } else {
                    continue
                };

                let mut destruction_animation = 0.5;

                while let Some(next_entity) = some_next_entity {
                    let mut chain_link = chain_links.get_mut(entities.entity(next_entity));

                    some_next_entity = chain_link.and_then(|chain_link| {
                        chain_link.expire = true;
                        chain_link.destruction_animation = destruction_animation;
                        destruction_animation += 0.04;
                        chain_link.next_link
                    });
                }

                lazy_update.remove::<RevoluteJoint>(entity);
                lazy_update.remove::<ChainLink>(entity);

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
        WriteStorage<'a, ChainLink>,
        ReadExpect<'a, LazyUpdate>,
    );

    fn run(&mut self, (entities, delta_time, mut jumps, mut chain_links, lazy_update): Self::SystemData) {
        for (_entity, mut jump) in (&*entities, &mut jumps).join() {
            if jump.cooldown > 0.0 {
                jump.cooldown = (jump.cooldown - delta_time.dt).max(0.0);
            }
        }

        for (entity, mut chain_link) in (&*entities, &mut chain_links).join() {
            if chain_link.creation_animation > 0.0 {
                chain_link.creation_animation = (chain_link.creation_animation - delta_time.dt).max(0.0);
            }
            if chain_link.destruction_animation > 0.0 {
                chain_link.destruction_animation = (chain_link.destruction_animation - delta_time.dt).max(0.0);

                if chain_link.expire && chain_link.destruction_animation == 0.0 {
                    lazy_update.insert(entity, DestroyEntity);
                }
            }
        }
    }
}
