//! Equipment, bag, materials and tools. With a mouse, items are dragged; with
//! keys or a pad, confirm picks an item up and confirm on another slot drops
//! it. Details appear only for the item under the cursor.

use super::state::{Cmd, Screen, SlotRef, UiData, tabs};
use super::{
    BAD, DIM, FAINT, GOLD, GOOD, INK, PANEL_EDGE, Rect, SKY, Ui, VIOLET, rarity_rgba, rect,
};
use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, rgb, with_alpha};
use crate::render::scene::rarity_color;
use babel_core::babel::rarity::Rarity;
use babel_core::data::{Consumable, Resource, Slot, ToolKind, tier_name};
use babel_core::game::Game;
use babel_core::progression::items::Item;
use babel_core::progression::player::INVENTORY_SIZE;
use babel_core::progression::stats::{describe, item_stats};

pub fn slot_icon(slot: Slot) -> &'static str {
    match slot {
        Slot::Weapon => "icon_weapon",
        Slot::Armor => "icon_armor",
        Slot::Boots => "icon_boots",
        Slot::Lamp => "icon_lantern",
        Slot::Charm => "icon_charm",
    }
}

pub fn tool_kind_icon(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Knife => "icon_knife",
        ToolKind::Hammer => "icon_hammer",
        ToolKind::Lamp => "icon_lamp",
        ToolKind::Lens => "icon_lens",
    }
}

pub fn consumable_icon(c: Consumable) -> &'static str {
    match c {
        Consumable::Torch => "icon_torch",
        Consumable::Tonic => "icon_tonic",
        Consumable::PageAmmo => "icon_ammo",
        Consumable::Index => "icon_index",
    }
}

pub fn resource_icon(r: Resource) -> String {
    format!("res_{}", format!("{r:?}").to_lowercase())
}

fn item_in(game: &Game, slot: SlotRef) -> Option<&Item> {
    match slot {
        SlotRef::Bag(i) => game.profile.player.inventory.get(i),
        SlotRef::Worn(s) => game.profile.player.equipped(s),
    }
}

/// Performs a move from the held slot onto the target.
fn drop_on(game: &mut Game, from: SlotRef, to: SlotRef, data: &mut UiData) {
    match (from, to) {
        (SlotRef::Bag(a), SlotRef::Bag(b)) => game.swap_inventory(a, b),
        (SlotRef::Bag(a), SlotRef::Worn(s)) => {
            if item_in(game, from).is_some_and(|i| i.slot == s) {
                game.equip(a);
            } else {
                data.message = format!("Eso no va en la ranura de {}", s.name());
            }
        }
        (SlotRef::Worn(s), SlotRef::Bag(b)) => {
            // Dropping onto a matching bag item swaps them.
            if game
                .profile
                .player
                .inventory
                .get(b)
                .is_some_and(|i| i.slot == s)
            {
                game.equip(b);
            } else {
                game.unequip(s);
            }
        }
        (SlotRef::Worn(_), SlotRef::Worn(_)) => {}
    }
}

/// Confirming twice on a slot equips or takes off what it holds.
fn quick_use(game: &mut Game, slot: SlotRef) {
    match slot {
        SlotRef::Bag(i) => {
            game.equip(i);
        }
        SlotRef::Worn(s) => {
            game.unequip(s);
        }
    }
}

/// Slot clicks gathered while the widgets are laid out.
#[derive(Default)]
struct Picks {
    focused: Option<SlotRef>,
    clicked: Option<SlotRef>,
    pressed: Option<SlotRef>,
    released: Option<SlotRef>,
    recycle: Option<usize>,
}

impl Picks {
    fn note(&mut self, ui: &Ui, response: &super::Response, slot: SlotRef) {
        if response.focused {
            self.focused = Some(slot);
        }
        if ui.input.confirm && response.focused {
            self.clicked = Some(slot);
        }
        if response.pressed {
            self.pressed = Some(slot);
        }
        if response.released {
            self.released = Some(slot);
        }
    }
}

/// The hero standing among the five equipment slots, with the totals below.
fn paper_doll(ui: &mut Ui, game: &Game, hero: &Atlas, picks: &mut Picks) {
    ui.card(rect(4, 24, 152, 228), SKY);
    let accent = ui.theme.accent;
    ui.canvas.glow(80.0, 96.0, 52.0, with_alpha(accent, 90));
    for (half, y) in [(34, 143), (30, 146)] {
        let stand = ui.theme.frame;
        ui.canvas.fill(80 - half, y, half * 2, 3, stand);
    }
    let frame = hero.animate("player_idle_down", ui.time, 3.0);
    let scale = 3.5;
    ui.canvas.sprite(
        hero,
        frame,
        80 - (frame.w as f32 * scale) as i32 / 2,
        147 - (frame.h as f32 * scale) as i32,
        Blit {
            scale,
            ..Default::default()
        },
    );
    let places = [(10, 36), (10, 66), (10, 96), (126, 36), (126, 66)];
    for (slot, (x, y)) in Slot::ALL.into_iter().zip(places) {
        let r = rect(x, y, 24, 24);
        let response = ui.focusable(r);
        let item = game.profile.player.equipped(slot);
        let edge = item.map(|it| rarity_color(it.rarity, ui.time));
        ui.cell(r, &response, edge.or(Some(with_alpha(PANEL_EDGE, 130))));
        let opts = Blit {
            alpha: if item.is_some() { 255 } else { 65 },
            ..Default::default()
        };
        ui.icon_at(slot_icon(slot), x + 12, y + 12, opts);
        picks.note(ui, &response, SlotRef::Worn(slot));
    }
    let totals = [
        (
            "icon_weapon",
            "Daño",
            format!("{}", game.derived.damage.round()),
        ),
        (
            "icon_armor",
            "Defensa",
            format!("{}", game.derived.defense.round()),
        ),
        (
            "icon_boots",
            "Veloc.",
            format!("{}", game.derived.speed.round()),
        ),
        (
            "icon_lantern",
            "Luz",
            format!("{:.1}", game.derived.light_radius),
        ),
    ];
    for (i, (icon, label, value)) in totals.into_iter().enumerate() {
        let (x, y) = (10 + (i as i32 % 2) * 72, 160 + (i as i32 / 2) * 28);
        let (well, edge) = (ui.theme.well, ui.theme.edge);
        ui.canvas.fill(x, y, 68, 24, well);
        ui.canvas.frame(x, y, 68, 24, with_alpha(edge, 150));
        ui.icon_at(icon, x + 11, y + 12, Blit::default());
        ui.text(&value, x + 23, y + 4, GOLD);
        ui.plain(label, x + 23, y + 14, DIM);
    }
    ui.centered(
        &format!("Nivel {}", game.profile.player.level),
        80,
        226,
        INK,
    );
}

fn bag(ui: &mut Ui, game: &Game, data: &mut UiData, picks: &mut Picks) {
    for i in 0..INVENTORY_SIZE {
        let r = rect(170 + (i as i32 % 6) * 28, 52 + (i as i32 / 6) * 28, 25, 25);
        let response = ui.focusable(r);
        let item = game.profile.player.inventory.get(i);
        let edge = item.map(|it| rarity_color(it.rarity, ui.time));
        ui.cell(r, &response, edge);
        let this = SlotRef::Bag(i);
        if let Some(it) = item
            && data.held != Some(this)
        {
            ui.icon_at(slot_icon(it.slot), r.x + 12, r.y + 12, Blit::default());
        }
        picks.note(ui, &response, this);
        if response.alt && item.is_some() {
            picks.recycle = Some(i);
        }
    }
    let bin = rect(170, 196, 165, 18);
    let bin_response = ui.focusable(bin);
    ui.cell(bin, &bin_response, Some(rgb(190, 74, 96)));
    ui.centered("Reciclar en tinta", bin.x + bin.w / 2, bin.y + 6, INK);
    if let Some(SlotRef::Bag(i)) = data.held
        && (bin_response.released || (bin_response.focused && ui.input.confirm))
    {
        picks.recycle = Some(i);
    }
}

/// Only the materials and supplies actually carried.
fn materials(ui: &mut Ui, game: &Game) {
    let player = &game.profile.player;
    let owned: Vec<(String, &str, u32)> = Resource::ALL
        .into_iter()
        .map(|r| (resource_icon(r), r.name(), player.resource(r)))
        .chain(Consumable::ALL.into_iter().map(|c| {
            (
                consumable_icon(c).to_string(),
                c.name(),
                player.consumable(c),
            )
        }))
        .filter(|entry| entry.2 > 0)
        .collect();
    if owned.is_empty() {
        ui.centered("Todavía no juntaste nada.", 316, 110, DIM);
        return;
    }
    for (i, (icon, name, count)) in owned.iter().enumerate() {
        let (x, y) = (170 + (i as i32 % 2) * 149, 52 + (i as i32 / 2) * 20);
        ui.canvas.fill(x, y, 143, 17, [52, 34, 84, 235]);
        ui.icon_at(icon, x + 10, y + 8, Blit::default());
        ui.plain(name, x + 21, y + 5, INK);
        ui.right(&count.to_string(), x + 139, y + 5, GOLD);
    }
}

fn tools(ui: &mut Ui, game: &Game) {
    let player = &game.profile.player;
    for (i, tool) in ToolKind::ALL.into_iter().enumerate() {
        let (x, y) = (170 + (i as i32 % 2) * 149, 52 + (i as i32 / 2) * 48);
        let tier = player.tool_tier(tool);
        ui.canvas.fill(x, y, 143, 43, [52, 34, 84, 235]);
        let big = Blit {
            scale: 2.0,
            ..Default::default()
        };
        ui.icon_at(tool_kind_icon(tool), x + 19, y + 21, big);
        ui.text(tool.name(), x + 40, y + 6, INK);
        ui.plain(tier_name(tier), x + 40, y + 17, DIM);
        // One pip per tier reached.
        for k in 0..6 {
            let fill = if k <= i32::from(tier) {
                GOLD
            } else {
                rgb(30, 18, 52)
            };
            ui.canvas.fill(x + 40 + k * 9, y + 31, 7, 4, fill);
        }
    }
    let relics: Vec<&str> = player
        .relics
        .iter()
        .filter_map(|id| game.data.items.relics.iter().find(|r| &r.id == id))
        .map(|r| r.name.as_str())
        .collect();
    if relics.is_empty() {
        return;
    }
    ui.text("Reliquias", 170, 152, GOLD);
    let color = rarity_rgba(Rarity::Legendary);
    for (i, name) in relics.iter().take(8).enumerate() {
        let (x, y) = (170 + (i as i32 % 2) * 149, 166 + (i as i32 / 2) * 12);
        ui.plain(&format!("♦ {name}"), x, y, color);
    }
}

fn detail(ui: &mut Ui, game: &Game, item: &Item) {
    let color = rarity_color(item.rarity, ui.time);
    let (x, w) = (356, 108);
    ui.card(ITEM_CARD, color);
    let mut line = 60;
    for part in ui.font.wrap(&item.name, w) {
        ui.text(&part, x, line, color);
        line += 10;
    }
    ui.plain(
        &format!("{} · {}", item.rarity.label(), item.slot.name()),
        x,
        line,
        DIM,
    );
    line += 14;
    for (stat, value) in item_stats(&game.data, item).iter().take(6) {
        ui.text(&describe(stat, value), x, line, INK);
        line += 10;
    }
    // Compare with what is worn in the same slot.
    if let Some(worn) = game.profile.player.equipped(item.slot)
        && worn.uid != item.uid
    {
        line += 2;
        let better = item.power() > worn.power();
        let text = if better {
            "↑ mejor que el puesto"
        } else {
            "↓ peor que el puesto"
        };
        ui.text(text, x, line, if better { GOOD } else { BAD });
        line += 11;
    }
    line += 3;
    for i in &item.inscriptions {
        if line < 202 {
            ui.text(&format!("♦ {}", i.word), x, line, rarity_rgba(i.rarity));
            line += 10;
        }
    }
    let words = ui.font.wrap(&format!("«{}»", item.words.join(" ")), w);
    for part in words {
        if line < 204 {
            ui.plain(&part, x, line, FAINT);
            line += 10;
        }
    }
}

/// The panel on the right, and the card of the item under the cursor inside it.
const BAG_PANEL: Rect = rect(160, 24, 316, 228);
const ITEM_CARD: Rect = rect(350, 52, 120, 162);

const BAG_TABS: [&str; 3] = ["Mochila", "Materiales", "Herramientas"];

pub fn draw(ui: &mut Ui, game: &mut Game, hero: &Atlas, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    tabs(ui, Screen::Inventory, game.catalog_unlocked(), cmds);
    let mut picks = Picks::default();
    paper_doll(ui, game, hero, &mut picks);
    ui.card(BAG_PANEL, VIOLET);
    for (i, label) in BAG_TABS.into_iter().enumerate() {
        let r = rect(170 + i as i32 * 100, 31, 97, 15);
        if ui.button(r, label) {
            data.bag_tab = i;
            data.held = None;
        }
        if data.bag_tab == i {
            ui.canvas.fill(r.x + 4, r.y + r.h - 2, r.w - 8, 2, GOLD);
        }
    }
    match data.bag_tab {
        0 => bag(ui, game, data, &mut picks),
        1 => materials(ui, game),
        _ => tools(ui, game),
    }
    let Picks {
        focused,
        clicked,
        pressed,
        released,
        recycle,
    } = picks;

    // Mouse drag: press picks up, release over another slot drops.
    if let Some(slot) = pressed
        && item_in(game, slot).is_some()
        && data.held.is_none()
    {
        data.held = Some(slot);
    } else if let Some(target) = released
        && let Some(from) = data.held
    {
        if from == target {
            // Press and release on the same slot is a plain click.
            quick_use(game, target);
        } else {
            drop_on(game, from, target, data);
        }
        data.held = None;
    } else if let Some(slot) = clicked {
        match data.held {
            None => {
                if item_in(game, slot).is_some() {
                    data.held = Some(slot);
                }
            }
            Some(from) if from == slot => {
                data.held = None;
                quick_use(game, slot);
            }
            Some(from) => {
                drop_on(game, from, slot, data);
                data.held = None;
            }
        }
    }
    if let Some(i) = recycle {
        game.recycle(i);
        data.held = None;
    }
    if ui.input.release {
        data.held = None;
    }

    if data.bag_tab == 0 {
        let shown = data.held.or(focused);
        if let Some(item) = shown.and_then(|s| item_in(game, s)).cloned() {
            detail(ui, game, &item);
        } else {
            for (i, line) in ["Señalá un", "objeto para", "ver qué hace."]
                .into_iter()
                .enumerate()
            {
                ui.centered(line, 412, 110 + i as i32 * 11, FAINT);
            }
        }
        let count = game.profile.player.inventory.len();
        ui.centered(&format!("{count} de {INVENTORY_SIZE}"), 252, 220, DIM);
    } else if let Some(item) = focused.and_then(|s| item_in(game, s)).cloned() {
        // An equipped item can still be inspected from the other tabs.
        detail(ui, game, &item);
    }

    // The dragged icon follows the cursor.
    if let Some(item) = data.held.and_then(|s| item_in(game, s))
        && let Some(m) = ui.input.mouse
        && ui.input.mouse_down
    {
        ui.icon_at(
            slot_icon(item.slot),
            m.x as i32,
            m.y as i32,
            Blit::default(),
        );
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.centered(&text, 318, 238, GOLD);
    }
    let confirm = ui.glyph("Enter", "A");
    let back = ui.glyph("Esc", "B");
    if data.held.is_some() && !ui.input.mouse_down {
        ui.hints(&[(confirm, "soltar aquí"), (back, "cancelar")]);
    } else {
        let recycle = ui.glyph("X", "X");
        ui.hints(&[(confirm, "mover"), (recycle, "reciclar"), (back, "cerrar")]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_item_card_leaves_room_inside_its_panel() {
        let [card, panel] = std::hint::black_box([ITEM_CARD, BAG_PANEL]);
        assert!(card.x + card.w <= panel.x + panel.w - 4);
        assert!(card.y + card.h <= panel.y + panel.h - 4);
    }
}
