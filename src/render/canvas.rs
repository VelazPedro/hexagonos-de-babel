//! A small software framebuffer. Everything pixel-art is composed here and
//! uploaded once per frame, so there is no atlas bleeding, no seams and
//! y-sorting is simply draw order.

use super::atlas::{Atlas, Frame};
use super::font::{Font, LINE_HEIGHT, decompose, mark_pixels};

pub type Rgba = [u8; 4];

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r, g, b, 255]
}

pub fn with_alpha(c: Rgba, a: u8) -> Rgba {
    [c[0], c[1], c[2], a]
}

pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
}

pub fn shade(c: Rgba, factor: f32) -> Rgba {
    let f = |x: u8| (f32::from(x) * factor).clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2]), c[3]]
}

/// `#rrggbb` to a colour; magenta marks a typo in the data.
pub fn hex(text: &str) -> Rgba {
    let t = text.trim_start_matches('#');
    let channel = |i: usize| t.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    match (channel(0), channel(2), channel(4)) {
        (Some(r), Some(g), Some(b)) => rgb(r, g, b),
        _ => rgb(255, 0, 255),
    }
}

/// How a sprite frame is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Blit {
    pub flip: bool,
    pub alpha: u8,
    /// Blend toward a colour (modifier tints).
    pub tint: Option<(Rgba, f32)>,
    /// Replace every opaque pixel (hit flash, silhouettes).
    pub solid: Option<Rgba>,
    /// Palette rotation used by word variants.
    pub variant: u32,
    pub scale: f32,
    /// Shear of the top rows in pixels, for swaying plants and lamps.
    pub sway: i32,
}

impl Default for Blit {
    fn default() -> Self {
        Blit {
            flip: false,
            alpha: 255,
            tint: None,
            solid: None,
            variant: 0,
            scale: 1.0,
            sway: 0,
        }
    }
}

fn rotate_hue(c: Rgba, variant: u32) -> Rgba {
    match variant % 6 {
        1 => [c[2], c[0], c[1], c[3]],
        2 => [c[1], c[2], c[0], c[3]],
        3 => [c[0], c[2], c[1], c[3]],
        4 => [c[2], c[1], c[0], c[3]],
        5 => [c[1], c[0], c[2], c[3]],
        _ => c,
    }
}

#[derive(Clone)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Canvas {
        Canvas {
            w,
            h,
            px: vec![0; (w.max(0) * h.max(0) * 4) as usize],
        }
    }

    pub fn clear(&mut self, c: Rgba) {
        for p in self.px.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&c);
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return [0; 4];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    /// Overwrites a pixel without blending.
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        self.px[i..i + 4].copy_from_slice(&c);
    }

    /// Source-over blend.
    pub fn put(&mut self, x: i32, y: i32, c: Rgba) {
        if c[3] == 0 || x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        if c[3] == 255 {
            self.px[i..i + 4].copy_from_slice(&c);
            return;
        }
        let a = u32::from(c[3]);
        let da = u32::from(self.px[i + 3]);
        let out_a = a + da * (255 - a) / 255;
        if out_a == 0 {
            return;
        }
        for k in 0..3 {
            let src = u32::from(c[k]) * a;
            let dst = u32::from(self.px[i + k]) * da * (255 - a) / 255;
            self.px[i + k] = ((src + dst) / out_a) as u8;
        }
        self.px[i + 3] = out_a as u8;
    }

    /// Blends another canvas over this one, shifted down and faded.
    pub fn composite(&mut self, other: &Canvas, dy: i32, alpha: u8) {
        for y in 0..other.h {
            for x in 0..other.w {
                let mut c = other.get(x, y);
                if c[3] == 0 {
                    continue;
                }
                c[3] = (u16::from(c[3]) * u16::from(alpha) / 255) as u8;
                self.put(x, y + dy, c);
            }
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.put(xx, yy, c);
            }
        }
    }

    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        self.fill(x, y, w, 1, c);
        self.fill(x, y + h - 1, w, 1, c);
        self.fill(x, y + 1, 1, h - 2, c);
        self.fill(x + w - 1, y + 1, 1, h - 2, c);
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = (x0 as f32 + (x1 - x0) as f32 * t).round() as i32;
            let y = (y0 as f32 + (y1 - y0) as f32 * t).round() as i32;
            self.put(x, y, c);
        }
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.put(x, y, c);
                }
            }
        }
    }

    pub fn ring(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        let steps = ((rx + ry) * 3.2).max(8.0) as i32;
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            self.put(
                (cx + a.cos() * rx).floor() as i32,
                (cy + a.sin() * ry).floor() as i32,
                c,
            );
        }
    }

    /// Soft radial dot used for glows.
    pub fn glow(&mut self, cx: f32, cy: f32, radius: f32, c: Rgba) {
        if radius <= 0.0 {
            return;
        }
        for y in (cy - radius).floor() as i32..=(cy + radius).ceil() as i32 {
            for x in (cx - radius).floor() as i32..=(cx + radius).ceil() as i32 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt() / radius;
                if d < 1.0 {
                    let a = (f32::from(c[3]) * (1.0 - d) * (1.0 - d)) as u8;
                    self.put(x, y, with_alpha(c, a));
                }
            }
        }
    }

    /// Draws a frame with its top-left corner at `(x, y)`.
    pub fn sprite(&mut self, atlas: &Atlas, frame: Frame, x: i32, y: i32, opts: Blit) {
        let scale = opts.scale.max(0.05);
        let dw = (frame.w as f32 * scale).round() as i32;
        let dh = (frame.h as f32 * scale).round() as i32;
        if x + dw + opts.sway.abs() <= 0
            || y + dh <= 0
            || x - opts.sway.abs() >= self.w
            || y >= self.h
        {
            return;
        }
        for dy in 0..dh {
            let sy = ((dy as f32 + 0.5) / scale) as i32;
            // Rows higher up lean further.
            let lean = if opts.sway != 0 && dh > 1 {
                (opts.sway as f32 * (1.0 - dy as f32 / (dh - 1) as f32)).round() as i32
            } else {
                0
            };
            for dx in 0..dw {
                let mut sx = ((dx as f32 + 0.5) / scale) as i32;
                if opts.flip {
                    sx = frame.w - 1 - sx;
                }
                let mut c = atlas.pixel(frame, sx, sy);
                if c[3] == 0 {
                    continue;
                }
                if let Some(solid) = opts.solid {
                    c = with_alpha(solid, c[3].min(solid[3]));
                } else if c != atlas.outline {
                    if opts.variant != 0 {
                        c = rotate_hue(c, opts.variant);
                    }
                    if let Some((tint, amount)) = opts.tint {
                        c = with_alpha(mix(c, tint, amount), c[3]);
                    }
                }
                if opts.alpha != 255 {
                    c[3] = (u16::from(c[3]) * u16::from(opts.alpha) / 255) as u8;
                }
                self.put(x + dx + lean, y + dy, c);
            }
        }
    }

    fn glyph_pixels(&mut self, font: &Font, c: char, x: i32, y: i32, color: Rgba) {
        let (base, mark) = decompose(c);
        let Some(glyph) = font.glyph(base).or_else(|| font.glyph('?')) else {
            return;
        };
        for row in 0..8 {
            for col in 0..glyph.width {
                if glyph.ink(col, row) {
                    self.put(x + col, y + row as i32, color);
                }
            }
        }
        if let Some(mark) = mark {
            // Capitals carry their mark above the line.
            let top = if base.is_uppercase() { -2 } else { 0 };
            let center = x + (glyph.width - 1) / 2;
            for &(mx, my) in mark_pixels(mark) {
                self.put(center + mx, y + top + my, color);
            }
        }
    }

    /// Draws text with its top-left at `(x, y)` and returns the end x.
    pub fn text(&mut self, font: &Font, text: &str, x: i32, y: i32, color: Rgba) -> i32 {
        let mut pen = x;
        for c in text.chars() {
            if c != ' ' {
                self.glyph_pixels(font, c, pen, y, color);
            }
            pen += font.advance(c);
        }
        pen
    }

    /// Text with a dark drop outline, legible over any background.
    pub fn text_outlined(&mut self, font: &Font, text: &str, x: i32, y: i32, color: Rgba) -> i32 {
        let shadow = with_alpha(rgb(26, 16, 40), color[3]);
        // A drop shadow keeps counters open; a full outline would fill the
        // one-pixel gaps between letters.
        for (dx, dy) in [(1, 0), (0, 1), (1, 1)] {
            self.text(font, text, x + dx, y + dy, shadow);
        }
        self.text(font, text, x, y, color)
    }

    pub fn text_centered(&mut self, font: &Font, text: &str, cx: i32, y: i32, color: Rgba) {
        let x = cx - font.width(text) / 2;
        self.text_outlined(font, text, x, y, color);
    }

    /// Large title text: every font pixel becomes a `scale`×`scale` block.
    pub fn text_scaled(
        &mut self,
        font: &Font,
        text: &str,
        x: i32,
        y: i32,
        scale: i32,
        color: Rgba,
    ) {
        let w = (font.width(text) + 2).max(1);
        let mut small = Canvas::new(w, LINE_HEIGHT + 4);
        small.text(font, text, 0, 2, color);
        let shadow = rgb(26, 16, 40);
        for pass in 0..2 {
            for sy in 0..small.h {
                for sx in 0..small.w {
                    let p = small.get(sx, sy);
                    if p[3] == 0 {
                        continue;
                    }
                    let (dx, dy) = (x + sx * scale, y + (sy - 2) * scale);
                    if pass == 0 {
                        self.fill(dx - 1, dy - 1, scale + 2, scale + 3, shadow);
                    } else {
                        self.fill(dx, dy, scale, scale, p);
                    }
                }
            }
        }
    }
}
