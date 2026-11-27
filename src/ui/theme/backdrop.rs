//! What each place has behind its panels.

use super::{Place, Theme};
use crate::render::canvas::{Canvas, Rgba, mix, with_alpha};
use crate::ui::{HEIGHT, WIDTH};

/// The colour of the background at a height: calm above, warmer below.
pub fn tone_at(theme: &Theme, y: i32) -> Rgba {
    let k = (y as f32 / HEIGHT as f32).powi(2);
    mix(theme.sky, theme.floor, k)
}

/// The whole background.
pub fn opaque(theme: &Theme) -> Canvas {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    for y in 0..HEIGHT {
        canvas.fill(0, y, WIDTH, 1, tone_at(theme, y));
    }
    decorate(&mut canvas, theme);
    canvas
}

/// Only the decoration, on nothing, so that the world shows through.
pub fn ornament(theme: &Theme) -> Canvas {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    decorate(&mut canvas, theme);
    canvas
}

fn decorate(canvas: &mut Canvas, theme: &Theme) {
    match theme.place {
        Place::Forge => bricks(canvas),
        Place::Binder => tooled_leather(canvas),
        Place::Alembic => still_room(canvas, theme),
        Place::Scriptorium => writing_desk(canvas),
        Place::Build => blueprint(canvas),
        Place::Chest => planks(canvas),
        Place::Lamps => lantern_night(canvas),
        Place::Inventory => canvas_bag(canvas, theme),
        Place::Skills => constellations(canvas),
        Place::Glossary => card_drawers(canvas, theme),
        Place::Map => chart_table(canvas),
        Place::Catalog => endless_hexagons(canvas),
        Place::Library => galleries(canvas, theme),
        Place::Rooms => two_rooms(canvas),
        Place::Clockwork => workings(canvas),
        Place::Reader => open_covers(canvas, theme),
        Place::Ashes => ruins(canvas),
        Place::Dawn => sunrise(canvas),
        Place::Records => ledger_cloth(canvas, theme),
        _ => {}
    }
}

/// A brick wall lit by the fire below.
fn bricks(canvas: &mut Canvas) {
    for row in 0..=HEIGHT / 12 {
        let y = row * 12;
        canvas.fill(0, y, WIDTH, 1, [0, 0, 0, 80]);
        let mut x = if row % 2 == 0 { 0 } else { 15 };
        while x < WIDTH {
            canvas.fill(x, y + 1, 1, 11, [0, 0, 0, 80]);
            canvas.fill(x + 1, y + 1, 27, 1, [255, 150, 90, 16]);
            x += 30;
        }
    }
    canvas.glow(120.0, 300.0, 170.0, [255, 110, 30, 120]);
    canvas.glow(390.0, 296.0, 130.0, [255, 70, 20, 90]);
}

/// The diamond lattice of a tooled cover.
fn tooled_leather(canvas: &mut Canvas) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let a = (x + y).rem_euclid(26);
            let b = (x - y).rem_euclid(26);
            if a == 0 || b == 0 {
                canvas.put(x, y, [226, 166, 96, 24]);
            }
            if a == 0 && b == 0 {
                canvas.fill(x - 1, y, 3, 1, [240, 196, 104, 70]);
                canvas.fill(x, y - 1, 1, 3, [240, 196, 104, 70]);
            }
        }
    }
}

/// Glass tubes and copper pipes.
fn still_room(canvas: &mut Canvas, theme: &Theme) {
    for (i, x) in [30, 74, 150, 322, 398, 442].into_iter().enumerate() {
        let level = 90 + (i as i32 * 47) % 130;
        canvas.fill(x, 0, 1, HEIGHT, [180, 255, 220, 34]);
        canvas.fill(x + 12, 0, 1, HEIGHT, [180, 255, 220, 22]);
        canvas.fill(x + 1, level, 11, HEIGHT - level, [70, 220, 140, 26]);
        canvas.fill(x + 1, level, 11, 1, [190, 255, 210, 80]);
        canvas.fill(x + 2, 0, 1, HEIGHT, [220, 255, 240, 18]);
    }
    for x in [9, 466] {
        canvas.fill(x, 0, 5, HEIGHT, theme.frame);
        canvas.fill(x + 1, 0, 1, HEIGHT, theme.frame_light);
        canvas.fill(x + 4, 0, 1, HEIGHT, theme.frame_dark);
        for y in (24..HEIGHT).step_by(58) {
            canvas.fill(x - 2, y, 9, 5, theme.frame_dark);
            canvas.fill(x - 1, y + 1, 7, 3, theme.frame_light);
        }
    }
    canvas.fill(0, 262, WIDTH, 4, theme.frame);
    canvas.fill(0, 262, WIDTH, 1, theme.frame_light);
    canvas.glow(240.0, 300.0, 200.0, [60, 230, 150, 60]);
}

/// A walnut desk with spilt ink and loose sheets.
fn writing_desk(canvas: &mut Canvas) {
    for y in 0..HEIGHT {
        for x in (0..WIDTH).step_by(3) {
            if super::frames::noise(x / 18, y).is_multiple_of(9) {
                canvas.fill(x, y, 3, 1, [0, 0, 0, 46]);
            }
        }
        if y % 34 == 0 {
            canvas.fill(0, y, WIDTH, 1, [0, 0, 0, 110]);
            canvas.fill(0, y + 1, WIDTH, 1, [255, 214, 160, 22]);
        }
    }
    for (x, y, w, h) in [(-8, 196, 44, 60), (446, 40, 46, 66), (436, 214, 30, 40)] {
        canvas.fill(x, y, w, h, [232, 214, 176, 36]);
        for line in (y + 7..y + h - 4).step_by(6) {
            canvas.fill(x + 5, line, w - 12, 1, [90, 70, 50, 80]);
        }
    }
    for (i, (x, y, r)) in [(18.0, 60.0, 13.0), (462.0, 160.0, 10.0), (36.0, 250.0, 8.0)]
        .into_iter()
        .enumerate()
    {
        let ink = [26, 36, 104, 190];
        canvas.ellipse(x, y, r, r * 0.7, ink);
        for k in 0..9 {
            let n = super::frames::noise(i as i32, k);
            let (dx, dy) = ((n % 41) as f32 - 20.0, ((n >> 8) % 31) as f32 - 15.0);
            canvas.ellipse(x + dx, y + dy, 1.6, 1.4, ink);
        }
    }
}

/// An architect's plan chalked on blue.
fn blueprint(canvas: &mut Canvas) {
    let faint = [190, 220, 255, 22];
    let firm = [190, 220, 255, 48];
    for x in (0..WIDTH).step_by(12) {
        canvas.fill(x, 0, 1, HEIGHT, if x % 60 == 0 { firm } else { faint });
    }
    for y in (0..HEIGHT).step_by(12) {
        canvas.fill(0, y, WIDTH, 1, if y % 60 == 0 { firm } else { faint });
    }
    let chalk = [224, 240, 255, 70];
    for (x, y, r) in [
        (18.0, 214.0, 38.0),
        (462.0, 50.0, 30.0),
        (460.0, 232.0, 18.0),
    ] {
        canvas.ring(x, y, r, r, chalk);
        canvas.line(x as i32 - 46, y as i32, x as i32 + 46, y as i32, faint);
        canvas.line(x as i32, y as i32 - 46, x as i32, y as i32 + 46, faint);
    }
    canvas.line(0, 270, 120, 150, faint);
    canvas.line(480, 270, 372, 162, faint);
}

/// The planks of a strong box.
fn planks(canvas: &mut Canvas) {
    for plank in 0..=WIDTH / 40 {
        let x = plank * 40;
        let tint = if plank % 2 == 0 { 20 } else { 0 };
        canvas.fill(x, 0, 40, HEIGHT, [255, 170, 90, tint]);
        canvas.fill(x, 0, 1, HEIGHT, [0, 0, 0, 150]);
        canvas.fill(x + 1, 0, 1, HEIGHT, [255, 200, 130, 34]);
        for k in 0..7 {
            let n = super::frames::noise(plank, k);
            let (gx, gy) = (x + 5 + (n % 30) as i32, ((n >> 8) % 270) as i32);
            canvas.fill(gx, gy, 1, 18 + (n >> 20 & 31) as i32, [0, 0, 0, 56]);
        }
        for y in [7, 258] {
            canvas.fill(x + 18, y, 4, 4, [30, 28, 34, 220]);
            canvas.fill(x + 18, y, 2, 2, [178, 172, 184, 220]);
        }
    }
    canvas.glow(240.0, 320.0, 190.0, [255, 190, 60, 70]);
}

/// A night hung with lanterns.
fn lantern_night(canvas: &mut Canvas) {
    for (x, y) in [
        (22, 96),
        (50, 150),
        (66, 56),
        (418, 66),
        (436, 170),
        (462, 112),
    ] {
        for link in (0..y).step_by(4) {
            canvas.fill(x, link, 1, 3, [150, 206, 200, 90]);
        }
        canvas.glow(x as f32, y as f32 + 7.0, 34.0, [255, 176, 60, 110]);
        canvas.fill(x - 4, y, 9, 2, [60, 110, 116, 255]);
        canvas.fill(x - 3, y + 2, 7, 10, [255, 194, 80, 235]);
        canvas.fill(x - 1, y + 4, 3, 6, [255, 236, 170, 255]);
        canvas.fill(x - 4, y + 12, 9, 2, [60, 110, 116, 255]);
    }
    canvas.glow(110.0, 300.0, 170.0, [40, 160, 160, 60]);
    canvas.glow(380.0, 300.0, 150.0, [40, 140, 170, 55]);
}

/// Waxed canvas with the two straps of a satchel.
fn canvas_bag(canvas: &mut Canvas, theme: &Theme) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // A basket weave, two threads at a time.
            if (x / 2 + y / 2) % 2 == 0 {
                canvas.put(x, y, [230, 230, 170, 11]);
            } else if (x + 3 * y) % 7 == 0 {
                canvas.put(x, y, [0, 0, 0, 40]);
            }
        }
    }
    for x in [64, 400] {
        canvas.fill(x, 0, 16, HEIGHT, [96, 62, 34, 150]);
        canvas.fill(x, 0, 1, HEIGHT, [20, 10, 4, 200]);
        canvas.fill(x + 15, 0, 1, HEIGHT, [20, 10, 4, 200]);
        for y in (4..HEIGHT).step_by(6) {
            canvas.fill(x + 3, y, 1, 3, [232, 204, 150, 150]);
            canvas.fill(x + 12, y, 1, 3, [232, 204, 150, 150]);
        }
        canvas.fill(x - 3, 256, 22, 10, theme.frame_dark);
        canvas.frame(x - 2, 257, 20, 8, [226, 190, 100, 255]);
        canvas.fill(x + 7, 257, 2, 8, [226, 190, 100, 255]);
    }
}

/// Stars joined into figures.
fn constellations(canvas: &mut Canvas) {
    for i in 0..170 {
        let n = super::frames::noise(i, 51);
        let (x, y) = ((n % 480) as i32, ((n >> 10) % 270) as i32);
        let alpha = 50 + (n >> 22 & 127) as u8;
        canvas.put(x, y, [214, 222, 255, alpha]);
    }
    let figures: [&[(i32, i32)]; 5] = [
        &[(6, 40), (28, 66), (14, 104), (34, 138), (8, 170)],
        &[(450, 30), (472, 62), (446, 90), (470, 118)],
        &[(20, 214), (60, 238), (100, 222), (128, 258)],
        &[(356, 248), (392, 226), (430, 252), (466, 220)],
        &[(150, 14), (186, 6), (214, 18)],
    ];
    for figure in figures {
        for pair in figure.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            canvas.line(a.0, a.1, b.0, b.1, [150, 164, 240, 56]);
        }
        for &(x, y) in figure {
            canvas.glow(x as f32, y as f32, 5.0, [180, 196, 255, 150]);
            canvas.put(x, y, [255, 255, 255, 255]);
        }
    }
    canvas.glow(400.0, 40.0, 130.0, [110, 80, 220, 46]);
    canvas.glow(70.0, 250.0, 150.0, [60, 90, 200, 40]);
}

/// The little drawers of a card catalogue, each with its brass holder.
fn card_drawers(canvas: &mut Canvas, theme: &Theme) {
    for row in 0..=HEIGHT / 30 {
        for column in 0..=WIDTH / 48 {
            let (x, y) = (column * 48, row * 30);
            canvas.frame(x + 1, y + 1, 46, 28, [0, 0, 0, 120]);
            canvas.fill(x + 2, y + 2, 44, 1, [255, 200, 180, 26]);
            let brass = with_alpha(theme.frame, 130);
            canvas.frame(x + 14, y + 8, 20, 8, brass);
            canvas.fill(x + 16, y + 10, 16, 4, [240, 224, 190, 40]);
            canvas.fill(x + 20, y + 20, 8, 3, brass);
        }
    }
    canvas.glow(240.0, -40.0, 220.0, [255, 190, 110, 40]);
}

/// A chart laid on the table: rhumb lines and a wind rose.
fn chart_table(canvas: &mut Canvas) {
    let line = [214, 184, 128, 38];
    for (ox, oy) in [(44, 224), (440, 40)] {
        for step in 0..16 {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            canvas.line(
                ox,
                oy,
                ox + (c * 520.0) as i32,
                oy + (s * 520.0) as i32,
                line,
            );
        }
        let ink = [228, 204, 152, 110];
        canvas.ring(ox as f32, oy as f32, 22.0, 22.0, ink);
        canvas.ring(ox as f32, oy as f32, 28.0, 28.0, ink);
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            for k in 0..26 {
                let half = (26 - k) / 7;
                for side in -half..=half {
                    canvas.put(ox + dx * k + dy * side, oy + dy * k + dx * side, ink);
                }
            }
        }
    }
    // Small waves.
    for i in 0..60 {
        let n = super::frames::noise(i, 77);
        let (x, y) = ((n % 470) as i32, ((n >> 10) % 262) as i32);
        let sea = [150, 214, 220, 52];
        canvas.fill(x, y + 1, 3, 1, sea);
        canvas.fill(x + 3, y, 3, 1, sea);
        canvas.fill(x + 6, y + 1, 3, 1, sea);
    }
}

/// The outline of a hexagon with flat top and bottom.
pub(super) fn hex_outline(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, turn: f32, c: Rgba) {
    let corner = |i: i32| {
        let angle = turn + i as f32 / 6.0 * std::f32::consts::TAU;
        let (s, k) = angle.sin_cos();
        ((cx + k * r).round() as i32, (cy + s * r).round() as i32)
    };
    for i in 0..6 {
        let (a, b) = (corner(i), corner(i + 1));
        canvas.line(a.0, a.1, b.0, b.1, c);
    }
}

/// Galleries without end.
fn endless_hexagons(canvas: &mut Canvas) {
    for column in -1..=WIDTH / 33 {
        let drop = if column % 2 == 0 { 0 } else { 19 };
        for row in -1..=HEIGHT / 38 {
            let (x, y) = (column * 33 + 12, row * 38 + drop);
            hex_outline(canvas, x as f32, y as f32, 22.0, 0.0, [196, 150, 255, 30]);
        }
    }
    canvas.glow(240.0, 135.0, 230.0, [140, 70, 230, 60]);
    canvas.glow(240.0, 300.0, 130.0, [255, 200, 90, 50]);
}

/// A lattice of galleries, with shelves of books down both sides and gold
/// filigree in the corners.
fn galleries(canvas: &mut Canvas, theme: &Theme) {
    for column in 0..=WIDTH / 42 {
        let drop = if column % 2 == 0 { 0 } else { 24 };
        for row in -1..=HEIGHT / 48 {
            let (x, y) = (column * 42 + 6, row * 48 + drop);
            hex_outline(canvas, x as f32, y as f32, 27.0, 0.0, [170, 150, 255, 22]);
        }
    }
    const SPINES: [Rgba; 6] = [
        [170, 62, 84, 235],
        [58, 120, 140, 235],
        [176, 134, 54, 235],
        [94, 72, 168, 235],
        [70, 128, 82, 235],
        [150, 92, 52, 235],
    ];
    for (side, left) in [0, WIDTH - 18].into_iter().enumerate() {
        canvas.fill(left, 0, 18, HEIGHT, [12, 8, 28, 150]);
        for shelf in 0..6 {
            let base = shelf * 46 + 40;
            let mut x = left + 1;
            for book in 0..6 {
                let n = super::frames::noise(side as i32 * 40 + shelf, book);
                let w = 2 + (n % 3) as i32;
                if x + w > left + 17 {
                    break;
                }
                let h = 22 + (n >> 8 & 15) as i32;
                let color = SPINES[(n >> 16) as usize % SPINES.len()];
                canvas.fill(x, base - h, w, h, color);
                canvas.fill(x, base - h, 1, h, [255, 240, 220, 60]);
                canvas.fill(x, base - h + 4, w, 1, [255, 214, 110, 190]);
                x += w + 1;
            }
            canvas.fill(left, base, 18, 3, [112, 76, 52, 235]);
            canvas.fill(left, base, 18, 1, [200, 150, 96, 235]);
        }
        let rail = if left == 0 { 18 } else { left - 1 };
        canvas.fill(rail, 0, 1, HEIGHT, with_alpha(theme.accent, 110));
    }
    for (x, sx) in [(23, 1), (WIDTH - 24, -1)] {
        for (y, sy) in [(5, 1), (HEIGHT - 6, -1)] {
            filigree(canvas, x, y, sx, sy, with_alpha(theme.accent, 170));
        }
    }
}

/// A curl of gold in a corner; `sx` and `sy` point inwards.
fn filigree(canvas: &mut Canvas, x: i32, y: i32, sx: i32, sy: i32, color: Rgba) {
    for k in 0..30 {
        canvas.put(x + k * sx, y, color);
        canvas.put(x, y + k * sy, color);
    }
    for (dx, dy) in [
        (4, 3),
        (5, 3),
        (6, 4),
        (7, 5),
        (7, 6),
        (6, 7),
        (5, 7),
        (4, 6),
        (5, 5),
        (3, 4),
        (3, 5),
        (3, 6),
        (9, 3),
        (11, 3),
        (3, 9),
        (3, 11),
    ] {
        canvas.put(x + dx * sx, y + dy * sy, color);
    }
    super::frames::diamond(canvas, x + 32 * sx, y, 2, color);
    super::frames::diamond(canvas, x, y + 32 * sy, 2, color);
}

/// Two galleries that share a doorway, one for each player.
fn two_rooms(canvas: &mut Canvas) {
    for r in [86.0, 96.0, 108.0] {
        hex_outline(canvas, 52.0, 206.0, r, 0.0, [90, 220, 210, 50]);
        hex_outline(canvas, 428.0, 66.0, r, 0.0, [255, 130, 176, 50]);
    }
    for r in [24.0, 32.0] {
        hex_outline(canvas, 424.0, 232.0, r, 0.0, [90, 220, 210, 60]);
        hex_outline(canvas, 456.0, 232.0, r, 0.0, [255, 130, 176, 60]);
        hex_outline(canvas, 22.0, 40.0, r, 0.0, [255, 130, 176, 60]);
        hex_outline(canvas, 54.0, 40.0, r, 0.0, [90, 220, 210, 60]);
    }
    // A thread from one to the other.
    for i in 0..60 {
        let x = i * 8;
        let y = 252 - i * 4 + ((i as f32 * 0.5).sin() * 5.0) as i32;
        canvas.fill(x, y, 3, 1, [200, 240, 255, 48]);
    }
    canvas.glow(40.0, 270.0, 150.0, [40, 200, 190, 56]);
    canvas.glow(440.0, 0.0, 150.0, [240, 90, 150, 56]);
}

/// A toothed wheel; `turn` is in radians.
pub(super) fn cog(canvas: &mut Canvas, cx: f32, cy: f32, r: f32, turn: f32, color: Rgba) {
    let teeth = (r / 2.6).max(7.0) as i32;
    canvas.ring(cx, cy, r, r, color);
    canvas.ring(cx, cy, r - 1.0, r - 1.0, color);
    canvas.ring(cx, cy, r * 0.28, r * 0.28, color);
    for k in 0..teeth {
        let angle = turn + k as f32 / teeth as f32 * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        let (x, y) = (cx + c * (r + 2.0), cy + s * (r + 2.0));
        canvas.fill(x.round() as i32 - 1, y.round() as i32 - 1, 3, 3, color);
    }
    for k in 0..5 {
        let angle = turn + k as f32 / 5.0 * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        canvas.line(
            (cx + c * r * 0.28) as i32,
            (cy + s * r * 0.28) as i32,
            (cx + c * (r - 2.0)) as i32,
            (cy + s * (r - 2.0)) as i32,
            color,
        );
    }
}

/// The back of a great clock.
fn workings(canvas: &mut Canvas) {
    for y in (0..HEIGHT).step_by(9) {
        canvas.fill(0, y, WIDTH, 1, [255, 220, 160, 8]);
    }
    for (x, y, r) in [
        (96.0, 8.0, 30.0),
        (6.0, 128.0, 22.0),
        (392.0, 266.0, 40.0),
        (474.0, 150.0, 26.0),
        (404.0, 6.0, 18.0),
        (78.0, 262.0, 20.0),
    ] {
        cog(canvas, x, y, r, 0.2, [196, 158, 76, 54]);
    }
    // The marks of a dial.
    for i in 0..60 {
        let angle = i as f32 / 60.0 * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        let inner = if i % 5 == 0 { 146.0 } else { 152.0 };
        canvas.line(
            (240.0 + c * inner * 1.5) as i32,
            (135.0 + s * inner) as i32,
            (240.0 + c * 158.0 * 1.5) as i32,
            (135.0 + s * 158.0) as i32,
            [226, 196, 120, 46],
        );
    }
    canvas.glow(240.0, 300.0, 190.0, [220, 170, 80, 46]);
}

/// The covers and page edges of a book lying open.
fn open_covers(canvas: &mut Canvas, theme: &Theme) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if super::frames::noise(x / 2, y / 2).is_multiple_of(11) {
                canvas.put(x, y, [0, 0, 0, 60]);
            }
        }
    }
    for (x, sx) in [(3, 1), (WIDTH - 4, -1)] {
        canvas.fill(x, 0, 1, HEIGHT, theme.frame_light);
        for y in (4..HEIGHT).step_by(7) {
            canvas.fill(x + 2 * sx, y, 1, 4, with_alpha(theme.accent, 150));
        }
        // The edges of the pages under the open one.
        for k in 0..4 {
            let at = x + (6 + k * 2) * sx;
            canvas.fill(at, 18, 1, 236, [226, 208, 170, 150 - k as u8 * 28]);
        }
    }
    canvas.glow(240.0, 135.0, 260.0, [255, 190, 120, 26]);
}

/// Cracks, and what is left of fallen shelves.
fn ruins(canvas: &mut Canvas) {
    for i in 0..9 {
        let mut x = (super::frames::noise(i, 3) % 480) as i32;
        let mut y = if i % 2 == 0 { 0 } else { 269 };
        let way = if i % 2 == 0 { 1 } else { -1 };
        for step in 0..9 {
            let n = super::frames::noise(i, step + 20);
            let (nx, ny) = (x + (n % 17) as i32 - 8, y + way * (3 + (n >> 8 & 7) as i32));
            canvas.line(x, y, nx, ny, [200, 190, 220, 80 - step as u8 * 7]);
            (x, y) = (nx, ny);
        }
    }
    // A heap of books at the bottom.
    for i in 0..70 {
        let n = super::frames::noise(i, 91);
        let x = (n % 470) as i32;
        let edge = (x - 240).abs() / 14;
        let y = HEIGHT - 3 - ((n >> 10) % (4 + edge as u32)) as i32;
        let w = 7 + (n >> 20 & 7) as i32;
        canvas.fill(x, y, w, 3, [92, 84, 108, 200]);
        canvas.fill(x, y, w, 1, [170, 160, 190, 170]);
    }
    canvas.glow(240.0, -30.0, 190.0, [140, 100, 200, 40]);
}

/// The sun coming up behind the shelves.
fn sunrise(canvas: &mut Canvas) {
    let (ox, oy) = (240.0_f32, 292.0_f32);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let angle = (y as f32 - oy).atan2(x as f32 - ox);
            if (angle * 16.0 / std::f32::consts::PI).rem_euclid(2.0) < 0.8 {
                let far = ((x as f32 - ox).hypot(y as f32 - oy) / 330.0).min(1.0);
                canvas.put(x, y, [255, 200, 110, (52.0 * (1.0 - far)) as u8]);
            }
        }
    }
    canvas.glow(ox, oy, 150.0, [255, 190, 90, 150]);
    // The skyline of the stacks.
    for column in 0..WIDTH / 12 {
        let n = super::frames::noise(column, 13);
        let edge = (column * 12 - 234).abs() / 9;
        let h = 8 + (n % 16) as i32 + edge;
        canvas.fill(column * 12, HEIGHT - h, 11, h, [24, 10, 30, 235]);
        canvas.fill(column * 12, HEIGHT - h, 11, 1, [255, 190, 110, 150]);
    }
}

/// The green cloth of a record book, ruled and with gilt corners.
fn ledger_cloth(canvas: &mut Canvas, theme: &Theme) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if (x + y) % 3 == 0 && super::frames::noise(x / 3, y / 3).is_multiple_of(3) {
                canvas.put(x, y, [0, 0, 0, 44]);
            }
        }
        if y % 11 == 10 {
            canvas.fill(0, y, WIDTH, 1, [190, 255, 214, 22]);
        }
    }
    for x in [3, 5, WIDTH - 6, WIDTH - 4] {
        canvas.fill(x, 0, 1, HEIGHT, [214, 82, 82, 110]);
    }
    let gilt = with_alpha(theme.frame_light, 190);
    for (x, sx) in [(0, 1), (WIDTH - 1, -1)] {
        for (y, sy) in [(0, 1), (HEIGHT - 1, -1)] {
            for dy in 0..22 {
                for dx in 0..22 - dy {
                    let edge = dx + dy > 17;
                    let color = if edge { theme.accent } else { gilt };
                    canvas.put(x + dx * sx, y + dy * sy, color);
                }
            }
        }
    }
    canvas.glow(240.0, 135.0, 240.0, [120, 220, 150, 22]);
}
