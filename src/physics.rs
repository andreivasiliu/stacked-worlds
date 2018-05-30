extern crate specs;
extern crate nphysics2d;
extern crate ncollide2d;

use specs::prelude::{WriteStorage, ReadStorage, VecStorage, DenseVecStorage, System, Entities, Join};
use specs::world::Index;
use specs::prelude::Entity;
use specs::prelude::ReadExpect;
use nphysics2d::world::World;
use nphysics2d::object::RigidBody;
use nphysics2d::object::BodyHandle;
use nphysics2d::object::Material;
use nphysics2d::algebra::Force2;
use nphysics2d::algebra::Velocity2;
use nphysics2d::joint::RevoluteConstraint;
use nphysics2d::joint::ConstraintHandle;
use nalgebra::{Vector2, Isometry2, Unit, zero};
use ncollide2d::shape::Ball;
use ncollide2d::shape::Plane;
use ncollide2d::shape::Cuboid;
use ncollide2d::shape::ShapeHandle;
use ncollide2d::world::CollisionObjectHandle;
use ncollide2d::world::CollisionGroups;
use std::collections::HashMap;

use saveload::DestroyEntity;
use draw::{Position, Size, Shape, ShapeClass};
use UpdateDeltaTime;


const COLLIDER_MARGIN: f64 = 0.1;

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
    pub continuous: (f64, f64),
    pub impulse: (f64, f64),
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

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[storage(DenseVecStorage)]
pub struct RevoluteJoint {
    pub linked_to_entity: Index,
    pub multibody_link: bool,
}

struct PhysicalObject {
    body_handle: BodyHandle,
    collision_object_handle: CollisionObjectHandle,
    room_entity: Entity,
    multibody_parent: Option<Entity>,
}

/// Internal object for the physics system
struct PhysicalRoom {
    world: World<f64>,
    walls: [CollisionObjectHandle; 4],
}

trait GetEntity {
    fn get_entity(&self) -> Option<Entity>;
}

pub struct PhysicalConstraint {
    revolute_constraint_handle: ConstraintHandle,
    room_entity: Entity,
    visited: bool,
}

pub struct PhysicsSystem {
    physical_objects: HashMap<Entity, PhysicalObject>,
    physical_rooms: HashMap<Entity, PhysicalRoom>,

    collision_object_to_entity: HashMap<(Entity, CollisionObjectHandle), Entity>,
    physical_constraints: HashMap<Entity, PhysicalConstraint>,
}

impl PhysicsSystem {
    pub fn new() -> Self {
        PhysicsSystem {
            physical_objects: HashMap::new(),
            physical_rooms: HashMap::new(),
            physical_constraints: HashMap::new(),

            collision_object_to_entity: HashMap::new(),
        }
    }

    fn get_world_mut(&mut self, entity: &Entity) -> Option<&mut World<f64>> {
        let room_entity = match self.physical_objects.get(entity) {
            Some(physical_object) => physical_object.room_entity,
            None => return None,
        };

        match self.physical_rooms.get_mut(&room_entity) {
            Some(physical_room) => Some(&mut physical_room.world),
            None => return None,
        }
    }

    fn get_rigid_body(&mut self, entity: &Entity) -> Option<&mut RigidBody<f64>> {
        let body_handle = match self.physical_objects.get_mut(entity) {
            Some(physical_object) => physical_object.body_handle,
            None => return None,
        };

        if let Some(world) = self.get_world_mut(entity) {
            if let Some(rigid_body) = world.rigid_body_mut(body_handle) {
                return Some(rigid_body)
            }
        }

        None
    }
}

impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Room>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, Size>,
        ReadStorage<'a, Shape>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Force>,
        WriteStorage<'a, Aim>,
        //WriteStorage<'a, Angle>, // eventually...
        WriteStorage<'a, CollisionSet>,
        ReadStorage<'a, RevoluteJoint>,
        ReadStorage<'a, DestroyEntity>,
        ReadExpect<'a, UpdateDeltaTime>,
    );

    fn run(&mut self, (entities, rooms, in_rooms, sizes, shapes, mut positions, mut velocities,
        forces, mut aims, mut collision_sets, revolute_joints, destroy_entities, delta_time): Self::SystemData) {
        for (entity, _room, size) in (&*entities, &rooms, &sizes).join() {
            let physical_rooms = &mut self.physical_rooms;
            let collision_object_to_entity = &mut self.collision_object_to_entity;

            let _physical_room = physical_rooms.entry(entity)
                .or_insert_with(|| {
                    let mut world = World::new();

                    world.set_gravity(Vector2::new(0.0, 500.0));

                    fn create_wall(world: &mut World<f64>, normal: Vector2<f64>, isometry: Isometry2<f64>) -> CollisionObjectHandle {
                        world.add_collider(
                            COLLIDER_MARGIN,
                            ShapeHandle::new(Plane::new(Unit::new_normalize(normal))),
                            BodyHandle::ground(),
                            isometry,
                            Material::default(),
                        )
                    }

                    let south_wall = Vector2::new(0.0, 1.0); // pointing north
                    let north_wall = Vector2::new(0.0, -1.0); // pointing south
                    let west_wall = Vector2::new(1.0, 0.0); // pointing east
                    let east_wall = Vector2::new(-1.0, 0.0); // pointing west

                    let walls = [
                        create_wall(&mut world, south_wall, Isometry2::new(zero(), 0.0)),
                        create_wall(&mut world, north_wall, Isometry2::new(Vector2::new(0.0, size.height), 0.0)),
                        create_wall(&mut world, west_wall, Isometry2::new(zero(), 0.0)),
                        create_wall(&mut world, east_wall, Isometry2::new(Vector2::new(size.width, 0.0), 0.0)),
                    ];

                    for collision_object_handle in walls.iter() {
                        collision_object_to_entity.insert((entity, *collision_object_handle), entity);
                    }

                    println!("Created room {:?}", entity);

                    PhysicalRoom {
                        world,
                        walls,
                    }
                });
        }

        // Find static objects in the room, and create terrain out of them
        // FIXME: Maybe consider using Shape instead of Size
        for (entity, in_room, position, size, ()) in (&*entities, &in_rooms, &positions, &sizes, !&velocities).join() {
            let room_entity = entities.entity(in_room.room_entity);

            let physical_room = match self.physical_rooms.get_mut(&room_entity) {
                Some(physical_room) => physical_room,
                None => continue,
            };

            let world = &mut physical_room.world;
            let collision_object_to_entity = &mut self.collision_object_to_entity;

            self.physical_objects.entry(entity)
                .or_insert_with(|| {
                    let position = Vector2::new(position.x, position.y);
                    let half_extents = Vector2::new(size.width / 2.0, size.height / 2.0);

                    let shape_handle = ShapeHandle::new(Cuboid::new(half_extents));
                    let body_handle = BodyHandle::ground();

                    let collision_object_handle = world.add_collider(
                        COLLIDER_MARGIN,
                        shape_handle,
                        body_handle,
                        Isometry2::new(position + half_extents, 0.0),
                        Material::default(),
                    );

                    collision_object_to_entity.insert((room_entity, collision_object_handle), entity);

                    println!("Terrain created for {:?}", entity);

                    PhysicalObject {
                        body_handle,
                        collision_object_handle,
                        room_entity,
                        multibody_parent: None,
                    }
                });
        }

        for (entity, in_room, shape, position, velocity) in (&*entities, &in_rooms, &shapes, &mut positions, &mut velocities).join() {
            let physical_objects = &mut self.physical_objects;
            let collision_object_to_entity = &mut self.collision_object_to_entity;

            let room_entity = entities.entity(in_room.room_entity);

            let (multibody_parent_handle, multibody_parent_entity) = {
                if let Some(revolute_joint) = revolute_joints.get(entity) {
                    if revolute_joint.multibody_link {
                        let linked_to_entity = entities.entity(revolute_joint.linked_to_entity);
                        let is_room = self.physical_rooms.contains_key(&linked_to_entity);
                        (
                            physical_objects
                                .get(&linked_to_entity)
                                .and_then(|object| Some(object.body_handle))
                                .or_else(|| { if is_room { Some(BodyHandle::ground()) } else { None } }),
                            Some(linked_to_entity),
                        )
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            };

            let world = if let Some(physical_room) = self.physical_rooms.get_mut(&room_entity) {
                &mut physical_room.world
            } else {
                eprintln!("Could not find the physical world {} for object {}",
                          room_entity.id(), entity.id());
                // Consider better logging and queuing InRoom component deletion
                continue
            };

            let physical_object = physical_objects.entry(entity)
                .or_insert_with(|| {
                    use nphysics2d::volumetric::Volumetric;

                    let density = match shape.class {
                        ShapeClass::ChainLink => 0.8,
                        ShapeClass::Ball => 1.0,
                    };

                    let shape_handle = ShapeHandle::new(Ball::new(shape.size));

                    let body_handle = if let Some(parent) = multibody_parent_handle {
                        use nphysics2d::joint;

                        let linked_body_position = world.body_part(parent).position().translation.vector;

                        println!("Multibody link added for {:?}", entity);

                        world.add_multibody_link(
                            parent,
                            joint::RevoluteJoint::new(0.0),
                            -linked_body_position + Vector2::new(position.x, position.y),
                            zero(),
                            shape_handle.inertia(density),
                            shape_handle.center_of_mass(),
                        )
                    } else {
                        println!("Rigid body added for {:?}", entity);

                        world.add_rigid_body(
                            Isometry2::new(Vector2::new(position.x, position.y), 0.0),
                            shape_handle.inertia(density),
                            shape_handle.center_of_mass(),
                        )
                    };

                    let collision_object_handle = world.add_collider(
                        COLLIDER_MARGIN,
                        shape_handle,
                        body_handle,
                        Isometry2::new(zero(), 0.0),
                        Material::default(),
                    );

                    collision_object_to_entity.insert((room_entity, collision_object_handle), entity);

                    if multibody_parent_handle.is_none() {
                        let body = world.rigid_body_mut(body_handle)
                            .expect("Cannot get reference to object that was just created");

                        body.set_linear_velocity(Vector2::new(velocity.x, velocity.y));
                    }

                    PhysicalObject {
                        body_handle,
                        collision_object_handle,
                        room_entity,
                        multibody_parent: multibody_parent_entity,
                    }
                });

            let body = world.body_part(physical_object.body_handle);

            let physical_position = body.position().translation.vector;
            position.x = physical_position.x;
            position.y = physical_position.y;

            let physical_velocity = body.velocity().linear;
            velocity.x = physical_velocity.x;
            velocity.y = physical_velocity.y;
        }

        // Clear the visited flag of all joints; after processing entities, all
        // unvisited ones will be deleted
        for constraint in self.physical_constraints.values_mut() {
            constraint.visited = false;
        }

        for (entity, revolute_joint) in (&*entities, &revolute_joints).join() {
            if revolute_joint.multibody_link {
                continue;
            }

            fn get_object_or_room_body(system: &PhysicsSystem, entity: &Entity) -> Option<BodyHandle> {
                if let Some(object) = system.physical_objects.get(entity) {
                    Some(object.body_handle)
                } else if let Some(_room) = system.physical_rooms.get(entity) {
                    Some(BodyHandle::ground())
                } else {
                    None
                }
            }

            fn get_room_entity(system: &PhysicsSystem, entity: &Entity) -> Option<Entity> {
                if let Some(object) = system.physical_objects.get(entity) {
                    Some(object.room_entity)
                } else if system.physical_rooms.contains_key(entity) {
                    Some(*entity)
                } else {
                    None
                }
            }

            fn get_body_position(world: &World<f64>, body_handle: BodyHandle) -> Vector2<f64> {
//                use nphysics2d::object::Body;

                world.body_part(body_handle).position().translation.vector
//                match world.body_part(body_handle) {
//                    Body::RigidBody(rigid_body) => rigid_body.position().translation.vector,
//                    Body::Ground(ground) => ground.position().translation.vector,
//                    Body::Multibody(_) => panic!("Attempted to add constraint to a multibody handle"),
//                }
            }

            let entity2 = entities.entity(revolute_joint.linked_to_entity);

            let body1 = get_object_or_room_body(&self, &entity);
            let body2 = get_object_or_room_body(&self, &entity2);

            let room_entity1 = get_room_entity(&self, &entity);
            let room_entity2 = get_room_entity(&self, &entity2);

            let room_entity = match (room_entity1, room_entity2) {
                (Some(room_entity1), Some(room_entity2)) if room_entity1 == room_entity2 => room_entity1,
                _ => { eprintln!("Could not find room entity for {:?} {:?}", entity, entity2); continue },
            };

            let world = match self.physical_rooms.get_mut(&room_entity) {
                Some(physical_room) => &mut physical_room.world,
                None => { eprintln!("Could not find room for body"); continue },
            };

            if let (Some(body1), Some(body2)) = (body1, body2) {
                let pos1 = get_body_position(&world, body1);
                let pos2 = get_body_position(&world, body2);

                let physical_constraint = self.physical_constraints.entry(entity)
                    .or_insert_with(|| {
                        use nphysics2d::math::Point;

                        let relative_position = Point::new(pos1.x - pos2.x, pos1.y - pos2.y);

                        let constraint = RevoluteConstraint::new(
                            body1,
                            body2,
                            Point::new(0.0, 0.0),
                            relative_position,
                        );

                        let revolute_constraint_handle = world.add_constraint(constraint);

                        PhysicalConstraint {
                            revolute_constraint_handle,
                            room_entity,
                            visited: true,
                        }
                    });

                physical_constraint.visited = true;
            } else {
                eprintln!("No physical body found to create a joint betwen: {:?} <-> {:?}",
                          entity, entity2);
            }
        }

        for (room_entity, physical_room) in self.physical_rooms.iter_mut() {
            // Delete all unvisited joints; it means their components were destroyed.
            self.physical_constraints.retain(|_entity, constraint| {
                if !constraint.visited && constraint.room_entity == *room_entity {
                    physical_room.world.remove_constraint(constraint.revolute_constraint_handle);
                }

                constraint.visited
            });
        };

        for (entity, position, mut aim) in (&*entities, &positions, &mut aims).join() {
            use nalgebra::Point2;
            use ncollide2d::query::Ray;

            let direction = Vector2::new(aim.aiming_toward.0, aim.aiming_toward.1).normalize();
            // FIXME: Find the proper trait for direction.is_zero()
            if direction == zero() {
                continue;
            }

            let room_entity = self.physical_objects.get(&entity)
                .and_then(|physical_object| Some(physical_object.room_entity));

            let world = room_entity
                .and_then(|room_entity| self.physical_rooms.get(&room_entity))
                .and_then(|room| Some(&room.world));

            if let (Some(room_entity), Some(world)) = (room_entity, world) {
                let source = Point2::new(position.x, position.y);
                let ray = Ray::new(source, direction);
                use std::f64::INFINITY;

                aim.aiming_at_point = None;
                aim.aiming_at_entity = None;
                let mut smallest_time_of_impact = INFINITY;

                for interference in world.collision_world().interferences_with_ray(&ray, &CollisionGroups::new()) {
                    let (collision_object, ray_intersection) = interference;

                    if let Some(intersected_entity) = self.collision_object_to_entity.get(&(room_entity, collision_object.handle())) {
                        if entity != *intersected_entity {
                            let intersection_point = source + direction * ray_intersection.toi;

                            if smallest_time_of_impact > ray_intersection.toi {
                                smallest_time_of_impact = ray_intersection.toi;

                                aim.aiming_at_point = Some((intersection_point.x, intersection_point.y));
                                aim.aiming_at_entity = Some(*intersected_entity);
                            }
                        }
                    }
                }
            }
        }

        for (entity, force) in (&*entities, &forces).join() {
            if let Some(rigid_body) = self.get_rigid_body(&entity) {
                let continuous_force = Vector2::new(force.continuous.0, force.continuous.1);
                let impulse_force = Vector2::new(force.impulse.0, force.impulse.1);

                assert!(!continuous_force.x.is_nan());
                assert!(!continuous_force.y.is_nan());

                rigid_body.apply_force(&Force2::new(continuous_force, 0.0));

                let velocity = rigid_body.velocity().clone();
                assert!(!velocity.linear.x.is_nan());
                assert!(!velocity.linear.y.is_nan());

                let impulse_force = if impulse_force.x.is_nan() || impulse_force.y.is_nan() {
                    println!("NaN eradicated.");
                    zero()
                } else {
                    impulse_force
                };

                assert!(!impulse_force.x.is_nan());
                assert!(!impulse_force.y.is_nan());

                rigid_body.set_velocity(velocity + Velocity2::new(impulse_force, 0.0));
                //rigid_body.apply_displacement(&Velocity2::new(impulse_force, 0.0));
            }
        }

        for (entity, revolute_joint, in_room) in (&*entities, &revolute_joints, &in_rooms).join() {
            let target_will_be_destroyed = destroy_entities
                .get(entities.entity(revolute_joint.linked_to_entity))
                .is_some();

            if target_will_be_destroyed {
                let world = self.physical_rooms
                    .get_mut(&entities.entity(in_room.room_entity))
                    .and_then(|room| Some(&mut room.world));

                let constraint = self.physical_constraints.remove(&entity);

                if let (Some(world), Some(constraint)) = (world, constraint) {
                    world.remove_constraint(constraint.revolute_constraint_handle);
                }
            }
        }

        for (entity, _destroy_entity, _in_room) in (&*entities, &destroy_entities, &in_rooms).join() {
            if let Some(physical_object) = self.physical_objects.remove(&entity) {
                self.collision_object_to_entity.remove(&(physical_object.room_entity, physical_object.collision_object_handle));

                let world = self.physical_rooms
                    .get_mut(&physical_object.room_entity)
                    .and_then(|room| Some(&mut room.world));

                if let Some(world) = world {
                    if let Some(_multibody_parent) = physical_object.multibody_parent {
                        world.remove_multibody_links(&[physical_object.body_handle]);
                    } else {
                        if let Some(physical_constraint) = self.physical_constraints.remove(&entity) {
                            world.remove_constraint(physical_constraint.revolute_constraint_handle);
                        }

                        world.remove_bodies(&[physical_object.body_handle]);
                    }
                } else {
                    eprintln!("Could not find object's physical world");
                    // FIXME: better error reporting
                }

                // println!("Destroyed object {:?}", entity);
            }
        }

        for (entity, _destroy_entity, _room) in (&*entities, &destroy_entities, &rooms).join() {
            if let Some(physical_room) = self.physical_rooms.remove(&entity) {
                for collision_object_handle in physical_room.walls.iter() {
                    self.collision_object_to_entity.remove(&(entity, *collision_object_handle));
                }
            }

            println!("Destroyed room {:?}", entity);
            // FIXME: destroy objects in the room too
        }

        for (entity, _destroy_entity, _revolute_joint) in (&*entities, &destroy_entities, &revolute_joints).join() {
            if let Some(physical_constraint) = self.physical_constraints.remove(&entity) {
                if let Some(world) = self.get_world_mut(&entity) {
                    world.remove_constraint(physical_constraint.revolute_constraint_handle);
                }
            }
        }

        // Let time flow in the physics world
        for physical_room in self.physical_rooms.values_mut() {
            physical_room.world.set_timestep(delta_time.dt);
            physical_room.world.step();
        }

        for (_entity, mut collision_set) in (&*entities, &mut collision_sets).join() {
            collision_set.colliding = false;
            collision_set.collision_normal = (0.0, 0.0);
        }

        for (room_entity, physical_room) in self.physical_rooms.iter_mut() {
            for (collision_object1, collision_object2, contact_manifold) in physical_room.world.collision_world().contact_manifolds() {
                let entity1 = self.collision_object_to_entity.get(&(*room_entity, collision_object1.handle()));
                let entity2 = self.collision_object_to_entity.get(&(*room_entity, collision_object2.handle()));

                if let Some(collision_set) = entity1.and_then(|entity| collision_sets.get_mut(*entity)) {
                    for tracked_contact in contact_manifold.contacts() {
                        let normal = tracked_contact.contact.normal;

                        let (x, y) = collision_set.collision_normal;
                        collision_set.collision_normal = (x + normal.x, y + normal.y);
                        collision_set.colliding = true;
                    }
                }

                if let Some(collision_set) = entity2.and_then(|entity| collision_sets.get_mut(*entity)) {
                    for tracked_contact in contact_manifold.contacts() {
                        let normal = -tracked_contact.contact.normal;

                        let (x, y) = collision_set.collision_normal;
                        collision_set.collision_normal = (x + normal.x, y + normal.y);
                        collision_set.colliding = true;
                    }
                }
            }

            // TODO: Handle cases where a body exists but is not in some of our hashmaps
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
