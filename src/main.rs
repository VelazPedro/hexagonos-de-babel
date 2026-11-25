// Players get no console window; tool flags borrow the one they were typed in.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// Bevy systems take many resources and queries as parameters.
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

mod app;
mod audio;
mod debug;
mod input;
mod render;
mod settings;
mod shell;
#[cfg(test)]
mod shell_tests;
mod ui;

use babel_core::data::GameData;
use babel_core::world::site::SiteKey;
use bevy::app::AppExit;
use render::atlas::Atlas;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use ui::state::Screen;

const HELP: &str = "Hexágonos de Babel

  --atlas-dump [archivo.png]          Exporta el atlas de sprites y sale.
  --screenshot <seed> <floor> <q> <r> Renderiza un sitio a PNG y sale.
      --out <archivo.png>             Destino de la captura.
      --seconds <n>                   Segundos simulados antes de capturar.
      --screen <nombre>               Captura una pantalla: title, look, newgame, options,
                                      controls, pause, inventory, skills, glossary,
                                      map, catalog, reader, crafting, build, chest,
                                      lamps, scriptorium, gameover, ending, credits.
  --find <seed> <criterio>            Busca un sitio: elite, canon, dark, bright,
                                      guardian:<piso>, biome:<id>, layout:<tipo>.
  --bench [segundos]                  Prueba de rendimiento (40 enemigos, 64 luces).
  --mute                              Sin audio.
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

/// Without a console, a crash would be silent: leave its message in a file
/// next to the save.
fn record_crashes() {
    let log = settings::data_dir().join("error.log");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&log, format!("{info}\n"));
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
    }) {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
