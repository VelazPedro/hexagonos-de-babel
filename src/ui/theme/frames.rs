//! Frames, options and the other framed pieces of each place.

use super::{FrameStyle, Theme};
use crate::render::canvas::{Canvas, Rgba, mix, with_alpha};

/// Panels smaller than this get a plain frame, which leaves their inside
/// clear.
const PLAIN_HEIGHT: i32 = 30;
const PLAIN_WIDTH: i32 = 60;
/// Room kept for the shadow.
const SHADOW: i32 = 3;

/// A number that looks random but is always the same for a place.
pub fn noise(x: i32, y: i32) -> u32 {
    let mut n = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    n ^= n >> 13;
    n = n.wrapping_mul(0xC2B2_AE35);
    n ^ (n >> 16)
}

pub fn panel(theme: &Theme, w: i32, h: i32) -> Canvas {
    let mut canvas = Canvas::new(w + SHADOW, h + SHADOW);
    canvas.fill(SHADOW, SHADOW, w, h, [0, 0, 0, 120]);
    canvas.fill(0, 0, w, h, theme.panel);
    if h < PLAIN_HEIGHT || w < PLAIN_WIDTH {
        plain(&mut canvas, theme, w, h);
        return canvas;
    }
    grain(&mut canvas, theme, w, h);
    canvas.frame(0, 0, w, h, theme.frame_dark);
    canvas.frame(1, 1, w - 2, h - 2, theme.frame);
    match theme.frame_style {
        FrameStyle::Riveted => riveted(&mut canvas, theme, w, h),
        FrameStyle::Stitched => stitched(&mut canvas, theme, w, h),
        FrameStyle::Pipes => piped(&mut canvas, theme, w, h),
        FrameStyle::Parchment => parchment(&mut canvas, theme, w, h),
        FrameStyle::Chalk => chalked(&mut canvas, theme, w, h),
        FrameStyle::Banded => banded(&mut canvas, theme, w, h),
        FrameStyle::Lantern => lamplit(&mut canvas, theme, w, h),
        FrameStyle::Strapped => strapped(&mut canvas, theme, w, h),
        FrameStyle::Starry => starry(&mut canvas, theme, w, h),
        FrameStyle::Gilded => gilded(&mut canvas, theme, w, h),
        FrameStyle::Charted => charted(&mut canvas, theme, w, h),
        FrameStyle::Hexed => hexed(&mut canvas, theme, w, h),
        FrameStyle::Filigree => curled(&mut canvas, theme, w, h),
        FrameStyle::Links => linked(&mut canvas, theme, w, h),
        FrameStyle::Gears => geared(&mut canvas, theme, w, h),
        FrameStyle::Covers => bound(&mut canvas, theme, w, h),
        FrameStyle::Cracked => cracked(&mut canvas, theme, w, h),
        FrameStyle::Rays => radiant(&mut canvas, theme, w, h),
        FrameStyle::Ledger => ruled(&mut canvas, theme, w, h),
        FrameStyle::Slim => canvas.fill(1, 1, w - 2, 1, theme.frame_light),
    }
    canvas
}

fn plain(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(0, 0, w, h, theme.frame_dark);
    canvas.frame(1, 1, w - 2, h - 2, theme.frame);
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
}

fn grain(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    let hammered = theme.frame_style == FrameStyle::Riveted;
    for y in 3..h - 3 {
        for x in 3..w - 3 {
            let n = noise(x, y) % 100;
            // Hammer marks come in little dashes.
            let mark = if hammered {
                noise(x / 3, y) % 100 < 4
            } else {
                n < 8
            };
            if mark {
                canvas.put(x, y, theme.grain_dark);
            } else if (hammered && n < 3) || (!hammered && n > 94) {
                canvas.put(x, y, theme.grain_light);
            }
        }
    }
}

fn rivet(canvas: &mut Canvas, theme: &Theme, x: i32, y: i32) {
    canvas.fill(x - 2, y - 1, 5, 3, theme.frame_dark);
    canvas.fill(x - 1, y - 2, 3, 5, theme.frame_dark);
    canvas.fill(x - 1, y - 1, 3, 3, theme.frame);
    canvas.put(x - 1, y - 1, theme.frame_light);
    canvas.put(x, y - 1, theme.frame_light);
    canvas.put(x - 1, y, theme.frame_light);
}

/// Iron lit from above, with the heat showing along the bottom.
fn riveted(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(2, 2, w - 4, h - 4, theme.frame_dark);
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.fill(1, 2, 1, h - 4, theme.frame_light);
    for k in 0..14 {
        let alpha = (45 - k * 3).max(0) as u8;
        canvas.fill(3, h - 4 - k, w - 6, 1, [255, 100, 20, alpha]);
    }
    for (x, y) in [
        (3, 3),
        (w - 4, 3),
        (3, h - 4),
        (w - 4, h - 4),
        (w / 2, 2),
        (w / 2, h - 3),
    ] {
        rivet(canvas, theme, x, y);
    }
}

/// A brass protector over one corner; `sx` and `sy` point inwards.
fn protector(canvas: &mut Canvas, theme: &Theme, x: i32, y: i32, sx: i32, sy: i32) {
    const SIZE: i32 = 9;
    for dy in 0..SIZE {
        for dx in 0..SIZE - dy {
            let color = if dx + dy == SIZE - 1 {
                theme.frame_dark
            } else if dx == 0 || dy == 0 {
                [255, 232, 160, 255]
            } else {
                [206, 156, 70, 255]
            };
            canvas.put(x + dx * sx, y + dy * sy, color);
        }
    }
    canvas.put(x + 2 * sx, y + 2 * sy, [120, 80, 30, 255]);
}

fn protectors(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    for (x, y, sx, sy) in [
        (0, 0, 1, 1),
        (w - 1, 0, -1, 1),
        (0, h - 1, 1, -1),
        (w - 1, h - 1, -1, -1),
    ] {
        protector(canvas, theme, x, y, sx, sy);
    }
}

/// Stitched leather with brass at the corners.
fn stitched(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(2, 2, w - 4, h - 4, theme.frame);
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    let thread = [236, 212, 160, 230];
    let hole = with_alpha(theme.frame_dark, 200);
    let mut x = 10;
    while x < w - 13 {
        for y in [4, h - 5] {
            canvas.fill(x, y, 3, 1, thread);
            canvas.fill(x, y + 1, 3, 1, hole);
        }
        x += 6;
    }
    let mut y = 10;
    while y < h - 13 {
        for x in [4, w - 5] {
            canvas.fill(x, y, 1, 3, thread);
        }
        y += 6;
    }
    protectors(canvas, theme, w, h);
}

const CORNERS: [(i32, i32); 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];

/// A corner of a `w`×`h` panel, `inset` pixels in from both of its sides.
fn corner(w: i32, h: i32, (right, bottom): (i32, i32), inset: i32) -> (i32, i32) {
    let x = if right == 1 { w - 1 - inset } else { inset };
    let y = if bottom == 1 { h - 1 - inset } else { inset };
    (x, y)
}

/// Copper tubing with its elbows and couplings.
fn piped(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(2, 2, w - 4, h - 4, theme.frame);
    canvas.frame(3, 3, w - 6, h - 6, theme.frame_dark);
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        canvas.fill(x - 3, y - 3, 7, 7, theme.frame_dark);
        canvas.fill(x - 2, y - 2, 5, 5, theme.frame);
        canvas.fill(x - 2, y - 2, 3, 2, theme.frame_light);
    }
    for y in [0, h - 4] {
        canvas.fill(w / 2 - 5, y, 11, 4, theme.frame_dark);
        canvas.fill(w / 2 - 4, y + 1, 9, 2, theme.frame_light);
    }
}

/// A sheet with a ragged margin and a drop of ink.
fn parchment(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    let edge = with_alpha(theme.frame_light, 170);
    canvas.frame(2, 2, w - 4, h - 4, edge);
    let ragged = with_alpha(theme.frame_light, 90);
    for x in 4..w - 4 {
        if noise(x, 1).is_multiple_of(5) {
            canvas.put(x, 3, ragged);
        }
        if noise(x, 2).is_multiple_of(5) {
            canvas.put(x, h - 4, ragged);
        }
    }
    for y in 4..h - 4 {
        if noise(3, y).is_multiple_of(5) {
            canvas.put(3, y, ragged);
        }
        if noise(4, y).is_multiple_of(5) {
            canvas.put(w - 4, y, ragged);
        }
    }
    let ink = [44, 66, 150, 255];
    canvas.ellipse(w as f32 - 4.0, 4.0, 3.5, 3.0, ink);
    canvas.fill(w - 11, 2, 2, 2, ink);
    canvas.fill(w - 3, 11, 2, 2, ink);
}

/// Drafting lines that cross at the corners.
fn chalked(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    let line = with_alpha(theme.frame_light, 190);
    for x in (3..w - 3).step_by(5) {
        canvas.fill(x, 3, 3, 1, line);
        canvas.fill(x, h - 4, 3, 1, line);
    }
    for y in (3..h - 3).step_by(5) {
        canvas.fill(3, y, 1, 3, line);
        canvas.fill(w - 4, y, 1, 3, line);
    }
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 0);
        let (cx, cy) = corner(w, h, which, 2);
        canvas.fill(
            x.min(x + 9 * (1 - 2 * which.0)),
            cy,
            10,
            1,
            theme.frame_light,
        );
        canvas.fill(
            cx,
            y.min(y + 9 * (1 - 2 * which.1)),
            1,
            10,
            theme.frame_light,
        );
        canvas.ring(cx as f32 + 0.5, cy as f32 + 0.5, 2.5, 2.5, theme.accent);
    }
}

/// Iron bands nailed down both sides.
fn banded(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(2, 2, w - 4, h - 4, theme.frame_dark);
    for x in [1, w - 6] {
        canvas.fill(x, 1, 5, h - 2, theme.frame);
        canvas.fill(x, 1, 1, h - 2, theme.frame_light);
        canvas.fill(x + 4, 1, 1, h - 2, theme.frame_dark);
        let mut y = 6;
        while y < h - 6 {
            canvas.fill(x + 1, y, 3, 3, theme.frame_dark);
            canvas.fill(x + 1, y, 2, 2, [255, 214, 110, 255]);
            y += 22;
        }
    }
}

/// Rounded, with a small lamp alight in each corner.
fn lamplit(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(3, 3, w - 6, h - 6, with_alpha(theme.frame, 190));
    for which in CORNERS {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
            let (x, y) = corner(w, h, which, 0);
            let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
            canvas.set(x + dx * sx, y + dy * sy, [0; 4]);
        }
        let (x, y) = corner(w, h, which, 4);
        canvas.glow(x as f32, y as f32, 8.0, [255, 190, 80, 150]);
        diamond(canvas, x, y, 2, theme.accent);
        canvas.put(x, y, [255, 244, 200, 255]);
    }
}

/// Canvas hemmed in leather, with a buckle on two corners.
fn strapped(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(2, 2, w - 4, h - 4, [96, 62, 34, 255]);
    let thread = with_alpha(theme.accent, 150);
    for x in (6..w - 8).step_by(6) {
        canvas.fill(x, 3, 3, 1, thread);
        canvas.fill(x, h - 4, 3, 1, thread);
    }
    for x in [10, w - 18] {
        canvas.fill(x, 0, 8, 6, theme.frame_dark);
        canvas.fill(x + 1, 0, 6, 5, [112, 72, 40, 255]);
        canvas.frame(x + 1, 1, 6, 4, [232, 196, 104, 255]);
    }
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 2);
        canvas.fill(x - 1, y - 1, 3, 3, [232, 196, 104, 255]);
        canvas.put(x, y, theme.frame_dark);
    }
}

/// A fine double line with a star on each corner.
fn starry(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(3, 3, w - 6, h - 6, with_alpha(theme.frame, 150));
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        canvas.glow(
            x as f32 + 0.5,
            y as f32 + 0.5,
            7.0,
            with_alpha(theme.accent, 160),
        );
        for k in 0..=3 {
            let c = with_alpha(theme.accent, (255 - k * 60) as u8);
            for (dx, dy) in [(k, 0), (-k, 0), (0, k), (0, -k)] {
                canvas.put(x + dx, y + dy, c);
            }
        }
        canvas.put(x, y, [255, 255, 255, 255]);
    }
}

/// Gilt edges with a leaf curling at each corner.
fn gilded(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(3, 3, w - 6, h - 6, theme.frame);
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
        for (dx, dy) in [
            (1, 1),
            (2, 2),
            (3, 2),
            (2, 3),
            (4, 2),
            (2, 4),
            (5, 1),
            (1, 5),
        ] {
            canvas.put(x + dx * sx, y + dy * sy, theme.frame_light);
        }
        diamond(canvas, x, y, 2, theme.frame_light);
        canvas.put(x, y, theme.frame_dark);
    }
}

/// The ticked border of a chart.
fn charted(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(4, 4, w - 8, h - 8, with_alpha(theme.frame, 180));
    for x in 2..w - 2 {
        let ink = if (x / 8) % 2 == 0 {
            theme.frame_light
        } else {
            theme.frame_dark
        };
        canvas.fill(x, 2, 1, 2, ink);
        canvas.fill(x, h - 4, 1, 2, ink);
    }
    for y in 4..h - 4 {
        let ink = if (y / 8) % 2 == 0 {
            theme.frame_light
        } else {
            theme.frame_dark
        };
        canvas.fill(2, y, 2, 1, ink);
        canvas.fill(w - 4, y, 2, 1, ink);
    }
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        diamond(canvas, x, y, 3, theme.frame_dark);
        diamond(canvas, x, y, 2, theme.accent);
    }
}

/// Gold with a small gallery set in each corner.
fn hexed(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(2, 2, w - 4, h - 4, with_alpha(theme.frame, 220));
    canvas.frame(4, 4, w - 8, h - 8, with_alpha(theme.frame_light, 90));
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 4);
        for dy in -3..=3 {
            let half = 4 - i32::abs(dy) / 2;
            canvas.fill(x - half, y + dy, half * 2 + 1, 1, theme.frame_light);
        }
        diamond(canvas, x, y, 2, theme.frame_dark);
        canvas.put(x, y, theme.accent);
    }
}

/// A double line with a gold curl in each corner.
fn curled(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(3, 3, w - 6, h - 6, with_alpha(theme.accent, 90));
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 2);
        let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
        for k in 0..9 {
            canvas.put(x + k * sx, y, theme.accent);
            canvas.put(x, y + k * sy, theme.accent);
        }
        for (dx, dy) in [(2, 2), (3, 2), (4, 3), (2, 3), (3, 4)] {
            canvas.put(x + dx * sx, y + dy * sy, theme.accent);
        }
    }
}

/// Teal and rose, hooked together at top and bottom.
fn linked(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(2, 2, w - 4, h - 4, with_alpha(theme.frame_light, 120));
    canvas.fill(1, 1, w / 2 - 1, 1, theme.heading);
    canvas.fill(w / 2, 1, w - w / 2 - 1, 1, theme.accent);
    for y in [1, h - 4] {
        canvas.frame(w / 2 - 9, y - 1, 12, 5, theme.heading);
        canvas.frame(w / 2 - 2, y - 1, 12, 5, theme.accent);
    }
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        let color = if (which.0 + which.1) % 2 == 0 {
            theme.heading
        } else {
            theme.accent
        };
        diamond(canvas, x, y, 3, theme.frame_dark);
        diamond(canvas, x, y, 2, color);
    }
}

/// Brass with a little cog riveted to each corner.
fn geared(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(3, 3, w - 6, h - 6, with_alpha(theme.frame, 200));
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 3);
        for (dx, dy) in [
            (0, -3),
            (0, 3),
            (-3, 0),
            (3, 0),
            (-2, -2),
            (2, 2),
            (2, -2),
            (-2, 2),
        ] {
            canvas.put(x + dx, y + dy, theme.frame_light);
        }
        diamond(canvas, x, y, 2, theme.frame);
        diamond(canvas, x, y, 1, theme.frame_dark);
    }
}

/// A thin leather binding with a ribbon marking the place.
fn bound(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(2, 2, w - 4, h - 4, with_alpha(theme.accent, 110));
    // Down the right margin, clear of the lines of text.
    let x = w - 11;
    canvas.fill(x, 0, 5, 22, [196, 52, 66, 255]);
    canvas.fill(x, 0, 1, 22, [244, 108, 110, 255]);
    canvas.fill(x, 22, 2, 3, [196, 52, 66, 255]);
    canvas.fill(x + 3, 22, 2, 3, [196, 52, 66, 255]);
}

/// Grey stone with a crack from two of its corners.
fn cracked(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.frame(2, 2, w - 4, h - 4, with_alpha(theme.frame, 160));
    for which in [(0, 0), (1, 1)] {
        let (mut x, mut y) = corner(w, h, which, 1);
        let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
        for step in 0..8 {
            let jag = (noise(step, which.0 + 5) % 3) as i32;
            let (nx, ny) = (x + sx * (1 + jag % 2), y + sy * (2 - jag % 2));
            canvas.line(x, y, nx, ny, theme.frame_light);
            (x, y) = (nx, ny);
        }
    }
}

/// Gold with rays fanning from each corner.
fn radiant(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    canvas.frame(3, 3, w - 6, h - 6, with_alpha(theme.accent, 110));
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 2);
        let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
        for (dx, dy) in [(8, 1), (7, 3), (5, 5), (3, 7), (1, 8)] {
            let color = with_alpha(theme.accent, 190);
            canvas.line(x, y, x + dx * sx, y + dy * sy, color);
        }
        diamond(canvas, x, y, 2, theme.frame_light);
    }
}

/// A page of accounts: a double margin down each side and gilt corners.
fn ruled(canvas: &mut Canvas, theme: &Theme, w: i32, h: i32) {
    canvas.fill(1, 1, w - 2, 1, theme.frame_light);
    for x in [3, 5, w - 6, w - 4] {
        canvas.fill(x, 3, 1, h - 6, [196, 84, 84, 120]);
    }
    for which in CORNERS {
        let (x, y) = corner(w, h, which, 0);
        let (sx, sy) = (1 - 2 * which.0, 1 - 2 * which.1);
        for dy in 0..7 {
            for dx in 0..7 - dy {
                let edge = dx + dy == 6;
                let color = if edge { theme.frame_dark } else { theme.accent };
                canvas.put(x + dx * sx, y + dy * sy, color);
            }
        }
    }
}

/// An option: always bordered, with its corners notched. The one under the
/// cursor glows. `meaning` is a border that says something, and is kept.
pub fn option(
    canvas: &mut Canvas,
    theme: &Theme,
    (x, y, w, h): (i32, i32, i32, i32),
    focused: bool,
    meaning: Option<Rgba>,
    strength: u8,
) {
    if focused {
        canvas.frame(x - 1, y - 1, w + 2, h + 2, with_alpha(theme.accent, 110));
    }
    let fill = if focused { theme.cell_hot } else { theme.cell };
    canvas.fill(x + 1, y, w - 2, h, fill);
    canvas.fill(x, y + 1, 1, h - 2, fill);
    canvas.fill(x + w - 1, y + 1, 1, h - 2, fill);
    let lift = if focused { 70 } else { 30 };
    canvas.fill(x + 1, y + 1, w - 2, 1, [255, 236, 214, lift]);
    canvas.fill(x + 1, y + h - 2, w - 2, 1, [0, 0, 0, 90]);
    let edge = match meaning {
        Some(color) => color,
        None if focused => theme.accent,
        None => with_alpha(theme.edge, strength),
    };
    canvas.fill(x + 1, y, w - 2, 1, edge);
    canvas.fill(x + 1, y + h - 1, w - 2, 1, edge);
    canvas.fill(x, y + 1, 1, h - 2, edge);
    canvas.fill(x + w - 1, y + 1, 1, h - 2, edge);
}

pub fn diamond(canvas: &mut Canvas, x: i32, y: i32, size: i32, color: Rgba) {
    for dy in -size..=size {
        let half = size - dy.abs();
        canvas.fill(x - half, y + dy, half * 2 + 1, 1, color);
    }
}

/// A rule that ends in a diamond, from `from` to `to`.
pub fn rule(canvas: &mut Canvas, theme: &Theme, from: i32, to: i32, y: i32, color: Rgba) {
    if to - from < 10 {
        return;
    }
    canvas.fill(from, y, to - from - 4, 1, with_alpha(color, 120));
    let shade = with_alpha(theme.frame_dark, 160);
    canvas.fill(from, y + 1, to - from - 4, 1, shade);
    diamond(canvas, to - 3, y, 2, color);
}

/// A hexagonal medallion, the shape of the galleries, to frame an icon.
pub fn medallion(canvas: &mut Canvas, theme: &Theme, cx: i32, cy: i32, size: i32) {
    let glow = with_alpha(theme.accent, 90);
    canvas.glow(cx as f32, cy as f32, size as f32 * 2.0, glow);
    let mut hexagon = |s: i32, color: Rgba| {
        for dy in -s..=s {
            let half = s + s / 4 - dy.abs() / 2;
            canvas.fill(cx - half, cy + dy, half * 2 + 1, 1, color);
        }
    };
    hexagon(size, theme.frame_dark);
    hexagon(size - 1, theme.accent);
    hexagon(size - 3, theme.frame_dark);
    hexagon(size - 4, mix(theme.panel, theme.accent, 0.18));
}

/// The strip a title stands on.
pub fn plaque(canvas: &mut Canvas, theme: &Theme, (x, y, w, h): (i32, i32, i32, i32)) {
    canvas.fill(x + 2, y + 2, w, h, [0, 0, 0, 120]);
    canvas.fill(x, y, w, h, theme.panel);
    canvas.frame(x, y, w, h, theme.frame_dark);
    canvas.frame(x + 1, y + 1, w - 2, h - 2, theme.frame);
    canvas.fill(x + 1, y + 1, w - 2, 1, theme.frame_light);
    match theme.frame_style {
        FrameStyle::Riveted => {
            for px in [x + 4, x + w - 5] {
                rivet(canvas, theme, px, y + h / 2);
            }
        }
        FrameStyle::Stitched => {
            for (px, py, sx, sy) in [
                (x, y, 1, 1),
                (x + w - 1, y, -1, 1),
                (x, y + h - 1, 1, -1),
                (x + w - 1, y + h - 1, -1, -1),
            ] {
                protector(canvas, theme, px, py, sx, sy);
            }
        }
        _ => {
            for px in [x + 4, x + w - 5] {
                diamond(canvas, px, y + h / 2, 2, theme.accent);
            }
        }
    }
}
