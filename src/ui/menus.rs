//! Title, new game, options, remapping, pause, death, ending and credits.

use super::coop::CoopStatus;
use super::state::{Cmd, Screen, UiData, keyboard, text_field, type_into};
use super::theme::{Place, frames, place_for};
use super::{BAD, DIM, FAINT, GOLD, GOOD, HEIGHT, INK, SKY, Ui, VIOLET, WIDTH, rect};
use crate::render::atlas::Atlas;
use crate::render::canvas::Blit;
use crate::render::hero;
use crate::settings::{Action, Settings};
use babel_core::game::Game;

pub struct MenuContext<'a> {
    pub settings: &'a mut Settings,
    pub has_save: bool,
    /// Warning about a damaged or outdated save file.
    pub notice: Option<&'a str>,
}

fn menu_button(ui: &mut Ui, y: i32, to: Place, icon: &str, label: &str) -> bool {
    ui.with_place(to, |ui| {
        let r = rect(WIDTH / 2 - 72, y, 144, 19);
        let pressed = ui.icon_button(r, icon, label);
        let mark = ui.theme.accent;
        ui.stripe(r, mark);
        pressed
    })
}

fn notice_card(ui: &mut Ui, notice: &str, bottom: i32) {
    let lines = ui.font.wrap(notice, 420);
    let h = lines.len() as i32 * 10 + 10;
    ui.card(rect(24, bottom - h, WIDTH - 48, h), BAD);
    for (i, line) in lines.iter().enumerate() {
        ui.centered(line, WIDTH / 2, bottom - h + 6 + i as i32 * 10, BAD);
    }
}

pub fn title(ui: &mut Ui, ctx: &MenuContext, cmds: &mut Vec<Cmd>) {
    ui.dim_background(120);
    let bob = ((ui.time * 1.4).sin() * 2.0) as i32;
    ui.title("HEXÁGONOS", WIDTH / 2, 30 + bob, 4, GOLD);
    ui.title("DE BABEL", WIDTH / 2, 68 + bob, 3, SKY);
    // Gold rules at both sides of the second line.
    let theme = ui.theme;
    let (y, half) = (79 + bob, ui.font.width("DE BABEL") * 3 / 2 + 14);
    frames::rule(ui.canvas, &theme, 70, WIDTH / 2 - half, y, theme.accent);
    frames::rule(ui.canvas, &theme, WIDTH / 2 + half, 410, y, theme.accent);
    let mut y = 120;
    if ctx.has_save {
        if menu_button(ui, y, Place::Dawn, "ui_play", "Continuar") {
            cmds.push(Cmd::Continue);
        }
        y += 24;
    }
    if menu_button(ui, y, Place::Library, "ui_person", "Nueva partida") {
        cmds.push(Cmd::Push(Screen::Look));
    }
    if menu_button(ui, y + 24, Place::Rooms, "ui_people", "Multijugador") {
        cmds.push(Cmd::Push(Screen::Coop));
    }
    if menu_button(ui, y + 48, Place::Clockwork, "ui_gear", "Opciones") {
        cmds.push(Cmd::Push(Screen::Options));
    }
    if menu_button(ui, y + 72, Place::Ashes, "ui_exit", "Salir") {
        cmds.push(Cmd::Quit);
    }
    if let Some(notice) = ctx.notice {
        notice_card(ui, notice, HEIGHT - 16);
    }
    let confirm = ui.glyph("Enter", "A");
    ui.hints(&[("↑ ↓", "mover"), (confirm, "elegir")]);
}

pub fn new_game(
    ui: &mut Ui,
    ctx: &MenuContext,
    hero_atlas: &Atlas,
    data: &mut UiData,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(232);
    ui.heading("Tu mundo", 8);
    let typed = type_into(ui, &mut data.text, 24, true);
    let unfolded = data.show_keys;
    if !unfolded {
        let frame = hero_atlas.animate("player_walk_down", ui.time, 6.0);
        ui.canvas.sprite(
            hero_atlas,
            frame,
            WIDTH / 2 - frame.w * 3 / 2,
            34,
            Blit {
                scale: 3.0,
                ..Default::default()
            },
        );
    }
    let field_y = if unfolded { 40 } else { 130 };
    text_field(
        ui,
        rect(WIDTH / 2 - 130, field_y, 260, 20),
        &data.text,
        "Semilla al azar",
    );
    if unfolded {
        keyboard(ui, &mut data.text, 68, 24, true);
    } else {
        ui.centered(
            "Escribí un número o una palabra para repetir un mundo.",
            WIDTH / 2,
            field_y + 25,
            DIM,
        );
    }
    // Enter after typing submits instead of pressing an on-screen key.
    if typed {
        ui.focus_next_widget();
    }
    let top = if unfolded { 152 } else { 170 };
    let label = if ctx.has_save && data.confirm_overwrite {
        "Sí, borrar la partida anterior"
    } else {
        "Comenzar aventura"
    };
    if ui.primary_button(rect(WIDTH / 2 - 90, top, 180, 19), label) {
        if ctx.has_save && !data.confirm_overwrite {
            data.confirm_overwrite = true;
        } else {
            let seed = (!data.text.trim().is_empty()).then(|| data.text.clone());
            cmds.push(Cmd::NewGame(seed));
        }
    }
    let keys_label = if unfolded {
        "Ocultar teclado"
    } else {
        "Teclado en pantalla"
    };
    if ui.button(rect(WIDTH / 2 - 90, top + 25, 112, 15), keys_label) {
        data.show_keys = !data.show_keys;
        ui.focus.index = 0;
    }
    if ui.button(rect(WIDTH / 2 + 28, top + 25, 62, 15), "Volver") {
        cmds.push(Cmd::Pop);
    }
    if ctx.has_save {
        let warning = if data.confirm_overwrite {
            ("Esto no se puede deshacer.", BAD)
        } else {
            ("Reemplaza la partida guardada.", FAINT)
        };
        ui.centered(warning.0, WIDTH / 2, top + 47, warning.1);
    }
    if let Some(notice) = ctx.notice {
        notice_card(ui, notice, HEIGHT - 14);
    }
    let (confirm, back) = (ui.glyph("Enter", "A"), ui.glyph("Esc", "B"));
    ui.hints(&[(confirm, "elegir"), (back, "volver")]);
}

/// What each row of the options screen explains while it is focused.
const OPTION_HELP: [&str; 9] = [
    "Volumen de todo el juego.",
    "Volumen de la música de cada galería.",
    "Volumen de golpes, objetos y menús.",
    "Tamaño de los píxeles. Auto usa el mayor que entre en la ventana.",
    "Ocupar toda la pantalla.",
    "La cámara tiembla con los golpes fuertes.",
    "Los nombres, en letra tenue sobre cada cabeza.",
    "Elegir tus propias teclas.",
    "Volver atrás. Los cambios ya quedaron guardados.",
];

pub fn options(ui: &mut Ui, ctx: &mut MenuContext, cmds: &mut Vec<Cmd>) {
    ui.dim_background(205);
    ui.heading("Opciones", 8);
    let s = &mut *ctx.settings;
    let before = s.clone();
    let x = WIDTH / 2 - 130;
    let row = |y: i32| rect(x, y, 260, 18);
    ui.card(rect(x - 8, 32, 276, 74), SKY);
    ui.text("Sonido", x, 38, SKY);
    s.master = ui.slider(row(49), "General", s.master);
    s.music = ui.slider(row(68), "Música", s.music);
    s.effects = ui.slider(row(87), "Efectos", s.effects);

    ui.card(rect(x - 8, 110, 276, 93), VIOLET);
    ui.text("Pantalla", x, 116, SKY);
    let scale = if s.pixel_scale == 0 {
        "Auto".to_string()
    } else {
        format!("x{}", s.pixel_scale)
    };
    let change = ui.selector(row(127), "Escala de píxel", &scale, None);
    s.pixel_scale = (i32::from(s.pixel_scale) + change).rem_euclid(7) as u8;
    let on = |b: bool| if b { "Sí" } else { "No" };
    if ui.selector(row(146), "Pantalla completa", on(s.fullscreen), None) != 0 {
        s.fullscreen = !s.fullscreen;
    }
    if ui.selector(row(165), "Sacudida", on(s.screen_shake), None) != 0 {
        s.screen_shake = !s.screen_shake;
    }
    if ui.selector(row(184), "Nombres en pantalla", on(s.show_names), None) != 0 {
        s.show_names = !s.show_names;
    }
    if ui.button(rect(x, 209, 126, 17), "Controles") {
        cmds.push(Cmd::Push(Screen::Controls));
    }
    if ui.button(rect(x + 134, 209, 126, 17), "Volver") {
        cmds.push(Cmd::Pop);
    }
    if *s != before {
        cmds.push(Cmd::SettingsChanged);
    }
    // Only the focused option explains itself.
    if let Some(help) = OPTION_HELP.get(ui.focus.index) {
        ui.centered(help, WIDTH / 2, 234, INK);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[("← →", "cambiar"), (back, "volver")]);
}

const CONTROL_TABS: [&str; 3] = ["En el juego", "Atajos", "Gamepad"];

/// Actions listed on the first tab; the rest go on the second.
const PLAY_ACTIONS: usize = 9;

const PAD_HELP: [(&str, &str); 12] = [
    ("X", "Atacar y recolectar"),
    ("A", "Interactuar"),
    ("B", "Esquivar"),
    ("Y", "Leer el libro"),
    ("R1", "Herramienta siguiente"),
    ("L1", "Herramienta anterior"),
    ("R2", "Lanzar páginas"),
    ("L2", "Beber tónico"),
    ("↑", "Lámpara de retorno"),
    ("↓", "Plantar antorcha"),
    ("←", "Inventario"),
    ("→", "Mapa"),
];

pub fn controls(ui: &mut Ui, ctx: &mut MenuContext, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    ui.heading("Controles", 6);
    if let Some(action) = data.remap {
        ui.card(rect(90, 100, 300, 62), GOLD);
        ui.centered(
            &format!("Nueva tecla para «{}»", action.label()),
            WIDTH / 2,
            114,
            INK,
        );
        ui.centered("Pulsá una tecla (Esc cancela)", WIDTH / 2, 134, DIM);
        if let Some(key) = &ui.input.raw_key {
            if key != "Escape" {
                ctx.settings.bind(action, key);
                cmds.push(Cmd::SettingsChanged);
            }
            data.remap = None;
        } else if ui.input.back || ui.input.right_click {
            data.remap = None;
        }
        return;
    }
    for (i, label) in CONTROL_TABS.into_iter().enumerate() {
        let r = rect(WIDTH / 2 - 150 + i as i32 * 101, 28, 98, 15);
        let active = data.controls_tab == i;
        if ui.button(r, label) {
            data.controls_tab = i;
        }
        if active {
            ui.canvas.fill(r.x + 4, r.y + r.h - 2, r.w - 8, 2, GOLD);
        }
    }
    if ui.input.tab != 0 {
        data.controls_tab = (data.controls_tab as i32 + ui.input.tab).rem_euclid(3) as usize;
    }
    if data.controls_tab < 2 {
        let range = if data.controls_tab == 0 {
            0..PLAY_ACTIONS
        } else {
            PLAY_ACTIONS..Action::ALL.len()
        };
        for (row, action) in Action::ALL[range].iter().copied().enumerate() {
            let r = rect(WIDTH / 2 - 130, 49 + row as i32 * 17, 260, 15);
            let response = ui.focusable(r);
            ui.cell(r, &response, None);
            let color = if response.focused { INK } else { DIM };
            ui.text(action.label(), r.x + 8, r.y + 4, color);
            let key = ctx.settings.key(action).to_string();
            let w = super::keycap_width(ui.font, &key);
            ui.keycap(&key, r.x + r.w - 8 - w, r.y + 4);
            if response.activated {
                data.remap = Some(action);
            }
        }
    } else {
        ui.card(rect(40, 52, 400, 144), SKY);
        for (i, (button, what)) in PAD_HELP.into_iter().enumerate() {
            let (x, y) = (60 + (i as i32 / 6) * 200, 66 + (i as i32 % 6) * 19);
            ui.keycap(button, x, y);
            ui.text(what, x + 24, y, INK);
        }
        ui.centered(
            "Stick izquierdo: moverse · stick derecho: apuntar",
            WIDTH / 2,
            182,
            DIM,
        );
    }
    if ui.button(rect(WIDTH / 2 - 130, 210, 126, 15), "Restaurar teclas") {
        ctx.settings.keys.clear();
        cmds.push(Cmd::SettingsChanged);
    }
    if ui.button(rect(WIDTH / 2 + 4, 210, 126, 15), "Volver") {
        cmds.push(Cmd::Pop);
    }
    if data.controls_tab < 2 {
        ui.centered(
            "Mouse: clic ataca, clic derecho lanza, rueda cambia herramienta.",
            WIDTH / 2,
            232,
            FAINT,
        );
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(ui.glyph("Tab", "R1"), "pestaña"), (back, "volver")]);
}

pub fn pause(
    ui: &mut Ui,
    game: &Game,
    hero_atlas: &Atlas,
    coop: Option<CoopStatus>,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(180);
    ui.heading("Pausa", 14);
    // With a partner, say whether the world is really stopped.
    let (line, color) = match coop {
        None => (
            format!(
                "Piso {} · {}",
                game.site.depth,
                play_time(game.profile.world.play_seconds)
            ),
            DIM,
        ),
        Some(c) if c.stopped() => ("Juego detenido: los dos en pausa (2/2)".into(), GOOD),
        Some(_) => (
            "El juego sigue hasta que tu compañero pause (1/2)".into(),
            BAD,
        ),
    };
    ui.centered(&line, WIDTH / 2, 40, color);
    if ui.primary_button(rect(WIDTH / 2 - 80, 58, 160, 20), "Seguir jugando") {
        cmds.push(Cmd::CloseAll);
    }
    let points = game.profile.player.skill_points;
    let badge = (points > 0).then(|| points.to_string());
    let mut entries = vec![
        ("ui_bag", "Inventario", Cmd::Replace(Screen::Inventory)),
        ("ui_tree", "Habilidades", Cmd::Replace(Screen::Skills)),
        ("ui_book", "Glosario", Cmd::Replace(Screen::Glossary)),
        ("ui_map", "Mapa", Cmd::Replace(Screen::Map)),
    ];
    if game.catalog_unlocked() {
        entries.push(("icon_index", "Catálogo", Cmd::Replace(Screen::Catalog)));
    }
    entries.push(("ui_person", "Personaje", Cmd::Push(Screen::Look)));
    entries.push(("ui_people", "De a dos", Cmd::Push(Screen::Coop)));
    entries.push(("ui_gear", "Opciones", Cmd::Push(Screen::Options)));
    entries.push(("ui_stats", "Estadísticas", Cmd::Push(Screen::Stats)));
    entries.push(("ui_exit", "Salir", Cmd::ToTitle));
    const PER_ROW: usize = 5;
    for (i, (icon, label, command)) in entries.into_iter().enumerate() {
        let (col, row) = ((i % PER_ROW) as i32, (i / PER_ROW) as i32);
        let r = rect(WIDTH / 2 - 188 + col * 76, 94 + row * 58, 72, 52);
        let badge = badge.as_deref().filter(|_| icon == "ui_tree");
        let pressed = ui.with_place(leads_to(&command), |ui| {
            let pressed = ui.tile(r, icon, label, badge);
            let mark = ui.theme.accent;
            ui.canvas.fill(r.x + 4, r.y + 3, r.w - 8, 2, mark);
            pressed
        });
        if pressed {
            if command == Cmd::ToTitle {
                cmds.push(Cmd::Save);
            }
            cmds.push(command);
        }
    }
    // A portrait and the world's seed, for sharing.
    ui.card(rect(WIDTH / 2 - 130, 220, 260, 24), SKY);
    hero::portrait(ui.canvas, hero_atlas, WIDTH / 2 - 123, 223, 1.0);
    let level = format!("Nivel {}", game.profile.player.level);
    let who = match game.profile.player.name.as_str() {
        "" => level,
        name => format!("{name} · {level}"),
    };
    ui.text(&who, WIDTH / 2 - 96, 229, GOLD);
    ui.right(
        &format!("Semilla {}", game.profile.world_seed),
        WIDTH / 2 + 122,
        229,
        DIM,
    );
    let back = ui.glyph("Esc", "Start");
    ui.hints(&[(back, "volver al juego")]);
}

/// The look of wherever a button takes the player.
fn leads_to(command: &Cmd) -> Place {
    match command {
        Cmd::Push(screen) | Cmd::Replace(screen) => match screen {
            Screen::Look => Place::Library,
            screen => place_for(*screen, None),
        },
        Cmd::ToTitle => Place::Ashes,
        _ => Place::Library,
    }
}

pub fn play_time(seconds: f64) -> String {
    let s = seconds as u64;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

pub fn game_over(ui: &mut Ui, game: &Game, data: &UiData, cmds: &mut Vec<Cmd>) {
    let age = ui.time - data.opened_at;
    ui.dim_background((age * 220.0).min(220.0) as u8);
    let accent = ui.theme.accent;
    ui.title("Te perdiste entre los estantes", WIDTH / 2, 76, 2, accent);
    let theme = ui.theme;
    frames::rule(ui.canvas, &theme, 110, 370, 99, accent);
    let place = game
        .profile
        .world
        .respawn
        .as_ref()
        .map_or("tu hexágono".to_string(), |l| l.label.clone());
    ui.centered(&format!("Despertarás en: {place}"), WIDTH / 2, 112, INK);
    if game.profile.world.death_bag.is_some() {
        ui.centered(
            "Una bolsa con el 20% de tus materiales quedó donde caíste.",
            WIDTH / 2,
            128,
            DIM,
        );
    }
    // A short delay so a mashed button does not skip the screen.
    if age > 0.8 && ui.primary_button(rect(WIDTH / 2 - 76, 154, 152, 19), "Despertar") {
        cmds.push(Cmd::Respawn);
    }
}

const ENDING: [&str; 5] = [
    "El último índice cae abierto sobre el suelo. En sus páginas están los nombres de todos los catálogos, y al final, en una letra que reconocés, el tuyo.",
    "Entendés que la Biblioteca nunca escondió un libro verdadero. Escondía lectores. Cada hexágono esperaba a alguien que mirara el ruido hasta encontrar una palabra.",
    "Las lámparas que encendiste siguen ardiendo, piso sobre piso, como un renglón escrito de arriba abajo.",
    "Podrías cerrar el libro. En cambio, mojás la pluma y agregás una línea que no estaba en ninguna combinación posible: «aquí hubo alguien que leyó».",
    "La escalera, que terminaba aquí, ahora sigue.",
];

pub fn ending(ui: &mut Ui, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(255);
    let age = ui.time - data.opened_at;
    let page = data.ending_page.min(ENDING.len() - 1);
    ui.heading("El Catálogo de catálogos", 40);
    let reveal = (age * 55.0) as usize;
    let lines = ui.font.wrap(ENDING[page], 340);
    let mut shown = 0;
    for (i, line) in lines.iter().enumerate() {
        let take = reveal.saturating_sub(shown).min(line.chars().count());
        let part: String = line.chars().take(take).collect();
        let x = (WIDTH - ui.font.width(line)) / 2;
        ui.text(&part, x, 100 + i as i32 * 12, INK);
        shown += line.chars().count();
    }
    let label = if page + 1 == ENDING.len() {
        "Créditos"
    } else {
        "Seguir"
    };
    if ui.button(rect(WIDTH / 2 - 50, 214, 100, 15), label) {
        if reveal < shown {
            data.opened_at -= 100.0;
        } else if page + 1 < ENDING.len() {
            data.ending_page += 1;
            data.opened_at = ui.time;
        } else {
            cmds.push(Cmd::Replace(Screen::Credits));
        }
    }
}

const CREDITS: [(&str, bool); 17] = [
    ("HEXÁGONOS DE BABEL", true),
    ("", false),
    ("Diseño, código, arte, léxico y sonido", false),
    ("generados para este proyecto", false),
    ("", false),
    ("Hecho en Rust con el motor Bevy", false),
    ("", false),
    ("Inspiración literaria", true),
    ("La idea de una biblioteca de galerías hexagonales", false),
    ("viene de un cuento de Jorge Luis Borges.", false),
    ("Todas las frases y textos del juego son originales.", false),
    ("", false),
    ("Gracias por leer.", true),
    ("", false),
    ("Se abrió el Modo infinito:", false),
    ("la escalera del piso 30 sigue bajando.", false),
    ("", false),
];

pub fn credits(ui: &mut Ui, data: &UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(255);
    let age = ui.time - data.opened_at;
    let scroll = (age * 20.0).min(200.0) as i32;
    for (i, (line, highlight)) in CREDITS.iter().enumerate() {
        let y = HEIGHT - 20 + i as i32 * 14 - scroll;
        if (-10..HEIGHT).contains(&y) {
            ui.centered(line, WIDTH / 2, y, if *highlight { GOLD } else { INK });
        }
    }
    // The lines pass behind a plain band where the button waits.
    let (band, edge) = (ui.theme.panel, ui.theme.accent);
    ui.canvas.fill(0, HEIGHT - 36, WIDTH, 36, band);
    ui.canvas.fill(0, HEIGHT - 36, WIDTH, 1, edge);
    if age > 1.5
        && ui.button(
            rect(WIDTH / 2 - 60, HEIGHT - 30, 120, 15),
            "Volver al juego",
        )
    {
        cmds.push(Cmd::FinishCredits);
    }
}
