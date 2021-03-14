use crate::common::message::*;

use glutin::event::*;

use serde::{Serialize, Deserialize};

use hashbrown::HashMap;
use hashbrown::HashSet;

use ustr::*;

pub type ButtonBinding = Ustr;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct MouseButtonEvent {
    pub button: MouseButton,
    pub state: ElementState,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ButtonInputState {
    PRESSED,
    RELEASED,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ButtonInputEvent {
    pub binding: ButtonBinding,
    pub state: ButtonInputState,
}

impl RegisteredMessage for ButtonInputEvent {
    fn msg_ty() -> Ustr {
        return ustr("gestalt.ButtonInput");
    }
}

pub struct InputSystem {
    pub key_bindings : HashMap<VirtualKeyCode, ButtonBinding>,
    pub mouse_button_bindings : HashMap<MouseButton, ButtonBinding>,
}

impl InputSystem {
    pub fn process_event(&self, evt: &WindowEvent) {
        match evt {
            _ => {},
        }
    }
}