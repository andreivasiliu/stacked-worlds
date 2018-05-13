extern crate specs;
extern crate nphysics2d;
extern crate ncollide;

use specs::prelude::{WriteStorage, ReadStorage, VecStorage, DenseVecStorage, System, Entities, Join};
use nphysics2d::world::World;
use nalgebra::{Vector2, Translation2};
use nphysics2d::object::RigidBody;
use ncollide::shape::Ball;
use saveload::DestroyEntity;
use std::collections::HashMap;
use nphysics2d::object::RigidBodyHandle;
use nalgebra::Real;
use specs::world::Index;
use draw::Position;
use specs::prelude::Entity;
use draw::Size;
use ncollide::shape::Plane;
use nphysics2d::object::RigidBodyCollisionGroups;
use specs::prelude::ReadExpect;
use UpdateDeltaTime;
use nphysics2d::detection::constraint::Constraint;
use nphysics2d::object::WorldObject;
use ncollide::world::CollisionObject;


#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy)]
#[storage(VecStorage)]
pub struct Room;

/// Component that allows an object to physically interact with other objects in the same room
#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[storage(VecStorage)]
pub struct InRoom {
    pub room_entity: Index,
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(VecStorage)]
pub struct Velocity {
    pub x: f64,
    pub y: f64,
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(VecStorage)]
pub struct Force {
    pub x: f64,
    pub y: f64,
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(DenseVecStorage)]
pub struct Aim {
    pub aiming: bool,
    pub aiming_toward: (f64, f64),

    #[serde(skip)]
    pub aiming_at_entity: Option<Entity>,
    #[serde(skip)]
    pub aiming_at_point: Option<(f64, f64)>,
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(VecStorage)]
pub struct CollisionSet {
    pub colliding: bool,
    pub collision_normal: (f64, f64),
    pub last_collision_normal: (f64, f64),
    pub time_since_collision: f64,
}

struct PhysicalObject<N: Real> {
    body: RigidBodyHandle<N>,
}

/// Internal object for the physics system
struct PhysicalRoom<N: Real> {
    walls: [RigidBodyHandle<N>; 4],
}

pub struct PhysicsSystem<N: Real = f64> {
    world: World<N>,

    physical_objects: HashMap<Entity, PhysicalObject<N>>,
    physical_rooms: HashMap<Entity, PhysicalRoom<N>>,
}

trait GetEntity {
    fn get_entity(&self) -> Option<Entity>;
}

impl<P: ncollide::math::Point, M> GetEntity for CollisionObject<P, M, WorldObject<f64>> {
    fn get_entity(&self) -> Option<Entity> {
        if let WorldObject::RigidBody(ref rigid_body) = self.data {
            if let Some(entity_box) = rigid_body.borrow().user_data() {
                if let Some(entity) = entity_box.downcast_ref::<Entity>() {
                    return Some(*entity)
                }
            }
        }
        None
    }
}

impl PhysicsSystem<f64> {
    pub fn new() -> Self {
        let mut world = World::new();

        world.set_gravity(Vector2::new(0.0, 500.0));

        PhysicsSystem {
            world,
            physical_objects: HashMap::new(),
            physical_rooms: HashMap::new(),
        }
    }
}

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Room>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, Size>,
        //ReadStorage<'a, Shape>, // eventually...
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Force>,
        WriteStorage<'a, Aim>,
        //WriteStorage<'a, Angle>, // eventually...
        WriteStorage<'a, CollisionSet>,
        ReadStorage<'a, DestroyEntity>,
        ReadExpect<'a, UpdateDeltaTime>,
    );

    fn run(&mut self, (entities, rooms, in_rooms, sizes, mut positions, mut velocities, forces, mut aims, mut collision_sets, destroy_entities, delta_time): Self::SystemData) {
        for (entity, _room, size) in (&*entities, &rooms, &sizes).join() {
            let world = &mut self.world;
            let physical_rooms = &mut self.physical_rooms;

            let _physical_room = physical_rooms.entry(entity)
                .or_insert_with(|| {
                    let south_wall = Plane::new(Vector2::new(0.0, 1.0)); // pointing north
                    let north_wall = Plane::new(Vector2::new(0.0, -1.0)); // pointing south
                    let west_wall = Plane::new(Vector2::new(1.0, 0.0)); // pointing east
                    let east_wall = Plane::new(Vector2::new(-1.0, 0.0)); // pointing west

                    let mut south_wall = RigidBody::new_static(south_wall, 0.5, 0.5);
                    let mut north_wall = RigidBody::new_static(north_wall, 0.5, 0.5);
                    north_wall.append_translation(&Translation2::new(0.0, size.height));
                    let mut west_wall = RigidBody::new_static(west_wall, 0.5, 0.5);
                    let mut east_wall = RigidBody::new_static(east_wall, 0.5, 0.5);
                    east_wall.append_translation(&Translation2::new(size.width, 0.0));

                    // Set a collision group so that this room's walls and the objects inside can
                    // only collide with each other.
                    let collision_group = entity.id() as usize; // FIXME: :(
                    let mut collision_groups = RigidBodyCollisionGroups::new_static();
                    collision_groups.set_membership(&[collision_group]);
                    collision_groups.set_whitelist(&[collision_group]);


                    for wall in [&mut south_wall, &mut north_wall, &mut west_wall, &mut east_wall].iter_mut() {
                        wall.set_collision_groups(collision_groups);
                        wall.set_user_data(Some(Box::new(entity)));
                    }

                    let south_wall = world.add_rigid_body(south_wall);
                    let north_wall = world.add_rigid_body(north_wall);
                    let west_wall = world.add_rigid_body(west_wall);
                    let east_wall = world.add_rigid_body(east_wall);

                    let walls = [south_wall, north_wall, west_wall, east_wall];

                    println!("Created room {:?}, group {}", entity, collision_group);

                    PhysicalRoom {
                        walls
                    }
                });
        }

        for (entity, in_room, position, velocity) in (&*entities, &in_rooms, &mut positions, &mut velocities).join() {
            let world = &mut self.world;
            let physical_objects = &mut self.physical_objects;

            let physical_object = physical_objects.entry(entity)
                .or_insert_with(|| {
                    let mut body = RigidBody::new_dynamic(Ball::new(10.0), 1.0, 0.3, 0.5);

                    let collision_group = in_room.room_entity as usize; // FIXME: :(
                    let mut collision_groups = RigidBodyCollisionGroups::new_dynamic();
                    collision_groups.set_membership(&[collision_group]);
                    collision_groups.set_whitelist(&[collision_group]);
                    body.set_collision_groups(collision_groups);

                    body.set_translation(Translation2::new(position.x, position.y));
                    body.set_lin_vel(Vector2::new(velocity.x, velocity.y));

                    body.set_user_data(Some(Box::new(entity)));

                    let body = world.add_rigid_body(body);

                    println!("Created object {:?}, group {}", entity, collision_group);

                    PhysicalObject {
                        body
                    }
                });

            let rigid_body = physical_object.body.borrow();

            let physical_position = rigid_body.position_center();
            position.x = physical_position.x;
            position.y = physical_position.y;

            let physical_velocity = rigid_body.lin_vel();
            velocity.x = physical_velocity.x;
            velocity.y = physical_velocity.y;
        }

        for (entity, position, mut aim) in (&*entities, &positions, &mut aims).join() {
            use nalgebra::{self, Point2};
            use ncollide::query::Ray;

            let direction = Vector2::new(aim.aiming_toward.0, aim.aiming_toward.1).normalize();
            // FIXME: Find the proper trait for direction.is_zero()
            if direction == nalgebra::zero() {
                continue;
            }

            if let Some(mut physical_object) = self.physical_objects.get(&entity) {
                let rigid_body = physical_object.body.borrow();
                let source = Point2::new(position.x, position.y);
                let ray = Ray::new(source, direction);
                let mut collision_groups = rigid_body.collision_groups().as_collision_groups();

                for interference in self.world.collision_world().interferences_with_ray(&ray, collision_groups) {
                    let (collision_object, ray_intersection) = interference;
                    if let Some(intersected_entity) = collision_object.get_entity() {
                        if entity != intersected_entity {
                            let intersection_point = source + direction * ray_intersection.toi;

                            // FIXME: there needs to be a system that resets these
                            aim.aiming_at_point = Some((intersection_point.x, intersection_point.y));
                            aim.aiming_at_entity = Some(intersected_entity);

                            break;
                        }
                    }
                }
            }
        }

        for (entity, force) in (&*entities, &forces).join() {
            if let Some(mut physical_object) = self.physical_objects.get(&entity) {
                let force = Vector2::new(force.x, force.y);
                let mut rigid_body = physical_object.body.borrow_mut();
                rigid_body.clear_forces();
                rigid_body.append_lin_force(force);
            }
        }

        for (entity, _destroy_entity, _in_room) in (&*entities, &destroy_entities, &in_rooms).join() {
            if let Some(physical_object) = self.physical_objects.remove(&entity) {
                self.world.remove_rigid_body(&physical_object.body);
                println!("Destroyed object {:?}", entity);
            }
        }

        for (entity, _destroy_entity, _room) in (&*entities, &destroy_entities, &rooms).join() {
            if let Some(physical_room) = self.physical_rooms.remove(&entity) {
                for wall in physical_room.walls.iter() {
                    self.world.remove_rigid_body(wall);
                }
                println!("Destroyed room {:?}", entity);
            }
            // FIXME: destroy objects in the room too
        }

        // Let time flow in the physics world
        self.world.step(delta_time.dt);

        // Collect collisions that happened during the above step
        let mut constraints = Vec::with_capacity(16);
        self.world.constraints(&mut constraints);

        for (_entity, mut collision_set) in (&*entities, &mut collision_sets).join() {
            collision_set.colliding = false;
            collision_set.collision_normal = (0.0, 0.0);
        }

        for constraint in constraints {
            if let Constraint::RBRB(rigid_body1, rigid_body2, mut contact_point) = constraint {
                let (rigid_body1, rigid_body2) = (rigid_body1.borrow(), rigid_body2.borrow());
                let (user_data1, user_data2) = (rigid_body1.user_data(), rigid_body2.user_data());

                if let (Some(box1), Some(box2)) = (user_data1, user_data2) {
                    if let (Some(entity1), Some(entity2)) = (box1.downcast_ref::<Entity>(), box2.downcast_ref::<Entity>()) {
                        if let Some(mut collision_set) = collision_sets.get_mut(*entity1) {
                            collision_set.colliding = true;
                            let (x, y) = collision_set.collision_normal;
                            collision_set.collision_normal = (x + contact_point.normal.x, y + contact_point.normal.y);
                        }
                        if let Some(mut collision_set) = collision_sets.get_mut(*entity2) {
                            collision_set.colliding = true;
                            contact_point.flip();
                            let (x, y) = collision_set.collision_normal;
                            collision_set.collision_normal = (x + contact_point.normal.x, y + contact_point.normal.y);
                        }
                    }
                } else {
                    println!("Collision with unregistered body.");
                }
            }
        }

        for (_entity, mut collision_set) in (&*entities, &mut collision_sets).join() {
            if collision_set.colliding {
                collision_set.last_collision_normal = collision_set.collision_normal;
                collision_set.time_since_collision = 0.0;
            } else {
                collision_set.time_since_collision += delta_time.dt;
            }
        }
    }
}

/*
use specs::prelude::{FlaggedStorage, BitSet, ReaderId, ModifiedFlag, InsertedFlag, RemovedFlag};

#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy)]
#[storage(FlaggedStorage)]
pub struct OldPhysicalObject;

impl OldPhysicalObject {
    pub fn new_static() -> Self {
        OldPhysicalObject { }
    }
}

pub struct ComponentEvents {
    inserted_id: ReaderId<InsertedFlag>,
    inserted: BitSet,
    modified_id: ReaderId<ModifiedFlag>,
    modified: BitSet,
    removed_id: ReaderId<RemovedFlag>,
    removed: BitSet,
}

pub struct OldPhysicsSystem<N: Real = f64> {
    world: World<N>,
    component_events: ComponentEvents,
}

impl OldPhysicsSystem {
    pub fn new(specs_world: &specs::prelude::World) -> Self {
        let mut world = World::new();

        world.set_gravity(Vector2::new(0.0, 9.81));

        let component_events = {
            let mut components = specs_world.write_storage::<OldPhysicalObject>();

            ComponentEvents {
                inserted_id: components.track_inserted(),
                inserted: BitSet::new(),
                modified_id: components.track_modified(),
                modified: BitSet::new(),
                removed_id: components.track_removed(),
                removed: BitSet::new(),
            }
        };

        OldPhysicsSystem { world, component_events }
    }
}

impl<'a> System<'a> for OldPhysicsSystem {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, OldPhysicalObject>,
        ReadStorage<'a, DestroyEntity>,
    );

    fn run(&mut self, (entities, mut physical_objects, destroy_entities): Self::SystemData) {
        for (entity, _destroy_entity) in (&*entities, &destroy_entities).join() {
            println!("destroyed: {:?}", entity);
        }

        physical_objects.populate_inserted(&mut self.component_events.inserted_id,
                                           &mut self.component_events.inserted);
        physical_objects.populate_modified(&mut self.component_events.modified_id,
                                           &mut self.component_events.modified);
        physical_objects.populate_removed(&mut self.component_events.removed_id,
                                          &mut self.component_events.removed);

        for (entity, _tracked, _) in (&*entities, &mut physical_objects.restrict_mut(),
                                      &self.component_events.inserted).join() {
            println!("inserted: {:?}", entity);
        }
        for (entity, _tracked, _) in (&*entities, &mut physical_objects.restrict_mut(),
                                      &self.component_events.removed).join() {
            println!("deleted: {:?}", entity);
        }
        for (entity, _tracked, _) in (&*entities, &mut physical_objects.restrict_mut(),
                                      &self.component_events.modified).join() {
            println!("modified: {:?}", entity);
        }

        self.component_events.inserted.clear();
        self.component_events.modified.clear();
        self.component_events.removed.clear();
    }
}
*/

//pub fn test() {
//    let mut world = World::new();
//
//    let rb = RigidBody::new_dynamic(Ball::new(1.0), 1.0, 0.3, 0.6);
//    let rb = world.add_rigid_body(rb);
//
//    world.set_gravity(Vector2::new(0.0, 9.81));
//
//    println!("Old: {}", rb.borrow().position());
//
//    rb.borrow_mut().set_lin_acc_scale(Vector2::new(0.0, 0.0));
//
//    world.step(1.0);
//
//    println!("New: {}", rb.borrow().position().rotation);
//    println!("New: {}", rb.borrow().position().translation);
//}
