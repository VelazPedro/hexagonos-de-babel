//! Gives the window the same icon as the executable. Bevy has no setting for
//! it, so this talks to the window library underneath.

use crate::shell::Shell;
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;

/// Sets the icon as soon as the window exists.
pub fn apply(
    mut done: Local<bool>,
    primary: Query<Entity, With<PrimaryWindow>>,
    shell: Res<Shell>,
    // Window functions only work on the main thread.
    _main_thread: NonSendMarker,
) {
    if *done {
        return;
    }
    let Ok(entity) = primary.single() else {
        return;
    };
    WINIT_WINDOWS.with_borrow(|windows| {
        if let Some(window) = windows.get_window(entity) {
            set(window, &shell);
            *done = true;
        }
    });
}

/// Windows draws a small icon in the title bar and a large one in the taskbar.
/// Both come from the executable, at the sizes this display scale calls for,
/// so that neither is a blurred resize.
#[cfg(windows)]
fn set(window: &winit::window::Window, _shell: &Shell) {
    use winit::dpi::PhysicalSize;
    use winit::platform::windows::{IconExtWindows, WindowExtWindows};
    use winit::window::Icon;
    /// Number of the icon group written by `build.rs`.
    const GROUP_ID: u16 = 1;
    let load = |points: f64| {
        let side = (points * window.scale_factor()).round() as u32;
        // Builds without the resource keep the system's default icon.
        Icon::from_resource(GROUP_ID, Some(PhysicalSize::new(side, side))).ok()
    };
    window.set_window_icon(load(16.0));
    window.set_taskbar_icon(load(32.0));
}

/// Elsewhere there is no executable resource, so the art is drawn on the spot.
#[cfg(not(windows))]
fn set(window: &winit::window::Window, shell: &Shell) {
    let art = crate::render::icon::art(&shell.atlas, 64);
    let icon = winit::window::Icon::from_rgba(art.px, 64, 64).ok();
    window.set_window_icon(icon);
}
