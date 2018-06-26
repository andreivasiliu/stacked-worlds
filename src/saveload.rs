extern crate specs;
extern crate ron;

use specs::saveload::{DeserializeComponents, SerializeComponents, U64Marker, U64MarkerAllocator};
use specs::prelude::{System, Entities, ReadStorage, Join, Write, WriteStorage};
use specs::storage::NullStorage;

use error::Error;
use draw::{Position, Size, Shape};
use shift::Shifter;
use animate::{Animation, RoomAnimation};
use physics::{Room, InRoom, Force, Velocity, CollisionSet, RevoluteJoint, Aim};
use input::PlayerController;
use control::{Jump, ChainLink};

pub struct SaveWorld {
    pub file_name: String,
}

impl <'a> System<'a> for SaveWorld {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Size>,
        ReadStorage<'a, Shape>,
        ReadStorage<'a, Room>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, PlayerController>,
        ReadStorage<'a, Velocity>,
        ReadStorage<'a, Force>,
        ReadStorage<'a, Aim>,
        ReadStorage<'a, CollisionSet>,
        ReadStorage<'a, RevoluteJoint>,
        ReadStorage<'a, ChainLink>,
        ReadStorage<'a, Shifter>,
        ReadStorage<'a, Jump>,
        ReadStorage<'a, Animation<RoomAnimation>>,
        ReadStorage<'a, U64Marker>,
    );

    fn run(&mut self, (entities, positions, sizes, shapes, rooms, in_rooms,
        player_controllers, velocities, forces, aims, collision_sets,
        revolute_joints, chain_links, shifters, jumps, animations, markers): Self::SystemData)
    {
        let mut serializer = ron::ser::Serializer::new(Some(Default::default()), true);
        SerializeComponents::<Error, U64Marker>::serialize(
            &(positions, sizes, shapes, rooms, in_rooms, player_controllers, velocities,
              forces, aims, collision_sets, revolute_joints, chain_links, shifters, jumps, animations),
            &entities,
            &markers,
            &mut serializer
        ).unwrap_or_else(|e| {
            // FIXME: handle this
            eprintln!("Error: {}", e);
        });

        let file_contents = serializer.into_output_string();

        use ::std::fs::File;
        use ::std::io::Write;

        let mut file = File::create(&self.file_name)
            .expect("Could not create save file.");
        file.write_all(file_contents.as_bytes())
            .expect("Could not write save file.");
    }
}

pub struct LoadWorld {
    pub file_name: String,
}

impl <'a> System<'a> for LoadWorld {
    type SystemData = (
        Entities<'a>,
        Write<'a, U64MarkerAllocator>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Size>,
        WriteStorage<'a, Shape>,
        WriteStorage<'a, Room>,
        WriteStorage<'a, InRoom>,
        WriteStorage<'a, PlayerController>,
        WriteStorage<'a, Velocity>,
        WriteStorage<'a, Force>,
        WriteStorage<'a, Aim>,
        WriteStorage<'a, CollisionSet>,
        WriteStorage<'a, RevoluteJoint>,
        WriteStorage<'a, ChainLink>,
        WriteStorage<'a, Shifter>,
        WriteStorage<'a, Jump>,
        WriteStorage<'a, Animation<RoomAnimation>>,
        WriteStorage<'a, U64Marker>,
    );

    fn run(&mut self, (entities, mut allocator, positions, sizes, shapes, rooms, in_rooms, player_controllers,
        velocities, forces, aims, collision_sets, revolute_joints, chain_links, shifters, jumps, animations, mut markers)
    : Self::SystemData) {
        use ::std::fs::File;
        use ::std::io::Read;

        let file_contents = {
            // FIXME: Replace panic! and expect! with actual error handling/recovery
            let mut file = match File::open(&self.file_name) {
                Ok(file) => file,
                Err(error) => {
                    if error.kind() == ::std::io::ErrorKind::NotFound {
                        eprintln!("Save file ({}) not found, starting empty world.", self.file_name);
                    } else {
                        panic!("Could not open save file: {} ({})", self.file_name, error);
                    }
                    return
                },
            };
            let mut file_contents = Vec::new();
            file.read_to_end(&mut file_contents)
                .expect("Could not read file.");
            file_contents
        };

        let mut deserializer = ron::de::Deserializer::from_bytes(&file_contents)
            .expect("Could not load"); // FIXME: handle error

        DeserializeComponents::<Error, _>::deserialize(
            &mut (positions, sizes, shapes, rooms, in_rooms, player_controllers, velocities,
                  forces, aims, collision_sets, revolute_joints, chain_links, shifters, jumps, animations),
            &entities,
            &mut markers,
            &mut allocator,
            &mut deserializer,
        ).unwrap_or_else(|e| {
            eprintln!("Error: {}", e); // FIXME: handle error
        })
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[storage(NullStorage)]
pub struct DestroyEntity;

pub struct ResetWorld;

impl <'a> System<'a> for ResetWorld {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, DestroyEntity>,
    );

    fn run(&mut self, (entities, mut destroy_entities): Self::SystemData) {
        for entity in entities.join() {
            destroy_entities.insert(entity, DestroyEntity)
                .expect("Could not insert DestroyEntity component");
        }
    }
}

pub struct DestroyEntities;

impl <'a> System<'a> for DestroyEntities {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, DestroyEntity>,
    );

    fn run(&mut self, (entities, destroy_entities): Self::SystemData) {
        for (entity, _destroy_entity) in (&*entities, &destroy_entities).join() {
            entities.delete(entity)
                .expect("Error deleting entity during world reset");
        }
    }
}
