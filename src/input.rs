//! Turns keyboard, mouse and gamepad into one device-independent snapshot.

use crate::settings::{Action, Settings};
use crate::ui::UiInput;
use babel_core::babel::alphabet;
use babel_core::game::events::Input;
use babel_core::game::vec::{V2, v2};
use babel_core::progression::pet::name_char;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

const STICK_DEAD_ZONE: f32 = 0.24;
const REPEAT_DELAY: f32 = 0.34;
const REPEAT_RATE: f32 = 0.11;

const KEYS: [(&str, KeyCode); 60] = [
    ("A", KeyCode::KeyA),
    ("B", KeyCode::KeyB),
    ("C", KeyCode::KeyC),
    ("D", KeyCode::KeyD),
    ("E", KeyCode::KeyE),
    ("F", KeyCode::KeyF),
    ("G", KeyCode::KeyG),
    ("H", KeyCode::KeyH),
    ("I", KeyCode::KeyI),
    ("J", KeyCode::KeyJ),
    ("K", KeyCode::KeyK),
    ("L", KeyCode::KeyL),
    ("M", KeyCode::KeyM),
    ("N", KeyCode::KeyN),
    ("O", KeyCode::KeyO),
    ("P", KeyCode::KeyP),
    ("Q", KeyCode::KeyQ),
    ("R", KeyCode::KeyR),
    ("S", KeyCode::KeyS),
    ("T", KeyCode::KeyT),
    ("U", KeyCode::KeyU),
    ("V", KeyCode::KeyV),
    ("W", KeyCode::KeyW),
    ("X", KeyCode::KeyX),
    ("Y", KeyCode::KeyY),
    ("Z", KeyCode::KeyZ),
    ("0", KeyCode::Digit0),
    ("1", KeyCode::Digit1),
    ("2", KeyCode::Digit2),
    ("3", KeyCode::Digit3),
    ("4", KeyCode::Digit4),
    ("5", KeyCode::Digit5),
    ("6", KeyCode::Digit6),
    ("7", KeyCode::Digit7),
    ("8", KeyCode::Digit8),
    ("9", KeyCode::Digit9),
    ("Espacio", KeyCode::Space),
    ("Enter", KeyCode::Enter),
    ("Mayús izq.", KeyCode::ShiftLeft),
    ("Mayús der.", KeyCode::ShiftRight),
    ("Ctrl izq.", KeyCode::ControlLeft),
    ("Ctrl der.", KeyCode::ControlRight),
    ("Alt", KeyCode::AltLeft),
    ("Tab", KeyCode::Tab),
    ("Escape", KeyCode::Escape),
    ("Borrar", KeyCode::Backspace),
    ("↑", KeyCode::ArrowUp),
    ("↓", KeyCode::ArrowDown),
    ("←", KeyCode::ArrowLeft),
    ("→", KeyCode::ArrowRight),
    (",", KeyCode::Comma),
    (".", KeyCode::Period),
    ("/", KeyCode::Slash),
    (";", KeyCode::Semicolon),
    ("'", KeyCode::Quote),
    ("[", KeyCode::BracketLeft),
    ("]", KeyCode::BracketRight),
    ("-", KeyCode::Minus),
    ("=", KeyCode::Equal),
    ("AltGr", KeyCode::AltRight),
];

pub fn key_code(name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|k| k.0 == name).map(|k| k.1)
}

pub fn key_name(code: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|k| k.1 == code).map(|k| k.0)
}

/// A typed character as text fields understand it: the canonical alphabet,
/// plus digits for numeric seeds.
fn typed_char(c: char) -> Option<char> {
    if c.is_ascii_digit() {
        Some(c)
    } else {
        alphabet::normalize_char(c)
    }
}

/// Menu shortcuts pressed this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hotkeys {
    pub pause: bool,
    pub inventory: bool,
    pub skills: bool,
    pub glossary: bool,
    pub map: bool,
    pub catalog: bool,
    pub debug: bool,
}

/// Everything the shell needs from the devices for one frame.
#[derive(Resource, Default)]
pub struct Snapshot {
    pub ui: UiInput,
    pub game: Input,
    pub hot: Hotkeys,
    /// The gamepad was the last device used.
    pub pad: bool,
    /// Mouse position in view pixels, when it should aim.
    pub aim_view: Option<V2>,
}

/// Facts the gatherer needs from the shell.
#[derive(Resource, Default)]
pub struct InputContext {
    pub settings: Settings,
    pub text_entry: bool,
    pub menu_open: bool,
    /// Integer scale and top-left corner of the view inside the window.
    pub scale: f32,
    pub origin: Vec2,
}

#[derive(Default)]
pub struct Memory {
    held_nav: (i32, i32),
    repeat: f32,
    last_cursor: Option<Vec2>,
    mouse_aiming: bool,
    pad: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn gather(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut typed: MessageReader<KeyboardInput>,
    pads: Query<&Gamepad>,
    windows: Query<&Window>,
    context: Res<InputContext>,
    time: Res<Time>,
    mut memory: Local<Memory>,
    mut out: ResMut<Snapshot>,
) {
    let settings = &context.settings;
    let bound = |a: Action| key_code(settings.key(a));
    let down = |a: Action| bound(a).is_some_and(|k| keys.pressed(k));
    let hit = |a: Action| bound(a).is_some_and(|k| keys.just_pressed(k));
    let pad_down = |b: GamepadButton| pads.iter().any(|p| p.pressed(b));
    let pad_hit = |b: GamepadButton| pads.iter().any(|p| p.just_pressed(b));
    let axis = |pick: fn(&Gamepad) -> Vec2| {
        pads.iter()
            .map(pick)
            .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .filter(|v| v.length() > STICK_DEAD_ZONE)
            .unwrap_or(Vec2::ZERO)
    };
    let left = axis(Gamepad::left_stick);
    let right = axis(Gamepad::right_stick);

    // Which device is in use decides prompts and aiming.
    let any_pad = left != Vec2::ZERO
        || right != Vec2::ZERO
        || pads.iter().any(|p| p.get_just_pressed().next().is_some());
    if any_pad {
        memory.pad = true;
        memory.mouse_aiming = false;
    }
    let cursor = windows.iter().next().and_then(Window::cursor_position);
    let moved = cursor.is_some() && cursor != memory.last_cursor;
    let mouse_used =
        moved || mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right);
    memory.last_cursor = cursor;
    if mouse_used {
        memory.mouse_aiming = true;
        memory.pad = false;
    }
    if keys.get_just_pressed().next().is_some() {
        memory.pad = false;
    }
    let view = cursor.map(|c| {
        let v = (c - context.origin) / context.scale.max(1.0);
        v2(v.x, v.y)
    });

    let mut movement = V2::ZERO;
    for (action, dir) in [
        (Action::Up, v2(0.0, -1.0)),
        (Action::Down, v2(0.0, 1.0)),
        (Action::Left, v2(-1.0, 0.0)),
        (Action::Right, v2(1.0, 0.0)),
    ] {
        if down(action) {
            movement += dir;
        }
    }
    for (key, dir) in [
        (KeyCode::ArrowUp, v2(0.0, -1.0)),
        (KeyCode::ArrowDown, v2(0.0, 1.0)),
        (KeyCode::ArrowLeft, v2(-1.0, 0.0)),
        (KeyCode::ArrowRight, v2(1.0, 0.0)),
    ] {
        if keys.pressed(key) {
            movement += dir;
        }
    }
    movement = movement.clamp_len(1.0);
    if left != Vec2::ZERO {
        movement = v2(left.x, -left.y).clamp_len(1.0);
    }

    let mut scroll = 0;
    for event in wheel.read() {
        let lines = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y / 40.0,
        };
        if lines > 0.0 {
            scroll -= 1;
        } else if lines < 0.0 {
            scroll += 1;
        }
    }
    let mut cycle = scroll;
    if hit(Action::NextTool) || pad_hit(GamepadButton::RightTrigger) {
        cycle += 1;
    }
    if pad_hit(GamepadButton::LeftTrigger) {
        cycle -= 1;
    }

    out.game = Input {
        movement,
        aim_at: None,
        aim_dir: (right != Vec2::ZERO).then(|| v2(right.x, -right.y)),
        attack_pressed: mouse.just_pressed(MouseButton::Left)
            || hit(Action::Attack)
            || pad_hit(GamepadButton::West),
        attack_held: mouse.pressed(MouseButton::Left)
            || down(Action::Attack)
            || pad_down(GamepadButton::West),
        dash: hit(Action::Dash) || pad_hit(GamepadButton::East),
        interact: hit(Action::Interact) || pad_hit(GamepadButton::South),
        ranged: mouse.pressed(MouseButton::Right)
            || down(Action::Ranged)
            || pad_down(GamepadButton::RightTrigger2),
        torch: hit(Action::Torch) || pad_hit(GamepadButton::DPadDown),
        tonic: hit(Action::Tonic) || pad_hit(GamepadButton::LeftTrigger2),
        lamp: hit(Action::Lamp) || pad_hit(GamepadButton::DPadUp),
        read: hit(Action::Read) || pad_hit(GamepadButton::North),
        cycle_tool: cycle,
    };
    out.hot = Hotkeys {
        pause: keys.just_pressed(KeyCode::Escape) || pad_hit(GamepadButton::Start),
        inventory: hit(Action::Inventory) || pad_hit(GamepadButton::DPadLeft),
        skills: hit(Action::Skills),
        glossary: hit(Action::Glossary),
        map: hit(Action::Map) || pad_hit(GamepadButton::DPadRight),
        catalog: hit(Action::Catalog),
        debug: keys.just_pressed(KeyCode::F3),
    };

    // Menu navigation with key repeat.
    let text = context.text_entry;
    let pressed_dir = |arrow: KeyCode, letter: KeyCode, pad: GamepadButton| {
        keys.pressed(arrow) || (!text && keys.pressed(letter)) || pad_down(pad)
    };
    let mut nav = (0, 0);
    if pressed_dir(KeyCode::ArrowUp, KeyCode::KeyW, GamepadButton::DPadUp) || left.y > 0.6 {
        nav = (0, -1);
    } else if pressed_dir(KeyCode::ArrowDown, KeyCode::KeyS, GamepadButton::DPadDown)
        || left.y < -0.6
    {
        nav = (0, 1);
    } else if pressed_dir(KeyCode::ArrowLeft, KeyCode::KeyA, GamepadButton::DPadLeft)
        || left.x < -0.6
    {
        nav = (-1, 0);
    } else if pressed_dir(KeyCode::ArrowRight, KeyCode::KeyD, GamepadButton::DPadRight)
        || left.x > 0.6
    {
        nav = (1, 0);
    }
    let mut step = (0, 0);
    if nav != memory.held_nav {
        memory.held_nav = nav;
        memory.repeat = REPEAT_DELAY;
        step = nav;
    } else if nav != (0, 0) {
        memory.repeat -= time.delta_secs();
        if memory.repeat <= 0.0 {
            memory.repeat = REPEAT_RATE;
            step = nav;
        }
    }

    let mut characters = Vec::new();
    let mut raw = Vec::new();
    let mut letters = Vec::new();
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let paste = text && control && keys.just_pressed(KeyCode::KeyV);
    let mut backspace = false;
    let mut raw_key = None;
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if raw_key.is_none() && !event.repeat {
            raw_key = key_name(event.key_code).map(str::to_string);
        }
        if !text {
            continue;
        }
        match &event.logical_key {
            Key::Character(s) if !control => {
                characters.extend(s.chars().filter_map(typed_char));
                raw.extend(s.chars().filter(char::is_ascii_graphic));
                letters.extend(s.chars().filter(|&c| name_char(c)));
            }
            Key::Space => {
                characters.push(' ');
                letters.push(' ');
            }
            Key::Backspace => backspace = true,
            _ => {}
        }
    }

    let confirm_key = keys.just_pressed(KeyCode::Enter)
        || (!text
            && (keys.just_pressed(KeyCode::Space) || (context.menu_open && hit(Action::Interact))));
    out.ui = UiInput {
        mouse: view,
        mouse_moved: moved,
        click: mouse.just_pressed(MouseButton::Left),
        mouse_down: mouse.pressed(MouseButton::Left),
        release: mouse.just_released(MouseButton::Left),
        right_click: mouse.just_pressed(MouseButton::Right),
        nav: step,
        confirm: confirm_key || pad_hit(GamepadButton::South),
        back: keys.just_pressed(KeyCode::Escape)
            || (!text && keys.just_pressed(KeyCode::Backspace))
            || pad_hit(GamepadButton::East)
            || pad_hit(GamepadButton::Start),
        alt: (!text && keys.just_pressed(KeyCode::KeyX)) || pad_hit(GamepadButton::West),
        tab: i32::from(
            keys.just_pressed(KeyCode::Tab)
                || keys.just_pressed(KeyCode::PageDown)
                || pad_hit(GamepadButton::RightTrigger),
        ) - i32::from(
            (!text && keys.just_pressed(KeyCode::KeyQ))
                || keys.just_pressed(KeyCode::PageUp)
                || pad_hit(GamepadButton::LeftTrigger),
        ),
        scroll,
        typed: characters,
        raw,
        letters,
        paste,
        backspace,
        raw_key,
    };
    out.pad = memory.pad;
    out.aim_view = if memory.mouse_aiming { view } else { None };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_and_accented_letters_can_be_typed() {
        let typed: String = "Árbol 123!".chars().filter_map(typed_char).collect();
        assert_eq!(typed, "arbol 123");
    }

    #[test]
    fn every_default_binding_maps_to_a_key() {
        for action in Action::ALL {
            let name = action.default_key();
            let code = key_code(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(key_name(code), Some(name));
        }
    }
}
