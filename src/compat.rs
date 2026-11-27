//! Compatibility mode: no GPU renderer at all. `render::software` lights and
//! enlarges every frame on the processor and the basic drawing functions of
//! Windows put it in the window, so it needs no graphics driver.

use crate::debug::ScreenshotJob;
use crate::render::pipeline::{Frame, Placement, PreferredScale};
use crate::render::software::{Compositor, Picture};
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;
use std::ffi::c_void;
use std::time::{Duration, Instant};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// `BITMAPINFOHEADER`.
#[repr(C)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    image_size: u32,
    pixels_per_metre: [i32; 2],
    colours: [u32; 2],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetDC(window: isize) -> isize;
    fn ReleaseDC(window: isize, context: isize) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn SetDIBitsToDevice(
        context: isize,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        source_x: i32,
        source_y: i32,
        first_line: u32,
        lines: u32,
        bits: *const c_void,
        header: *const BitmapHeader,
        colour_use: u32,
    ) -> i32;
}

/// Copies a picture to the top-left corner of a window.
fn blit(window: isize, picture: &Picture) {
    if picture.pixels.len() < picture.w * picture.h || picture.pixels.is_empty() {
        return;
    }
    let header = BitmapHeader {
        size: size_of::<BitmapHeader>() as u32,
        width: picture.w as i32,
        // Negative: the first row is the top one.
        height: -(picture.h as i32),
        planes: 1,
        bit_count: 32,
        compression: 0,
        image_size: 0,
        pixels_per_metre: [0; 2],
        colours: [0; 2],
    };
    // SAFETY: the handle belongs to a live window of this thread; `pixels`
    // holds at least `w * h` 32-bit values, as the header describes, and both
    // outlive the call. The device context is released before returning.
    unsafe {
        let context = GetDC(window);
        if context == 0 {
            return;
        }
        SetDIBitsToDevice(
            context,
            0,
            0,
            picture.w as u32,
            picture.h as u32,
            0,
            0,
            0,
            picture.h as u32,
            picture.pixels.as_ptr().cast(),
            &header,
            0,
        );
        ReleaseDC(window, context);
    }
}

#[derive(Default)]
struct Software {
    compositor: Compositor,
    picture: Picture,
}

fn place(
    windows: Query<&Window, With<PrimaryWindow>>,
    preferred: Res<PreferredScale>,
    mut placement: ResMut<Placement>,
) {
    if let Ok(window) = windows.single() {
        *placement = Placement::fit(window.width(), window.height(), preferred.0);
    }
}

fn show(
    mut software: Local<Software>,
    frame: Res<Frame>,
    preferred: Res<PreferredScale>,
    primary: Query<Entity, With<PrimaryWindow>>,
    // Window functions only work on the main thread.
    _main_thread: NonSendMarker,
) {
    let Ok(entity) = primary.single() else {
        return;
    };
    WINIT_WINDOWS.with_borrow(|windows| {
        let Some(window) = windows.get_window(entity) else {
            return;
        };
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let size = window.inner_size();
        // A minimised window has no area.
        if size.width == 0 || size.height == 0 {
            return;
        }
        let Software {
            compositor,
            picture,
        } = &mut *software;
        picture.w = size.width as usize;
        picture.h = size.height as usize;
        let placement = Placement::fit(size.width as f32, size.height as f32, preferred.0);
        compositor.present(&frame, placement, picture);
        blit(handle.hwnd.get(), picture);
    });
}

/// `--screenshot` without a GPU: the same picture the window would show.
fn capture(
    job: Option<ResMut<ScreenshotJob>>,
    frame: Res<Frame>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut job) = job else {
        return;
    };
    job.frames += 1;
    if job.frames != 90 {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let mut picture = Picture {
        w: window.physical_width() as usize,
        h: window.physical_height() as usize,
        pixels: Vec::new(),
    };
    let placement = Placement::fit(picture.w as f32, picture.h as f32, 0);
    Compositor::default().present(&frame, placement, &mut picture);
    match crate::debug::save_picture(&picture, &job.path) {
        Ok(()) => exit.write(AppExit::Success),
        Err(e) => {
            error!("No se pudo guardar la captura: {e}");
            exit.write(AppExit::error())
        }
    };
}

/// With no screen refresh to wait for, the frame rate is capped by hand.
fn pace(mut last: Local<Option<Instant>>) {
    if let Some(spent) = last.map(|at| at.elapsed())
        && let Some(left) = FRAME.checked_sub(spent)
    {
        std::thread::sleep(left);
    }
    *last = Some(Instant::now());
}

/// Replaces `PipelinePlugin`.
pub struct CompatPlugin {
    /// The benchmark runs as fast as it can.
    pub limit_frames: bool,
}

impl Plugin for CompatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frame>()
            .init_resource::<PreferredScale>()
            .init_resource::<Placement>()
            .add_systems(PostUpdate, (place, show, capture).chain());
        if self.limit_frames {
            app.add_systems(Last, pace);
        }
    }
}
