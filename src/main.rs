// Players get no console window; tool flags borrow the one they were typed in.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// Bevy systems take many resources and queries as parameters.
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

mod app;
mod audio;
mod clipboard;
#[cfg(windows)]
mod compat;
mod debug;
mod input;
mod net;
mod render;
mod saver;
mod settings;
mod shell;
#[cfg(test)]
mod shell_tests;
mod startup;
mod ui;
mod window_icon;

use babel_core::data::GameData;
use babel_core::world::site::SiteKey;
use bevy::app::AppExit;
use render::atlas::Atlas;
use startup::Graphics;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use ui::state::Screen;

const HELP: &str = "Hexágonos de Babel

  --atlas-dump [archivo.png]          Exporta el atlas de sprites y sale.
  --icon-dump [carpeta]               Regenera el icono (por defecto assets/icon) y sale.
  --screenshot <seed> <floor> <q> <r> Renderiza un sitio a PNG y sale.
      --out <archivo.png>             Destino de la captura.
      --seconds <n>                   Segundos simulados antes de capturar.
      --screen <nombre>               Captura una pantalla: title, look, newgame, options,
                                      controls, pause, inventory, skills, glossary,
                                      map, catalog, reader, crafting, build, chest,
                                      lamps, scriptorium, stats, gameover, ending,
                                      credits.
  --find <seed> <criterio>            Busca un sitio: elite, canon, dark, bright,
                                      guardian:<piso>, biome:<id>, layout:<tipo>.
  --bench [segundos]                  Prueba de rendimiento (40 enemigos, 64 luces).
  --mute                              Sin audio.
  --compat                            Modo compatible: sin placa de video.
  --net-check                         Dice si una sala creada acá se alcanza desde internet.
  --coop-host <archivo>              Sala solo para esta PC; escribe el código (pruebas).
  --coop-join <código>                Entra a una sala (pruebas).
  --coop-direct <archivo>             Con las dos anteriores: prueba la conexión directa en
                                      esta PC; la respuesta viaja en <archivo>.reply.
  --help                              Esta ayuda.
";

/// A folder that replaces the built-in content while developing.
fn assets_override() -> Option<PathBuf> {
    std::env::var_os("HEXBABEL_ASSETS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Loading the game's own data is the one place allowed to stop the program.
#[allow(clippy::expect_used)]
fn load_content() -> (Arc<GameData>, Atlas) {
    let (data, atlas) = match assets_override() {
        Some(dir) => (GameData::load(&dir), Atlas::load(&dir)),
        None => (GameData::built_in(), Atlas::built_in()),
    };
    let data = data.expect("No se pudieron cargar los datos del juego");
    let atlas = atlas.expect("No se pudieron cargar los sprites del juego");
    (Arc::new(data), atlas)
}

/// Lets `--help`, `--bench` and friends print when started from a terminal.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_to_parent_console() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    const PARENT: u32 = u32::MAX;
    // SAFETY: takes no pointers; failing (no parent console) is harmless.
    unsafe {
        AttachConsole(PARENT);
    }
}

/// Without a console, a crash would be silent: add its message to the log
/// next to the save.
fn record_crashes() {
    let dir = settings::data_dir();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        startup::log(&dir, &info.to_string());
        previous(info);
    }));
}

fn screen_named(name: &str) -> Option<Screen> {
    Some(match name {
        "title" => Screen::Title,
        "look" => Screen::Look,
        "newgame" => Screen::NewGame,
        "options" => Screen::Options,
        "controls" => Screen::Controls,
        "pause" => Screen::Pause,
        "stats" => Screen::Stats,
        "inventory" => Screen::Inventory,
        "skills" => Screen::Skills,
        "glossary" => Screen::Glossary,
        "map" => Screen::Map,
        "catalog" => Screen::Catalog,
        "reader" => Screen::Reader,
        "crafting" => Screen::Crafting,
        "build" => Screen::Build(0),
        "chest" => Screen::Chest,
        "lamps" => Screen::Lamps,
        "scriptorium" => Screen::Scriptorium,
        "gameover" => Screen::GameOver,
        "ending" => Screen::Ending,
        "credits" => Screen::Credits,
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(all(windows, not(debug_assertions)))]
    if !args.is_empty() {
        attach_to_parent_console();
    }
    record_crashes();
    let flag = |name: &str| args.iter().position(|a| a == name);
    let value = |name: &str| flag(name).and_then(|i| args.get(i + 1)).cloned();
    if flag("--help").is_some() {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if flag("--net-check").is_some() {
        net::check();
        return ExitCode::SUCCESS;
    }
    // A player's launch is watched; developer tools run directly.
    let assigned = Graphics::assigned();
    let playing = args.iter().all(|a| a == "--compat" || a == "--mute");
    let preferred = if cfg!(windows) && flag("--compat").is_some() {
        Graphics::Compat
    } else {
        Graphics::Gpu
    };
    if assigned.is_none()
        && playing
        && let Some(code) = startup::supervise(&settings::data_dir(), &args, preferred)
    {
        return code;
    }
    let (data, atlas) = load_content();

    if flag("--atlas-dump").is_some() {
        let path = value("--atlas-dump")
            .filter(|v| !v.starts_with("--"))
            .unwrap_or_else(|| "atlas.png".into());
        return match debug::dump_atlas(&atlas, std::path::Path::new(&path)) {
            Ok(()) => {
                println!("Atlas guardado en {path}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("No se pudo guardar el atlas: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if flag("--icon-dump").is_some() {
        let dir = value("--icon-dump")
            .filter(|v| !v.starts_with("--"))
            .unwrap_or_else(|| "assets/icon".into());
        return match debug::dump_icon(&atlas, std::path::Path::new(&dir)) {
            Ok(()) => {
                println!("Icono guardado en {dir}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("No se pudo guardar el icono: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if let Some(i) = flag("--find") {
        let seed = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(1);
        let wanted = args.get(i + 2).cloned().unwrap_or_default();
        return match debug::find_site(&data, seed, &wanted) {
            Some((floor, q, r)) => {
                println!("{seed} {floor} {q} {r}");
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("No se encontró un sitio «{wanted}»");
                ExitCode::FAILURE
            }
        };
    }

    let mut shell = shell::Shell::new(data, atlas, settings::data_dir());
    let mut screenshot = None;
    if let Some(i) = flag("--screenshot") {
        let number = |k: usize| args.get(i + k).and_then(|s| s.parse::<i64>().ok());
        let seed = args.get(i + 1).and_then(|s| s.parse::<u64>().ok());
        let (Some(seed), Some(floor), Some(q), Some(r)) = (seed, number(2), number(3), number(4))
        else {
            eprintln!("Uso: --screenshot <seed> <floor> <q> <r>");
            return ExitCode::FAILURE;
        };
        let out =
            value("--out").unwrap_or_else(|| format!("screenshot_{seed}_{floor}_{q}_{r}.png"));
        let seconds = value("--seconds")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.2);
        let screen = value("--screen").and_then(|s| screen_named(&s));
        let key = SiteKey::walk(floor.max(1) as u32, q as i32, r as i32);
        debug::stage(&mut shell, seed, key, seconds, screen, true);
        screenshot = Some(debug::ScreenshotJob::new(PathBuf::from(out)));
    }
    // With `--coop-direct`, only the direct way works and the reply code
    // travels in a file next to the room code.
    let direct = value("--coop-direct");
    if let Some(path) = value("--coop-host") {
        let opened = if direct.is_some() {
            shell.open_direct_room()
        } else {
            shell.open_local_room()
        };
        let Some(code) = opened else {
            eprintln!("No se pudo abrir la sala");
            return ExitCode::FAILURE;
        };
        if let Err(e) = std::fs::write(&path, code) {
            eprintln!("No se pudo escribir el código: {e}");
            return ExitCode::FAILURE;
        }
        if direct.is_some() {
            debug::answer_from(&mut shell, PathBuf::from(format!("{path}.reply")));
        }
    }
    if let Some(invitation) = value("--coop-join") {
        match &direct {
            Some(path) => {
                shell.join_direct(&invitation);
                debug::leave_reply(&shell, PathBuf::from(format!("{path}.reply")));
            }
            None => shell.join_invitation(&invitation),
        }
    }
    let bench = flag("--bench").map(|_| {
        let seconds = value("--bench")
            .and_then(|s| s.parse().ok())
            .unwrap_or(12.0);
        debug::stage_bench(&mut shell);
        debug::BenchJob::new(seconds)
    });
    let mute = flag("--mute").is_some() || screenshot.is_some();

    match app::run(app::Launch {
        shell,
        screenshot,
        bench,
        mute,
        graphics: assigned.unwrap_or(preferred),
    }) {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
