// SPDX-License-Identifier: Apache-2.0
//! Shared native button material; colors generated from the design contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonMaterial {
    pub face: u32,
    pub hot: u32,
    pub pressed: u32,
    pub highlight: u32,
    pub shadow: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub focus: u32,
    pub radius: i32,
    pub profile: u8,
    pub elevation: i32,
    pub offset: i32,
}
include!(concat!(env!("OUT_DIR"), "/buttons.rs"));
pub fn material(index: usize) -> ButtonMaterial {
    BUTTON_MATERIALS
        .get(index)
        .copied()
        .unwrap_or(BUTTON_MATERIALS[0])
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Rest,
    Hot,
    Pressed,
    Disabled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonVisual {
    pub face: u32,
    pub text: u32,
    pub highlight: u32,
    pub shadow: u32,
    pub elevation: i32,
    pub offset: i32,
}
/// Forced colors must retain the unmodified native button renderer.
pub fn visual(m: ButtonMaterial, state: ButtonState, forced: bool) -> Option<ButtonVisual> {
    if forced {
        return None;
    }
    let down = state == ButtonState::Pressed;
    let disabled = state == ButtonState::Disabled;
    Some(ButtonVisual {
        face: match state {
            ButtonState::Hot => m.hot,
            ButtonState::Pressed => m.pressed,
            _ => m.face,
        },
        text: if disabled { m.muted } else { m.text },
        highlight: if down { m.shadow } else { m.highlight },
        shadow: if down { m.highlight } else { m.shadow },
        elevation: if down || disabled { 0 } else { m.elevation },
        offset: if down { m.offset } else { 0 },
    })
}
