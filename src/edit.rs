use specs::prelude::{System, Entities, ReadExpect, WriteExpect, LazyUpdate};
use specs::world::EntitiesRes;
use specs::saveload::U64Marker;
use std::collections::VecDeque;

use input;
use draw;
use physics;
use animate;
use control;


pub struct EditorController {
    edit_events: VecDeque<EditEvent>,
}

impl EditorController {
    pub fn new() -> Self {
        EditorController {
            edit_events: VecDeque::with_capacity(16),
        }
    }

    pub fn push_event(&mut self, edit_event: EditEvent) {
        self.edit_events.push_back(edit_event);
    }
}

pub enum EditEvent {
    CreateRoom { x: f64, y: f64, width: f64, height: f64 },
    CreateTerrainBox { x: f64, y: f64, width: f64, height: f64 },
}


pub struct CreateRoom;

fn create_room(entities: &EntitiesRes, lazy_update: &LazyUpdate,
               x: f64, y: f64, width: f64, height: f64)
{
    let entity = lazy_update.create_entity(entities)
        .with(draw::Position { x, y })
        .with(draw::Size { width, height })
        .with(physics::Room)
        .with(animate::Animation::<animate::RoomAnimation>::new(32))
        .marked::<U64Marker>()
        .build();

    lazy_update.create_entity(entities)
        .with(draw::Position { x: width / 2.0 + 5.0, y: height / 2.0 + 10.0 })
        .with(draw::Shape { size: 10.0, class: draw::ShapeClass::Ball })
        .with(physics::Velocity::default())
        .with(physics::InRoom { room_entity: entity.id() })
        .marked::<U64Marker>()
        .build();

    lazy_update.create_entity(entities)
        .with(draw::Position { x: width / 2.0 - 5.0, y: height / 2.0 - 10.0 })
        .with(draw::Shape { size: 10.0, class: draw::ShapeClass::Ball })
        .with(physics::Velocity::default())
        .with(physics::InRoom { room_entity: entity.id() })
        .marked::<U64Marker>()
        .build();

    if entity.id() == 0 {
        lazy_update.create_entity(entities)
            .with(draw::Position { x: width / 2.0, y: 20.0 })
            .with(draw::Shape { size: 10.0, class: draw::ShapeClass::Ball })
            .with(physics::Velocity::default())
            .with(physics::InRoom { room_entity: entity.id() })
            .with(input::PlayerController::default())
            .with(control::Jump::default())
            .with(physics::Force::default())
            .with(physics::Aim::default())
            .with(physics::CollisionSet::default())
            .marked::<U64Marker>()
            .build();
    }
}

impl <'a> System<'a> for CreateRoom {
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, EditorController>,
        ReadExpect<'a, LazyUpdate>,
    );

    fn run(&mut self, (entities, mut editor_controller, lazy_update): Self::SystemData) {
        while let Some(edit_event) = editor_controller.edit_events.pop_front() {
            match edit_event {
                EditEvent::CreateRoom { x, y, width, height } => {
                    create_room(&entities, &lazy_update, x, y, width, height);
                },
                EditEvent::CreateTerrainBox { .. } => (),
            };
        }
    }
}
