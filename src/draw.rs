extern crate opengl_graphics;
extern crate specs;

use specs::prelude::{World, VecStorage, ReadStorage, ReadExpect, Join, System, Entities, RunNow};
use piston::input::RenderArgs;
use opengl_graphics::GlGraphics;
use MouseInput;
use animate::{Animation, RoomAnimation};
use physics::InRoom;

#[derive(Debug, Component, Serialize, Deserialize, Clone, Copy)]
#[storage(VecStorage)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy)]
#[storage(VecStorage)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy)]
#[storage(VecStorage)]
pub struct Room;

//#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
//pub struct Animator {
//    current: u32,
//    limit: u32,
//}

fn rectangle_to_lines(rect: [f64; 4]) -> [[f64; 4]; 4] {
    let x1 = rect[0];
    let y1 = rect[1];
    let x2 = rect[0] + rect[2];
    let y2 = rect[1] + rect[3];

    [
        [x1, y1, x2, y1],
        [x2, y1, x2, y2],
        [x2, y2, x1, y2],
        [x1, y2, x1, y1],
    ]
}

pub struct ClearScreen<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for ClearScreen<'b> {
    type SystemData = ();

    fn run(&mut self, (): Self::SystemData) {
        use graphics::clear;

        self.gl_graphics.draw(self.render_args.viewport(), |_context, gl| {
            clear([0.0, 0.0, 0.0, 1.0], gl);
        });
    }
}

pub struct DrawRooms<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for DrawRooms<'b> {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Size>,
        ReadStorage<'a, Animation<RoomAnimation>>,
    );

    fn run(&mut self, (entities, positions, sizes, animations): Self::SystemData) {
        for (_entity, position, size, animation) in (&*entities, &positions, &sizes, &animations).join() {
            if size.width < 5.0 || size.height < 5.0 {
                continue;
            }

            let room_rectangle = [
                position.x as f64, position.y as f64,
                size.width as f64, size.height as f64,
            ];

            let brightness = 0.25 + 0.75 * ((32 - animation.current) as f32 / 32.0);
            let color = [brightness, brightness, brightness, 1.0];

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

//                rectangle([0.2, 0.2, 0.5, 0.01], room_rectangle, context.transform, gl);

                for l in rectangle_to_lines(room_rectangle).iter() {
                    line(color, 0.5, *l, context.transform, gl);
                }
            });
        }
    }
}

pub struct DrawBalls<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for DrawBalls<'b> {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, InRoom>,
    );

    fn run(&mut self, (entities, positions, in_rooms): Self::SystemData) {
        for (_entity, position, in_room) in (&*entities, &positions, &in_rooms).join() {
            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Transformed, circle_arc};

                let rect = [position.x - 10.0, position.y - 10.0, 20.0, 20.0];
                let context = context.trans(room_position.x, room_position.y);
//                rectangle([0.2, 0.2, 0.5, 0.01], room_rectangle, context.transform, gl);

                // Why can't I use 2.0 instead of 1.9999? Who knows.
                circle_arc([0.0, 0.0, 1.0, 1.0], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
                           rect, context.transform, gl);
            });
        }
    }
}

pub struct DrawSelectionBox<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for DrawSelectionBox<'b> {
    type SystemData = ReadExpect<'a, MouseInput>;

    fn run(&mut self, mouse_input: Self::SystemData) {
        self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
            if mouse_input.dragging {
                use graphics::{rectangle, line};

                let rect = mouse_input.selection_rectangle();

                rectangle([0.25, 1.0, 0.25, 0.01], rect, context.transform, gl);
                for l in rectangle_to_lines(rect).iter() {
                    line([0.25, 1.0, 0.25, 1.0], 0.5, *l, context.transform, gl);
                }
            }
        });
    }
}

pub fn run_draw_systems(specs_world: &mut World,
                        gl_graphics: &mut GlGraphics,
                        render_args: RenderArgs) {
    ClearScreen { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawRooms { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawBalls { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawSelectionBox { gl_graphics, render_args }
        .run_now(&mut specs_world.res);
}
