//! Title, new game, options, remapping, pause, death, ending and credits.

use super::state::{Cmd, Screen, UiData, keyboard};
use super::{BAD, DIM, FAINT, GOLD, HEIGHT, INK, Ui, WIDTH, rect};
use crate::render::canvas::{rgb, with_alpha};
use crate::settings::{Action, Settings};
use babel_core::game::Game;

pub struct MenuContext<'a> {
    pub settings: &'a mut Settings,
    pub has_save: bool,
    /// Warning about a damaged or outdated save file.
    pub notice: Option<&'a str>,
}

fn big_button(ui: &mut Ui, y: i32, label: &str, enabled: bool) -> bool {
    ui.button_enabled(rect(WIDTH / 2 - 76, y, 152, 17), label, enabled)
}

pub fn title(ui: &mut Ui, ctx: &MenuContext, cmds: &mut Vec<Cmd>) {
    ui.dim_background(120);
    let bob = ((ui.time * 1.4).sin() * 2.0) as i32;
    ui.title("HEXÁGONOS", WIDTH / 2, 26 + bob, 4, GOLD);
    ui.title("DE BABEL", WIDTH / 2, 64 + bob, 3, rgb(120, 232, 220));
    ui.centered(
        "Las palabras de cada libro dan forma a su hexágono.",
        WIDTH / 2,
        98,
        DIM,
    );
    let mut y = 118;
    if ctx.has_save && big_button(ui, y, "Continuar", true) {
        cmds.push(Cmd::Continue);
    }
    if ctx.has_save {
        y += 22;
    }
    if big_button(ui, y, "Nueva partida", true) {
        cmds.push(Cmd::Push(Screen::NewGame));
    }
    if big_button(ui, y + 22, "Opciones", true) {
        cmds.push(Cmd::Push(Screen::Options));
    }
    if big_button(ui, y + 44, "Salir", true) {
        cmds.push(Cmd::Quit);
    }
    if let Some(notice) = ctx.notice {
        let lines = ui.font.wrap(notice, 420);
        let h = lines.len() as i32 * 10 + 8;
        ui.panel(rect(24, HEIGHT - h - 14, WIDTH - 48, h));
        for (i, line) in lines.iter().enumerate() {
            ui.centered(line, WIDTH / 2, HEIGHT - h - 10 + i as i32 * 10, BAD);
        }
    }
    ui.footer("Flechas o stick: moverse · Enter / A: elegir");
}

pub fn new_game(ui: &mut Ui, ctx: &MenuContext, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(170);
    ui.title("Nueva partida", WIDTH / 2, 14, 2, GOLD);
    ui.centered(
        "Semilla del mundo (opcional: vacía = al azar)",
        WIDTH / 2,
        40,
        DIM,
    );
    ui.panel(rect(WIDTH / 2 - 130, 52, 260, 18));
    let cursor = if (ui.time * 3.0).sin() > 0.0 {
        "_"
    } else {
        " "
    };
    let shown = format!("{}{cursor}", data.text);
    ui.centered(&shown, WIDTH / 2, 58, INK);
    keyboard(ui, &mut data.text, 80, 24, true);
    let label = if ctx.has_save && data.confirm_overwrite {
        "¿Borrar la partida anterior? Sí"
    } else {
        "Comenzar"
    };
    if ui.button(rect(WIDTH / 2 - 108, 176, 216, 17), label) {
        if ctx.has_save && !data.confirm_overwrite {
            data.confirm_overwrite = true;
        } else {
            let seed = (!data.text.trim().is_empty()).then(|| data.text.clone());
            cmds.push(Cmd::NewGame(seed));
        }
    }
    if ui.button(rect(WIDTH / 2 - 108, 198, 216, 15), "Volver") {
        cmds.push(Cmd::Pop);
    }
    if ctx.has_save {
        ui.centered(
            "Empezar de nuevo reemplaza la partida guardada.",
            WIDTH / 2,
            222,
            FAINT,
        );
    }
    ui.footer("Escribí o usá el teclado en pantalla · Esc: volver");
}

pub fn options(ui: &mut Ui, ctx: &mut MenuContext, cmds: &mut Vec<Cmd>) {
    ui.dim_background(190);
    ui.title("Opciones", WIDTH / 2, 16, 2, GOLD);
    let s = &mut *ctx.settings;
    let before = s.clone();
    let row = |i: i32| rect(WIDTH / 2 - 120, 46 + i * 20, 240, 16);
    let volume = |ui: &mut Ui, r, label: &str, value: &mut u8| {
        let change = ui.stepper(r, label, &format!("{}", *value));
        *value = (i32::from(*value) + change).clamp(0, 10) as u8;
    };
    volume(ui, row(0), "Volumen general", &mut s.master);
    volume(ui, row(1), "Música", &mut s.music);
    volume(ui, row(2), "Efectos", &mut s.effects);
    let scale = if s.pixel_scale == 0 {
        "Auto".to_string()
    } else {
        format!("x{}", s.pixel_scale)
    };
    let change = ui.stepper(row(3), "Escala de píxel", &scale);
    s.pixel_scale = (i32::from(s.pixel_scale) + change).rem_euclid(7) as u8;
    let on = |b: bool| if b { "Sí" } else { "No" };
    if ui.stepper(row(4), "Pantalla completa", on(s.fullscreen)) != 0 {
        s.fullscreen = !s.fullscreen;
    }
    if ui.stepper(row(5), "Sacudida de pantalla", on(s.screen_shake)) != 0 {
        s.screen_shake = !s.screen_shake;
    }
    if ui.button(row(6), "Remapear controles") {
        cmds.push(Cmd::Push(Screen::Controls));
    }
    if ui.button(row(8), "Volver") {
        cmds.push(Cmd::Pop);
    }
    if *s != before {
        cmds.push(Cmd::SettingsChanged);
    }
    ui.footer("Izquierda/derecha: cambiar · Esc / B: volver");
}

pub fn controls(ui: &mut Ui, ctx: &mut MenuContext, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(215);
    ui.title("Controles", WIDTH / 2, 6, 2, GOLD);
    if let Some(action) = data.remap {
        ui.panel(rect(90, 100, 300, 60));
        ui.centered(
            &format!("Nueva tecla para «{}»", action.label()),
            WIDTH / 2,
            112,
            INK,
        );
        ui.centered("Pulsá una tecla (Esc cancela)", WIDTH / 2, 130, DIM);
        if let Some(key) = &ui.input.raw_key {
            if key != "Escape" {
                ctx.settings.bind(action, key);
                cmds.push(Cmd::SettingsChanged);
            }
            data.remap = None;
        }
        return;
    }
    for (i, action) in Action::ALL.into_iter().enumerate() {
        let (col, line) = (i as i32 / 9, i as i32 % 9);
        let r = rect(24 + col * 220, 28 + line * 17, 212, 14);
        let response = ui.focusable(r);
        ui.cell(r, &response, None);
        ui.text(action.label(), r.x + 5, r.y + 4, INK);
        let key = ctx.settings.key(action).to_string();
        ui.right(&key, r.x + r.w - 5, r.y + 4, GOLD);
        if response.activated {
            data.remap = Some(action);
        }
    }
    if ui.button(rect(WIDTH / 2 - 150, 188, 140, 15), "Restaurar") {
        ctx.settings.keys.clear();
        cmds.push(Cmd::SettingsChanged);
    }
    if ui.button(rect(WIDTH / 2 + 10, 188, 140, 15), "Volver") {
        cmds.push(Cmd::Pop);
    }
    ui.centered(
        "Mouse: clic atacar, clic derecho lanzar, rueda herramientas.",
        WIDTH / 2,
        214,
        DIM,
    );
    ui.centered(
        "Gamepad: X atacar · A interactuar · B esquivar · Y leer · R1/L1 herramienta",
        WIDTH / 2,
        228,
        DIM,
    );
    ui.centered(
        "R2 lanzar · L2 tónico · ↑ lámpara · ↓ antorcha · ← inventario · → mapa",
        WIDTH / 2,
        240,
        DIM,
    );
}

pub fn pause(ui: &mut Ui, game: &Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(170);
    ui.title("Pausa", WIDTH / 2, 20, 2, GOLD);
    ui.centered(
        &format!(
            "Semilla {} · Piso {} · {}",
            game.profile.world_seed,
            game.site.depth,
            play_time(game.profile.world.play_seconds)
        ),
        WIDTH / 2,
        46,
        DIM,
    );
    let mut y = 62;
    let mut item = |ui: &mut Ui, label: &str| {
        let clicked = big_button(ui, y, label, true);
        y += 20;
        clicked
    };
    if item(ui, "Seguir jugando") {
        cmds.push(Cmd::CloseAll);
    }
    for (label, screen) in [
        ("Inventario", Screen::Inventory),
        ("Habilidades", Screen::Skills),
        ("Glosario", Screen::Glossary),
        ("Mapa", Screen::Map),
    ] {
        if item(ui, label) {
            cmds.push(Cmd::Replace(screen));
        }
    }
    if game.catalog_unlocked() && item(ui, "Catálogo") {
        cmds.push(Cmd::Replace(Screen::Catalog));
    }
    if item(ui, "Opciones") {
        cmds.push(Cmd::Push(Screen::Options));
    }
    if item(ui, "Guardar y salir al menú") {
        cmds.push(Cmd::Save);
        cmds.push(Cmd::ToTitle);
    }
    ui.footer("Esc / Start: volver al juego");
}

pub fn play_time(seconds: f64) -> String {
    let s = seconds as u64;
    format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

pub fn game_over(ui: &mut Ui, game: &Game, data: &UiData, cmds: &mut Vec<Cmd>) {
    let age = ui.time - data.opened_at;
    ui.dim_background((age * 220.0).min(220.0) as u8);
    ui.title(
        "Te perdiste entre los estantes",
        WIDTH / 2,
        76,
        2,
        rgb(220, 170, 255),
    );
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
    if age > 0.8 && big_button(ui, 154, "Despertar", true) {
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
    ui.canvas.fill(0, 0, WIDTH, HEIGHT, [16, 8, 30, 255]);
    let age = ui.time - data.opened_at;
    let page = data.ending_page.min(ENDING.len() - 1);
    ui.title("El Catálogo de catálogos", WIDTH / 2, 40, 2, GOLD);
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
    for i in 0..60 {
        let x = (i * 83 + (ui.time * 4.0) as i32 * (1 + i % 3)) % WIDTH;
        let y = HEIGHT - (((ui.time * (6.0 + (i % 5) as f32)) as i32 + i * 47) % HEIGHT);
        ui.canvas.put(x, y, with_alpha(GOLD, 90));
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
    ui.canvas.fill(0, 0, WIDTH, HEIGHT, [16, 8, 30, 255]);
    let age = ui.time - data.opened_at;
    let scroll = (age * 20.0).min(200.0) as i32;
    for (i, (line, highlight)) in CREDITS.iter().enumerate() {
        let y = HEIGHT - 20 + i as i32 * 14 - scroll;
        if (-10..HEIGHT).contains(&y) {
            ui.centered(line, WIDTH / 2, y, if *highlight { GOLD } else { INK });
        }
    }
    if age > 1.5
        && ui.button(
            rect(WIDTH / 2 - 60, HEIGHT - 30, 120, 15),
            "Volver al juego",
        )
    {
        cmds.push(Cmd::FinishCredits);
    }
}
