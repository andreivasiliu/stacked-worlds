/// Systems:
/// saveload:
///   - `LoadWorld`
///   - `SaveWorld`
///   - `ResetWorld`
/// draw.rs:
///   - `ClearScreen`
///   - `DrawRooms`
///   - `DrawSelectionBox`
/// animate.rs:
///   - `UpdateAnimations`
/// physics:
///   - `PhysicsSystem`
///
/// Components:
/// lib.rs:
///   - `Position`
///   - `Size`
///   - `Room`
/// animate.rs:
///   - `Animation<T>`
/// physics.rs:
///   - `PhysicalObject`
///   - `PhysicalRoom`?
/// saveload.rs:
///   - `DestroyEntity`


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

#[macro_use]
extern crate failure;
extern crate opengl_graphics;
extern crate piston;
extern crate graphics;
extern crate glutin_window;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate specs;
#[macro_use]
extern crate specs_derive;
extern crate ron;
extern crate nalgebra;
extern crate nphysics2d;
extern crate ncollide;


use opengl_graphics::{GlGraphics, OpenGL};
use glutin_window::GlutinWindow;
use piston::input::{UpdateEvent, UpdateArgs};
use piston::input::{RenderEvent, RenderArgs};
use piston::input::{PressEvent, ReleaseEvent, Key, Button, MouseButton};
use piston::input::{MouseCursorEvent};
use piston::window::WindowSettings;
use piston::event_loop::{Events, EventSettings};
use specs::prelude::{World, RunNow};
use specs::saveload::U64Marker;
use specs::saveload::U64MarkerAllocator;

mod draw;
mod input;
mod animate;
mod physics;
mod saveload;
mod error;

use error::{GameError, Error};
use saveload::{SaveWorld, LoadWorld, DestroyEntity};
use draw::run_draw_systems;
use physics::PhysicsSystem;


struct Game {
    gl: GlGraphics,
    specs_world: World,
    physics_system: PhysicsSystem,
}

#[derive(Debug, Default)]
pub struct MouseInput {
    pub mouse: (i32, i32),
    pub dragging_source: (i32, i32),
    pub dragging: bool,
}

impl MouseInput {
    pub fn selection_box(&self) -> [i32; 4] {
        [self.dragging_source.0, self.dragging_source.1, self.mouse.0, self.mouse.1]
    }

    /// Return an array of type [i32; 4] with [x, y, width, height].
    pub fn selection_rectangle(&self) -> [i32; 4] {
        use std::cmp::{min, max};

        let x1 = min(self.dragging_source.0, self.mouse.0);
        let y1 = min(self.dragging_source.1, self.mouse.1);
        let x2 = max(self.dragging_source.0, self.mouse.0);
        let y2 = max(self.dragging_source.1, self.mouse.1);

        [x1, y1, x2 - x1, y2 - y1]
    }

    pub fn selection_rectangle_f64(&self) -> [f64; 4] {
        let rect = self.selection_rectangle();

        [rect[0] as f64, rect[1] as f64, rect[2] as f64, rect[3] as f64]
    }
}

pub struct UpdateDeltaTime {
    pub dt: f64,
}

impl Game {
    fn render(&mut self, args: &RenderArgs) {
        run_draw_systems(&mut self.specs_world, &mut self.gl, *args);
    }

    fn update(&mut self, args: &UpdateArgs) {
        let () = {
            let mut update_delta_time = self.specs_world.write_resource::<UpdateDeltaTime>();
            update_delta_time.dt = args.dt;
        };

        self.physics_system.run_now(&mut self.specs_world.res);
        animate::UpdateAnimations.run_now(&mut self.specs_world.res);

        // Must be left at the end in order to allow every other system to react on destroyed
        // entities.
        saveload::DestroyEntities.run_now(&mut self.specs_world.res);
        self.specs_world.maintain();
    }

    fn press(&mut self, args: &Button) {
        if let &Button::Mouse(MouseButton::Left) = args {
            let mut mouse_input = self.specs_world.write_resource::<MouseInput>();

            mouse_input.dragging = true;
            mouse_input.dragging_source = mouse_input.mouse;
        }

        if let &Button::Keyboard(Key::R) = args {
            saveload::ResetWorld.run_now(&mut self.specs_world.res);
            self.specs_world.maintain();
        }
    }

    fn release(&mut self, args: &Button) {
        if let &Button::Mouse(MouseButton::Left) = args {
            let rect = {
                let mut mouse_input = self.specs_world.write_resource::<MouseInput>();
                mouse_input.dragging = false;
                mouse_input.selection_rectangle()
                // Drop .write_resource()'s borrow so we can access .create_entity() later
            };

            let (x, y, width, height) = (rect[0], rect[1], rect[2], rect[3]);

            let entity = self.specs_world.create_entity()
                .with(draw::Position { x, y})
                .with(draw::Size { width, height })
                .with(physics::Room)
                .with(animate::Animation::<animate::RoomAnimation>::new(32))
                .marked::<U64Marker>()
                .build();

            self.specs_world.create_entity()
                .with(draw::Position { x: width / 2 + 5, y: height / 2 + 10 })
                .with(physics::InRoom { room_entity: entity.id() })
                .marked::<U64Marker>()
                .build();

            self.specs_world.create_entity()
                .with(draw::Position { x: width / 2 - 5, y: height / 2 - 10 })
                .with(physics::InRoom { room_entity: entity.id() })
                .marked::<U64Marker>()
                .build();
        }
    }

    fn mouse_cursor(&mut self, x: f64, y: f64) {
        let mut mouse_input = self.specs_world.write_resource::<MouseInput>();
        mouse_input.mouse = (x as i32, y as i32);
    }
}

pub fn run() -> Result<(), Error> {
    let opengl_version = OpenGL::V3_2;

    let mut window: GlutinWindow = WindowSettings::new("hellopiston", [640, 480])
        .opengl(opengl_version)
        .exit_on_esc(true)
        .build()
        .map_err(|err| GameError::WindowError { reason: err })?;

//    let game_state = {
//        let state_file = std::fs::File::open("state.json");
//
//        match state_file {
//            Ok(state_file) =>
//                serde_json::from_reader::<_, GameState>(state_file)
//                    .context("Cannot deserialize game state file")?,
//            Err(err) =>
//                if err.kind() == std::io::ErrorKind::NotFound {
//                    GameState::default()
//                } else {
//                    return Err(Error::from(err).context("Cannot open game state file").into())
//                },
//        }
//    };

    let mut world = World::new();

    world.register::<saveload::DestroyEntity>();
    world.register::<draw::Position>();
    world.register::<draw::Size>();
    world.register::<animate::Animation<animate::RoomAnimation>>();
    world.register::<physics::Room>();
    world.register::<physics::InRoom>();
    world.register::<U64Marker>();

    world.add_resource(U64MarkerAllocator::new());
    world.add_resource(MouseInput::default());
    world.add_resource(UpdateDeltaTime { dt: 0.0 });

    let mut game = Game {
        gl: GlGraphics::new(opengl_version),
        physics_system: PhysicsSystem::new(),
        specs_world: world,
    };

    LoadWorld { file_name: "storage.ron".into() }.run_now(&mut game.specs_world.res);

    let mut events = Events::new(EventSettings::new());

    while let Some(event) = events.next(&mut window) {
        if let Some(render_args) = event.render_args() {
            game.render(&render_args);
        }

        if let Some(update_args) = event.update_args() {
            game.update(&update_args);
        }

        if let Some(press_args) = event.press_args() {
            game.press(&press_args);
        }

        if let Some(release_args) = event.release_args() {
            game.release(&release_args);
        }

        if let Some(mouse_cursor_args) = event.mouse_cursor_args() {
            game.mouse_cursor(mouse_cursor_args[0], mouse_cursor_args[1]);
        }
    }

    SaveWorld { file_name: "storage.ron".into() }.run_now(&game.specs_world.res);

//    let state_file = std::fs::File::create("state.json")
//        .context("Cannot create file to save game state")?;
//    serde_json::to_writer(state_file, &game.state)
//        .context("Cannot write game state to file")?;

    Ok(())
}
