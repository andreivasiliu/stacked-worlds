extern crate opengl_graphics;
extern crate specs;

use specs::prelude::{World, VecStorage, ReadStorage, ReadExpect, Join, System, Entities, RunNow};
use piston::input::RenderArgs;
use graphics::Context;
use opengl_graphics::GlGraphics;
use animate::{Animation, RoomAnimation};
use physics::InRoom;
use physics::CollisionSet;
use nalgebra::Vector2;
use control::Jump;
use physics::Aim;
use control::ChainLink;
use input::InputState;
use physics::Room;
use specs::WriteExpect;
use input::PlayerController;
use UpdateDeltaTime;

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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ShapeClass {
    Ball,
    ChainLink,
}

#[derive(Component, Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[storage(VecStorage)]
pub struct Shape {
    pub size: f64,
    pub class: ShapeClass,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,

    pub target_x: f64,
    pub target_y: f64,
    pub target_zoom: f64,

    pub mode: CameraMode,
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,

            target_x: 0.0,
            target_y: 0.0,
            target_zoom: 1.0,

            mode: CameraMode::EditorMode,
        }
    }

    pub fn apply_transform(self, context: Context) -> Context {
        use graphics::Transformed;

        context.trans(-self.x, -self.y)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Normal,
    EditorMode,
}

impl CameraMode {
    pub fn next_mode(&self) -> Self {
        match *self {
            CameraMode::Normal => CameraMode::EditorMode,
            CameraMode::EditorMode => CameraMode::Normal,
        }
    }
}

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
        ReadStorage<'a, Room>,
        ReadStorage<'a, InRoom>,
        ReadExpect<'a, InputState>,
        ReadExpect<'a, Camera>,
    );

    fn run(&mut self, (entities, positions, sizes, animations, rooms, in_rooms, input_state, camera): Self::SystemData) {
        // Draw room borders
        for (entity, position, size, animation, _room) in (&*entities, &positions, &sizes, &animations, &rooms).join() {
            if size.width < 5.0 || size.height < 5.0 {
                continue;
            }

            let room_rectangle = [
                position.x, position.y,
                size.width, size.height,
            ];

            let mut brightness: f32 = 0.25 + 0.75 * ((32 - animation.current) as f32 / 32.0);

            if input_state.room_focused == Some(entity) {
                brightness = brightness.max(0.4);
            }

            let color = [brightness, brightness, brightness, 1.0];

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

                let context = camera.apply_transform(context);

//                rectangle([0.2, 0.2, 0.5, 0.01], room_rectangle, context.transform, gl);

                for l in rectangle_to_lines(room_rectangle).iter() {
                    line(color, 0.5, *l, context.transform, gl);
                }
            });
        }

        // Draw terrain entities in rooms
        for (_entity, position, size, animation, in_room) in (&*entities, &positions, &sizes, &animations, &in_rooms).join() {
            let room_position = match positions.get(entities.entity(in_room.room_entity)) {
                Some(room_position) => room_position,
                None => continue,
            };

            let terrain_rectangle = [
                room_position.x + position.x, room_position.y + position.y,
                size.width, size.height,
            ];

            let brightness = 0.25 + 0.75 * ((32 - animation.current) as f32 / 32.0);
            let color = [brightness, brightness, brightness, 1.0];

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{line, rectangle};

                let context = camera.apply_transform(context);

                rectangle([0.05, 0.05, 0.05, 1.0], terrain_rectangle, context.transform, gl);

                for l in rectangle_to_lines(terrain_rectangle).iter() {
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
        ReadStorage<'a, Shape>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, CollisionSet>,
        ReadStorage<'a, Jump>,
        ReadStorage<'a, Aim>,
        ReadExpect<'a, Camera>,
    );

    fn run(&mut self, (entities, positions, shapes, in_rooms, collision_sets, jumps, aims, camera): Self::SystemData) {
        for (_entity, position, shape, in_room) in (&*entities, &positions, &shapes, &in_rooms).join() {
            if shape.class != ShapeClass::Ball {
                continue
            }

            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Transformed, circle_arc};

                let context = camera.apply_transform(context);

                let size = shape.size;
                let rect = [position.x - size, position.y - size, size * 2.0, size * 2.0];
                let context = context.trans(room_position.x, room_position.y);

                // Why can't I use 2.0 instead of 1.9999? Who knows.
                circle_arc([0.3, 0.3, 1.0, 1.0], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
                           rect, context.transform, gl);
            });
        }

        for (_entity, position, in_room, collision_set) in (&*entities, &positions, &in_rooms, &collision_sets).join() {
            if collision_set.time_since_collision > 0.2 {
                continue;
            }

            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            let normal = Vector2::new(collision_set.last_collision_normal.0,
                                      collision_set.last_collision_normal.1).normalize();

            let (x1, y1) = (position.x + room_position.x, position.y + room_position.y);
            let (x2, y2) = (x1 + normal.x * 10.0, y1 + normal.y * 10.0);

            let alpha = (0.2 - collision_set.time_since_collision) / 0.2;

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

                let context = camera.apply_transform(context);

                line([0.0, 1.0, 0.0, alpha as f32], 0.5, [x1, y1, x2, y2], context.transform, gl);
            });
        }

        for (_entity, position, in_room, jump) in (&*entities, &positions, &in_rooms, &jumps).join() {
            if jump.cooldown <= 0.0 {
                continue;
            }

            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Transformed, circle_arc};

                let context = camera.apply_transform(context);

                let rect = [position.x - 7.0, position.y - 7.0, 14.0, 14.0];
                let context = context.trans(room_position.x, room_position.y);

                let alpha = jump.cooldown / 0.2;

                circle_arc([0.7, 0.7, 1.0, alpha as f32], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
                           rect, context.transform, gl);
            });
        }

        // Draw aiming reticule
        for (_entity, position, in_room, aim) in (&*entities, &positions, &in_rooms, &aims).join() {
            if !aim.aiming {
                continue;
            }

            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            let direction = Vector2::new(aim.aiming_toward.0, aim.aiming_toward.1).normalize();
            let position = Vector2::new(position.x + room_position.x, position.y + room_position.y);

            let p1 = position + direction * 4.0;
            let p2 = position + direction * 8.0;

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

                let context = camera.apply_transform(context);

                line([1.0, 0.3, 0.3, 1.0], 0.5,
                     [p1.x, p1.y, p2.x, p2.y], context.transform, gl);

                if let Some(aiming_at_point) = aim.aiming_at_point {
                    let p3 = position + direction * 15.0;
                    let p4 = Vector2::new(aiming_at_point.0 + room_position.x,
                                          aiming_at_point.1 + room_position.y);

                    line([0.5, 0.0, 0.0, 0.3], 0.5,
                         [p3.x, p3.y, p4.x, p4.y], context.transform, gl);
                }
            });
        }
    }
}

pub struct DrawChainLinks<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for DrawChainLinks<'b> {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Shape>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, ChainLink>,
        ReadExpect<'a, Camera>,
    );

    fn run(&mut self, (entities, positions, shapes, in_rooms, chain_links, camera): Self::SystemData) {
        for (_entity, position, shape, in_room, chain_link) in (&*entities, &positions, &shapes, &in_rooms, &chain_links).join() {
            if shape.class != ShapeClass::ChainLink {
                continue;
            }

            let room_entity = entities.entity(in_room.room_entity);

            let room_position = match positions.get(room_entity) {
                Some(room_position) => room_position,
                None => continue,
            };

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Transformed, circle_arc};

                let context = camera.apply_transform(context);

                let size = shape.size;
                let rect = [position.x - size, position.y - size, size * 2.0, size * 2.0];
                let context = context.trans(room_position.x, room_position.y);
                let animation = chain_link.destruction_animation as f32;

                let brightness = if chain_link.expire {
                    if animation >= 0.2 {
                        0.3
                    } else if animation >= 0.1 {
                        0.3 + 0.7 * (1.0 - (animation / 0.1 - 1.0))
                    } else {
                        1.0
                    }
                } else {
                    (0.3 + 5.0 * chain_link.creation_animation as f32).min(1.0)
                };

                circle_arc([0.3, 0.3, brightness, 1.0],
                           0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
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
    type SystemData = (
        ReadExpect<'a, InputState>,
        ReadExpect<'a, Camera>,
    );

    fn run(&mut self, (input_state, camera): Self::SystemData) {
        self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
            if let Some(selection_box) = input_state.mouse.selection_box() {
                use graphics::{rectangle, line};

                let context = camera.apply_transform(context);

                let rect = selection_box.to_rectangle().snap_to_grid(16).to_array();

                rectangle([0.25, 1.0, 0.25, 0.01], rect, context.transform, gl);
                for l in rectangle_to_lines(rect).iter() {
                    line([0.25, 1.0, 0.25, 1.0], 0.5, *l, context.transform, gl);
                }
            }
        });
    }
}

pub struct SetCameraTarget<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for SetCameraTarget<'b> {
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, Camera>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, PlayerController>,
    );

    fn run(&mut self, (entities, mut camera, positions, in_rooms, player_controllers): Self::SystemData) {
        match camera.mode {
            // Follow the player.
            CameraMode::Normal => {
                // Get the first entity that has a PlayerController
                // FIXME: Maybe add a specific 'focusable' component instead?
                for (_entity, position, in_room, _player_controller) in (&*entities, &positions, &in_rooms, &player_controllers).join() {
                    let room_entity = entities.entity(in_room.room_entity);

                    let room_position = match positions.get(room_entity) {
                        Some(room_position) => room_position,
                        None => continue,
                    };

                    let screen_halfwidth = self.render_args.width as f64 / 2.0;
                    let screen_halfheight = self.render_args.height as f64 / 2.0;

                    camera.target_x = room_position.x + position.x - screen_halfwidth;
                    camera.target_y = room_position.y + position.y - screen_halfheight;
                    camera.target_zoom = 2.0;

                    break;
                }
            },

            // Static zoomed-out camera.
            CameraMode::EditorMode => {
                camera.target_x = 0.0;
                camera.target_y = 0.0;
                camera.target_zoom = 1.0;
            },
        }
    }
}

pub struct UpdateCamera;

impl <'a> System<'a> for UpdateCamera {
    type SystemData = (
        WriteExpect<'a, Camera>,
        ReadExpect<'a, UpdateDeltaTime>,
    );

    fn run(&mut self, (mut camera, _delta_time): Self::SystemData) {
        // TODO: Make camera movement slower and based on delta_time
        camera.x += (camera.target_x - camera.x) * 1.0;
        camera.y += (camera.target_y - camera.y) * 1.0;
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

    DrawChainLinks { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawSelectionBox { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    SetCameraTarget { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    UpdateCamera.run_now(&mut specs_world.res);
}
