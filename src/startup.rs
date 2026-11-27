//! Starting the game so that it never just fails to appear. The program first
//! runs a copy of itself and watches it: if that copy dies or hangs before it
//! is running well with the normal graphics, another is started with safer
//! ones, ending in the compatibility mode that needs no graphics driver. If
//! even that fails, a message box says so and points to the log, which records
//! every step.
//!
//! A second process is the only way to survive every kind of failure,
//! including a graphics driver that takes the whole program down.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::time::{Duration, Instant};

/// Set on the watched copy: which graphics to use.
const GRAPHICS_VAR: &str = "HEXBABEL_GRAPHICS";
/// Set on the watched copy: the file to create once it is running well.
const READY_VAR: &str = "HEXBABEL_READY_FILE";
/// For automated tests: report on the console instead of in a message box.
const QUIET_VAR: &str = "HEXBABEL_NO_DIALOG";
/// Exit code of a copy that only found graphics emulated by the processor.
pub const SOFTWARE_ONLY: u8 = 86;
const LOG: &str = "arranque.log";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
    /// Whatever the system prefers.
    Gpu,
    /// DirectX only, for PCs with a broken Vulkan driver.
    GpuDx12,
    /// Everything on the processor.
    Compat,
}

impl Graphics {
    const ALL: [Graphics; 3] = [Graphics::Gpu, Graphics::GpuDx12, Graphics::Compat];

    fn name(self) -> &'static str {
        match self {
            Graphics::Gpu => "gpu",
            Graphics::GpuDx12 => "gpu-dx12",
            Graphics::Compat => "compat",
        }
    }

    /// How long a copy may take to be running well. The compatibility mode is
    /// the last resort, so a slow PC gets longer there.
    fn patience(self) -> Duration {
        match self {
            Graphics::Gpu | Graphics::GpuDx12 => Duration::from_secs(30),
            Graphics::Compat => Duration::from_secs(90),
        }
    }

    fn named(name: &str) -> Option<Graphics> {
        Graphics::ALL.into_iter().find(|g| g.name() == name)
    }

    /// What the watched copy was told to use. `None` when not watched.
    pub fn assigned() -> Option<Graphics> {
        Graphics::named(&std::env::var(GRAPHICS_VAR).ok()?)
    }
}

/// How a watched copy ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// The player closed the game.
    Closed,
    /// There is no real graphics processor.
    SoftwareOnly,
    /// It died before it was running well.
    NeverStarted,
    /// It died while being played.
    Crashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    Done,
    Try(Graphics),
    ReportCrash,
    GiveUp,
}

fn outcome(code: Option<i32>, was_ready: bool) -> Outcome {
    match code {
        Some(0) => Outcome::Closed,
        Some(c) if c == i32::from(SOFTWARE_ONLY) => Outcome::SoftwareOnly,
        _ if was_ready => Outcome::Crashed,
        _ => Outcome::NeverStarted,
    }
}

fn next(tried: Graphics, outcome: Outcome, has_compat: bool) -> Next {
    let compat = if has_compat {
        Next::Try(Graphics::Compat)
    } else {
        Next::GiveUp
    };
    match (outcome, tried) {
        (Outcome::Closed, _) => Next::Done,
        (Outcome::Crashed, _) => Next::ReportCrash,
        (_, Graphics::Compat) => Next::GiveUp,
        (Outcome::SoftwareOnly, _) => compat,
        (Outcome::NeverStarted, Graphics::Gpu) => Next::Try(Graphics::GpuDx12),
        (Outcome::NeverStarted, Graphics::GpuDx12) => compat,
    }
}

/// Adds a line to the log the player can send when something goes wrong.
pub fn log(data_dir: &Path, line: &str) {
    let _ = std::fs::create_dir_all(data_dir);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join(LOG));
    if let Ok(mut file) = file {
        let _ = writeln!(file, "{line}");
    }
}

/// Tells the watcher that this copy started well.
pub fn report_ready(data_dir: &Path) {
    if let Some(path) = std::env::var_os(READY_VAR) {
        log(data_dir, "  funcionando");
        let _ = std::fs::write(path, "ok");
    }
}

/// How a watched process ended.
#[derive(Debug, PartialEq, Eq)]
struct Ended {
    code: Option<i32>,
    was_ready: bool,
    /// It was stopped because it never got going.
    hung: bool,
}

/// Waits for a process to end. If it is not ready within `patience`, it is
/// stopped: a frozen driver would otherwise leave nothing on screen for ever.
fn watch(mut child: Child, ready: &Path, patience: Duration) -> std::io::Result<Ended> {
    let started = Instant::now();
    let mut was_ready = false;
    loop {
        was_ready = was_ready || ready.exists();
        if let Some(status) = child.try_wait()? {
            return Ok(Ended {
                code: status.code(),
                was_ready: was_ready || ready.exists(),
                hung: false,
            });
        }
        if !was_ready && started.elapsed() >= patience {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Ended {
                code: None,
                was_ready: false,
                hung: true,
            });
        }
        // Once the game runs there is nothing left to hurry for.
        std::thread::sleep(Duration::from_millis(if was_ready { 500 } else { 50 }));
    }
}

/// Shows a message box and returns whether the player answered "yes".
fn tell(text: &str, ask: bool) -> bool {
    if std::env::var_os(QUIET_VAR).is_some() || !cfg!(windows) {
        eprintln!("{text}");
        return false;
    }
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(owner: isize, text: *const u16, title: *const u16, kind: u32) -> i32;
        }
        const ERROR_ICON: u32 = 0x10;
        const YES_NO: u32 = 0x04;
        const YES: i32 = 6;
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, title) = (wide(text), wide("Hexágonos de Babel"));
        let kind = ERROR_ICON | if ask { YES_NO } else { 0 };
        // SAFETY: both strings end in zero and outlive the call.
        unsafe { MessageBoxW(0, text.as_ptr(), title.as_ptr(), kind) == YES }
    }
    #[cfg(not(windows))]
    {
        let _ = ask;
        false
    }
}

/// Runs the game in a watched copy of this program. Returns `None` if no copy
/// can be started, so that the caller runs the game itself.
pub fn supervise(data_dir: &Path, args: &[String], first: Graphics) -> Option<ExitCode> {
    let program = std::env::current_exe().ok()?;
    let ready: PathBuf = data_dir.join(format!("ready-{}", std::process::id()));
    let log_path = data_dir.join(LOG);
    let _ = std::fs::remove_file(&log_path);
    let has_compat = cfg!(windows);
    let version = env!("CARGO_PKG_VERSION");
    log(data_dir, &format!("Hexágonos de Babel {version}: inicio"));
    let mut graphics = first;
    loop {
        let _ = std::fs::remove_file(&ready);
        let mut command = Command::new(&program);
        command
            .args(args)
            .env(GRAPHICS_VAR, graphics.name())
            .env(READY_VAR, &ready);
        if graphics == Graphics::GpuDx12 {
            command.env("WGPU_BACKEND", "dx12");
        }
        log(data_dir, &format!("modo {}: abriendo", graphics.name()));
        let child = command.spawn().ok()?;
        let ended = watch(child, &ready, graphics.patience()).ok()?;
        let how = outcome(ended.code, ended.was_ready);
        let _ = std::fs::remove_file(&ready);
        let detail = match (ended.hung, ended.code) {
            (true, _) => format!(
                "no respondió en {} s y se detuvo",
                graphics.patience().as_secs()
            ),
            (false, Some(code)) => format!("terminó con código {code}"),
            (false, None) => "terminó sin código".to_string(),
        };
        let name = graphics.name();
        log(data_dir, &format!("modo {name}: {how:?}, {detail}"));
        match next(graphics, how, has_compat) {
            Next::Done => return Some(ExitCode::SUCCESS),
            Next::Try(safer) => graphics = safer,
            Next::ReportCrash => {
                let offer = has_compat && graphics != Graphics::Compat;
                let question = if offer {
                    "\n\n¿Querés volver a abrirlo en modo compatible?"
                } else {
                    ""
                };
                let text = format!(
                    "El juego se cerró por un error. La partida se guarda al cambiar de sala.\n\n\
                     Detalles en:\n{}{question}",
                    log_path.display()
                );
                if tell(&text, offer) {
                    graphics = Graphics::Compat;
                } else {
                    return Some(ExitCode::FAILURE);
                }
            }
            Next::GiveUp => {
                let text = format!(
                    "No se pudo abrir el juego en esta PC.\n\n\
                     Hace falta Windows 10 u 11 de 64 bits. Probá instalar los controladores \
                     de video desde Windows Update.\n\nDetalles en:\n{}",
                    log_path.display()
                );
                tell(&text, false);
                return Some(ExitCode::FAILURE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The graphics tried, in order, when every attempt ends the same way.
    fn ladder(how: Outcome) -> (Vec<Graphics>, Next) {
        let mut tried = vec![Graphics::Gpu];
        loop {
            let last = tried[tried.len() - 1];
            match next(last, how, true) {
                Next::Try(g) => tried.push(g),
                end => return (tried, end),
            }
        }
    }

    #[test]
    fn a_pc_that_cannot_start_ends_in_compatibility_mode_before_giving_up() {
        assert_eq!(
            ladder(Outcome::NeverStarted),
            (
                vec![Graphics::Gpu, Graphics::GpuDx12, Graphics::Compat],
                Next::GiveUp
            )
        );
    }

    #[test]
    fn without_a_graphics_processor_compatibility_mode_comes_next() {
        assert_eq!(
            next(Graphics::Gpu, Outcome::SoftwareOnly, true),
            Next::Try(Graphics::Compat)
        );
    }

    #[test]
    fn closing_the_game_ends_it_and_a_crash_while_playing_is_reported() {
        for g in Graphics::ALL {
            assert_eq!(next(g, Outcome::Closed, true), Next::Done);
            assert_eq!(next(g, Outcome::Crashed, true), Next::ReportCrash);
        }
    }

    #[test]
    fn where_there_is_no_compatibility_mode_it_is_not_tried() {
        assert_eq!(
            next(Graphics::GpuDx12, Outcome::NeverStarted, false),
            Next::GiveUp
        );
    }

    #[test]
    fn exit_codes_are_understood() {
        assert_eq!(outcome(Some(0), false), Outcome::Closed);
        assert_eq!(outcome(Some(86), false), Outcome::SoftwareOnly);
        assert_eq!(outcome(Some(101), false), Outcome::NeverStarted);
        assert_eq!(outcome(Some(101), true), Outcome::Crashed);
        assert_eq!(outcome(None, true), Outcome::Crashed);
    }

    /// A real process that runs for about `seconds`.
    fn sleeper(seconds: u32) -> Child {
        let mut command = if cfg!(windows) {
            let mut ping = Command::new("ping");
            ping.args(["-n", &(seconds + 1).to_string(), "127.0.0.1"]);
            ping
        } else {
            let mut sleep = Command::new("sleep");
            sleep.arg(seconds.to_string());
            sleep
        };
        command
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("a system program")
    }

    #[test]
    fn a_copy_that_hangs_before_being_ready_is_stopped() {
        let missing = std::env::temp_dir().join("hexbabel-never-ready");
        let started = Instant::now();
        let ended = watch(sleeper(30), &missing, Duration::from_millis(300)).expect("watch");
        assert!(ended.hung && !ended.was_ready);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(outcome(ended.code, ended.was_ready), Outcome::NeverStarted);
    }

    #[test]
    fn a_ready_copy_is_never_stopped_for_taking_long() {
        let ready = std::env::temp_dir().join(format!("hexbabel-ready-{}", std::process::id()));
        std::fs::write(&ready, "ok").expect("write");
        let ended = watch(sleeper(1), &ready, Duration::from_millis(100)).expect("watch");
        let _ = std::fs::remove_file(&ready);
        let expected = Ended {
            code: Some(0),
            was_ready: true,
            hung: false,
        };
        assert_eq!(ended, expected);
    }

    #[test]
    fn every_mode_survives_the_trip_through_the_environment() {
        for g in Graphics::ALL {
            assert_eq!(Graphics::named(g.name()), Some(g));
        }
    }
}
