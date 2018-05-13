use specs::prelude::{System, DenseVecStorage, WriteStorage, ReadStorage, WriteExpect, Entities, Join};
use std::collections::VecDeque;
use super::{Button, Key, MouseButton};
use std::collections::HashSet;
use std::collections::HashMap;
use physics::Aim;
use draw::Position;
use physics::InRoom;

pub enum InputEvent {
    PressEvent(Button),
    ReleaseEvent(Button),
    MotionEvent(Motion),
}

#[derive(Default, Copy, Clone)] // FIXME: derive more
pub struct Motion {
    pub position: (f64, f64),
    pub dragging_from: Option<(f64, f64)>,
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
    pub mouse: Motion,
    // Consider adding events
}

impl InputState {
    pub fn new() -> Self {
        InputState {
            button_held: HashSet::with_capacity(16),
            button_pressed: HashMap::with_capacity(16),
            mouse: Motion::default(),
        }
    }

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

        while let Some(input_event) = input_events.events.pop_front() {
            match input_event {
                InputEvent::PressEvent(button) => {
                    input_state.button_held.insert(button);

                    let mut press_count = input_state.button_pressed
                        .entry(button).or_insert(0);
                    *press_count += 1;
                },
                InputEvent::ReleaseEvent(button) => { input_state.button_held.remove(&button); },
                InputEvent::MotionEvent(motion) => { input_state.mouse = motion; },
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
        for (_entity, mut player_controller) in (&*entities, &mut player_controllers).join() {
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

            player_controller.moving = movement;
            player_controller.jumping = jumping;
            player_controller.hooking = hooking;
        }
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

