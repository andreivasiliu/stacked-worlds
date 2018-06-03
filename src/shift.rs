/// Phase shift between rooms/dimensions
///
/// If two rooms are interconnected, objects can phase shift from one room to
/// another at the same (or similar) position.
///
/// Overview:
/// * All entities capable of phase-shifting have a Shifter component
///   * Every update, the TrackShiftTarget figures out the target room, if there is one
/// * ...

use specs::prelude::{System, DenseVecStorage, Entities, ReadStorage, WriteStorage, Join};
use specs::world::{Index, EntitiesRes};

use physics::{Room, InRoom};
use input::PlayerController;

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[storage(DenseVecStorage)]
pub struct Shifter {
    pub target_room: Option<Index>,
    pub target_entity: Option<Index>,
    pub shifting: bool,
    pub sensing: bool,
}


pub struct TrackShiftTarget;

fn get_next_room<'a>(current_room: Index, entities: &EntitiesRes, rooms: &ReadStorage<'a, Room>) -> Option<Index> {
    let iteration1 = (entities, rooms).join();
    let iteration2 = (entities, rooms).join();

    let next_room = iteration1.chain(iteration2)
        .map(|(entity, _room)| entity.id())
        .skip_while(|room_index| current_room != *room_index)
        .nth(1);

    next_room
}

impl <'a> System<'a> for TrackShiftTarget {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Shifter>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, Room>,
    );

    fn run(&mut self, (entities, mut shifters, in_rooms, rooms): Self::SystemData) {
        // Figure out the target room. Currently, it's just the next room in the Room storage.
        for (_entity, mut shifter, in_room) in (&*entities, &mut shifters, &in_rooms).join() {
            shifter.target_room = get_next_room(in_room.room_entity, &*entities, &rooms);
        }
    }
}

pub struct StartPhaseShift;

impl <'a> System<'a> for StartPhaseShift {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, PlayerController>,
        WriteStorage<'a, Shifter>,
    );

    fn run(&mut self, (entities, player_controllers, mut shifters): Self::SystemData) {
        for (_entity, player_controller, shifter) in (&*entities, &player_controllers, &mut shifters).join() {
            if player_controller.shifting && shifter.target_entity.is_none() {
                shifter.sensing = true;
                // Create Sensor
            } else if !player_controller.shifting && shifter.sensing && !shifter.shifting {
                shifter.shifting = true;
                println!("Shifting to: {:?}", shifter.target_room);
            }
        }
    }
}

pub struct PhaseShift;

impl <'a> System<'a> for PhaseShift {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Shifter>,
        WriteStorage<'a, InRoom>,
    );

    fn run(&mut self, (entities, mut shifters, mut in_rooms): Self::SystemData) {
        for (_entity, shifter, in_room) in (&*entities, &mut shifters, &mut in_rooms).join() {
            if shifter.shifting {
                if let Some(target_room) = shifter.target_room {
                    shifter.shifting = false;
                    shifter.sensing = false;
                    in_room.room_entity = target_room;
                }
            }
        }
    }
}