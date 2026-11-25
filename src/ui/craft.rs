//! Crafting, building, chest, return lamps and the Scriptorium.

use super::inventory::{consumable_icon, resource_icon, slot_icon, tool_kind_icon};
use super::state::{Cmd, SlotRef, UiData};
use super::{BAD, DIM, FAINT, GOLD, GOOD, INK, SKY, Ui, WIDTH, rarity_rgba, rect};
use crate::render::canvas::{Blit, rgb};
use crate::render::scene::rarity_color;
use babel_core::data::{RecipeDef, RecipeOutput, Resource, Slot, Station};
use babel_core::game::Game;
use babel_core::progression::crafting::{self, CraftError};
use babel_core::progression::inscribe::{self, Target};
use babel_core::progression::items::{Item, word_effect};

/// One chip per ingredient: its icon and "owned / needed". The colour alone
/// says whether there is enough.
fn cost(ui: &mut Ui, game: &Game, recipe: &RecipeDef, x: i32, y: i32, per_row: i32) -> i32 {
    let entries = crafting::effective_cost(recipe, &game.derived);
    for (i, &(resource, need)) in entries.iter().enumerate() {
        let cx = x + (i as i32 % per_row) * 70;
        let cy = y + (i as i32 / per_row) * 40;
        let have = game.profile.player.resource(resource);
        let enough = have >= need;
        let edge = if enough {
            rgb(70, 140, 90)
        } else {
            rgb(150, 66, 84)
        };
        ui.canvas.fill(cx, cy, 66, 36, [26, 15, 46, 220]);
        ui.canvas.frame(cx, cy, 66, 36, edge);
        let big = Blit {
            scale: 1.5,
            ..Default::default()
        };
        ui.icon_at(&resource_icon(resource), cx + 16, cy + 13, big);
        let amount = format!("{have}/{need}");
        ui.right(&amount, cx + 62, cy + 10, if enough { GOOD } else { BAD });
        ui.centered(resource.name(), cx + 33, cy + 26, DIM);
    }
    let rows = (entries.len() as i32 + per_row - 1) / per_row;
    y + rows * 40
}

fn recipe_icon(output: RecipeOutput) -> String {
    match output {
        RecipeOutput::Tool(kind, _) => tool_kind_icon(kind).into(),
        RecipeOutput::Item(c, _) => consumable_icon(c).into(),
        RecipeOutput::Material(r, _) => resource_icon(r),
        RecipeOutput::Build(_) => "icon_hammer".into(),
        RecipeOutput::ReturnLamp => "icon_lantern".into(),
    }
}

fn station_of(r: &RecipeDef) -> usize {
    match r.output {
        RecipeOutput::Tool(..) => 0,
        _ => 1,
    }
}

pub fn crafting_screen(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(210);
    ui.title("Fabricación", WIDTH / 2, 4, 2, GOLD);
    for (i, label) in ["Herramientas", "Materiales"].into_iter().enumerate() {
        let r = rect(8 + i as i32 * 94, 24, 90, 15);
        if ui.button(r, label) {
            data.craft_tab = i;
        }
        if data.craft_tab == i {
            ui.canvas.fill(r.x + 4, r.y + r.h - 2, r.w - 8, 2, GOLD);
        }
    }
    if ui.input.tab != 0 {
        data.craft_tab = 1 - data.craft_tab.min(1);
    }
    ui.card(rect(4, 43, 222, 213), SKY);
    ui.card(rect(232, 43, 244, 213), rgb(190, 150, 255));
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
    let step = (200 / recipes.len().max(1) as i32).clamp(15, 22);
    for (i, recipe) in recipes.iter().enumerate() {
        let r = rect(10, 50 + i as i32 * step, 210, step - 2);
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        let state = game.craft_check(&recipe.id);
        let color = match state {
            Ok(()) => INK,
            Err(CraftError::MissingResources) => DIM,
            Err(_) => FAINT,
        };
        let middle = r.y + r.h / 2;
        ui.icon_at(
            &recipe_icon(recipe.output),
            r.x + 11,
            middle,
            Blit::default(),
        );
        ui.text(&recipe.name, r.x + 22, middle - 3, color);
        let dot = if state.is_ok() {
            GOOD
        } else {
            rgb(96, 74, 130)
        };
        ui.canvas.fill(r.x + r.w - 9, middle - 2, 4, 4, dot);
        if response.focused {
            selected = Some(recipe);
        }
        if response.activated {
            chosen = Some(recipe.id.clone());
        }
    }
    if recipes.is_empty() {
        ui.centered("Todas tus herramientas son Áureas.", 115, 90, DIM);
    }
    if let Some(recipe) = selected {
        let big = Blit {
            scale: 3.0,
            ..Default::default()
        };
        ui.canvas.glow(264.0, 72.0, 24.0, [150, 120, 230, 150]);
        ui.icon_at(&recipe_icon(recipe.output), 264, 72, big);
        let mut y = 56;
        for line in ui.font.wrap(&recipe.name, 170) {
            ui.text(&line, 296, y, GOLD);
            y += 10;
        }
        ui.paragraph(&recipe.description, 296, y + 4, 172, DIM);
        let below = cost(ui, game, recipe, 250, 116, 3);
        match game.craft_check(&recipe.id) {
            Ok(()) => {
                let key = ui.glyph("Enter", "A");
                let end = ui.keycap(key, 250, below + 8);
                ui.text("fabricar", end + 5, below + 8, GOOD);
            }
            Err(e) => {
                ui.icon("icon_lock", 250, below + 6);
                ui.paragraph(&e.message(), 264, below + 8, 204, BAD);
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
        ui.centered(&text, 354, 240, GOLD);
    }
    if ui.button(rect(398, 24, 74, 15), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(ui.glyph("Tab", "R1"), "pestaña"), (back, "cerrar")]);
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
    ui.card(rect(20, 38, 216, 190), SKY);
    ui.card(rect(244, 38, 216, 190), rgb(190, 150, 255));
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
        let r = rect(28, 48 + row * 24, 200, 20);
        row += 1;
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        let done = built.contains(&station);
        ui.text(
            station.name(),
            r.x + 8,
            r.y + 6,
            if done { FAINT } else { INK },
        );
        if done {
            ui.right("✓", r.x + r.w - 7, r.y + 6, GOOD);
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
        cost(ui, game, recipe, 252, y + 10, 3);
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
    if ui.button(rect(WIDTH / 2 - 50, 241, 100, 14), "Cancelar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
}

pub fn chest_screen(ui: &mut Ui, game: &mut Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(200);
    ui.title("Cofre", WIDTH / 2, 6, 2, GOLD);
    ui.centered(
        "Lo guardado no se pierde al caer. Cada pulsación mueve 10.",
        WIDTH / 2,
        28,
        DIM,
    );
    ui.card(rect(40, 40, 400, 210), SKY);
    let stored_of =
        |game: &Game, r: Resource| game.profile.world.storage.get(&r).copied().unwrap_or(0);
    // Materials that are neither carried nor stored are left out.
    let present: Vec<Resource> = Resource::ALL
        .into_iter()
        .filter(|&r| game.profile.player.resource(r) > 0 || stored_of(game, r) > 0)
        .collect();
    if present.is_empty() {
        ui.centered(
            "No llevás materiales y el cofre está vacío.",
            WIDTH / 2,
            120,
            DIM,
        );
    } else {
        ui.text("Llevás", 168, 47, GOLD);
        ui.text("Guardado", 228, 47, GOLD);
    }
    let step = (162 / present.len().max(1) as i32).clamp(12, 18);
    for (i, &resource) in present.iter().enumerate() {
        let y = 59 + i as i32 * step;
        let carried = game.profile.player.resource(resource);
        let stored = stored_of(game, resource);
        let middle = y + (step - 1) / 2;
        ui.icon_at(&resource_icon(resource), 58, middle, Blit::default());
        ui.plain(resource.name(), 70, middle - 3, INK);
        let dim = |n: u32| if n > 0 { INK } else { FAINT };
        ui.plain(&carried.to_string(), 180, middle - 3, dim(carried));
        ui.plain(&stored.to_string(), 248, middle - 3, dim(stored));
        if ui.button(rect(290, y, 68, step - 1), "Guardar") {
            game.deposit(resource, 10);
        }
        if ui.button(rect(362, y, 68, step - 1), "Sacar") {
            game.withdraw(resource, 10);
        }
    }
    if ui.primary_button(rect(60, 232, 130, 13), "Guardar todo") {
        for r in Resource::ALL {
            game.deposit(r, u32::MAX);
        }
    }
    if ui.button(rect(300, 232, 130, 13), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
}

pub fn lamps_screen(ui: &mut Ui, game: &mut Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(200);
    ui.title("Lámparas de retorno", WIDTH / 2, 10, 2, GOLD);
    ui.card(rect(80, 36, 320, 206), SKY);
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
        if ui.icon_button(
            rect(92, 44 + row * 15, 296, 13),
            "icon_lantern",
            &lamp.label,
        ) {
            cmds.push(Cmd::CloseAll);
            game.travel_to_lamp(i);
        }
    }
    if ui.button(rect(190, 226, 100, 13), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
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
const ITEM_ROWS: usize = 11;

pub fn scriptorium(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    ui.title("Scriptorium", WIDTH / 2, 3, 2, GOLD);
    let ink = game.profile.player.resource(Resource::Ink);
    ui.right(&format!("Tinta: {ink}"), WIDTH - 8, 8, INK);
    ui.card(rect(4, 22, 150, 234), SKY);
    ui.card(rect(158, 22, 190, 234), rgb(190, 150, 255));
    ui.card(rect(352, 22, 124, 234), GOLD);

    // Items: worn first, then the bag.
    let mut targets: Vec<SlotRef> = Slot::ALL
        .into_iter()
        .filter(|&s| game.profile.player.equipped(s).is_some())
        .map(SlotRef::Worn)
        .collect();
    targets.extend((0..game.profile.player.inventory.len()).map(SlotRef::Bag));
    if data
        .script_target
        .is_none_or(|t| item_ref(game, t).is_none())
    {
        data.script_target = targets.first().copied();
    }
    ui.text("1. Objeto", 10, 27, GOLD);
    // The list scrolls only when asked to, so the row under the cursor never
    // changes identity by itself.
    let last_scroll = targets.len().saturating_sub(ITEM_ROWS);
    data.script_scroll =
        (data.script_scroll as i32 + ui.input.scroll).clamp(0, last_scroll as i32) as usize;
    for (row, &slot) in targets
        .iter()
        .skip(data.script_scroll)
        .take(ITEM_ROWS)
        .enumerate()
    {
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
        if response.activated {
            data.script_target = Some(slot);
        }
    }
    if targets.len() > ITEM_ROWS {
        if ui.button(rect(8, 223, 70, 12), "Subir") {
            data.script_scroll = data.script_scroll.saturating_sub(ITEM_ROWS - 1);
        }
        if ui.button(rect(80, 223, 70, 12), "Bajar") {
            data.script_scroll = (data.script_scroll + ITEM_ROWS - 1).min(last_scroll);
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
        let r = rect(163 + (i as i32 % 2) * 91, 39 + (i as i32 / 2) * 16, 89, 14);
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
    ui.centered(&format!("{}/{pages}", data.script_page + 1), 253, 240, DIM);
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
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(ui.glyph("Tab", "R1"), "más palabras"), (back, "cerrar")]);
}
