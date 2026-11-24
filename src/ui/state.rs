//! Screen identifiers, their transient state and the commands they emit.

use super::{DIM, GOLD, INK, Ui, WIDTH, rect};
use crate::settings::Action;
use babel_core::data::Slot;
use babel_core::progression::reading::ReadResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    NewGame,
    Options,
    Controls,
    Pause,
    Inventory,
    Skills,
    Glossary,
    Map,
    Catalog,
    Reader,
    Crafting,
    Build(u8),
    Chest,
    Lamps,
    Scriptorium,
    GameOver,
    Ending,
    Credits,
}

impl Screen {
    /// Screens whose letters are typed rather than used for navigation.
    pub fn takes_text(self) -> bool {
        matches!(self, Screen::NewGame | Screen::Catalog)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Push(Screen),
    /// Swap the top screen (tabs).
    Replace(Screen),
    Pop,
    CloseAll,
    NewGame(Option<String>),
    Continue,
    Quit,
    ToTitle,
    Respawn,
    SettingsChanged,
    Save,
    /// The credits are over.
    FinishCredits,
}

/// Where an item sits while it is being moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRef {
    Bag(usize),
    Worn(Slot),
}

#[derive(Default)]
pub struct UiData {
    pub held: Option<SlotRef>,
    pub text: String,
    /// Feedback line of the current screen.
    pub message: String,
    pub reading: ReadResult,
    pub reader_half: usize,
    pub glossary_filter: usize,
    pub glossary_page: usize,
    pub glossary_selected: Option<String>,
    pub craft_tab: usize,
    pub script_target: Option<SlotRef>,
    pub script_page: usize,
    pub remap: Option<Action>,
    pub confirm_overwrite: bool,
    pub ending_page: usize,
    /// Time the current screen was opened.
    pub opened_at: f32,
}

impl UiData {
    pub fn reset_for_screen(&mut self, time: f32) {
        self.held = None;
        self.message.clear();
        self.remap = None;
        self.confirm_overwrite = false;
        self.opened_at = time;
    }
}

/// Tab bar shared by the character screens.
pub fn tabs(ui: &mut Ui, current: Screen, catalog: bool, cmds: &mut Vec<Cmd>) {
    let mut list = vec![
        (Screen::Inventory, "Inventario"),
        (Screen::Skills, "Habilidades"),
        (Screen::Glossary, "Glosario"),
        (Screen::Map, "Mapa"),
    ];
    if catalog {
        list.push((Screen::Catalog, "Catálogo"));
    }
    let w = 86;
    let start = (WIDTH - list.len() as i32 * (w + 4)) / 2;
    let here = list.iter().position(|t| t.0 == current).unwrap_or(0);
    for (i, (screen, label)) in list.iter().enumerate() {
        let r = rect(start + i as i32 * (w + 4), 4, w, 13);
        let response = ui.focusable(r);
        let active = *screen == current;
        ui.cell(r, &response, active.then_some(GOLD));
        ui.centered(
            label,
            r.x + w / 2,
            r.y + 3,
            if active {
                GOLD
            } else if response.focused {
                INK
            } else {
                DIM
            },
        );
        if response.activated && !active {
            cmds.push(Cmd::Replace(*screen));
        }
    }
    if ui.input.tab != 0 {
        let next = (here as i32 + ui.input.tab).rem_euclid(list.len() as i32) as usize;
        cmds.push(Cmd::Replace(list[next].0));
    }
}

const KEYS: [&str; 4] = ["abcdefghij", "klmnñopqrs", "tuvwxyz,. ", "0123456789"];

/// On-screen keyboard so text can be entered with a gamepad. Physical typing
/// works at the same time.
pub fn keyboard(ui: &mut Ui, text: &mut String, y: i32, max: usize, digits: bool) {
    for &c in &ui.input.typed {
        if text.chars().count() < max {
            text.push(c);
        }
    }
    if ui.input.backspace {
        text.pop();
    }
    let rows: &[&str] = if digits { &KEYS } else { &KEYS[..3] };
    let cell = 20;
    for (row, letters) in rows.iter().enumerate() {
        let x0 = (WIDTH - 10 * (cell + 2)) / 2;
        for (col, c) in letters.chars().enumerate() {
            let r = rect(x0 + col as i32 * (cell + 2), y + row as i32 * 15, cell, 13);
            let label = if c == ' ' {
                "_".to_string()
            } else {
                c.to_string()
            };
            if ui.button(r, &label) && text.chars().count() < max {
                text.push(c);
            }
        }
    }
    let by = y + rows.len() as i32 * 15;
    if ui.button(rect(WIDTH / 2 - 108, by, 106, 13), "Borrar") {
        text.pop();
    }
    if ui.button(rect(WIDTH / 2 + 2, by, 106, 13), "Vaciar") {
        text.clear();
    }
}
