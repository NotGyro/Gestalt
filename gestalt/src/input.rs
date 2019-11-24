//! Input management.


use winit::{ElementState, VirtualKeyCode, KeyboardInput};
use hashbrown::HashMap;


#[derive(Clone)]
struct KeyState {
    pub held: bool,
    /// true if this is the first frame a key is held
    pub first_frame: bool,
}


/// Holds the current game input state.
#[derive(Clone, Default)]
pub struct InputState {
    key_states: HashMap<VirtualKeyCode, KeyState>,
    pub mouse_delta: (f32, f32),
    pub right_mouse_pressed: bool
}


impl InputState {
    pub fn new() -> InputState {
        InputState {
            key_states: HashMap::new(),
            mouse_delta: (0.0, 0.0),
            right_mouse_pressed: false
        }
    }


    pub fn update_keys(&mut self) {
        for (_, v) in self.key_states.iter_mut() {
            v.first_frame = false;
        }
    }


    /// Gets whether a key is currently pressed.
    pub fn get_key_down(&self, key: &VirtualKeyCode) -> bool {
        match self.key_states.get(key) {
            Some(k) => {
                k.held
            },
            None => false
        }
    }


    /// Gets whether a key was pressed on this frame.
    pub fn get_key_just_pressed(&self, key: &VirtualKeyCode) -> bool {
        match self.key_states.get(key) {
            Some(k) => {
                k.first_frame
            },
            None => false
        }
    }


    /// Updates whether a key is currently pressed. Used in the game update loop.
    pub fn update_key(&mut self, input: KeyboardInput) {
        match input.state {
            ElementState::Pressed => {
                if let Some(keycode) = input.virtual_keycode {
                    match self.key_states.get_mut(&keycode) {
                        Some(s) => {
                            s.held = true;
                            s.first_frame = true;
                        },
                        None => {
                            self.key_states.insert(keycode, KeyState {
                                held: true,
                                first_frame: true
                            });
                        }
                    }
                }
            },
            ElementState::Released => {
                if let Some(keycode) = input.virtual_keycode {
                    match self.key_states.get_mut(&keycode) {
                        Some(s) => {
                            s.held = false;
                            s.first_frame = false;
                        },
                        None => {
                            self.key_states.insert(keycode, KeyState {
                                held: false,
                                first_frame: false
                            });
                        }
                    }
                }
            }
        }
    }


    /// Adds mouse input. Used in the game update loop.
    pub fn add_mouse_delta(&mut self, delta: (f32, f32)) {
        self.mouse_delta = (self.mouse_delta.0 + delta.0, self.mouse_delta.1 + delta.1);
    }
}