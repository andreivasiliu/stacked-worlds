use specs::prelude::{System, DenseVecStorage, WriteStorage, ReadStorage, WriteExpect, Entities, Join};
use std::collections::VecDeque;
use super::{Button, Key, MouseButton};
use std::collections::HashSet;
use std::collections::HashMap;
use physics::Aim;
use draw::Position;
use physics::InRoom;
use edit::{EditorController, EditEvent};

pub enum InputEvent {
    PressEvent(Button),
    ReleaseEvent(Button),
    MotionEvent(f64, f64),
}

#[derive(Default, Copy, Clone)] // FIXME: derive more
pub struct MouseState {
    pub position: (f64, f64),
    pub dragging_from: Option<(f64, f64)>,
}

impl MouseState {
    pub fn selection_box(&self) -> Option<SelectionBox> {
        self.dragging_from.map(|dragging_from| {
            SelectionBox {
                x1: dragging_from.0,
                y1: dragging_from.1,
                x2: self.position.0,
                y2: self.position.1,
            }
        })
    }
}

/// The coordinates of the mouse selection box. The first set of coordinates (`x1`, `x2`) are for
/// the point the selection box is being dragged from.
#[derive(Debug)] // FIXME: derive more
pub struct SelectionBox {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl SelectionBox {
    /// Return an array of type [f64; 4] with [x1, y1, y1, y2].
    pub fn to_array(&self) -> [f64; 4] {
        [self.x1, self.y1, self.x2, self.y2]
    }

    pub fn to_rectangle(&self) -> SelectionRectangle {
        let x1 = self.x1.min(self.x2);
        let y1 = self.y1.min(self.y2);
        let x2 = self.x1.max(self.x2);
        let y2 = self.y1.max(self.y2);

        SelectionRectangle {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }
}

/// Similar to `SelectionBox`, but with coordinates flipped so that width and height are always
/// positive. Information about where the box is being dragged from is lost.
#[derive(Debug)] // FIXME: derive more
pub struct SelectionRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl SelectionRectangle {
    /// Enlarge the rectangle so that all corners snap to a grid
    pub fn snap_to_grid(&self, cell_size: i32) -> Self {
        assert_ne!(cell_size, 0);

        // Snap the top-left corner to the grid
        let x = (self.x as i32 / cell_size * cell_size) as f64;
        let y = (self.y as i32 / cell_size * cell_size) as f64;

        // Bring back the bottom-right corner where it originally was by compensating
        let width = self.width + (self.x - x as f64);
        let height = self.height + (self.y - y as f64);

        // Snap the bottom-right corner to the grid
        let width = ((width as i32 / cell_size + 1) * cell_size) as f64;
        let height = ((height as i32 / cell_size + 1) * cell_size) as f64;

        SelectionRectangle { x, y, width, height }
    }

    /// Return an array of type [f64; 4] with [x, y, width, height].
    pub fn to_array(&self) -> [f64; 4] {
        [self.x, self.y, self.width, self.height]
    }
}

pub struct InputEvents {
    pub events: VecDeque<InputEvent>,
}

impl InputEvents {
    pub fn new() -> Self {
        InputEvents {
            events: VecDeque::with_capacity(32),
        }
    }
}

pub struct InputState {
    pub button_held: HashSet<Button>,
    pub button_pressed: HashMap<Button, i32>,
    pub mouse: MouseState,
    pub selected_region: Option<SelectionBox>,
    // Consider adding mouse motion events
    // Consider changing selected_region to a per-event state
}

impl InputState {
    pub fn new() -> Self {
        InputState {
            button_held: HashSet::with_capacity(16),
            button_pressed: HashMap::with_capacity(16),
            mouse: MouseState::default(),
            selected_region: None,
        }
    }

    // FIXME: The fact that this mutates InputState is surprising; think of another name.
    // Maybe handle_button, pop_button, pop_press_event_or_held.
    pub fn button_pressed_or_held(&mut self, button: &Button) -> bool {
        if let Some(_press_count) = self.button_pressed.remove(&button) {
            return true;
        } else {
            return self.button_held.contains(&button);
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    None,
    Left,
    Right,
}

impl Default for Movement {
    fn default() -> Movement {
        Movement::None
    }
}

#[derive(Component, Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[storage(DenseVecStorage)]
pub struct PlayerController {
    pub moving: Movement,
    pub jumping: bool,
    pub hooking: bool,
    pub hook_established: bool,
}

pub struct InputEventsToState;

impl <'a> System<'a> for InputEventsToState {
    type SystemData = (
        WriteExpect<'a, InputEvents>,
        WriteExpect<'a, InputState>,
    );

    fn run(&mut self, (mut input_events, mut input_state): Self::SystemData) {
        input_state.button_pressed.clear();
        input_state.selected_region = None;

        while let Some(input_event) = input_events.events.pop_front() {
            match input_event {
                InputEvent::PressEvent(button) => {
                    input_state.button_held.insert(button);

                    if let Button::Mouse(MouseButton::Left) = button {
                        input_state.mouse.dragging_from = Some(input_state.mouse.position);
                    }

                    let mut press_count = input_state.button_pressed
                        .entry(button).or_insert(0);
                    *press_count += 1;

                },
                InputEvent::ReleaseEvent(button) => {
                    input_state.button_held.remove(&button);

                    if let Button::Mouse(MouseButton::Left) = button {
                        input_state.selected_region = input_state.mouse.selection_box();
                        input_state.mouse.dragging_from = None;
                    }
                },
                InputEvent::MotionEvent(x, y) => {
                    input_state.mouse.position = (x, y);
                },
            };
        }
    }
}

pub struct PlayerControllerInput;

impl <'a> System<'a> for PlayerControllerInput {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, PlayerController>,
        WriteExpect<'a, InputState>,
    );

    fn run(&mut self, (entities, mut player_controllers, mut input_state): Self::SystemData) {
        let moving_left = input_state.button_pressed_or_held(&Button::Keyboard(Key::Left)) ||
            input_state.button_pressed_or_held(&Button::Keyboard(Key::A));
        let moving_right = input_state.button_pressed_or_held(&Button::Keyboard(Key::Right)) ||
            input_state.button_pressed_or_held(&Button::Keyboard(Key::D));
        let jumping = input_state.button_pressed_or_held(&Button::Keyboard(Key::Space));

        let movement = match (moving_left, moving_right) {
            (true, false) => Movement::Left,
            (false, true) => Movement::Right,
            (true, true) => Movement::None,
            (false, false) => Movement::None,
        };

        let hooking = input_state.button_pressed_or_held(&Button::Mouse(MouseButton::Right));

        for (_entity, mut player_controller) in (&*entities, &mut player_controllers).join() {
            player_controller.moving = movement;
            player_controller.jumping = jumping;
            player_controller.hooking = hooking;
        }
    }
}

pub struct EditorControllerInput;

impl <'a> System<'a> for EditorControllerInput {
    type SystemData = (
        WriteExpect<'a, EditorController>,
        WriteExpect<'a, InputState>,
    );

    fn run(&mut self, (mut editor_controller, mut input_state): Self::SystemData) {
        // FIXME: Loop over a mouse motion event queue instead, to handle cases where multiple
        // boxes are drawn in a single update (e.g. during lag or testing code)
        if let Some(ref selection_box) = input_state.selected_region {
            let rectangle = selection_box.to_rectangle().snap_to_grid(16);

            editor_controller.push_event(EditEvent::CreateRoom {
                x: rectangle.x,
                y: rectangle.y,
                width: rectangle.width,
                height: rectangle.height,
            });
        };

//
//
//        if let &Button::Mouse(MouseButton::Left) = args {
//            let rect = {
//                let mut mouse_input = self.specs_world
//                    .write_resource::<MouseInput>();
//                mouse_input.dragging = false;
//                mouse_input.selection_rectangle()
//            };
//
//            let (x, y, width, height) = (rect[0], rect[1], rect[2], rect[3]);

    }
}

pub struct AimObjects;

impl <'a> System<'a> for AimObjects {
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, InputState>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, InRoom>,
        WriteStorage<'a, Aim>,
    );

    fn run(&mut self, (entities, mut input_state, positions, in_rooms, mut aims): Self::SystemData) {
        for (_entity, position, in_room, mut aim) in (&*entities, &positions, &in_rooms, &mut aims).join() {
            if input_state.button_pressed_or_held(&Button::Keyboard(Key::LCtrl)) {
                let room_entity = entities.entity(in_room.room_entity);

                let room_position = match positions.get(room_entity) {
                    Some(room_position) => room_position,
                    None => continue,
                };

                let source = (position.x + room_position.x, position.y + room_position.y);
                let aim_at = input_state.mouse.position;

                aim.aiming = true;
                aim.aiming_toward = (aim_at.0 - source.0, aim_at.1 - source.1);
            } else {
                aim.aiming = false;
            }
        }
    }
}

pub struct GlobalInput;

impl <'a> System<'a> for GlobalInput {
    type SystemData = WriteExpect<'a, InputState>;

    fn run(&mut self, mut input_state: Self::SystemData) {
        for (button, press_count) in input_state.button_pressed.drain() {
            println!("Unhandled key {:?} pressed {} times", button, press_count);
        }
    }
}

