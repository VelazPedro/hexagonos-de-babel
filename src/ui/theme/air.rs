//! What floats behind the panels. It depends only on the time, so that a
//! frame can always be drawn again.

use super::{Air, Theme, frames::noise};
use crate::render::canvas::{Canvas, with_alpha};
use crate::ui::{HEIGHT, WIDTH};

pub fn draw(canvas: &mut Canvas, theme: &Theme, time: f32) {
    match theme.air {
        Air::Pages => pages(canvas, time),
        Air::Embers => embers(canvas, theme, time),
        Air::Dust => dust(canvas, time),
        Air::Bubbles => bubbles(canvas, time),
        Air::Letters => letters(canvas, time),
        Air::Chalk => chalk_marks(canvas, time),
        Air::Glints => glints(canvas, time),
        Air::Fireflies => fireflies(canvas, time),
        Air::Stars => stars(canvas, time),
        Air::GoldDust => gold_dust(canvas, time),
        Air::Hexagons => hexagons(canvas, time),
        Air::TwoMotes => two_motes(canvas, theme, time),
        Air::Gears => gears(canvas, time),
        Air::AshLeaves => ash_leaves(canvas, time),
        Air::Sparks => sparks(canvas, time),
        Air::None => {}
    }
}

/// Where particle `i` is on its way up: it starts again from below.
fn rising(i: i32, time: f32, slowest: f32) -> (i32, i32) {
    let seed = noise(i, 7);
    let speed = slowest + (seed % 11) as f32;
    let travel = (time * speed + (seed >> 8 & 511) as f32) % (HEIGHT as f32 + 20.0);
    let sway = ((time * 1.7 + i as f32).sin() * 3.0) as i32;
    let x = ((seed >> 3) % WIDTH as u32) as i32 + sway;
    (x, HEIGHT + 10 - travel as i32)
}

/// Loose pages drifting upwards.
fn pages(canvas: &mut Canvas, time: f32) {
    for i in 0..26 {
        let speed = 5.0 + (i % 7) as f32 * 1.7;
        let travel = (time * speed + i as f32 * 53.0) % (HEIGHT as f32 + 20.0);
        let y = HEIGHT + 10 - travel as i32;
        let sway = ((time * 0.7 + i as f32) * 1.3).sin() * 9.0;
        let x = (i * 97 + 23) % WIDTH + sway as i32;
        let w = 3 + (i % 3);
        let flutter = i32::from((time * 2.0 + i as f32).sin() > 0.0);
        let alpha = 40 + (i % 4) as u8 * 22;
        canvas.fill(x, y + flutter, w, w + 1 - flutter, [248, 234, 200, alpha]);
        canvas.fill(x + 1, y + 1 + flutter, w - 2, 1, [150, 120, 100, alpha]);
    }
}

fn embers(canvas: &mut Canvas, theme: &Theme, time: f32) {
    for i in 0..46 {
        let (x, y) = rising(i, time, 6.0);
        let hot = if i % 3 == 0 {
            [255, 222, 120, 255]
        } else {
            theme.accent
        };
        canvas.glow(x as f32, y as f32, 4.0, [255, 120, 30, 90]);
        canvas.put(x, y, hot);
        if i % 5 == 0 {
            canvas.put(x, y + 1, with_alpha(theme.accent, 150));
        }
    }
}

/// Motes and scraps of paper over the books.
fn dust(canvas: &mut Canvas, time: f32) {
    for i in 0..46 {
        let (x, y) = rising(i, time, 4.0);
        canvas.put(x, y, [250, 232, 192, 150]);
        if i % 6 == 0 {
            canvas.fill(x, y, 3, 4, [250, 232, 192, 110]);
        }
    }
}

fn bubbles(canvas: &mut Canvas, time: f32) {
    for i in 0..34 {
        let (x, y) = rising(i, time, 7.0);
        let r = 1.5 + (i % 3) as f32;
        canvas.ring(x as f32, y as f32, r, r, [170, 255, 210, 120]);
        canvas.put(x - 1, y - 1, [230, 255, 240, 190]);
    }
}

/// Letters that came loose from their pages.
fn letters(canvas: &mut Canvas, time: f32) {
    const SHAPES: [[u8; 5]; 5] = [
        [0b010, 0b101, 0b111, 0b101, 0b101],
        [0b110, 0b101, 0b110, 0b101, 0b110],
        [0b011, 0b100, 0b100, 0b100, 0b011],
        [0b111, 0b001, 0b010, 0b100, 0b111],
        [0b101, 0b101, 0b010, 0b101, 0b101],
    ];
    for i in 0..30 {
        let (x, y) = rising(i, time, 3.0);
        let shape = SHAPES[i as usize % SHAPES.len()];
        let alpha = 70 + (i % 3) as u8 * 30;
        for (dy, bits) in shape.into_iter().enumerate() {
            for dx in 0..3 {
                if bits & (0b100 >> dx) != 0 {
                    canvas.put(x + dx, y + dy as i32, [240, 224, 184, alpha]);
                }
            }
        }
    }
}

/// A fixed spot for particle `i`.
fn spot(i: i32) -> (i32, i32) {
    let seed = noise(i, 31);
    let x = (seed % WIDTH as u32) as i32;
    let y = ((seed >> 12) % HEIGHT as u32) as i32;
    (x, y)
}

/// How bright particle `i` is now, from 0 to 1.
fn twinkle(i: i32, time: f32, speed: f32) -> f32 {
    ((time * speed + i as f32 * 1.9).sin() * 0.5 + 0.5).powi(3)
}

fn chalk_marks(canvas: &mut Canvas, time: f32) {
    for i in 0..40 {
        let (x, y) = spot(i);
        let alpha = (twinkle(i, time, 1.6) * 190.0) as u8;
        canvas.fill(x - 2, y, 5, 1, [224, 240, 255, alpha]);
        canvas.fill(x, y - 2, 1, 5, [224, 240, 255, alpha]);
    }
}

/// Gold catching the light.
fn glints(canvas: &mut Canvas, time: f32) {
    for i in 0..38 {
        let (x, y) = spot(i);
        let shine = twinkle(i, time, 2.4);
        let alpha = (shine * 255.0) as u8;
        let arm = (shine * 3.0).round() as i32;
        canvas.fill(x - arm, y, arm * 2 + 1, 1, [255, 214, 90, alpha]);
        canvas.fill(x, y - arm, 1, arm * 2 + 1, [255, 214, 90, alpha]);
        canvas.put(x, y, [255, 246, 200, alpha]);
    }
}

fn fireflies(canvas: &mut Canvas, time: f32) {
    for i in 0..30 {
        let (x, y) = spot(i);
        let k = i as f32;
        let dx = ((time * 0.6 + k).sin() * 11.0 + (time * 1.3 + k * 2.0).sin() * 4.0) as i32;
        let dy = ((time * 0.8 + k * 3.0).cos() * 8.0) as i32;
        let alpha = (90.0 + twinkle(i, time, 2.0) * 165.0) as u8;
        canvas.glow(
            (x + dx) as f32,
            (y + dy) as f32,
            5.0,
            [210, 255, 110, alpha / 2],
        );
        canvas.put(x + dx, y + dy, [240, 255, 170, alpha]);
    }
}

fn stars(canvas: &mut Canvas, time: f32) {
    for i in 0..48 {
        let (x, y) = spot(i + 100);
        let shine = twinkle(i, time, 1.4);
        let alpha = (40.0 + shine * 215.0) as u8;
        canvas.put(x, y, [230, 236, 255, alpha]);
        if i % 7 == 0 {
            let arm = (shine * 2.0).round() as i32;
            canvas.fill(x - arm, y, arm * 2 + 1, 1, [200, 212, 255, alpha / 2]);
            canvas.fill(x, y - arm, 1, arm * 2 + 1, [200, 212, 255, alpha / 2]);
        }
    }
}

fn gold_dust(canvas: &mut Canvas, time: f32) {
    for i in 0..44 {
        let (x, y) = rising(i, time, 2.0);
        let alpha = (80.0 + twinkle(i, time, 2.0) * 175.0) as u8;
        canvas.put(x, y, [255, 214, 110, alpha]);
        if i % 9 == 0 {
            canvas.glow(x as f32, y as f32, 4.0, [255, 200, 90, alpha / 2]);
        }
    }
}

/// Galleries turning slowly as they climb.
fn hexagons(canvas: &mut Canvas, time: f32) {
    for i in 0..22 {
        let (x, y) = rising(i, time, 3.0);
        let r = 4.0 + (i % 4) as f32 * 2.0;
        let turn = time * 0.4 + i as f32;
        let color = if i % 3 == 0 {
            [255, 212, 96, 110]
        } else {
            [200, 160, 255, 90]
        };
        super::backdrop::hex_outline(canvas, x as f32, y as f32, r, turn, color);
    }
}

/// Motes of two colours, one for each player.
fn two_motes(canvas: &mut Canvas, theme: &Theme, time: f32) {
    for i in 0..40 {
        let (x, y) = rising(i, time, 4.0);
        let color = if i % 2 == 0 {
            theme.heading
        } else {
            theme.accent
        };
        canvas.glow(x as f32, y as f32, 3.5, with_alpha(color, 120));
        canvas.put(x, y, with_alpha(color, 200));
    }
}

/// Slow wheels, each turning against its neighbour.
fn gears(canvas: &mut Canvas, time: f32) {
    for (i, (x, y, r)) in [
        (28.0, 40.0, 15.0),
        (50.0, 62.0, 10.0),
        (452.0, 226.0, 17.0),
        (430.0, 248.0, 9.0),
        (458.0, 52.0, 11.0),
        (24.0, 226.0, 12.0),
    ]
    .into_iter()
    .enumerate()
    {
        let way = if i % 2 == 0 { 1.0 } else { -1.0 };
        let turn = time * way * 3.2 / r;
        super::backdrop::cog(canvas, x, y, r, turn, [226, 190, 104, 110]);
    }
}

/// Grey leaves coming down, turning as they go.
fn ash_leaves(canvas: &mut Canvas, time: f32) {
    for i in 0..40 {
        let (x, up) = rising(i, time, 5.0);
        // The same path, downwards.
        let y = HEIGHT - up;
        let wide = 1 + i32::from((time * 2.0 + i as f32).sin() > 0.0) * 2;
        let alpha = 70 + (i % 4) as u8 * 30;
        canvas.fill(x, y, wide, 2, [190, 182, 206, alpha]);
        canvas.put(x + wide / 2, y + 2, [120, 112, 138, alpha]);
    }
}

fn sparks(canvas: &mut Canvas, time: f32) {
    for i in 0..60 {
        let (x, y) = rising(i, time, 6.0);
        let alpha = (60.0 + twinkle(i, time, 3.0) * 195.0) as u8;
        canvas.put(x, y, [255, 212, 110, alpha]);
        if i % 8 == 0 {
            canvas.fill(x - 1, y, 3, 1, [255, 236, 170, alpha / 2]);
            canvas.fill(x, y - 1, 1, 3, [255, 236, 170, alpha / 2]);
        }
    }
}
