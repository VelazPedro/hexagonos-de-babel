//! The GPU's share of a frame done on the processor, for PCs whose graphics
//! cannot run the normal renderer. It mirrors `light.wgsl`, `composite.wgsl`
//! and the upscale of `pipeline.rs`: change them together.

use super::canvas::Canvas;
use super::pipeline::{Frame, LightGeometry, Placement};
use super::scene::{SCENE_H, SCENE_W, VIEW_H, VIEW_W};

const W: usize = SCENE_W as usize;
const H: usize = SCENE_H as usize;
/// Colour the shadows lean toward instead of black.
const SHADOW_TINT: [f32; 3] = [0.0045, 0.0022, 0.0125];
const BLOOM_TAPS: [(i32, i32); 12] = [
    (2, 0),
    (-2, 0),
    (0, 2),
    (0, -2),
    (3, 3),
    (-3, 3),
    (3, -3),
    (-3, -3),
    (5, 0),
    (-5, 0),
    (0, 5),
    (0, -5),
];
/// The colour around the picture, as `0x00RRGGBB`.
const LETTERBOX: u32 = 0x0010_091a;
const FALLOFF_STEPS: usize = 1024;
const ENCODE_STEPS: usize = 8192;

type Light = [f32; 3];

/// Precomputed curves.
struct Tables {
    /// sRGB byte to linear light.
    linear: [f32; 256],
    /// Linear light in `0..=1` to an sRGB byte.
    srgb: Vec<u8>,
    /// Brightness by distance from a light, as a fraction of its radius.
    falloff: Vec<f32>,
}

impl Tables {
    fn new() -> Tables {
        let mut linear = [0.0; 256];
        for (byte, value) in linear.iter_mut().enumerate() {
            let c = byte as f32 / 255.0;
            *value = if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        let srgb = (0..ENCODE_STEPS)
            .map(|i| {
                let c = i as f32 / (ENCODE_STEPS - 1) as f32;
                let encoded = if c <= 0.003_130_8 {
                    c * 12.92
                } else {
                    1.055 * c.powf(1.0 / 2.4) - 0.055
                };
                (encoded * 255.0).round() as u8
            })
            .collect();
        let falloff = (0..FALLOFF_STEPS)
            .map(|i| {
                let reach = 1.0 - i as f32 / (FALLOFF_STEPS - 1) as f32;
                let smooth = reach * reach * (3.0 - 2.0 * reach);
                let banded = (smooth * 24.0 + 0.5).floor() / 24.0;
                smooth + (banded - smooth) * 0.35
            })
            .collect();
        Tables {
            linear,
            srgb,
            falloff,
        }
    }

    fn colour(&self, rgb: &[u8]) -> Light {
        [rgb[0], rgb[1], rgb[2]].map(|c| self.linear[usize::from(c)])
    }

    /// Linear light to a `0x00RRGGBB` pixel.
    fn pack(&self, colour: Light) -> u32 {
        let [r, g, b] = colour.map(|c| {
            let step = (c.clamp(0.0, 1.0) * (ENCODE_STEPS - 1) as f32 + 0.5) as usize;
            u32::from(self.srgb[step.min(ENCODE_STEPS - 1)])
        });
        r << 16 | g << 8 | b
    }
}

/// A window-sized picture, one `0x00RRGGBB` per pixel, rows from the top.
#[derive(Default)]
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<u32>,
}

/// Lights, composes and enlarges frames. It keeps its buffers between frames.
pub struct Compositor {
    tables: Tables,
    light: Vec<Light>,
    scratch: Vec<Light>,
    bloom: Vec<Light>,
    /// The lit world, still at its own resolution.
    world: Vec<u32>,
    crossings: Vec<f32>,
}

impl Default for Compositor {
    fn default() -> Self {
        Compositor {
            tables: Tables::new(),
            light: vec![[0.0; 3]; W * H],
            scratch: vec![[0.0; 3]; W * H],
            bloom: vec![[0.0; 3]; W * H],
            world: vec![0; W * H],
            crossings: Vec::new(),
        }
    }
}

impl Compositor {
    /// The light map: every light adds its fading colour inside the polygon it
    /// can see.
    fn rasterise(&mut self, lights: &LightGeometry) {
        self.light.fill([0.0; 3]);
        for fan in lights.fans() {
            // From mesh coordinates (centred, y up) to pixels.
            let pixel = |p: &[f32; 3]| (p[0] + W as f32 / 2.0, H as f32 / 2.0 - p[1]);
            let (cx, cy) = pixel(fan.centre);
            let radius = fan.radius.max(1.0);
            let (mut top, mut bottom) = (f32::MAX, f32::MIN);
            for p in fan.ring {
                let y = pixel(p).1;
                top = top.min(y);
                bottom = bottom.max(y);
            }
            let top = top.max(cy - radius).max(0.0) as usize;
            let bottom = bottom.min(cy + radius).ceil().clamp(0.0, H as f32) as usize;
            for row in top..bottom {
                let y = row as f32 + 0.5;
                self.crossings.clear();
                let mut previous = fan.ring.last().map(pixel);
                for p in fan.ring {
                    let b = pixel(p);
                    if let Some(a) = previous
                        && (a.1 <= y) != (b.1 <= y)
                    {
                        self.crossings
                            .push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
                    }
                    previous = Some(b);
                }
                self.crossings.sort_unstable_by(f32::total_cmp);
                let dy = y - cy;
                // Nothing is lit beyond the radius.
                let half = (radius * radius - dy * dy).max(0.0).sqrt();
                for span in self.crossings.as_chunks::<2>().0 {
                    let from = (span[0].max(cx - half) - 0.5).ceil().max(0.0) as usize;
                    let to = (span[1].min(cx + half) - 0.5).ceil().clamp(0.0, W as f32) as usize;
                    if to <= from {
                        continue;
                    }
                    let pixels = &mut self.light[row * W + from..row * W + to];
                    for (column, light) in (from..).zip(pixels) {
                        let dx = column as f32 + 0.5 - cx;
                        let distance = (dx * dx + dy * dy).sqrt() / radius;
                        if distance >= 1.0 {
                            continue;
                        }
                        let step = (distance * (FALLOFF_STEPS - 1) as f32) as usize;
                        let strength = self.tables.falloff[step];
                        for (sum, colour) in light.iter_mut().zip(fan.colour) {
                            *sum += colour * strength;
                        }
                    }
                }
            }
        }
    }

    /// The 9-tap tent blur of the shader, as two passes of three taps each.
    fn soften(&mut self) {
        fn tap(a: Light, centre: Light, b: Light) -> Light {
            std::array::from_fn(|i| (a[i] + centre[i] * 2.0 + b[i]) * 0.25)
        }
        for y in 0..H {
            let row = &self.light[y * W..(y + 1) * W];
            for (x, out) in self.scratch[y * W..(y + 1) * W].iter_mut().enumerate() {
                *out = tap(row[x.saturating_sub(2)], row[x], row[(x + 2).min(W - 1)]);
            }
        }
        for y in 0..H {
            let (above, below) = (y.saturating_sub(2), (y + 2).min(H - 1));
            for x in 0..W {
                self.light[y * W + x] = tap(
                    self.scratch[above * W + x],
                    self.scratch[y * W + x],
                    self.scratch[below * W + x],
                );
            }
        }
    }

    /// Glow around bright things. Few pixels glow, so each one hands its light
    /// to the neighbours that would have sampled it.
    fn gather_bloom(&mut self, emissive: &Canvas) {
        self.bloom.fill([0.0; 3]);
        for (i, pixel) in emissive.px.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let share = f32::from(pixel[3]) / 255.0 / BLOOM_TAPS.len() as f32;
            let glow = self.tables.colour(pixel);
            let (x, y) = ((i % W) as i32, (i / W) as i32);
            for (dx, dy) in BLOOM_TAPS {
                let (tx, ty) = (x - dx, y - dy);
                if (0..SCENE_W).contains(&tx) && (0..SCENE_H).contains(&ty) {
                    let sum = &mut self.bloom[ty as usize * W + tx as usize];
                    for (total, colour) in sum.iter_mut().zip(glow) {
                        *total += colour * share;
                    }
                }
            }
        }
    }

    /// `scene * (ambient + light) + emissive + bloom`, then vignette and flash.
    fn compose(&mut self, frame: &Frame) {
        self.rasterise(&frame.lights);
        self.soften();
        self.gather_bloom(&frame.emissive);
        let settings = &frame.settings;
        let ambient = settings.ambient.truncate().to_array();
        let strength = settings.ambient.w;
        let flash = settings.flash.truncate().to_array();
        let amount = settings.flash.w;
        let edge = settings.params.z;
        let scene = frame.scene.px.as_chunks::<4>().0;
        let emissive = frame.emissive.px.as_chunks::<4>().0;
        for y in 0..H {
            let v = (y as f32 + 0.5) / H as f32 - 0.5;
            for x in 0..W {
                let i = y * W + x;
                let u = (x as f32 + 0.5) / W as f32 - 0.5;
                let base = self.tables.colour(&scene[i]);
                let light: Light =
                    std::array::from_fn(|c| (ambient[c] + self.light[i][c]).min(1.2));
                let darkness = (1.0 - light[0].max(light[1]).max(light[2])).clamp(0.0, 1.0);
                let mut colour: Light =
                    std::array::from_fn(|c| base[c] * light[c] + SHADOW_TINT[c] * darkness);
                let glow = emissive[i];
                if glow[3] != 0 {
                    let alpha = f32::from(glow[3]) / 255.0;
                    let own = self.tables.colour(&glow);
                    colour = std::array::from_fn(|c| colour[c] + (own[c] - colour[c]) * alpha);
                }
                let vignette = 1.0 - (u * u + v * v) * edge;
                colour = std::array::from_fn(|c| {
                    let lit = (colour[c] + self.bloom[i][c] * strength) * vignette;
                    lit + (flash[c] - lit) * amount
                });
                self.world[i] = self.tables.pack(colour);
            }
        }
    }

    /// An interface pixel over a world pixel, blended in linear light.
    fn over(&self, ui: [u8; 4], under: u32) -> u32 {
        let top = self.tables.colour(&ui);
        let below = [(under >> 16) as u8, (under >> 8) as u8, under as u8];
        let below = self.tables.colour(&below);
        let alpha = f32::from(ui[3]) / 255.0;
        self.tables.pack(std::array::from_fn(|c| {
            below[c] + (top[c] - below[c]) * alpha
        }))
    }

    /// Draws the whole window: the lit world enlarged by whole pixels, with
    /// the interface on top and a border around.
    pub fn present(&mut self, frame: &Frame, place: Placement, picture: &mut Picture) {
        self.compose(frame);
        let (w, h) = (picture.w, picture.h);
        picture.pixels.clear();
        picture.pixels.resize(w * h, LETTERBOX);
        let scale = place.scale.max(1.0);
        // Which view pixel each window pixel shows; `shift` scrolls the world.
        let source = |at: usize, start: f32, shift: f32, limit: i32| {
            let inside = (at as f32 + 0.5 - start) / scale;
            if inside >= 0.0 && inside < limit as f32 {
                Some((inside as usize, (inside + shift) as usize))
            } else {
                None
            }
        };
        // The world scrolls in steps of whole window pixels.
        let scroll = |fraction: f32| (fraction * scale).floor() / scale;
        let (sx, sy) = (scroll(frame.fraction.x), scroll(frame.fraction.y));
        let columns: Vec<_> = (0..w)
            .filter_map(|x| Some((x, source(x, place.origin.x, sx, VIEW_W)?)))
            .collect();
        let ui = frame.ui.px.as_chunks::<4>().0;
        let mut previous: Option<((usize, usize), usize)> = None;
        for y in 0..h {
            let Some(rows) = source(y, place.origin.y, sy, VIEW_H) else {
                continue;
            };
            // Enlarged rows repeat, so most are plain copies.
            if let Some((same, at)) = previous
                && same == rows
            {
                picture.pixels.copy_within(at * w..(at + 1) * w, y * w);
                continue;
            }
            previous = Some((rows, y));
            let (ui_row, world_row) = rows;
            let world = &self.world[world_row.min(H - 1) * W..][..W];
            let interface = &ui[ui_row * VIEW_W as usize..][..VIEW_W as usize];
            for &(x, (ui_column, world_column)) in &columns {
                let under = world[world_column.min(W - 1)];
                let top = interface[ui_column];
                picture.pixels[y * w + x] = match top[3] {
                    0 => under,
                    255 => u32::from(top[0]) << 16 | u32::from(top[1]) << 8 | u32::from(top[2]),
                    _ => self.over(top, under),
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::game::light::Light as Lamp;
    use babel_core::game::vec::v2;
    use babel_core::world::grid::Grid;
    use bevy::math::{Vec2, Vec4};

    /// A white world under a dim grey ambient light, with no vignette.
    fn dim_white() -> Frame {
        let mut frame = Frame::default();
        frame.scene.clear([255; 4]);
        frame.settings.ambient = Vec4::new(0.2, 0.2, 0.2, 0.0);
        frame
    }

    fn whole_view(scale: f32) -> (Placement, Picture) {
        let (w, h) = (
            VIEW_W as usize * scale as usize,
            VIEW_H as usize * scale as usize,
        );
        let place = Placement {
            scale,
            origin: Vec2::ZERO,
        };
        (
            place,
            Picture {
                w,
                h,
                pixels: Vec::new(),
            },
        )
    }

    fn show(frame: &Frame, place: Placement, picture: &mut Picture) {
        Compositor::default().present(frame, place, picture);
    }

    fn red(pixel: u32) -> u32 {
        pixel >> 16
    }

    #[test]
    fn without_lights_only_the_ambient_light_remains() {
        let (place, mut picture) = whole_view(1.0);
        show(&dim_white(), place, &mut picture);
        // Linear 0.2, plus the violet of the shadows, in sRGB.
        let expected = Tables::new().pack([0.2 + 0.0045 * 0.8, 0.2 + 0.0022 * 0.8, 0.21]);
        assert_eq!(picture.pixels[100 * picture.w + 100], expected);
    }

    #[test]
    fn a_light_is_brightest_at_its_centre_and_ends_at_its_radius() {
        let mut frame = dim_white();
        let grid = Grid::hexagon();
        // In the middle of the open gallery, and of the screen.
        let lamp = Lamp {
            pos: v2(320.0, 272.0),
            radius: 60.0,
            color: [0.7, 0.7, 0.7],
            flicker: 0.0,
            reveals: true,
            priority: 1,
        };
        frame.lights.build(&grid, vec![lamp], (79, 136), 0.0);
        let (place, mut picture) = whole_view(1.0);
        show(&frame, place, &mut picture);
        let at = |x: usize| red(picture.pixels[136 * VIEW_W as usize + x]);
        let dark = at(20);
        assert!(
            at(241) > at(270) && at(270) > dark,
            "{} {}",
            at(241),
            at(270)
        );
        assert_eq!(at(310), dark, "beyond the radius");
    }

    #[test]
    fn what_glows_ignores_the_darkness() {
        let mut frame = dim_white();
        frame.emissive.set(100, 100, [255, 0, 0, 255]);
        let (place, mut picture) = whole_view(1.0);
        show(&frame, place, &mut picture);
        assert_eq!(picture.pixels[100 * picture.w + 100] >> 16, 255);
    }

    #[test]
    fn the_interface_is_enlarged_by_whole_pixels_inside_a_border() {
        let mut frame = dim_white();
        frame.ui.set(10, 20, [1, 2, 3, 255]);
        let place = Placement {
            scale: 2.0,
            origin: Vec2::new(6.0, 4.0),
        };
        let mut picture = Picture {
            w: VIEW_W as usize * 2 + 12,
            h: VIEW_H as usize * 2 + 8,
            pixels: Vec::new(),
        };
        show(&frame, place, &mut picture);
        let at = |x: usize, y: usize| picture.pixels[y * picture.w + x];
        for (x, y) in [(26, 44), (27, 44), (26, 45), (27, 45)] {
            assert_eq!(at(x, y), 0x01_0203, "{x},{y}");
        }
        assert_ne!(at(28, 44), 0x01_0203);
        assert_eq!(at(0, 0), LETTERBOX);
        assert_eq!(at(picture.w - 1, picture.h - 1), LETTERBOX);
    }

    #[test]
    fn a_window_smaller_than_the_view_is_clipped() {
        let place = Placement {
            scale: 1.0,
            origin: Vec2::new(-90.0, -50.0),
        };
        let mut picture = Picture {
            w: 300,
            h: 170,
            pixels: Vec::new(),
        };
        show(&dim_white(), place, &mut picture);
        assert_eq!(picture.pixels.len(), 300 * 170);
        assert!(picture.pixels.iter().all(|&p| p != LETTERBOX));
    }
}
