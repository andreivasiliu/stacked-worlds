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

use failure::{Error, ResultExt};
use opengl_graphics::{GlGraphics, OpenGL};
use glutin_window::GlutinWindow;
use piston::input::{UpdateEvent, UpdateArgs};
use piston::input::{RenderEvent, RenderArgs};
use piston::input::{PressEvent, ReleaseEvent, Key, Button, MouseButton};
use piston::input::{MouseCursorEvent};
use piston::window::WindowSettings;
use piston::event_loop::{Events, EventSettings};

struct Game {
    gl: GlGraphics,
    state: GameState,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Room {
    rectangle: [f64; 4],

    #[serde(skip)]
    animation: u8,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GameState {
    #[serde(skip)]
    mouse: (f64, f64),
    #[serde(skip)]
    dragging: bool,
    #[serde(skip)]
    dragging_source: (f64, f64),

    rooms: Vec<Room>,
}

trait PointsToRectangle<T> {
    fn to(&self, other: (T, T)) -> [T; 4];
}

impl PointsToRectangle<f64> for (f64, f64) {
    fn to(&self, other: (f64, f64)) -> [f64; 4] {
        [self.0, self.1, other.0, other.1]
    }
}

impl Game {
    fn render(&mut self, args: &RenderArgs) {
        use graphics::{clear, line, rectangle};
        use graphics::math::Matrix2d;

        fn rectangle_lines(color: [f32; 4], rect: [f64; 4], transform: Matrix2d, gl: &mut GlGraphics) {
            let lines = [
                [rect[0], rect[1], rect[2], rect[1]],
                [rect[2], rect[1], rect[2], rect[3]],
                [rect[2], rect[3], rect[0], rect[3]],
                [rect[0], rect[3], rect[0], rect[1]],
            ];

            for l in lines.into_iter() {
                line(color, 0.5, *l, transform, gl);
            }
        }

        let dragging_region = [
            self.state.dragging_source.0,
            self.state.dragging_source.1,
            self.state.mouse.0,
            self.state.mouse.1,
        ];
//        let (x, y) = ((args.width / 2) as f64,
//                      (args.height / 2) as f64);

        let dragging = self.state.dragging;
        let game_state = &self.state;

        self.gl.draw(args.viewport(), |context, gl| {
            clear([0.0, 0.0, 0.0, 1.0], gl);

            for room in game_state.rooms.iter() {
                let brightness = 0.25 + 0.75 * (room.animation as f32 / 32.0);
                let color = [brightness, brightness, brightness, 1.0];
                rectangle_lines(color, room.rectangle, context.transform, gl);
            }

            if dragging {
                let r = rectangle::rectangle_by_corners(
                    dragging_region[0],
                    dragging_region[1],
                    dragging_region[2],
                    dragging_region[3],
                );

                rectangle_lines([0.25, 1.0, 0.25, 1.0], dragging_region, context.transform, gl);
                rectangle([0.25, 1.0, 0.25, 0.01], r, context.transform, gl);
            }
        });
    }

    fn update(&mut self, _args: &UpdateArgs) {
        for room in self.state.rooms.iter_mut() {
            if room.animation > 0 {
                room.animation -= 1;
            }
        }
    }

    fn press(&mut self, args: &Button) {
        if let &Button::Mouse(MouseButton::Left) = args {
            self.state.dragging = true;
            self.state.dragging_source = self.state.mouse;
        }

        if let &Button::Keyboard(Key::R) = args {
            self.state.rooms.clear();
        }
    }

    fn release(&mut self, args: &Button) {
        if let &Button::Mouse(MouseButton::Left) = args {
            self.state.dragging = false;

            let new_room = Room {
                rectangle: self.state.dragging_source.to(self.state.mouse),
                animation: 32,
            };

            self.state.rooms.push(new_room);
        }
    }

    fn mouse_cursor(&mut self, x: f64, y: f64) {
        self.state.mouse = (x, y);
    }
}

#[derive(Debug, Fail)]
enum GameError {
    #[fail(display = "cannot create game window: {}", reason)]
    WindowError { reason: String }
}

pub fn run() -> Result<(), Error> {
    let opengl_version = OpenGL::V3_2;

    let mut window: GlutinWindow = WindowSettings::new("hellopiston", [640, 480])
        .opengl(opengl_version)
        .exit_on_esc(true)
        .build()
        .map_err(|err| GameError::WindowError { reason: err })?;

    let game_state = {
        let state_file = std::fs::File::open("state.json");

        match state_file {
            Ok(state_file) =>
                serde_json::from_reader::<_, GameState>(state_file)
                    .context("Cannot deserialize game state file")?,
            Err(err) =>
                if err.kind() == std::io::ErrorKind::NotFound {
                    GameState::default()
                } else {
                    return Err(Error::from(err).context("Cannot open game state file").into())
                },
        }
    };

    let mut game = Game {
        gl: GlGraphics::new(opengl_version),
        state: game_state,
    };

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

    let state_file = std::fs::File::create("state.json")
        .context("Cannot create file to save game state")?;
    serde_json::to_writer(state_file, &game.state)
        .context("Cannot write game state to file")?;

    Ok(())
}
