//! Bevy wiring: plugins, states and the per-frame systems.

use crate::audio::{GameAudioPlugin, Mixer};
use crate::debug::{BenchJob, ScreenshotJob};
use crate::input::{self, InputContext, Snapshot};
use crate::render::pipeline::{Frame, PipelinePlugin, Placement, PreferredScale};
use crate::shell::{AppState, Shell};
use crate::startup::{self, Graphics};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::settings::WgpuSettings;
use bevy::window::{
    MonitorSelection, PresentMode, PrimaryWindow, WindowCloseRequested, WindowMode,
    WindowResolution,
};

pub struct Launch {
    pub shell: Shell,
    pub screenshot: Option<ScreenshotJob>,
    pub bench: Option<BenchJob>,
    pub mute: bool,
    pub graphics: Graphics,
}

/// Records which graphics were found. A watched copy that only finds graphics
/// emulated by the processor leaves at once: they are slow and can crash, and
/// the compatibility mode is better.
fn inspect_graphics(adapter: Option<Res<RenderAdapterInfo>>) {
    if Graphics::assigned().is_none() {
        return;
    }
    let data_dir = crate::settings::data_dir();
    let Some(adapter) = adapter else {
        startup::log(&data_dir, "  sin motor gráfico: dibuja el procesador");
        return;
    };
    let found = format!(
        "  placa: {} ({:?}, {:?}), controlador {} {}",
        adapter.name, adapter.backend, adapter.device_type, adapter.driver, adapter.driver_info
    );
    startup::log(&data_dir, &found);
    if format!("{:?}", adapter.device_type) == "Cpu" {
        // Nothing has been played yet, so there is nothing to save.
        std::process::exit(i32::from(startup::SOFTWARE_ONLY));
    }
}

/// After a few seconds of frames, the graphics are known to work.
fn report_running(mut frames: Local<u32>, mut reported: Local<bool>, time: Res<Time<Real>>) {
    *frames = frames.saturating_add(1);
    if !*reported && *frames >= 30 && time.elapsed_secs() >= 4.0 {
        *reported = true;
        startup::report_ready(&crate::settings::data_dir());
    }
}

fn run_shell(
    mut shell: ResMut<Shell>,
    snapshot: Res<Snapshot>,
    mut frame: ResMut<Frame>,
    mut mixer: ResMut<Mixer>,
    time: Res<Time>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    shell.frame(
        time.delta_secs().min(0.05),
        &snapshot,
        &mut frame,
        &mut mixer,
    );
    let mode = shell.mode();
    if *state.get() != mode {
        next.set(mode);
    }
    if shell.quit {
        exit.write(AppExit::Success);
    }
}

/// Feeds the input gatherer with what it needs from the shell and the window.
fn publish_context(shell: Res<Shell>, screen: Res<Placement>, mut context: ResMut<InputContext>) {
    if context.settings != shell.settings {
        context.settings = shell.settings.clone();
    }
    context.text_entry = shell.top().is_some_and(|s| s.takes_text());
    context.menu_open = !shell.screens.is_empty();
    context.scale = screen.scale;
    context.origin = screen.origin;
}

fn apply_window_settings(
    shell: Res<Shell>,
    mut scale: ResMut<PreferredScale>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut last: Local<Option<(bool, u8)>>,
) {
    let wanted = (shell.settings.fullscreen, shell.settings.pixel_scale);
    if *last == Some(wanted) {
        return;
    }
    let first = last.is_none();
    *last = Some(wanted);
    scale.0 = u32::from(wanted.1);
    let Ok(mut window) = window.single_mut() else {
        return;
    };
    if shell.ephemeral {
        return;
    }
    window.mode = if wanted.0 {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
    if !wanted.0 && wanted.1 > 0 && !first {
        let factor = f32::from(wanted.1);
        window.resolution.set(480.0 * factor, 270.0 * factor);
    }
}

fn measure(mut shell: ResMut<Shell>, diagnostics: Res<DiagnosticsStore>, entities: Query<Entity>) {
    if !shell.show_debug {
        return;
    }
    let value = |path: bevy::diagnostic::DiagnosticPath| {
        diagnostics
            .get(&path)
            .and_then(|d| d.smoothed())
            .unwrap_or(0.0) as f32
    };
    shell.stats.fps = value(FrameTimeDiagnosticsPlugin::FPS);
    shell.stats.frame_ms = value(FrameTimeDiagnosticsPlugin::FRAME_TIME);
    shell.stats.engine_entities = entities.iter().count();
}

/// Closing the window always saves first.
fn save_on_close(
    mut requests: MessageReader<WindowCloseRequested>,
    mut shell: ResMut<Shell>,
    mut exit: MessageWriter<AppExit>,
) {
    if requests.read().next().is_some() {
        shell.save();
        shell.flush_saves();
        exit.write(AppExit::Success);
    }
}

pub fn run(launch: Launch) -> AppExit {
    let tool = launch.screenshot.is_some();
    let bench = launch.bench.is_some();
    let (width, height) = if tool { (960, 540) } else { (1280, 720) };
    let mut resolution = WindowResolution::new(width, height);
    // One logical pixel is one physical pixel, so upscaling stays exact.
    resolution = resolution.with_scale_factor_override(1.0);
    let window = Window {
        title: "Hexágonos de Babel".into(),
        resolution,
        resizable: !tool,
        present_mode: if bench {
            PresentMode::AutoNoVsync
        } else {
            PresentMode::AutoVsync
        },
        ..default()
    };

    let compat = launch.graphics == Graphics::Compat;
    let mut plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(window),
            close_when_requested: false,
            ..default()
        })
        .set(ImagePlugin::default_nearest());
    if compat {
        // No GPU renderer: nothing here may touch a graphics driver.
        plugins = plugins.set(RenderPlugin {
            render_creation: WgpuSettings {
                backends: None,
                ..default()
            }
            .into(),
            ..default()
        });
    }

    let mut app = App::new();
    app.add_plugins(plugins)
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), GameAudioPlugin));
    #[cfg(windows)]
    if compat {
        app.add_plugins(crate::compat::CompatPlugin {
            limit_frames: !bench && !tool,
        });
    } else {
        app.add_plugins(PipelinePlugin);
    }
    #[cfg(not(windows))]
    app.add_plugins(PipelinePlugin);
    app.add_systems(Startup, inspect_graphics)
        .add_systems(Update, report_running)
        .init_state::<AppState>()
        .init_resource::<Snapshot>()
        .init_resource::<InputContext>()
        .insert_resource(ClearColor(Color::srgb_u8(26, 16, 40)))
        .insert_resource(launch.shell)
        .add_systems(
            Update,
            (
                publish_context,
                input::gather,
                crate::debug::drive_bench,
                run_shell,
                apply_window_settings,
                crate::window_icon::apply,
                measure,
                save_on_close,
            )
                .chain(),
        );
    // Without a GPU the compatibility mode takes the pictures itself.
    if !compat {
        app.add_systems(Update, crate::debug::drive_screenshot.after(save_on_close));
    }
    app.world_mut().resource_mut::<Mixer>().muted = launch.mute;
    if let Some(job) = launch.screenshot {
        app.insert_resource(job);
    }
    if let Some(job) = launch.bench {
        app.insert_resource(job);
    }
    app.run()
}
