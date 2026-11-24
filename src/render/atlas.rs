//! Compiles the readable `.ron` pixel art into one atlas at startup, with a
//! 1 px extruded padding around every frame.

use super::canvas::{Canvas, Rgba, hex};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

const PADDING: i32 = 1;
const ATLAS_WIDTH: i32 = 512;
const OUTLINE: &str = "#1a1028";

#[derive(Deserialize)]
struct SpriteDef {
    name: String,
    #[serde(default)]
    outline: bool,
    frames: Vec<Vec<String>>,
}

/// A frame's rectangle inside the atlas.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct Atlas {
    pub image: Canvas,
    sprites: HashMap<String, Vec<Frame>>,
    pub outline: Rgba,
}

/// A decoded frame waiting to be packed.
struct Pending {
    sprite: String,
    index: usize,
    pixels: Canvas,
}

fn decode(rows: &[String], palette: &HashMap<char, Rgba>) -> Result<Canvas, String> {
    let h = rows.len() as i32;
    let w = rows.first().map_or(0, |r| r.chars().count()) as i32;
    let mut out = Canvas::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        if row.chars().count() as i32 != w {
            return Err(format!("row {y} has a different width"));
        }
        for (x, c) in row.chars().enumerate() {
            if c == '.' {
                continue;
            }
            let color = palette
                .get(&c)
                .ok_or_else(|| format!("unknown palette character '{c}'"))?;
            out.set(x as i32, y as i32, *color);
        }
    }
    Ok(out)
}

/// Adds the 1 px dark silhouette outline, growing the frame to fit it.
fn outlined(src: &Canvas, outline: Rgba) -> Canvas {
    let mut out = Canvas::new(src.w + 2, src.h + 2);
    for y in 0..out.h {
        for x in 0..out.w {
            let c = src.get(x - 1, y - 1);
            if c[3] != 0 {
                out.set(x, y, c);
            } else if [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| src.get(x - 1 + dx, y - 1 + dy)[3] != 0)
            {
                out.set(x, y, outline);
            }
        }
    }
    out
}

impl Atlas {
    pub fn load(assets: &Path) -> Result<Atlas, String> {
        let dir = assets.join("sprites");
        let read = |path: &Path| {
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let palette_path = dir.join("palette.ron");
        let raw: HashMap<char, String> = ron::from_str(&read(&palette_path)?)
            .map_err(|e| format!("{}: {e}", palette_path.display()))?;
        let palette: HashMap<char, Rgba> = raw.iter().map(|(&c, v)| (c, hex(v))).collect();

        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension().is_some_and(|x| x == "ron")
                    && p.file_name().is_some_and(|n| n != "palette.ron")
            })
            .collect();
        files.sort();
        let outline = hex(OUTLINE);
        let mut pending = Vec::new();
        for file in files {
            let defs: Vec<SpriteDef> =
                ron::from_str(&read(&file)?).map_err(|e| format!("{}: {e}", file.display()))?;
            for def in defs {
                for (index, rows) in def.frames.iter().enumerate() {
                    let mut pixels = decode(rows, &palette)
                        .map_err(|e| format!("{} {}: {e}", file.display(), def.name))?;
                    if def.outline {
                        pixels = outlined(&pixels, outline);
                    }
                    pending.push(Pending {
                        sprite: def.name.clone(),
                        index,
                        pixels,
                    });
                }
            }
        }
        Ok(Atlas::pack(pending, outline))
    }

    /// Shelf packing, tallest first.
    fn pack(mut pending: Vec<Pending>, outline: Rgba) -> Atlas {
        pending.sort_by(|a, b| {
            b.pixels
                .h
                .cmp(&a.pixels.h)
                .then(a.sprite.cmp(&b.sprite))
                .then(a.index.cmp(&b.index))
        });
        let mut places = Vec::with_capacity(pending.len());
        let (mut x, mut y, mut shelf) = (0, 0, 0);
        for p in &pending {
            let (cw, ch) = (p.pixels.w + PADDING * 2, p.pixels.h + PADDING * 2);
            if x + cw > ATLAS_WIDTH {
                x = 0;
                y += shelf;
                shelf = 0;
            }
            places.push(Frame {
                x: x + PADDING,
                y: y + PADDING,
                w: p.pixels.w,
                h: p.pixels.h,
            });
            x += cw;
            shelf = shelf.max(ch);
        }
        let mut image = Canvas::new(ATLAS_WIDTH, (y + shelf).max(1));
        let mut sprites: HashMap<String, Vec<Frame>> = HashMap::new();
        for (p, f) in pending.iter().zip(places) {
            // Extrude: the padding repeats the nearest edge pixel.
            for dy in -PADDING..f.h + PADDING {
                for dx in -PADDING..f.w + PADDING {
                    let c = p.pixels.get(dx.clamp(0, f.w - 1), dy.clamp(0, f.h - 1));
                    image.set(f.x + dx, f.y + dy, c);
                }
            }
            let frames = sprites.entry(p.sprite.clone()).or_default();
            if frames.len() <= p.index {
                frames.resize(p.index + 1, Frame::default());
            }
            frames[p.index] = f;
        }
        Atlas {
            image,
            sprites,
            outline,
        }
    }

    /// Pixel of a frame; coordinates are clamped so sampling never bleeds.
    pub fn pixel(&self, f: Frame, x: i32, y: i32) -> Rgba {
        self.image.get(
            f.x + x.clamp(0, (f.w - 1).max(0)),
            f.y + y.clamp(0, (f.h - 1).max(0)),
        )
    }

    /// Frame `index` (wrapping) of a sprite; an empty frame if unknown.
    pub fn get(&self, name: &str, index: usize) -> Frame {
        self.sprites
            .get(name)
            .and_then(|f| f.get(index % f.len().max(1)))
            .copied()
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn has(&self, name: &str) -> bool {
        self.sprites.contains_key(name)
    }

    /// Frame for a looping animation at `time` seconds.
    pub fn animate(&self, name: &str, time: f32, fps: f32) -> Frame {
        self.get(name, (time.max(0.0) * fps) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::data::{GameData, dev_assets_dir};

    #[test]
    fn every_referenced_sprite_exists_and_padding_is_extruded() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let atlas = Atlas::load(&assets).expect("atlas");
        let data = GameData::load(&dev_assets_dir()).expect("data");
        for n in &data.items.nodes {
            assert!(atlas.has(&n.sprite), "{}", n.sprite);
        }
        for a in &data.enemies.archetypes {
            assert!(atlas.has(&a.sprite), "{}", a.sprite);
        }
        for g in &data.enemies.guardians {
            assert!(atlas.has(&g.sprite), "{}", g.sprite);
        }
        for b in &data.biomes {
            for d in &b.decor {
                assert!(atlas.has(&format!("decor_{d}")), "decor {d}");
            }
        }
        for dir in ["down", "up", "side"] {
            assert!(atlas.has(&format!("player_walk_{dir}")));
        }
        let f = atlas.get("player_idle_down", 0);
        assert_eq!(atlas.image.get(f.x - 1, f.y), atlas.image.get(f.x, f.y));
        assert_eq!(
            atlas.image.get(f.x + f.w, f.y + 5),
            atlas.image.get(f.x + f.w - 1, f.y + 5)
        );
    }
}
