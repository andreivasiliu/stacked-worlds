extern crate opengl_graphics;
extern crate specs;

use specs::prelude::{World, VecStorage, ReadStorage, ReadExpect, Join, System, Entities, RunNow};
use specs::world::Index;
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
use shift::Shifter;

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

    pub panning_direction: Option<(f64, f64)>,

    pub mode: CameraMode,

    pub phase_overlay: Option<PhaseOverlay>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Screen {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct PhaseOverlay {
    pub sphere_center: (f64, f64),
    pub sphere_size: f64,
    pub sphere_state: PhaseSphereState,
    pub source_room: Index,
    pub target_room: Index,
    pub target_room_offset: (f64, f64),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum PhaseSphereState {
    /// A phase-shift bubble is forming
    Forming,
    /// A phase-shift bubble is entered, and expanding to the edges of the screen
    Expanding,
    /// A phase-shift bubble has been cancelled, and is retracting
    Retracting,
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

            panning_direction: None,

            mode: CameraMode::Normal,

            phase_overlay: None,
        }
    }

    pub fn apply_stencil(self, gl: &mut GlGraphics, mut context: Context, phase_overlay: &PhaseOverlay, offset: bool) -> Context {
        use graphics::Transformed;
        use graphics::Graphics;
        use graphics::draw_state::DrawState;
        use graphics::ellipse::Ellipse;

        gl.clear_stencil(0);

        // Draw on stencil buffer
        context.draw_state = DrawState::new_clip();

        let half_size = 200.0 * phase_overlay.sphere_size;

        let rect = [-half_size, -half_size, half_size * 2.0, half_size * 2.0];

        let sphere_context = context
            .trans(phase_overlay.sphere_center.0, phase_overlay.sphere_center.1);

        let sphere_context = if offset {
            sphere_context.trans(phase_overlay.target_room_offset.0, phase_overlay.target_room_offset.1)
        } else {
            sphere_context
        };

        Ellipse::new([1.0, 1.0, 1.0, 1.0]).draw(rect, &sphere_context.draw_state, sphere_context.transform, gl);

        // Apply stencil
        context.draw_state = DrawState::new_inside();
        context
    }

    // Modifies the context according to the camera's coordinates
    // On 'Normal' camera mode, helps draw the target room during a phase shift overlay
    //
    // Forming:
    //   Source room:
    //     - full alpha (static)
    //     - no stencil
    //   Target room:
    //     - semi alpha (static, 0.5)
    //     - inside stencil
    //     - camera offset
    //
    // Expanding:
    //   Source room:
    //     - semi alpha (dynamic, 0.5 to 1.0)
    //     - inside stencil
    //   Target room:
    //     - no stencil
    //     - semi alpha (dynamic, 1.0 to 0.0)
    //     - camera offset

    pub fn apply_transform(self, gl: &mut GlGraphics, context: Context, room: Option<Index>) -> (Context, f32) {
        use graphics::Transformed;

        let context = context.trans(-self.x, -self.y);

        // Overlay the target room when shifting/sensing
        if let Some(room) = room {
            if let Camera { phase_overlay: Some(phase_overlay), mode: CameraMode::Normal, .. } = self {
                let expanding = phase_overlay.sphere_state == PhaseSphereState::Expanding;

                return if phase_overlay.target_room == room {
                    // Draw the target room on top of the current one
                    let context = context.trans(
                        -phase_overlay.target_room_offset.0,
                        -phase_overlay.target_room_offset.1,
                    );

                    if expanding {
                        (context, 1.0 - phase_overlay.sphere_size as f32 / 3.0)
                    } else {
                        (self.apply_stencil(gl, context, &phase_overlay, true), 0.5)
                    }
                } else if phase_overlay.source_room == room {
                    if expanding {
                        let alpha = 0.5 + phase_overlay.sphere_size as f32 / 3.0 / 2.0;

                        (self.apply_stencil(gl, context, &phase_overlay, false), alpha)
                    } else {
                        (context, 1.0)
                    }
                } else {
                    (context, 1.0)
                };
            }
        }

        (context, 1.0)
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

pub struct DrawPhaseSphere<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for DrawPhaseSphere<'b> {
    type SystemData = ReadExpect<'a, Camera>;

    fn run(&mut self, camera: Self::SystemData) {
        if let Some(phase_overlay) = camera.phase_overlay {
            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Transformed, circle_arc};

                let (context, alpha) = camera.apply_transform(gl, context, None);

                let half_size = 200.0 * phase_overlay.sphere_size;
                let center = phase_overlay.sphere_center;

                let rect = [-half_size, -half_size, half_size * 2.0, half_size * 2.0];
                let context = context.trans(center.0, center.1);

                circle_arc([0.7, 1.0, 0.7, alpha], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
                           rect, context.transform, gl);
            });
        }
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

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

                let (context, alpha) = camera.apply_transform(gl, context, Some(entity.id()));

                let color = [brightness, brightness, brightness, alpha];

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

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::{Rectangle, Line};

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

                //rectangle([0.05, 0.05, 0.05, 1.0], terrain_rectangle, context.transform, gl);
                Rectangle::new([0.05, 0.05, 0.05, alpha])
                    .draw(terrain_rectangle, &context.draw_state, context.transform, gl);

                let color = [brightness, brightness, brightness, alpha];

                for l in rectangle_to_lines(terrain_rectangle).iter() {
                    Line::new(color, 0.5)
                        .draw(*l, &context.draw_state, context.transform, gl);
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
                use graphics::{Transformed, CircleArc};

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

                let size = shape.size;
                let rect = [position.x - size, position.y - size, size * 2.0, size * 2.0];
                let context = context.trans(room_position.x, room_position.y);

                CircleArc::new([0.3, 0.3, 1.0, alpha], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI)
                    .draw(rect, &context.draw_state, context.transform, gl);
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

            let collision_alpha = ((0.2 - collision_set.time_since_collision) / 0.2) as f32;

            self.gl_graphics.draw(self.render_args.viewport(), |context, gl| {
                use graphics::line;

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

                line([0.0, 1.0, 0.0, collision_alpha * alpha],
                     0.5, [x1, y1, x2, y2], context.transform, gl);
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

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

                let rect = [position.x - 7.0, position.y - 7.0, 14.0, 14.0];
                let context = context.trans(room_position.x, room_position.y);

                let jump_alpha = jump.cooldown as f32 / 0.2;

                circle_arc([0.7, 0.7, 1.0, jump_alpha * alpha], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI,
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

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

                line([1.0, 0.3, 0.3, 1.0 * alpha], 0.5,
                     [p1.x, p1.y, p2.x, p2.y], context.transform, gl);

                if let Some(aiming_at_point) = aim.aiming_at_point {
                    let p3 = position + direction * 15.0;
                    let p4 = Vector2::new(aiming_at_point.0 + room_position.x,
                                          aiming_at_point.1 + room_position.y);

                    line([0.5, 0.0, 0.0, 0.3 * alpha], 0.5,
                         [p3.x, p3.y, p4.x, p4.y], context.transform, gl);

                    if (p4 - position).norm() >= 150.0 {
                        let p3 = position + direction * 149.0;
                        let p4 = position + direction * 151.0;

                        line([0.7, 0.7, 0.7, 1.0 * alpha], 1.0,
                             [p3.x, p3.y, p4.x, p4.y], context.transform, gl);
                    }

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
                use graphics::{Transformed, CircleArc};

                let (context, alpha) = camera.apply_transform(gl, context, Some(in_room.room_entity));

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

                CircleArc::new([0.3, 0.3, brightness, 1.0 * alpha], 0.5, 0.0, 1.9999 * ::std::f64::consts::PI)
                    .draw(rect, &context.draw_state, context.transform, gl);
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
            if let Some(selection_box) = input_state.world_mouse.selection_box() {
                use graphics::{rectangle, line};

                let (context, _alpha) = camera.apply_transform(gl, context, None);

                let rect = selection_box
                    .to_rectangle()
                    .snap_to_grid(16)
                    .to_array();

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
        ReadStorage<'a, Size>,
        ReadStorage<'a, InRoom>,
        ReadStorage<'a, Shifter>,
        ReadStorage<'a, PlayerController>,
    );

    fn run(&mut self, (entities, mut camera, positions, sizes, in_rooms, shifters, player_controllers): Self::SystemData) {
        // Camera panning overrides any other camera targets
        if camera.panning_direction.is_some() {
            return;
        }

        match camera.mode {
            // Follow the player.
            CameraMode::Normal => {
                // Get the first entity that has a PlayerController
                // FIXME: Maybe add a specific 'focusable' component instead?
                for (entity, position, in_room, _player_controller) in (&*entities, &positions, &in_rooms, &player_controllers).join() {
                    let room_entity = entities.entity(in_room.room_entity);

                    let room_position = match positions.get(room_entity) {
                        Some(room_position) => room_position,
                        None => continue,
                    };

                    let room_size = match sizes.get(room_entity) {
                        Some(room_size) => room_size,
                        None => continue,
                    };

                    let screen_halfwidth = self.render_args.width as f64 / 2.0;
                    let screen_halfheight = self.render_args.height as f64 / 2.0;

                    camera.target_y = room_position.y + position.y - screen_halfheight;

                    camera.target_x = if room_size.width < screen_halfwidth * 2.0 * 0.8 {
                        // If the room's width is smaller than the screen, then no camera movement
                        // is needed.
                        // The middle of the room should always be at the middle of the screen.
                        room_size.width / 2.0 - screen_halfwidth
                    } else if position.x < screen_halfwidth * 0.8 {
                        // Otherwise, if the player's at the left-side of the room,
                        // don't focus on the player anymore, stop the camera at this point.
                        screen_halfwidth * (0.8 - 1.0)
                    } else if position.x > room_size.width - screen_halfwidth * 0.8 {
                        // Same for right side.
                        room_size.width - screen_halfwidth * (1.8)
                    } else {
                        // Otherwise, focus on the player.
                        position.x - screen_halfwidth
                    } + room_position.x;

                    camera.target_y = if room_size.height < screen_halfheight * 2.0 * 0.8 {
                        room_size.height / 2.0 - screen_halfheight
                    } else if position.y < screen_halfheight * 0.8 {
                        screen_halfheight * (0.8 - 1.0)
                    } else if position.y > room_size.height - screen_halfheight * 0.8 {
                        room_size.height - screen_halfheight * (1.8)
                    } else {
                        position.y - screen_halfheight
                    } + room_position.y;

                    camera.target_zoom = 2.0;

                    if let Some(shifter) = shifters.get(entity) {
                        if shifter.sensing && camera.phase_overlay.is_none() {
                            if let Some(target_room) = shifter.target_room {
                                if let Some(target_room_position) = positions.get(entities.entity(target_room)) {
                                    camera.phase_overlay = Some(PhaseOverlay {
                                        sphere_center: (room_position.x + position.x, room_position.y + position.y),
                                        sphere_size: 0.0,
                                        sphere_state: PhaseSphereState::Forming,
                                        source_room: room_entity.id(),
                                        target_room,
                                        target_room_offset: (
                                            target_room_position.x - room_position.x,
                                            target_room_position.y - room_position.y,
                                        ),
                                    });
                                }
                            }
                        } else if !shifter.sensing {
                            let mut update_camera = None;

                            if let Some(ref mut phase_overlay) = camera.phase_overlay {
                                if phase_overlay.sphere_state != PhaseSphereState::Expanding {
                                    phase_overlay.sphere_state = PhaseSphereState::Expanding;
                                    // We've now shifted into the target room
                                    // Switch offsets so that we draw the source room on top of the target room instead
                                    phase_overlay.sphere_center = (
                                        phase_overlay.sphere_center.0 + phase_overlay.target_room_offset.0,
                                        phase_overlay.sphere_center.1 + phase_overlay.target_room_offset.1,
                                    );

                                    // Instantly offset the camera, as opposed to setting camera.target_x/y
                                    // This is to give the illusion that we shifted there
                                    update_camera = Some(phase_overlay.target_room_offset);

                                    phase_overlay.target_room_offset = (
                                        -phase_overlay.target_room_offset.0,
                                        -phase_overlay.target_room_offset.1,
                                    );

                                    use std::mem::swap;
                                    swap(&mut phase_overlay.source_room, &mut phase_overlay.target_room);
                                }
                            }

                            if let Some(offset) = update_camera {
                                camera.x += offset.0;
                                camera.y += offset.1;
                            }
                        }
                    }

                    break;
                }
            },

            // Static zoomed-out camera.
            CameraMode::EditorMode => {
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

    fn run(&mut self, (mut camera, delta_time): Self::SystemData) {
        // Edge panning is enabled while dragging with the mouse
        if let Some(panning_direction) = camera.panning_direction {
            camera.target_x += panning_direction.0 * delta_time.dt * 400.0;
            camera.target_y += panning_direction.1 * delta_time.dt * 400.0;
        }

        camera.x += (camera.target_x - camera.x) * 0.9_f64.powf(1.0 / (delta_time.dt * 10.0));
        camera.y += (camera.target_y - camera.y) * 0.9_f64.powf(1.0 / (delta_time.dt * 10.0));

        let mut disable_overlay = false;

        if let Some(ref mut phase_overlay) = camera.phase_overlay {
            let size = phase_overlay.sphere_size;
            let dt = delta_time.dt * 200.0;

            let size = match phase_overlay.sphere_state {
                PhaseSphereState::Forming => 1.0 - ((1.0 - size) * 0.9_f64.powf(dt)),
                PhaseSphereState::Expanding => size * 1.05_f64.powf(dt),
                PhaseSphereState::Retracting => size * 0.5_f64.powf(dt),
            };

            phase_overlay.sphere_size = match phase_overlay.sphere_state {
                PhaseSphereState::Forming => {
                    if size > 0.999 {
                        1.0
                    } else {
                        size
                    }
                },
                PhaseSphereState::Expanding => {
                    if size > 5.0 {
                        disable_overlay = true;
                    }
                    size
                },
                PhaseSphereState::Retracting => {
                    if size < 0.001 {
                        disable_overlay = true;
                        0.0
                    }else {
                        size
                    }
                }
            };
        }

        if disable_overlay {
            camera.phase_overlay = None;
        }
    }
}

pub struct SetScreenSize<'a> {
    pub gl_graphics: &'a mut GlGraphics,
    pub render_args: RenderArgs,
}

impl <'a, 'b> System<'a> for SetScreenSize<'b> {
    type SystemData = WriteExpect<'a, Screen>;

    fn run(&mut self, mut screen: Self::SystemData) {
        screen.width = self.render_args.width as f64;
        screen.height = self.render_args.height as f64;
    }
}

pub fn run_draw_systems(specs_world: &mut World,
                        gl_graphics: &mut GlGraphics,
                        render_args: RenderArgs) {
    SetScreenSize { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    SetCameraTarget { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    UpdateCamera.run_now(&mut specs_world.res);

    ClearScreen { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawRooms { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawBalls { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawChainLinks { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawPhaseSphere { gl_graphics, render_args }
        .run_now(&mut specs_world.res);

    DrawSelectionBox { gl_graphics, render_args }
        .run_now(&mut specs_world.res);
}
