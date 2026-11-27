//! The application icon: the hero's bust in front of a candle-lit gallery,
//! framed by the hexagon of a Babel cell. It is composed with the same code
//! that draws the player, so the two can never drift apart.
//!
//! `--icon-dump` writes it to `assets/icon/`; `build.rs` links the `.ico` into
//! the Windows executable.

use super::atlas::Atlas;
use super::canvas::{Canvas, Rgba, mix, rgb};
use super::gear::Gear;
use super::hero;
use babel_core::progression::look::Look;

/// Every size in the `.ico`, with the hand-composed size it is enlarged from.
pub const SIZES: [(i32, i32); 7] = [
    (16, 16),
    (24, 24),
    (32, 32),
    (48, 24),
    (64, 32),
    (128, 32),
    (256, 32),
];

const OUTLINE: Rgba = rgb(0x1a, 0x10, 0x28);
const GOLD: Rgba = rgb(0xff, 0xd9, 0x5c);
const GOLD_SHADE: Rgba = rgb(0xd9, 0x96, 0x2c);
const WOOD: Rgba = rgb(0xae, 0x6a, 0x3e);
const WOOD_SHADE: Rgba = rgb(0x6c, 0x3c, 0x38);
const MOTE: Rgba = rgb(0xbf, 0xf6, 0xff);
const MOTE_DIM: Rgba = rgb(0x48, 0xdc, 0xc4);
/// Candlelight fading into the violet dark, in the flat bands the game uses.
const GLOW: [Rgba; 5] = [
    rgb(0xf0, 0xb4, 0x66),
    rgb(0xc6, 0x84, 0x50),
    rgb(0x8c, 0x54, 0x4c),
    rgb(0x54, 0x30, 0x4e),
    rgb(0x2a, 0x1c, 0x40),
];
const SPINES: [Rgba; 6] = [
    rgb(0xa2, 0x2f, 0x50),
    rgb(0x23, 0x84, 0x90),
    rgb(0xd9, 0x96, 0x2c),
    rgb(0x2f, 0x8c, 0x5c),
    rgb(0x5e, 0x3a, 0x9a),
    rgb(0x38, 0x54, 0xb4),
];

/// How one hand-composed size is laid out.
struct Layout {
    /// Width of the hexagon; its height is the whole icon.
    width: f32,
    /// Thickness of the gold frame.
    gold: f32,
    /// Where the hero's outlined frame goes.
    hero: (i32, i32),
    /// Rows of the shelf boards.
    shelves: &'static [i32],
    /// Sparks of stored light.
    motes: &'static [(i32, i32)],
    /// Column of the spear behind the hero. At 16 pixels it would only be
    /// noise.
    spear: Option<i32>,
}

fn layout(size: i32) -> Layout {
    match size {
        16 => Layout {
            width: 16.0,
            gold: 1.0,
            hero: (-7, 1),
            shelves: &[],
            motes: &[],
            spear: None,
        },
        24 => Layout {
            width: 23.0,
            gold: 1.0,
            hero: (-3, 4),
            shelves: &[8, 15],
            motes: &[(4, 9)],
            spear: Some(19),
        },
        _ => Layout {
            width: 30.0,
            gold: 1.5,
            hero: (1, 7),
            shelves: &[10, 18, 26],
            motes: &[(6, 8), (5, 20)],
            spear: Some(26),
        },
    }
}

/// Whether the centre of a pixel is inside the pointed-top hexagon that fills
/// the icon, shrunk by `inset` pixels.
fn in_hexagon(size: i32, width: f32, inset: f32, x: i32, y: i32) -> bool {
    let half = size as f32 / 2.0;
    let (dx, dy) = ((x as f32 + 0.5 - half).abs(), (y as f32 + 0.5 - half).abs());
    // The slanted sides drop half the half-height across the half-width.
    let slope = half / width;
    let reach = half - inset * (1.0 + slope * slope).sqrt();
    dx <= width / 2.0 - inset && dy + dx * slope <= reach
}

/// How far a pixel is from the candle, which burns behind the hero's head.
fn darkness(size: i32, x: i32, y: i32) -> f32 {
    let (cx, cy) = (size as f32 * 0.5, size as f32 * 0.4);
    let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
    (dx * dx + dy * dy).sqrt() / (size as f32 * 0.62)
}

/// The gallery wall: candlelight, then shelves of books fading into the dark.
fn wall(size: i32, plan: &Layout, x: i32, y: i32) -> Rgba {
    let far = darkness(size, x, y);
    let band = ((far * GLOW.len() as f32) as usize).min(GLOW.len() - 1);
    let air = GLOW[band];
    if plan.shelves.contains(&y) {
        return mix(WOOD, OUTLINE, (far * 0.75).min(0.8));
    }
    if plan.shelves.contains(&(y - 1)) {
        return mix(WOOD_SHADE, OUTLINE, (far * 0.75).min(0.8));
    }
    let Some(&board) = plan.shelves.iter().find(|&&board| board > y) else {
        return air;
    };
    // Books stand two pixels wide with a gap now and then.
    let book = x / 2;
    let pick = (book * 7 + board * 3) as usize;
    let height = 4 + (book * 5 + board) % 3;
    if board - y > height || (book + board) % 6 == 0 {
        return air;
    }
    let mut spine = SPINES[pick % SPINES.len()];
    if x % 2 == 1 {
        spine = mix(spine, OUTLINE, 0.35);
    } else if board - y == height - 1 {
        spine = mix(spine, GOLD, 0.6);
    }
    // The candle washes the books near the head into its own warmth.
    mix(spine, air, (0.62 - far * 0.5).clamp(0.12, 0.55))
}

/// One of the hand-composed sizes.
fn compose(source: &Atlas, size: i32) -> Canvas {
    let plan = layout(size);
    let gear = Gear::starting();
    let hero = hero::build(source, Look::default(), &gear);
    let mut out = Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            if !in_hexagon(size, plan.width, 0.0, x, y) {
                continue;
            }
            let color = if !in_hexagon(size, plan.width, 1.0, x, y) {
                OUTLINE
            } else if !in_hexagon(size, plan.width, 1.0 + plan.gold, x, y) {
                // Lit from above, like everything in the game.
                if y < size / 2 { GOLD } else { GOLD_SHADE }
            } else if !in_hexagon(size, plan.width, 2.0 + plan.gold, x, y) {
                OUTLINE
            } else {
                wall(size, &plan, x, y)
            };
            out.set(x, y, color);
        }
    }
    for &(x, y) in plan.motes {
        out.set(x, y, MOTE);
        out.set(x, y + 1, MOTE_DIM);
    }
    let inside = |x, y| in_hexagon(size, plan.width, 1.0 + plan.gold, x, y);
    // The spear rises behind one shoulder, so that it never crosses the face,
    // and its head breaks out of the frame into the empty corner.
    if let Some(column) = plan.spear {
        let at = (column - hero::ITEM_GRIP.0, -hero::held_top(&hero));
        paste(&mut out, &hero, "held", at, |x, y| {
            y < size / 3 || inside(x, y)
        });
    }
    // The hero stands in front, cut off by the frame at the shoulders.
    paste(&mut out, &hero, "bare_idle_down", plan.hero, inside);
    out
}

/// Draws a sprite of the hero with its corner at `at`, where `keep` allows it.
fn paste(
    out: &mut Canvas,
    hero: &Atlas,
    sprite: &str,
    (left, top): (i32, i32),
    keep: impl Fn(i32, i32) -> bool,
) {
    let frame = hero.get(sprite, 0);
    for y in 0..frame.h {
        for x in 0..frame.w {
            if keep(left + x, top + y) {
                out.put(left + x, top + y, hero.pixel(frame, x, y));
            }
        }
    }
}

/// The icon at one of the `SIZES`, enlarged by whole pixels.
pub fn art(source: &Atlas, size: i32) -> Canvas {
    let native = SIZES
        .iter()
        .find(|(s, _)| *s == size)
        .map_or(32, |&(_, native)| native);
    let small = compose(source, native);
    let scale = (size / native).max(1);
    let mut out = Canvas::new(native * scale, native * scale);
    for y in 0..out.h {
        for x in 0..out.w {
            out.set(x, y, small.get(x / scale, y / scale));
        }
    }
    out
}

/// An image as stored inside an `.ico`: a bitmap header, the pixels bottom-up
/// in BGRA, and a one-bit mask of the transparent ones.
fn bitmap(image: &Canvas) -> Vec<u8> {
    let (w, h) = (image.w, image.h);
    let mut out = Vec::new();
    out.extend(40u32.to_le_bytes());
    out.extend(w.to_le_bytes());
    // The height counts the colour rows and the mask rows.
    out.extend((h * 2).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    // No compression; the remaining fields may stay zero.
    out.extend([0u8; 24]);
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b, a] = image.get(x, y);
            out.extend([b, g, r, a]);
        }
    }
    let row = ((w + 31) / 32 * 4) as usize;
    for y in (0..h).rev() {
        let mut bits = vec![0u8; row];
        for x in 0..w {
            if image.get(x, y)[3] == 0 {
                bits[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        out.extend(bits);
    }
    out
}

/// The Windows icon file with every size.
pub fn ico(source: &Atlas) -> Vec<u8> {
    let images: Vec<(i32, Vec<u8>)> = SIZES
        .iter()
        .map(|&(size, _)| (size, bitmap(&art(source, size))))
        .collect();
    let mut out = Vec::new();
    for value in [0u16, 1, images.len() as u16] {
        out.extend(value.to_le_bytes());
    }
    let mut offset = 6 + 16 * images.len();
    for (size, data) in &images {
        // A side of 256 is written as 0.
        let side = (*size % 256) as u8;
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Atlas {
        Atlas::built_in().expect("atlas")
    }

    #[test]
    fn every_size_is_a_hexagon_on_a_transparent_square() {
        let source = source();
        for (size, _) in SIZES {
            let image = art(&source, size);
            assert_eq!((image.w, image.h), (size, size));
            for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
                assert_eq!(image.get(x, y)[3], 0, "{size}: corner {x},{y}");
            }
            assert_eq!(image.get(size / 2, size / 2)[3], 255, "{size}: centre");
        }
    }

    #[test]
    fn the_icon_shows_the_protagonist() {
        let source = source();
        let hero = hero::build(&source, Look::default(), &Gear::starting());
        let frame = hero.get("bare_idle_down", 0);
        for size in [16, 24, 32] {
            let image = art(&source, size);
            let plan = layout(size);
            // The eyes of the in-game sprite land on the same colours.
            for &(ex, ey) in hero::eye_pixels("down") {
                let (x, y) = (ex + plan.hero.0, ey + plan.hero.1);
                assert_eq!(image.get(x, y), hero.pixel(frame, ex, ey), "{size}");
            }
        }
    }

    #[test]
    fn the_icon_file_lists_every_size_with_data_of_the_right_length() {
        let bytes = ico(&source());
        let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let long = |at: usize| usize::from(word(at)) | usize::from(word(at + 2)) << 16;
        assert_eq!((word(0), word(2)), (0, 1));
        assert_eq!(usize::from(word(4)), SIZES.len());
        let mut expected = 6 + 16 * SIZES.len();
        for (i, (size, _)) in SIZES.iter().enumerate() {
            let entry = 6 + 16 * i;
            assert_eq!(i32::from(bytes[entry]), size % 256);
            let (length, offset) = (long(entry + 8), long(entry + 12));
            assert_eq!(offset, expected);
            let mask = ((size + 31) / 32 * 4 * size) as usize;
            assert_eq!(length, 40 + (size * size * 4) as usize + mask);
            expected += length;
        }
        assert_eq!(bytes.len(), expected);
    }

    #[test]
    fn the_committed_icon_matches_the_sprites() {
        let committed = include_bytes!("../../assets/icon/hexagonos.ico");
        assert!(
            committed.as_slice() == ico(&source()),
            "stale icon: run `cargo run -- --icon-dump`"
        );
    }
}
