//! Screen identifiers, their transient state and the commands they emit.

use super::theme::place_for;
use super::{DIM, GOLD, INK, Ui, WIDTH, rect};
use crate::render::canvas::Blit;
use crate::settings::Action;
use babel_core::data::{Slot, Station};
use babel_core::progression::look::Look;
use babel_core::progression::pet::PetLook;
use babel_core::progression::reading::ReadResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    /// The character creator, also reachable from the pause menu.
    Look,
    /// Opening, joining and leaving rooms.
    Coop,
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
    /// What the character has done.
    Stats,
    GameOver,
    Ending,
    Credits,
}

impl Screen {
    /// Screens whose letters are typed rather than used for navigation.
    pub fn takes_text(self) -> bool {
        matches!(
            self,
            Screen::Look | Screen::NewGame | Screen::Catalog | Screen::Coop
        )
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
    HostRoom,
    JoinRoom,
    LeaveRoom,
    CopyCode,
    PasteCode,
    /// The host pastes the guest's reply code and uses it at once.
    PasteReply,
    SendReply,
    CopyReply,
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
    /// The station whose crafting screen was opened, if one was used.
    pub bench: Option<Station>,
    pub script_target: Option<SlotRef>,
    /// First item row shown in the Scriptorium list.
    pub script_scroll: usize,
    pub script_page: usize,
    pub remap: Option<Action>,
    pub confirm_overwrite: bool,
    pub ending_page: usize,
    /// The look being designed for a new game.
    pub look: Look,
    /// The pet being designed for a new game.
    pub pet: PetLook,
    /// Both names as they are being typed.
    pub hero_name: String,
    pub pet_name: String,
    /// The creator is on the pet's tab.
    pub creator_pet: bool,
    /// The skill screen shows the pet's tree.
    pub skill_pet: bool,
    /// Quarter turns of the character preview.
    pub preview_turn: u8,
    /// Whether the on-screen keyboard is unfolded.
    pub show_keys: bool,
    pub bag_tab: usize,
    pub controls_tab: usize,
    /// Skill just learned and when, for its burst of light.
    pub learned: Option<(String, f32)>,
    /// Time the current screen was opened.
    pub opened_at: f32,
    /// The room code being typed.
    pub invitation: String,
    /// The reply code the host is typing in.
    pub reply: String,
    /// Why the last attempt to play together failed.
    pub coop_error: String,
}

impl UiData {
    pub fn reset_for_screen(&mut self, time: f32) {
        self.held = None;
        self.message.clear();
        self.remap = None;
        self.confirm_overwrite = false;
        self.show_keys = false;
        self.learned = None;
        self.opened_at = time;
    }
}

/// Tab bar shared by the character screens: icons, and a name only on the
/// active tab, so the bar stays quiet.
pub fn tabs(ui: &mut Ui, current: Screen, catalog: bool, cmds: &mut Vec<Cmd>) {
    let mut list = vec![
        (Screen::Inventory, "ui_bag", "Inventario"),
        (Screen::Skills, "ui_tree", "Habilidades"),
        (Screen::Glossary, "ui_book", "Glosario"),
        (Screen::Map, "ui_map", "Mapa"),
    ];
    if catalog {
        list.push((Screen::Catalog, "icon_index", "Catálogo"));
    }
    let here = list.iter().position(|t| t.0 == current).unwrap_or(0);
    let wide = ui.font.width(list[here].2) + 34;
    let total = wide + (list.len() as i32 - 1) * 28;
    let mut x = (WIDTH - total) / 2;
    ui.keycap(ui.glyph("Q", "L1"), x - 22, 7);
    for (screen, icon, label) in &list {
        let active = *screen == current;
        let w = if active { wide } else { 26 };
        let r = rect(x, 3, w, 16);
        let response = ui.focusable(r);
        ui.cell(r, &response, active.then_some(GOLD));
        ui.icon_at(icon, r.x + 13, r.y + 8, Blit::default());
        if !active {
            let theirs = place_for(*screen, None).theme().accent;
            ui.canvas.fill(r.x + 7, r.y + r.h - 3, r.w - 14, 2, theirs);
        }
        if active {
            ui.text(label, r.x + 25, r.y + 5, GOLD);
        } else if response.focused {
            // The name of a hovered tab appears as a tooltip.
            ui.tooltip(label, r.x + 13, 21);
        }
        if response.activated && !active {
            cmds.push(Cmd::Replace(*screen));
        }
        x += w + 2;
    }
    ui.keycap(ui.glyph("Tab", "R1"), x + 6, 7);
    if ui.input.tab != 0 {
        let next = (here as i32 + ui.input.tab).rem_euclid(list.len() as i32) as usize;
        cmds.push(Cmd::Replace(list[next].0));
    }
}

const KEYS: [&str; 4] = ["abcdefghij", "klmnñopqrs", "tuvwxyz,. ", "0123456789"];

/// Applies what was typed on a physical keyboard. Returns whether anything
/// was, in which case Enter must submit rather than press an on-screen key.
pub fn type_into(ui: &Ui, text: &mut String, max: usize, digits: bool) -> bool {
    for &c in &ui.input.typed {
        // The Catalog's alphabet has no digits.
        if (digits || !c.is_ascii_digit()) && text.chars().count() < max {
            text.push(c);
        }
    }
    if ui.input.backspace {
        text.pop();
    }
    !ui.input.typed.is_empty() || ui.input.backspace
}

/// On-screen keyboard so text can be entered with a mouse or a gamepad.
pub fn keyboard(ui: &mut Ui, text: &mut String, y: i32, max: usize, digits: bool) {
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

/// A one-line text field with a blinking cursor and a placeholder.
pub fn text_field(ui: &mut Ui, area: super::Rect, text: &str, placeholder: &str) {
    ui.card(area, GOLD);
    let cursor = if (ui.time * 3.0).sin() > 0.0 {
        "_"
    } else {
        " "
    };
    let y = area.y + (area.h - 7) / 2;
    if text.is_empty() {
        let end = ui.text(cursor, area.x + 10, y, INK);
        ui.text(placeholder, end + 2, y, DIM);
    } else {
        // Long phrases keep their end in view.
        let mut start = 0;
        while ui.font.width(&text[start..]) > area.w - 26 {
            start += text[start..].chars().next().map_or(1, char::len_utf8);
        }
        ui.text(&format!("{}{cursor}", &text[start..]), area.x + 10, y, INK);
    }
}
