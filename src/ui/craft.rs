//! Crafting, building, chest, return lamps and the Scriptorium.

use super::inventory::{consumable_icon, resource_icon, slot_icon, tool_kind_icon};
use super::state::{Cmd, SlotRef, UiData};
use super::theme::{Place, art};
use super::{BAD, DIM, FAINT, GOLD, GOOD, INK, SKY, Ui, WIDTH, rarity_rgba, rect};
use crate::render::canvas::{Blit, rgb};
#[cfg(test)]
use crate::render::font::Font;
use crate::render::scene::rarity_color;
use babel_core::data::{
    Consumable, RecipeDef, RecipeOutput, Resource, Slot, Station, ToolKind, UpgradeTarget,
};
use babel_core::game::Game;
use babel_core::progression::crafting::{self, CraftError};
use babel_core::progression::guide::{self, Guide};
use babel_core::progression::inscribe::{self, Target};
use babel_core::progression::items::{Item, word_effect};
use babel_core::progression::pins::{MAX_PINS, PinResult};
use babel_core::progression::upgrades::{self, Change};

/// One chip per ingredient: its icon and "owned / needed". The colour alone
/// says whether there is enough. All of them share one row `width` wide.
fn cost(ui: &mut Ui, game: &Game, entries: &[(Resource, u32)], x: i32, y: i32, width: i32) -> i32 {
    let count = entries.len().max(1) as i32;
    let chip = ((width - GAP * (count - 1)) / count).min(66);
    for (i, &(resource, need)) in entries.iter().enumerate() {
        let cx = x + i as i32 * (chip + GAP);
        let cy = y;
        let have = game.profile.player.resource(resource);
        let enough = have >= need;
        let edge = if enough {
            rgb(70, 140, 90)
        } else {
            rgb(150, 66, 84)
        };
        let well = ui.theme.well;
        ui.canvas.fill(cx, cy, chip, 36, well);
        ui.canvas.frame(cx, cy, chip, 36, edge);
        let big = Blit {
            scale: 1.5,
            ..Default::default()
        };
        ui.icon_at(&resource_icon(resource), cx + 12, cy + 13, big);
        let amount = format!("{have}/{need}");
        ui.right(
            &amount,
            cx + chip - 3,
            cy + 10,
            if enough { GOOD } else { BAD },
        );
        ui.centered(resource.name(), cx + chip / 2, cy + 26, DIM);
    }
    y + 40
}

/// Width and height each screen keeps for a recipe's explanation.
pub const CRAFTING_TEXT: (i32, i32) = (228, 108);
pub const BUILDING_TEXT: (i32, i32) = (200, 122);

/// The parts of an explanation: a heading, if any, its text and its colour.
fn parts(guide: &Guide) -> impl Iterator<Item = (Option<&'static str>, &str, [u8; 4])> {
    [
        (Some("Qué hace"), guide.does.as_str(), INK),
        (Some("En qué te ayuda"), guide.helps.as_str(), INK),
    ]
    .into_iter()
    .chain(guide.missing.as_deref().map(|text| (None, text, BAD)))
}

/// Space between two parts.
const GAP: i32 = 4;
const LINE: i32 = 10;

/// How tall an explanation is when written `width` wide, to check that every
/// one fits.
#[cfg(test)]
pub fn guide_height(font: &Font, guide: &Guide, width: i32) -> i32 {
    parts(guide)
        .enumerate()
        .map(|(i, (heading, text, _))| {
            let lines = font.wrap(text, width).len() as i32 + i32::from(heading.is_some());
            lines * LINE + if i > 0 { GAP } else { 0 }
        })
        .sum()
}

/// The look a station will have once built.
fn station_place(station: Station) -> Place {
    match station {
        Station::Binder => Place::Binder,
        Station::Forge => Place::Forge,
        Station::Alembic => Place::Alembic,
        Station::Scriptorium => Place::Scriptorium,
        Station::Chest => Place::Chest,
    }
}

/// The secondary press keeps a recipe in sight, or lets it go.
fn pin(game: &mut Game, recipe: &str, data: &mut UiData) {
    data.message = match game.toggle_pin(recipe) {
        PinResult::Full => format!("Ya hay {MAX_PINS} fijadas: soltá una primero"),
        PinResult::Pinned | PinResult::Unpinned | PinResult::Unknown => String::new(),
    };
}

/// A small gold diamond beside what is pinned.
fn pin_mark(ui: &mut Ui, x: i32, y: i32) {
    for (dy, half) in [0, 1, 2, 1, 0].into_iter().enumerate() {
        ui.canvas
            .fill(x - half, y + dy as i32, half * 2 + 1, 1, GOLD);
    }
}

/// What a recipe does and how it helps, before anything is spent.
fn explanation(ui: &mut Ui, guide: &Guide, x: i32, mut y: i32, width: i32) {
    for (i, (heading, text, color)) in parts(guide).enumerate() {
        if i > 0 {
            y += GAP;
        }
        if let Some(heading) = heading {
            ui.section(heading, x, x + width, y, SKY);
            y += LINE;
        }
        y = ui.paragraph(text, x, y, width, color);
    }
}

/// The colour that says which thing an option is about.
fn thing_color(id: &str) -> [u8; 4] {
    match id {
        "knife" => rgb(150, 206, 255),
        "hammer" => rgb(255, 154, 96),
        "lamp" => rgb(255, 224, 100),
        "lens" => rgb(104, 232, 214),
        "cover" => rgb(255, 134, 134),
        "pages" => rgb(250, 232, 192),
        "torches" => rgb(255, 180, 84),
        "tonic" => rgb(204, 150, 255),
        "ink" => rgb(128, 160, 255),
        _ => rgb(170, 222, 150),
    }
}

fn output_color(output: RecipeOutput) -> [u8; 4] {
    match output {
        RecipeOutput::Tool(kind, _) => thing_color(match kind {
            ToolKind::Knife => "knife",
            ToolKind::Hammer => "hammer",
            ToolKind::Lamp => "lamp",
            ToolKind::Lens => "lens",
        }),
        RecipeOutput::Whetstone(_) => rgb(206, 200, 226),
        RecipeOutput::Item(thing, _) => thing_color(match thing {
            Consumable::Torch => "torches",
            Consumable::Tonic => "tonic",
            Consumable::PageAmmo => "pages",
            Consumable::Index => "lamp",
        }),
        RecipeOutput::Material(..) => thing_color("ink"),
        RecipeOutput::Build(_) | RecipeOutput::ReturnLamp => thing_color(""),
    }
}

fn recipe_icon(output: RecipeOutput) -> String {
    match output {
        RecipeOutput::Tool(kind, _) => tool_kind_icon(kind).into(),
        RecipeOutput::Whetstone(_) => "ui_whetstone".into(),
        RecipeOutput::Item(c, _) => consumable_icon(c).into(),
        RecipeOutput::Material(r, _) => resource_icon(r),
        RecipeOutput::Build(_) => "icon_hammer".into(),
        RecipeOutput::ReturnLamp => "icon_lantern".into(),
    }
}

fn station_of(r: &RecipeDef) -> usize {
    match r.output {
        RecipeOutput::Tool(..) | RecipeOutput::Whetstone(_) => 0,
        _ => 1,
    }
}

pub fn crafting_screen(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(210);
    let name = data.bench.map_or("Fabricación", Station::name);
    ui.heading(name, 4);
    // A station that improves things adds a tab of its own.
    let bench = data
        .bench
        .and_then(|station| Some((station, bench_tab(station)?)));
    let labels: Vec<&str> = ["Herramientas", "Materiales"]
        .into_iter()
        .chain(bench.map(|b| b.1))
        .collect();
    data.craft_tab = data.craft_tab.min(labels.len() - 1);
    for (i, label) in labels.iter().copied().enumerate() {
        let r = rect(8 + i as i32 * 94, 24, 90, 15);
        if ui.button(r, label) {
            data.craft_tab = i;
        }
        if data.craft_tab == i {
            let mark = ui.theme.accent;
            ui.canvas.fill(r.x + 3, r.y + r.h - 3, r.w - 6, 2, mark);
            ui.canvas.fill(r.x + r.w / 2 - 2, r.y + r.h - 1, 5, 2, mark);
        }
    }
    if ui.input.tab != 0 {
        let count = labels.len() as i32;
        data.craft_tab = (data.craft_tab as i32 + ui.input.tab).rem_euclid(count) as usize;
    }
    ui.card(rect(4, 43, 222, 213), SKY);
    ui.card(rect(232, 43, 244, 213), rgb(190, 150, 255));
    let improving = bench.filter(|_| data.craft_tab == IMPROVE_TAB);
    match improving {
        Some((station, _)) => improvements(ui, game, station, data),
        None => recipe_list(ui, game, data),
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.centered(&text, 354, 246, GOLD);
    }
    if ui.button(rect(398, 24, 74, 15), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    let mut hints = vec![(ui.glyph("Tab", "R1"), "pestaña")];
    if improving.is_none() {
        hints.push((ui.glyph("X", "X"), "fijar en pantalla"));
    }
    hints.push((back, "cerrar"));
    ui.hints(&hints);
}

/// The recipes of the tab shown, and what the one under the cursor is.
fn recipe_list(ui: &mut Ui, game: &mut Game, data: &mut UiData) {
    let data_ref = game.data.clone();
    // Tools show only the next tier of each, which keeps the list short.
    let recipes: Vec<&RecipeDef> = data_ref
        .recipes
        .iter()
        .filter(|r| station_of(r) == data.craft_tab)
        .filter(|r| match r.output {
            RecipeOutput::Tool(kind, tier) => tier == game.profile.player.tool_tier(kind) + 1,
            RecipeOutput::Whetstone(level) => level == game.profile.player.whetstones + 1,
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
        ui.stripe(r, output_color(recipe.output));
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
        if game.profile.player.pinned(&recipe.id) {
            pin_mark(ui, r.x + r.w - 18, middle - 2);
        }
        if response.focused {
            selected = Some(recipe);
        }
        if response.activated {
            chosen = Some(recipe.id.clone());
        }
        if response.alt {
            pin(game, &recipe.id, data);
        }
    }
    if recipes.is_empty() {
        ui.centered("Ya fabricaste todo lo de esta pestaña.", 115, 90, DIM);
    }
    if let Some(recipe) = selected {
        let big = Blit {
            scale: 2.5,
            ..Default::default()
        };
        ui.medallion(258, 63, 14);
        ui.icon_at(&recipe_icon(recipe.output), 258, 63, big);
        let mut y = 54;
        for line in ui.font.wrap(&recipe.name, 188) {
            ui.text(&line, 282, y, GOLD);
            y += 10;
        }
        let guide = guide::explain(&game.data, &game.profile.player, recipe);
        explanation(ui, &guide, 240, 80, CRAFTING_TEXT.0);
        let entries = crafting::effective_cost(recipe, &game.derived);
        let below = cost(ui, game, &entries, 240, 190, CRAFTING_TEXT.0);
        match game.craft_check(&recipe.id) {
            Ok(()) => {
                let key = ui.glyph("Enter", "A");
                let end = ui.keycap(key, 240, below + 4);
                ui.text("fabricar", end + 5, below + 4, GOOD);
            }
            Err(e) => {
                ui.icon("icon_lock", 240, below + 2);
                ui.paragraph(&e.message(), 254, below + 4, 214, BAD);
            }
        }
    }
    if let Some(id) = chosen {
        match game.craft(&id, None) {
            Ok(()) => data.message = "¡Listo!".into(),
            Err(e) => data.message = e.message(),
        }
    }
}

/// Position of a station's own tab, after the two every one has.
const IMPROVE_TAB: usize = 2;
/// Height of each row of improvements.
const BENCH_ROW: i32 = 12;

/// What a station calls improving its things, as a tab and as an action.
fn bench_words(station: Station) -> Option<(&'static str, &'static str)> {
    match station {
        Station::Forge => Some(("Templar", "templar")),
        Station::Binder => Some(("Encuadernar", "encuadernar")),
        Station::Alembic => Some(("Destilar", "destilar")),
        Station::Scriptorium | Station::Chest => None,
    }
}

fn bench_tab(station: Station) -> Option<&'static str> {
    bench_words(station).map(|words| words.0)
}

fn thing_icon(target: &UpgradeTarget) -> String {
    if let Some(kind) = target.tool {
        return tool_kind_icon(kind).into();
    }
    match target.id.as_str() {
        "pages" => consumable_icon(Consumable::PageAmmo).into(),
        "torches" => consumable_icon(Consumable::Torch).into(),
        "tonic" => consumable_icon(Consumable::Tonic).into(),
        "ink" => resource_icon(Resource::Ink),
        _ => "ui_book".into(),
    }
}

/// Each thing the station works on with its choices; one at most is fitted.
fn improvements(ui: &mut Ui, game: &mut Game, station: Station, data: &mut UiData) {
    let all = game.data.clone();
    let mut selected = None;
    let mut chosen = None;
    let mut y = 47;
    for target in all.upgrades.targets.iter().filter(|t| t.station == station) {
        ui.icon_at(&thing_icon(target), 18, y + 6, Blit::default());
        let tone = thing_color(&target.id);
        ui.section(&target.name, 30, 218, y + 3, tone);
        y += BENCH_ROW;
        let own = all.upgrades.upgrades.iter();
        for upgrade in own.filter(|u| u.target == target.id) {
            let r = rect(18, y, 202, BENCH_ROW - 1);
            y += BENCH_ROW;
            let response = ui.focusable(r);
            let fitted = game.profile.player.upgrades.get(&target.id);
            let level = fitted.filter(|f| f.id == upgrade.id).map(|f| f.level);
            ui.cell(r, &response, level.map(|_| GOLD));
            ui.stripe(r, thing_color(&target.id));
            let ready = game.upgrade_check(&upgrade.id).is_ok();
            let color = match (level, ready) {
                (Some(_), _) => GOLD,
                (None, true) => INK,
                (None, false) => DIM,
            };
            ui.plain(&upgrade.name, r.x + 6, r.y + 2, color);
            if let Some(level) = level {
                let text = format!("Nivel {}", upgrades::numeral(level));
                ui.right(&text, r.x + r.w - 15, r.y + 2, GOLD);
            }
            let dot = if ready { GOOD } else { rgb(96, 74, 130) };
            ui.canvas.fill(r.x + r.w - 9, r.y + 4, 4, 4, dot);
            if response.focused {
                selected = Some((target, upgrade));
            }
            if response.activated {
                chosen = Some(upgrade.id.clone());
            }
        }
    }
    if let Some((target, upgrade)) = selected {
        let big = Blit {
            scale: 2.5,
            ..Default::default()
        };
        ui.medallion(258, 63, 14);
        ui.icon_at(&thing_icon(target), 258, 63, big);
        let accent = ui.theme.accent;
        ui.text(&upgrade.name, 282, 54, accent);
        ui.text(&target.name, 282, 65, DIM);
        if let Some(guide) = upgrades::explain(&all, &game.profile.player, &upgrade.id) {
            explanation(ui, &guide, 240, 80, CRAFTING_TEXT.0);
        }
        let offer = upgrades::offer(&all, &game.profile.player, &game.derived, &upgrade.id);
        let below = match offer {
            Ok(offer) => cost(ui, game, &offer.cost, 240, 190, CRAFTING_TEXT.0),
            Err(_) => 230,
        };
        match game.upgrade_check(&upgrade.id) {
            Ok(offer) => {
                let verb = match offer.change {
                    Change::New => bench_words(station).map_or("mejorar", |w| w.1).to_string(),
                    Change::Raise => {
                        format!("subir al nivel {}", upgrades::numeral(offer.level))
                    }
                    Change::Swap => "cambiar por esta".into(),
                };
                let key = ui.glyph("Enter", "A");
                let end = ui.keycap(key, 240, below + 4);
                ui.text(&verb, end + 5, below + 4, GOOD);
            }
            Err(e) => {
                ui.icon("icon_lock", 240, below + 2);
                ui.paragraph(&e.message(), 254, below + 4, 214, BAD);
            }
        }
    }
    if let Some(id) = chosen {
        match game.upgrade(&id) {
            Ok(()) => data.message = "¡Listo!".into(),
            Err(e) => data.message = e.message(),
        }
    }
}

pub fn build_screen(
    ui: &mut Ui,
    game: &mut Game,
    plot: u8,
    data: &mut UiData,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(195);
    ui.heading("Construir", 14);
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
        let done = built.contains(&station);
        ui.with_place(station_place(station), |ui| {
            ui.cell(r, &response, None);
            let theme = ui.theme;
            ui.stripe(r, theme.accent);
            let emblem = art::emblem(theme.place);
            let (ew, eh) = (emblem[0].len() as i32, emblem.len() as i32);
            let at = (r.x + r.w - 32 - ew / 2, r.y + (r.h - eh) / 2);
            art::stamp(ui.canvas, emblem, at.0, at.1, &theme);
            let color = if done { FAINT } else { INK };
            ui.text(station.name(), r.x + 9, r.y + 6, color);
        });
        if done {
            ui.right("✓", r.x + r.w - 7, r.y + 6, GOOD);
        }
        if game.profile.player.pinned(&recipe.id) {
            pin_mark(ui, r.x + r.w - 56, r.y + 7);
        }
        if response.alt && !done {
            pin(game, &recipe.id, data);
        }
        if response.focused {
            selected = Some(recipe);
        }
        if response.activated && !done {
            chosen = Some(recipe.id.clone());
        }
    }
    if let Some(recipe) = selected {
        ui.text(&recipe.name, 252, 45, GOLD);
        let guide = guide::explain(&game.data, &game.profile.player, recipe);
        explanation(ui, &guide, 252, 60, BUILDING_TEXT.0);
        let entries = crafting::effective_cost(recipe, &game.derived);
        cost(ui, game, &entries, 252, 186, BUILDING_TEXT.0);
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
    ui.hints(&[(ui.glyph("X", "X"), "fijar en pantalla"), (back, "cerrar")]);
}

pub fn chest_screen(ui: &mut Ui, game: &mut Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(200);
    ui.heading("Cofre", 6);
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
    ui.heading("Lámparas de retorno", 10);
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
    ui.heading("Scriptorium", 3);
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
