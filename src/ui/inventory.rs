//! Equipment and bag with drag and drop. With keys or a pad, confirm picks an
//! item up and confirm on another slot drops it.

use super::state::{Cmd, Screen, SlotRef, UiData, tabs};
use super::{BAD, DIM, FAINT, GOLD, INK, PANEL_EDGE, Ui, rarity_rgba, rect};
use crate::render::canvas::{Blit, rgb};
use crate::render::scene::rarity_color;
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

fn detail(ui: &mut Ui, game: &Game, item: &Item, x: i32, y: i32, w: i32) {
    let color = rarity_color(item.rarity, ui.time);
    let mut line = y;
    for part in ui.font.wrap(&item.name, w) {
        ui.text(&part, x, line, color);
        line += 10;
    }
    ui.text(
        &format!("{} · {}", item.rarity.label(), item.slot.name()),
        x,
        line,
        DIM,
    );
    line += 13;
    for (stat, value) in item_stats(&game.data, item).iter() {
        ui.text(&describe(stat, value), x, line, INK);
        line += 9;
    }
    // Compare with what is worn in the same slot.
    if let Some(worn) = game.profile.player.equipped(item.slot)
        && worn.uid != item.uid
    {
        line += 2;
        let better = item.power() > worn.power();
        ui.text(
            if better {
                "Mejor que lo puesto"
            } else {
                "Más débil que lo puesto"
            },
            x,
            line,
            if better { super::GOOD } else { BAD },
        );
        line += 10;
    }
    line += 3;
    line = ui.paragraph(
        &format!("Palabras: {}", item.words.join(" ")),
        x,
        line,
        w,
        DIM,
    );
    for i in &item.inscriptions {
        ui.text(&format!("♦ {}", i.word), x, line, rarity_rgba(i.rarity));
        line += 9;
    }
}

pub fn draw(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(205);
    tabs(ui, Screen::Inventory, game.catalog_unlocked(), cmds);
    ui.panel(rect(4, 22, 112, 236));
    ui.panel(rect(120, 22, 214, 150));
    ui.panel(rect(338, 22, 138, 236));
    ui.panel(rect(120, 176, 214, 82));

    let mut focused: Option<SlotRef> = None;
    let mut clicked: Option<SlotRef> = None;
    let mut pressed: Option<SlotRef> = None;
    let mut released: Option<SlotRef> = None;
    let mut recycle: Option<usize> = None;

    ui.text("Equipo", 10, 27, GOLD);
    for (i, slot) in Slot::ALL.into_iter().enumerate() {
        let r = rect(10, 40 + i as i32 * 24, 20, 20);
        let response = ui.focusable(r);
        let item = game.profile.player.equipped(slot);
        let edge = item.map(|it| rarity_color(it.rarity, ui.time));
        ui.cell(r, &response, edge.or(Some(PANEL_EDGE)));
        let alpha = if item.is_some() { 255 } else { 70 };
        ui.icon_with(
            slot_icon(slot),
            r.x + 4,
            r.y + 4,
            Blit {
                alpha,
                ..Default::default()
            },
        );
        let name = item.map_or_else(|| slot.name().to_string(), |it| it.name.clone());
        let short: String = name.chars().take(15).collect();
        ui.plain(&short, 34, r.y + 6, item.map_or(FAINT, |_| INK));
        let this = SlotRef::Worn(slot);
        if response.focused {
            focused = Some(this);
        }
        if ui.input.confirm && response.focused {
            clicked = Some(this);
        }
        if response.pressed {
            pressed = Some(this);
        }
        if response.released {
            released = Some(this);
        }
    }
    ui.text("Herramientas", 10, 166, GOLD);
    for (i, tool) in ToolKind::ALL.into_iter().enumerate() {
        let y = 179 + i as i32 * 19;
        let tier = game.profile.player.tool_tier(tool);
        let icon = ["icon_knife", "icon_hammer", "icon_lamp", "icon_lens"][i];
        ui.icon(icon, 10, y);
        ui.plain(tool.name(), 26, y, INK);
        ui.plain(tier_name(tier), 26, y + 8, DIM);
    }

    ui.text(
        &format!(
            "Mochila {}/{INVENTORY_SIZE}",
            game.profile.player.inventory.len()
        ),
        126,
        27,
        GOLD,
    );
    for i in 0..INVENTORY_SIZE {
        let r = rect(126 + (i as i32 % 6) * 34, 40 + (i as i32 / 6) * 21, 30, 19);
        let response = ui.focusable(r);
        let item = game.profile.player.inventory.get(i);
        let edge = item.map(|it| rarity_color(it.rarity, ui.time));
        ui.cell(r, &response, edge);
        let this = SlotRef::Bag(i);
        if let Some(it) = item
            && data.held != Some(this)
        {
            ui.icon(slot_icon(it.slot), r.x + 10, r.y + 4);
        }
        if response.focused {
            focused = Some(this);
        }
        if ui.input.confirm && response.focused {
            clicked = Some(this);
        }
        if response.pressed {
            pressed = Some(this);
        }
        if response.released {
            released = Some(this);
        }
        if response.alt && item.is_some() {
            recycle = Some(i);
        }
    }
    let bin = rect(126, 146, 202, 19);
    let bin_response = ui.focusable(bin);
    ui.cell(bin, &bin_response, Some(rgb(200, 80, 100)));
    ui.centered("Reciclar (da tinta)", bin.x + bin.w / 2, bin.y + 6, INK);
    if let Some(SlotRef::Bag(i)) = data.held
        && (bin_response.released || (bin_response.focused && ui.input.confirm))
    {
        recycle = Some(i);
    }

    ui.text("Materiales", 126, 181, GOLD);
    for (i, r) in Resource::ALL.into_iter().enumerate() {
        let x = 126 + (i as i32 % 5) * 41;
        let y = 193 + (i as i32 / 5) * 21;
        let count = game.profile.player.resource(r);
        ui.icon_with(
            &resource_icon(r),
            x,
            y,
            Blit {
                alpha: if count > 0 { 255 } else { 80 },
                ..Default::default()
            },
        );
        ui.plain(
            &count.to_string(),
            x + 15,
            y + 3,
            if count > 0 { INK } else { FAINT },
        );
        if ui
            .input
            .mouse
            .is_some_and(|m| rect(x, y, 38, 13).contains(m))
        {
            data.message = r.name().to_string();
        }
    }

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
            data.held = None;
            match target {
                SlotRef::Bag(i) => {
                    game.equip(i);
                }
                SlotRef::Worn(s) => {
                    game.unequip(s);
                }
            }
        } else {
            drop_on(game, from, target, data);
            data.held = None;
        }
    } else if let Some(slot) = clicked {
        match data.held {
            None => {
                if item_in(game, slot).is_some() {
                    data.held = Some(slot);
                }
            }
            Some(from) if from == slot => {
                data.held = None;
                match slot {
                    SlotRef::Bag(i) => {
                        game.equip(i);
                    }
                    SlotRef::Worn(s) => {
                        game.unequip(s);
                    }
                }
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

    let shown = data.held.or(focused);
    if let Some(item) = shown.and_then(|s| item_in(game, s)).cloned() {
        detail(ui, game, &item, 344, 28, 126);
    } else {
        ui.paragraph(
            "Elegí un objeto para ver sus palabras y efectos.",
            344,
            30,
            126,
            DIM,
        );
        let p = &game.profile.player;
        let mut y = 70;
        ui.text("Consumibles", 344, y, GOLD);
        y += 12;
        for c in Consumable::ALL {
            ui.plain(&format!("{}: {}", c.name(), p.consumable(c)), 344, y, INK);
            y += 10;
        }
        y += 8;
        ui.text("Totales", 344, y, GOLD);
        y += 12;
        for line in [
            format!("Daño {}", game.derived.damage.round()),
            format!("Defensa {}", game.derived.defense.round()),
            format!("Velocidad {}", game.derived.speed.round()),
            format!("Luz {:.1} baldosas", game.derived.light_radius),
        ] {
            ui.plain(&line, 344, y, INK);
            y += 10;
        }
        y += 8;
        for relic in &game.profile.player.relics {
            if let Some(def) = game.data.items.relics.iter().find(|r| &r.id == relic)
                && y < 246
            {
                ui.plain(
                    &format!("♦ {}", def.name),
                    344,
                    y,
                    rarity_rgba(babel_core::babel::rarity::Rarity::Legendary),
                );
                y += 10;
            }
        }
    }

    // The dragged icon follows the cursor.
    if let Some(item) = data.held.and_then(|s| item_in(game, s))
        && let Some(m) = ui.input.mouse
        && ui.input.mouse_down
    {
        let icon = slot_icon(item.slot);
        ui.icon(icon, m.x as i32 - 5, m.y as i32 - 5);
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.text(&text, 120, 261, GOLD);
    }
    if data.held.is_some() && !ui.input.mouse_down {
        ui.footer("Elegí destino · mismo lugar: equipar/quitar · Esc: soltar");
    } else {
        ui.footer("Arrastrar o Enter/A: mover · X: reciclar · Q/Tab o L1/R1: pestañas");
    }
}
