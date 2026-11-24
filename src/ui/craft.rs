//! Crafting, building, chest, return lamps and the Scriptorium.

use super::inventory::{resource_icon, slot_icon};
use super::state::{Cmd, SlotRef, UiData};
use super::{BAD, DIM, FAINT, GOLD, GOOD, INK, PANEL_EDGE, Ui, WIDTH, rarity_rgba, rect};
use crate::render::canvas::Blit;
use crate::render::scene::rarity_color;
use babel_core::data::{RecipeDef, RecipeOutput, Resource, Slot, Station};
use babel_core::game::Game;
use babel_core::progression::crafting::{self, CraftError};
use babel_core::progression::inscribe::{self, Target};
use babel_core::progression::items::{Item, word_effect};

/// Cost list showing what is owned and what is missing.
fn cost(ui: &mut Ui, game: &Game, recipe: &RecipeDef, x: i32, mut y: i32) -> i32 {
    for (resource, need) in crafting::effective_cost(recipe, &game.derived) {
        let have = game.profile.player.resource(resource);
        ui.icon(&resource_icon(resource), x, y - 2);
        let color = if have >= need { GOOD } else { BAD };
        let end = ui.text(
            &format!("{} {have}/{need}", resource.name()),
            x + 15,
            y,
            color,
        );
        if have < need {
            ui.text(&format!("faltan {}", need - have), end + 6, y, BAD);
        }
        y += 13;
    }
    y
}

fn station_of(r: &RecipeDef) -> usize {
    match r.output {
        RecipeOutput::Tool(..) => 0,
        _ => 1,
    }
}

pub fn crafting_screen(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(205);
    ui.title("Fabricación", WIDTH / 2, 4, 2, GOLD);
    for (i, label) in ["Herramientas", "Materiales"].into_iter().enumerate() {
        let r = rect(8 + i as i32 * 94, 24, 90, 13);
        let response = ui.focusable(r);
        ui.cell(r, &response, (data.craft_tab == i).then_some(GOLD));
        ui.centered(label, r.x + 45, r.y + 3, if data.craft_tab == i { GOLD } else { INK });
        if response.activated {
            data.craft_tab = i;
        }
    }
    if ui.input.tab != 0 {
        data.craft_tab = 1 - data.craft_tab.min(1);
    }
    ui.panel(rect(4, 40, 236, 216));
    ui.panel(rect(244, 40, 232, 216));
    let data_ref = game.data.clone();
    // Tools show only the next tier of each, which keeps the list short.
    let recipes: Vec<&RecipeDef> = data_ref
        .recipes
        .iter()
        .filter(|r| station_of(r) == data.craft_tab)
        .filter(|r| match r.output {
            RecipeOutput::Tool(kind, tier) => tier == game.profile.player.tool_tier(kind) + 1,
            RecipeOutput::Build(_) | RecipeOutput::ReturnLamp => false,
            _ => true,
        })
        .collect();
    let mut selected: Option<&RecipeDef> = None;
    let mut chosen: Option<String> = None;
    for (i, recipe) in recipes.iter().enumerate() {
        let r = rect(10, 46 + i as i32 * 17, 224, 15);
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        let state = game.craft_check(&recipe.id);
        let color = match state {
            Ok(()) => INK,
            Err(CraftError::MissingResources) => DIM,
            Err(_) => FAINT,
        };
        ui.text(&recipe.name, r.x + 5, r.y + 4, color);
        if state.is_ok() {
            ui.right("✓", r.x + r.w - 5, r.y + 4, GOOD);
        }
        if response.focused {
            selected = Some(recipe);
        }
        if response.activated {
            chosen = Some(recipe.id.clone());
        }
    }
    if recipes.is_empty() {
        ui.centered("Todas tus herramientas son Áureas.", 122, 90, DIM);
    }
    if let Some(recipe) = selected {
        ui.text(&recipe.name, 252, 48, GOLD);
        let mut y = ui.paragraph(&recipe.description, 252, 62, 216, DIM) + 6;
        if let Some(station) = recipe.station {
            let built = game.stations_built().contains(&station);
            ui.text(
                &format!("Estación: {}", station.name()),
                252,
                y,
                if built { INK } else { BAD },
            );
            y += 14;
        }
        y = cost(ui, game, recipe, 252, y + 2);
        match game.craft_check(&recipe.id) {
            Ok(()) => {
                ui.text("Enter / A / clic para fabricar", 252, y + 6, GOOD);
            }
            Err(e) => {
                ui.text(&e.message(), 252, y + 6, BAD);
            }
        }
    }
    if let Some(id) = chosen {
        match game.craft(&id, None) {
            Ok(()) => data.message = "¡Listo!".into(),
            Err(e) => data.message = e.message(),
        }
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.text(&text, 252, 240, GOLD);
    }
    if ui.button(rect(396, 22, 78, 15), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    ui.footer("Q/Tab o L1/R1: pestañas · Esc / B: cerrar");
}

pub fn build_screen(
    ui: &mut Ui,
    game: &mut Game,
    plot: u8,
    data: &mut UiData,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(195);
    ui.title("Construir", WIDTH / 2, 14, 2, GOLD);
    ui.panel(rect(20, 38, 216, 190));
    ui.panel(rect(244, 38, 216, 190));
    let built = game.stations_built();
    let data_ref = game.data.clone();
    let mut selected = None;
    let mut chosen = None;
    let mut row = 0;
    for station in Station::ALL {
        let Some(recipe) = data_ref
            .recipes
            .iter()
            .find(|r| r.output == RecipeOutput::Build(station))
        else {
            continue;
        };
        let r = rect(28, 48 + row * 22, 200, 18);
        row += 1;
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        let done = built.contains(&station);
        ui.text(
            station.name(),
            r.x + 6,
            r.y + 5,
            if done { FAINT } else { INK },
        );
        if done {
            ui.right("construida", r.x + r.w - 5, r.y + 5, GOOD);
        }
        if response.focused {
            selected = Some(recipe);
        }
        if response.activated && !done {
            chosen = Some(recipe.id.clone());
        }
    }
    if let Some(recipe) = selected {
        ui.text(&recipe.name, 252, 46, GOLD);
        let y = ui.paragraph(&recipe.description, 252, 60, 200, DIM);
        cost(ui, game, recipe, 252, y + 10);
    }
    if let Some(id) = chosen {
        match game.craft(&id, Some(plot)) {
            Ok(()) => cmds.push(Cmd::CloseAll),
            Err(e) => data.message = e.message(),
        }
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.centered(&text, WIDTH / 2, 232, BAD);
    }
    if ui.button(rect(WIDTH / 2 - 50, 244, 100, 13), "Cancelar") {
        cmds.push(Cmd::CloseAll);
    }
    ui.footer("Las estaciones se usan acercándote a ellas.");
}

pub fn chest_screen(ui: &mut Ui, game: &mut Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(200);
    ui.title("Cofre", WIDTH / 2, 6, 2, GOLD);
    ui.centered(
        "Lo guardado aquí no se pierde cuando caés.",
        WIDTH / 2,
        28,
        DIM,
    );
    ui.panel(rect(40, 40, 400, 200));
    ui.text("Material", 52, 46, GOLD);
    ui.text("Llevás", 170, 46, GOLD);
    ui.text("Guardado", 230, 46, GOLD);
    for (i, resource) in Resource::ALL.into_iter().enumerate() {
        let y = 58 + i as i32 * 12;
        let carried = game.profile.player.resource(resource);
        let stored = game.profile.world.storage.get(&resource).copied().unwrap_or(0);
        ui.icon(&resource_icon(resource), 52, y - 1);
        ui.plain(resource.name(), 68, y + 1, INK);
        ui.plain(&carried.to_string(), 176, y + 1, INK);
        ui.plain(&stored.to_string(), 242, y + 1, INK);
        if ui.button(rect(290, y, 68, 11), "Guardar") {
            game.deposit(resource, 10);
        }
        if ui.button(rect(362, y, 68, 11), "Sacar") {
            game.withdraw(resource, 10);
        }
    }
    if ui.button(rect(52, 226, 120, 12), "Guardar todo") {
        for r in Resource::ALL {
            game.deposit(r, u32::MAX);
        }
    }
    if ui.button(rect(310, 226, 120, 12), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    ui.footer("Cada botón mueve hasta 10 unidades.");
}

pub fn lamps_screen(ui: &mut Ui, game: &mut Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(200);
    ui.title("Lámparas de retorno", WIDTH / 2, 10, 2, GOLD);
    ui.panel(rect(80, 36, 320, 206));
    let lamps = game.profile.world.lamps.clone();
    if lamps.is_empty() {
        ui.paragraph(
            "Todavía no encendiste ninguna lámpara. Encendé una con aceite en cualquier sitio: sirve de viaje rápido y de punto de reaparición.",
            96,
            56,
            288,
            DIM,
        );
    }
    for (i, lamp) in lamps.iter().enumerate().rev() {
        let row = (lamps.len() - 1 - i) as i32;
        if ui.button(rect(92, 44 + row * 15, 296, 13), &lamp.label) {
            cmds.push(Cmd::CloseAll);
            game.travel_to_lamp(i);
        }
    }
    if ui.button(rect(190, 226, 100, 13), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    ui.footer("Elegí una lámpara para viajar.");
}

fn item_ref(game: &Game, slot: SlotRef) -> Option<&Item> {
    match slot {
        SlotRef::Bag(i) => game.profile.player.inventory.get(i),
        SlotRef::Worn(s) => game.profile.player.equipped(s),
    }
}

fn target_of(slot: SlotRef) -> Target {
    match slot {
        SlotRef::Bag(i) => Target::Inventory(i),
        SlotRef::Worn(s) => Target::Equipped(s as usize),
    }
}

const WORDS_PER_PAGE: usize = 24;

pub fn scriptorium(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    ui.title("Scriptorium", WIDTH / 2, 3, 2, GOLD);
    let ink = game.profile.player.resource(Resource::Ink);
    ui.right(&format!("Tinta: {ink}"), WIDTH - 8, 8, INK);
    ui.panel(rect(4, 22, 150, 234));
    ui.panel(rect(158, 22, 190, 234));
    ui.panel(rect(352, 22, 124, 234));

    // Items: worn first, then the bag.
    let mut targets: Vec<SlotRef> = Slot::ALL
        .into_iter()
        .filter(|&s| game.profile.player.equipped(s).is_some())
        .map(SlotRef::Worn)
        .collect();
    targets.extend((0..game.profile.player.inventory.len()).map(SlotRef::Bag));
    if data.script_target.is_none_or(|t| item_ref(game, t).is_none()) {
        data.script_target = targets.first().copied();
    }
    ui.text("1. Objeto", 10, 27, GOLD);
    let selected_at = targets
        .iter()
        .position(|&t| Some(t) == data.script_target)
        .unwrap_or(0);
    let first = selected_at.saturating_sub(10).min(targets.len().saturating_sub(13));
    for (row, &slot) in targets.iter().skip(first).take(13).enumerate() {
        let Some(item) = item_ref(game, slot) else {
            continue;
        };
        let r = rect(8, 39 + row as i32 * 16, 142, 14);
        let response = ui.focusable(r);
        let chosen = data.script_target == Some(slot);
        ui.cell(r, &response, chosen.then_some(GOLD));
        ui.icon(slot_icon(item.slot), r.x + 3, r.y + 2);
        let short: String = item.name.chars().take(20).collect();
        let color = rarity_color(item.rarity, ui.time);
        ui.plain(&short, r.x + 17, r.y + 4, color);
        if response.activated || response.focused {
            data.script_target = Some(slot);
        }
    }
    if targets.is_empty() {
        ui.paragraph("No tenés objetos todavía.", 10, 42, 136, DIM);
    }

    ui.text("2. Palabra del Glosario", 164, 27, GOLD);
    let mut words: Vec<(String, babel_core::babel::rarity::Rarity)> = game
        .profile
        .player
        .glossary
        .iter()
        .map(|(w, g)| (w.clone(), g.rarity))
        .collect();
    words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let pages = words.len().div_ceil(WORDS_PER_PAGE).max(1);
    data.script_page = (data.script_page as i32 + ui.input.tab).rem_euclid(pages as i32) as usize;
    let mut hovered: Option<String> = None;
    let mut chosen: Option<String> = None;
    for (i, (word, rarity)) in words
        .iter()
        .skip(data.script_page * WORDS_PER_PAGE)
        .take(WORDS_PER_PAGE)
        .enumerate()
    {
        let r = rect(
            163 + (i as i32 % 2) * 91,
            39 + (i as i32 / 2) * 16,
            89,
            14,
        );
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        ui.plain(word, r.x + 4, r.y + 4, rarity_rgba(*rarity));
        if response.focused {
            hovered = Some(word.clone());
        }
        if response.activated {
            chosen = Some(word.clone());
        }
    }
    if words.is_empty() {
        ui.paragraph(
            "Leé libros con la Lente para llenar tu Glosario.",
            166,
            44,
            174,
            DIM,
        );
    }
    if ui.button(rect(164, 237, 44, 13), "<") {
        data.script_page = (data.script_page + pages - 1) % pages;
    }
    ui.centered(
        &format!("{}/{pages}", data.script_page + 1),
        253,
        240,
        DIM,
    );
    if ui.button(rect(298, 237, 44, 13), ">") {
        data.script_page = (data.script_page + 1) % pages;
    }

    if let Some(slot) = data.script_target
        && let Some(item) = item_ref(game, slot).cloned()
    {
        let mut y = 28;
        for line in ui.font.wrap(&item.name, 114) {
            ui.text(&line, 358, y, rarity_color(item.rarity, ui.time));
            y += 10;
        }
        let slots = inscribe::max_slots(&item, &game.derived);
        ui.text(
            &format!("Renglones {}/{slots}", item.inscriptions.len()),
            358,
            y + 2,
            DIM,
        );
        y += 14;
        for i in &item.inscriptions {
            ui.text(&format!("♦ {}", i.word), 358, y, rarity_rgba(i.rarity));
            y += 10;
        }
        y += 4;
        for line in item.effect_lines(&game.data) {
            if y < 180 {
                ui.plain(&line, 358, y, INK);
                y += 9;
            }
        }
        if let Some(word) = &hovered {
            let tag = word_effect(&game.data, word);
            let effect = game.data.items.effects.iter().find(|e| e.tag == tag);
            ui.text(&format!("«{word}»"), 358, 190, GOLD);
            if let Some(e) = effect {
                let rarity = game
                    .profile
                    .player
                    .glossary
                    .get(word)
                    .map_or(babel_core::babel::rarity::Rarity::Common, |g| g.rarity);
                let value = e.amount * rarity.stat_multiplier();
                ui.paragraph(
                    &babel_core::progression::stats::describe(e.stat, value),
                    358,
                    202,
                    114,
                    GOOD,
                );
            }
            match inscribe::check(&item, word, &game.profile.player, &game.derived) {
                Ok(c) => {
                    ui.text(&format!("Cuesta {c} de tinta"), 358, 224, INK);
                }
                Err(e) => {
                    ui.paragraph(&e.message(), 358, 224, 114, BAD);
                }
            }
        }
        if let Some(word) = chosen {
            match game.inscribe(target_of(slot), &word) {
                Ok(()) => data.message = format!("Inscribiste «{word}»"),
                Err(e) => data.message = e,
            }
        }
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.centered(&text, WIDTH / 2, 251, GOLD);
    }
    if ui.button(rect(8, 237, 80, 13), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let _ = (PANEL_EDGE, Blit::default());
    ui.footer("Q/Tab o L1/R1: páginas de palabras · Esc / B: cerrar");
}
