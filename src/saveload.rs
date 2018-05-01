extern crate specs;
extern crate ron;

use specs::saveload::{DeserializeComponents, SerializeComponents, U64Marker, U64MarkerAllocator};
use specs::prelude::{System, Entities, ReadStorage, Join, Write, WriteStorage};

use error::Error;
use draw::{Position, Size};
use animate::{Animation, RoomAnimation};

pub struct SaveWorld {
    pub file_name: String,
}

impl <'a> System<'a> for SaveWorld {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Size>,
        ReadStorage<'a, Animation<RoomAnimation>>,
        ReadStorage<'a, U64Marker>,
    );

    fn run(&mut self, (entities, positions, sizes, animations, markers): Self::SystemData) {
        let mut serializer = ron::ser::Serializer::new(Some(Default::default()), true);
        SerializeComponents::<Error, U64Marker>::serialize(
            &(&positions, &sizes, &animations),
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
        WriteStorage<'a, Animation<RoomAnimation>>,
        WriteStorage<'a, U64Marker>,
    );

    fn run(&mut self, (entities, mut allocator, positions, sizes, animations, mut markers): Self::SystemData) {
        use ::std::fs::File;
        use ::std::io::Read;

        let file_contents = {
            let mut file = File::open(&self.file_name)
                .expect("Could not open file.");
            let mut file_contents = Vec::new();
            file.read_to_end(&mut file_contents)
                .expect("Could not read file.");
            file_contents
        };

        let mut deserializer = ron::de::Deserializer::from_bytes(&file_contents)
            .expect("Could not load"); // FIXME: handle error

        DeserializeComponents::<Error, _>::deserialize(
            &mut (positions, sizes, animations),
            &entities,
            &mut markers,
            &mut allocator,
            &mut deserializer,
        ).unwrap_or_else(|e| {
            eprintln!("Error: {}", e); // FIXME: handle error
        })
    }
}

pub struct ResetWorld;

impl <'a> System<'a> for ResetWorld {
    type SystemData = Entities<'a>;

    fn run(&mut self, entities: Self::SystemData) {
        for entity in entities.join() {
            entities.delete(entity)
                .expect("Error deleting entity during world reset");
        }
    }
}
