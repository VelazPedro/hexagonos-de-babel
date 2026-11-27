//! The book reader, the Glossary, the hexagon map and the Catalog.

use super::state::{Cmd, Screen, UiData, keyboard, tabs, text_field, type_into};
use super::{BAD, DIM, FAINT, GOLD, GOOD, INK, SKY, Ui, WIDTH, rarity_rgba, rect};
use crate::render::canvas::{Rgba, hex, mix, rgb, with_alpha};
use crate::render::scene::rarity_color;
use babel_core::babel::body_gen::{LINES_PER_PAGE, PAGES};
use babel_core::babel::detect::{Hit, Place};
use babel_core::babel::lexicon::Category;
use babel_core::babel::rarity::Rarity;
use babel_core::data::Consumable;
use babel_core::game::Game;
use babel_core::progression::items::word_effect;
use babel_core::progression::{catalog, reading, stats};
use babel_core::world::floors;
use babel_core::world::hex::{Dir, neighbor};
use babel_core::world::site::SiteKey;

const HALF: usize = LINES_PER_PAGE / 2;

/// Draws one line of Babel with detected words highlighted by rarity.
fn book_line(ui: &mut Ui, line: &str, hits: &[&Hit], x: i32, y: i32, base: Rgba) {
    let mut ordered: Vec<&&Hit> = hits.iter().collect();
    ordered.sort_by_key(|h| h.start);
    let mut cursor = 0usize;
    let mut pen = x;
    for h in ordered {
        let (start, end) = (h.start as usize, (h.end as usize).min(line.len()));
        if start < cursor {
            continue;
        }
        let Some(before) = line.get(cursor..start) else {
            continue;
        };
        let Some(word) = line.get(start..end) else {
            continue;
        };
        pen = ui.canvas.text(ui.font, before, pen, y, base);
        let color = rarity_color(h.rarity, ui.time);
        let width = ui.font.width(word);
        ui.canvas
            .fill(pen - 1, y - 1, width + 2, 10, with_alpha(color, 62));
        ui.canvas.fill(pen - 1, y + 8, width + 2, 1, color);
        pen = ui.canvas.text_outlined(ui.font, word, pen, y, color);
        cursor = end;
    }
    if let Some(rest) = line.get(cursor..) {
        ui.canvas.text(ui.font, rest, pen, y, base);
    }
}

pub fn reader(ui: &mut Ui, game: &Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(250);
    // The open page stays dark, so that rarities stand out.
    ui.panel(rect(14, 20, 452, 235));
    let readable = reading::readable_pages(&game.profile.player, &game.derived);
    let halves = PAGES * 2;
    data.reader_half = (data.reader_half as i32 + ui.input.tab + ui.input.scroll)
        .clamp(0, halves as i32 - 1) as usize;
    let page = data.reader_half / 2;
    let first = (data.reader_half % 2) * HALF;
    ui.text("Inscripción", 22, 4, GOLD);
    let hits_at = |place: Place| -> Vec<&Hit> {
        game.site
            .analysis
            .hits
            .iter()
            .filter(|h| h.place == place)
            .collect()
    };
    let well = ui.theme.well;
    ui.canvas.fill(18, 23, 432, 13, well);
    book_line(
        ui,
        &game.site.book.inscription,
        &hits_at(Place::Inscription),
        22,
        26,
        rgb(232, 214, 180),
    );
    ui.right(
        &format!(
            "Página {}/{PAGES} · renglones {}-{}",
            page + 1,
            first + 1,
            first + HALF
        ),
        WIDTH - 18,
        4,
        INK,
    );
    let text_color = rgb(178, 160, 172);
    if page < readable {
        if let Some(lines) = game.site.book.pages.get(page) {
            for (row, line) in lines.iter().skip(first).take(HALF).enumerate() {
                let place = Place::Body {
                    page: page as u8,
                    line: (first + row) as u8,
                };
                book_line(
                    ui,
                    line,
                    &hits_at(place),
                    22,
                    40 + row as i32 * 10 - row as i32 / 2,
                    text_color,
                );
            }
        }
    } else {
        ui.centered(
            "Tu lente no alcanza a leer esta página.",
            WIDTH / 2,
            120,
            DIM,
        );
        ui.centered(
            "Fabricá una lente mejor o aprendé «Página de más».",
            WIDTH / 2,
            134,
            FAINT,
        );
    }
    let found = data.reading.new_words.len();
    let summary = if found > 0 {
        format!("Palabras nuevas: {found} · +{} XP", data.reading.xp)
    } else {
        "Nada nuevo para tu Glosario".to_string()
    };
    ui.text(&summary, 22, 241, if found > 0 { GOOD } else { DIM });
    // A legend of only the rarities this book holds.
    let mut x = 18;
    for r in Rarity::ALL {
        if game.site.analysis.hits.iter().any(|h| h.rarity == r) {
            x = ui.text(r.label(), x, 257, rarity_rgba(r)) + 10;
        }
    }
    if ui.button(rect(262, 238, 60, 13), "Antes") {
        data.reader_half = data.reader_half.saturating_sub(1);
    }
    if ui.button(rect(326, 238, 68, 13), "Después") {
        data.reader_half = (data.reader_half + 1).min(halves - 1);
    }
    if ui.button(rect(398, 238, 62, 13), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    if !game.site.analysis.canons.is_empty() {
        ui.right(
            "¡Hay una frase canónica!",
            WIDTH - 18,
            257,
            rarity_color(Rarity::Canonical, ui.time),
        );
    }
}

const GLOSS_ROWS: usize = 18;

pub fn glossary(ui: &mut Ui, game: &Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    tabs(ui, Screen::Glossary, game.catalog_unlocked(), cmds);
    ui.card(rect(4, 36, 324, 220), SKY);
    let filters = [
        "Todas", "Bio", "Luz", "Orient", "Criat", "Mater", "Objeto", "Cual", "Acción",
    ];
    for (i, label) in filters.into_iter().enumerate() {
        let r = rect(4 + i as i32 * 52, 20, 50, 13);
        if ui.button(r, label) {
            data.glossary_filter = i;
            data.glossary_page = 0;
        }
        if data.glossary_filter == i {
            ui.canvas.frame(r.x, r.y, r.w, r.h, GOLD);
        }
    }
    let wanted: Option<Category> = data
        .glossary_filter
        .checked_sub(1)
        .and_then(|i| Category::ALL.get(i))
        .copied();
    let lexicon = &game.data.lexicon;
    let words: Vec<(&String, Rarity)> = game
        .profile
        .player
        .glossary
        .iter()
        .filter(|(w, _)| wanted.is_none_or(|c| lexicon.find(w).is_some_and(|e| e.category == c)))
        .map(|(w, g)| (w, g.rarity))
        .collect();
    let pages = words.len().div_ceil(GLOSS_ROWS).max(1);
    data.glossary_page =
        (data.glossary_page as i32 + ui.input.scroll).rem_euclid(pages as i32) as usize;
    ui.text(
        &format!(
            "{} de {} palabras",
            game.profile.player.glossary.len(),
            lexicon.len()
        ),
        14,
        43,
        DIM,
    );
    for (i, (word, rarity)) in words
        .iter()
        .skip(data.glossary_page * GLOSS_ROWS)
        .take(GLOSS_ROWS)
        .enumerate()
    {
        let r = rect(12 + (i as i32 % 3) * 104, 56 + (i as i32 / 3) * 30, 100, 25);
        let response = ui.focusable(r);
        ui.cell(r, &response, Some(with_alpha(rarity_rgba(*rarity), 130)));
        ui.centered(word, r.x + 50, r.y + 9, rarity_color(*rarity, ui.time));
        if response.focused {
            data.glossary_selected = Some((*word).clone());
        }
    }
    if words.is_empty() {
        ui.centered("Todavía no hay palabras en esta categoría.", 166, 120, DIM);
    }
    if ui.button(rect(100, 240, 40, 13), "<") {
        data.glossary_page = (data.glossary_page + pages - 1) % pages;
    }
    ui.centered(
        &format!("{}/{pages}", data.glossary_page + 1),
        166,
        243,
        DIM,
    );
    if ui.button(rect(192, 240, 40, 13), ">") {
        data.glossary_page = (data.glossary_page + 1) % pages;
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[("rueda", "pasar página"), (back, "cerrar")]);
    let Some(word) = data.glossary_selected.clone() else {
        for (i, line) in ["Elegí una palabra", "para ver su poder."]
            .into_iter()
            .enumerate()
        {
            ui.centered(line, 404, 120 + i as i32 * 11, FAINT);
        }
        return;
    };
    let (Some(entry), Some(gloss)) = (lexicon.find(&word), game.profile.player.glossary.get(&word))
    else {
        return;
    };
    ui.card(rect(332, 36, 144, 220), rarity_color(gloss.rarity, ui.time));
    ui.title(
        &word,
        404,
        46,
        if ui.font.width(&word) * 2 < 136 { 2 } else { 1 },
        rarity_color(gloss.rarity, ui.time),
    );
    let mut y = 70;
    ui.text(entry.category.label(), 338, y, INK);
    y += 11;
    ui.text(
        gloss.rarity.label(),
        338,
        y,
        rarity_color(gloss.rarity, ui.time),
    );
    y += 11;
    ui.text(&format!("Leída {} veces", gloss.times), 338, y, DIM);
    y += 18;
    let tag = word_effect(&game.data, &word);
    if let Some(e) = game.data.items.effects.iter().find(|e| e.tag == tag) {
        ui.text(&format!("Al inscribirla: {}", e.name), 338, y, GOLD);
        y += 11;
        let value = e.amount * gloss.rarity.stat_multiplier();
        y = ui.paragraph(&stats::describe(e.stat, value), 338, y, 132, GOOD) + 6;
    }
    let world = match entry.category {
        Category::Bioma => format!(
            "En un libro: tiñe el sitio ({}).",
            entry
                .tag_value("biome")
                .map_or("galería", |id| game.data.biome(id).name.as_str())
        ),
        Category::Luz => format!(
            "En un libro: luz ambiente {}%.",
            entry.tag_value("lv").unwrap_or("60")
        ),
        Category::Orientacion => "En un libro: define la forma o las salidas.".into(),
        Category::Criatura => {
            let arch = entry.tag_value("arq").unwrap_or("");
            let name = &game.data.enemies.archetypes[game.data.archetype_index(arch)].name;
            format!("En un libro: convoca ({name}).")
        }
        Category::Material => "En un libro: deja recursos de este material.".into(),
        Category::Objeto => "En un libro: puede volverse un objeto.".into(),
        Category::Cualidad => entry
            .tag_value("mod")
            .and_then(|m| game.data.modifier_index(m))
            .map_or_else(
                || "En un libro: describe el lugar.".to_string(),
                |m| {
                    format!(
                        "Junto a una criatura la vuelve «{}».",
                        game.data.enemies.modifiers[m].name
                    )
                },
            ),
        Category::Accion => "Una acción: poder puro para tu equipo.".into(),
    };
    ui.paragraph(&world, 338, y, 132, DIM);
}

pub fn map(ui: &mut Ui, game: &Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(225);
    tabs(ui, Screen::Map, game.catalog_unlocked(), cmds);
    ui.card(rect(4, 22, 472, 234), SKY);
    let SiteKey::Walk { floor, q, r } = game.site.key else {
        ui.centered(
            "Los sitios del Catálogo no figuran en ningún mapa.",
            WIDTH / 2,
            120,
            DIM,
        );
        ui.centered("Volvé por el portal del centro.", WIDTH / 2, 136, FAINT);
        let back = ui.glyph("Esc", "B");
        ui.hints(&[(back, "cerrar")]);
        return;
    };
    let (cx, cy) = (WIDTH / 2, 140);
    let place = |dq: i32, dr: i32| (cx + dq * 24, cy + dr * 22 + dq * 11);
    let hexagon = |ui: &mut Ui, x: i32, y: i32, color: Rgba| {
        for row in 0..21 {
            let d: i32 = row - 10;
            let half = 14 - d.abs() / 2;
            ui.canvas.fill(x - half, y + d, half * 2 + 1, 1, color);
        }
    };
    let visible = |x: i32, y: i32| (22..458).contains(&x) && (40..230).contains(&y);
    for v in game
        .profile
        .world
        .visited
        .values()
        .filter(|v| v.floor == floor)
    {
        let (x, y) = place(v.q - q, v.r - r);
        if !visible(x, y) {
            continue;
        }
        let biome = &game.data.biomes[v.biome.min(game.data.biomes.len() - 1)];
        hexagon(
            ui,
            x,
            y,
            mix(hex(&biome.floor[0]), hex(&biome.wall_top), 0.35),
        );
        for d in Dir::ALL {
            if v.exits[d.index()] {
                let (dq, dr) = neighbor(0, 0, d);
                let (nx, ny) = place(v.q - q + dq, v.r - r + dr);
                ui.canvas.line(
                    (x * 2 + nx) / 3,
                    (y * 2 + ny) / 3,
                    (x + nx) / 2,
                    (y + ny) / 2,
                    INK,
                );
            }
        }
        let id = SiteKey::walk(floor, v.q, v.r).id();
        if v.stairs {
            ui.icon("icon_star", x - 4, y - 5);
        }
        if game.profile.world.lamps.iter().any(|l| l.key.id() == id) {
            ui.canvas.fill(x - 2, y + 4, 5, 4, rgb(140, 245, 255));
        }
        if floors::is_home(floor, v.q, v.r) {
            ui.centered("casa", x, y + 13, INK);
        }
        if game
            .profile
            .world
            .death_bag
            .as_ref()
            .is_some_and(|b| b.site.id() == id)
        {
            ui.centered("bolsa", x, y - 3, GOLD);
        }
    }
    let pulse = if (ui.time * 4.0).sin() > 0.0 {
        INK
    } else {
        GOLD
    };
    ui.canvas.ring(cx as f32, cy as f32, 7.0, 7.0, pulse);
    ui.canvas.fill(cx - 2, cy - 2, 5, 5, pulse);
    if game.derived.has(babel_core::data::Unlock::Compass) {
        let (sq, sr) = floors::nearest_stairs(game.profile.world_seed, floor, q, r);
        let (x, y) = place(sq - q, sr - r);
        if visible(x, y) {
            ui.canvas.ring(x as f32, y as f32, 12.0, 10.0, GOLD);
            ui.centered("escalera", x, y + 12, GOLD);
        }
    }
    ui.text(&format!("Piso {floor}"), 14, 30, GOLD);
    ui.right(
        &format!("{} despejados", game.profile.world.sites_cleared),
        WIDTH - 14,
        30,
        DIM,
    );
    // A small legend with the same marks the map uses.
    ui.icon("icon_star", 300, 238);
    ui.text("escalera", 312, 240, DIM);
    ui.canvas.fill(372, 240, 5, 4, rgb(140, 245, 255));
    ui.text("lámpara", 381, 240, DIM);
    if ui.button(rect(12, 236, 70, 14), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
}

pub fn catalog_screen(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(228);
    tabs(ui, Screen::Catalog, true, cmds);
    ui.heading("El Catálogo", 24);
    ui.centered(
        "Viajá al sitio cuya inscripción es exactamente tu frase.",
        WIDTH / 2,
        46,
        DIM,
    );
    let typed = type_into(ui, &mut data.text, 80, false);
    text_field(
        ui,
        rect(12, 60, 456, 22),
        &data.text,
        "escribí hasta 80 letras",
    );
    let unfolded = data.show_keys;
    if unfolded {
        keyboard(ui, &mut data.text, 88, 80, false);
    }
    // What the phrase will cost and which words it carries.
    let info = if unfolded { 150 } else { 98 };
    let phrase = catalog::sanitize(&data.text);
    if !phrase.is_empty() {
        let cost = catalog::index_cost(&game.data, &phrase);
        let have = game.profile.player.consumable(Consumable::Index);
        let text = format!(
            "{cost} Índices (tenés {have}) · dificultad de piso {}",
            game.catalog_difficulty()
        );
        let w = ui.font.width(&text) + 18;
        let x = (WIDTH - w) / 2;
        ui.icon("icon_index", x, info - 2);
        ui.text(&text, x + 18, info, if have >= cost { GOOD } else { BAD });
        let mut found = babel_core::babel::detect::Analysis::default();
        game.data.detector.analyze_line(
            &game.data.lexicon,
            &phrase,
            Place::Inscription,
            game.catalog_difficulty(),
            &mut found,
        );
        let mut x = 20;
        for h in found.hits.iter().take(9) {
            let word = &game.data.lexicon.get(h.word).word;
            let rarity = h.rarity.min(Rarity::Epic);
            x = ui.text(word, x, info + 14, rarity_rgba(rarity)) + 8;
        }
    }
    // Enter after typing travels instead of pressing an on-screen key.
    if typed {
        ui.focus_next_widget();
    }
    if ui.primary_button(rect(WIDTH / 2 - 108, 188, 106, 19), "Viajar") {
        match game.catalog_travel(&data.text) {
            Ok(()) => cmds.push(Cmd::CloseAll),
            Err(e) => data.message = e,
        }
    }
    if ui.button(rect(WIDTH / 2 + 2, 188, 106, 19), "Cerrar") {
        cmds.push(Cmd::CloseAll);
    }
    let keys_label = if unfolded {
        "Ocultar teclado"
    } else {
        "Teclado en pantalla"
    };
    if ui.button(rect(WIDTH / 2 - 62, 214, 124, 14), keys_label) {
        data.show_keys = !data.show_keys;
        // Past the five tabs: the first key, or "Viajar" once folded.
        ui.focus.index = 5;
    }
    if !data.message.is_empty() {
        let text = data.message.clone();
        ui.centered(&text, WIDTH / 2, 234, BAD);
    } else {
        ui.centered(
            "En sitios remotos ninguna palabra supera lo Épico.",
            WIDTH / 2,
            234,
            FAINT,
        );
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(ui.glyph("Tab", "R1"), "pestañas"), (back, "cerrar")]);
}
