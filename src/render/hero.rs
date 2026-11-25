//! Builds the player's sprites from layers, following the chosen look and the
//! equipment worn. The result is a small atlas with the animation names the
//! scene draws: `player_{idle,walk}_{dir}`, the same without anything in the
//! hand (`bare_…`, used while it swings through the air) and `held`, that item
//! on its own.

use super::atlas::{Atlas, Frame};
use super::canvas::{Blit, Canvas, Rgba, mix, rgb};
use super::gear::{Armor, Charm, Colors, Footwear, Gear, Held, LampGear, Piece};
use babel_core::progression::look::{self, Look};

/// Size of every body layer.
const W: i32 = 20;
const H: i32 = 28;
/// Room left on each side for what the hand holds.
const MARGIN: i32 = 4;
const FRAME_W: i32 = W + 2 * MARGIN;
/// Held items are drawn upright, this wide, gripped at `GRIP`.
const ITEM_W: i32 = 7;
const GRIP: (i32, i32) = (3, 20);

pub const BODY_NAMES: [&str; look::BODIES as usize] = ["Hombre", "Mujer", "No binario"];
pub const HAIR_NAMES: [&str; look::HAIRS as usize] = [
    "Largo ondulado",
    "Corto",
    "Recogido",
    "Trenza",
    "Rizos",
    "Media melena",
    "Rapado",
];

/// Name, base and shade.
pub const SKINS: [(&str, Rgba, Rgba); look::SKINS as usize] = [
    ("Marfil", rgb(0xf6, 0xd6, 0xbc), rgb(0xd6, 0xa9, 0x8c)),
    ("Claro", rgb(0xee, 0xbd, 0x98), rgb(0xc8, 0x90, 0x6e)),
    ("Trigo", rgb(0xe2, 0xac, 0x7c), rgb(0xb9, 0x80, 0x56)),
    ("Bronce", rgb(0xd9, 0x9a, 0x6c), rgb(0xb0, 0x71, 0x4e)),
    ("Canela", rgb(0xa8, 0x69, 0x3f), rgb(0x84, 0x49, 0x2c)),
    ("Ébano", rgb(0x74, 0x47, 0x2f), rgb(0x55, 0x31, 0x20)),
];

/// A hair colour: name, base, shade and highlight.
pub struct HairColor {
    pub name: &'static str,
    pub base: Rgba,
    shade: Rgba,
    light: Rgba,
}

const fn color(v: u32) -> Rgba {
    rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const fn tress(name: &'static str, c: [u32; 3]) -> HairColor {
    HairColor {
        name,
        base: color(c[0]),
        shade: color(c[1]),
        light: color(c[2]),
    }
}

pub const HAIR_COLORS: [HairColor; look::HAIR_COLORS as usize] = [
    tress("Negro", [0x2b2438, 0x1c1726, 0x4a4262]),
    tress("Moreno", [0x4a2e22, 0x321f18, 0x6e4632]),
    tress("Castaño", [0x7a4a2c, 0x57331f, 0xa2683e]),
    tress("Cobrizo", [0xb45a2a, 0x7c391f, 0xe08038]),
    tress("Rubio", [0xd9ae55, 0xa87f38, 0xf6d888]),
    tress("Blanco", [0xd8d6e6, 0xa29ebc, 0xffffff]),
    tress("Rojo", [0xb0303c, 0x761f30, 0xe0545a]),
    tress("Azul", [0x2c5a9e, 0x1e3c72, 0x4e8cd8]),
    tress("Violeta", [0x6a3e9e, 0x4a2a72, 0x9c6ad2]),
];

pub const EYES: [(&str, Rgba); look::EYE_COLORS as usize] = [
    ("Pardos", rgb(0x5a, 0x34, 0x20)),
    ("Celestes", rgb(0x3c, 0xa8, 0xe8)),
    ("Verdes", rgb(0x2f, 0x86, 0x48)),
    ("Grises", rgb(0x66, 0x6e, 0x8a)),
    ("Ámbar", rgb(0xb4, 0x76, 0x18)),
    ("Violetas", rgb(0x7c, 0x3c, 0xc0)),
];

/// Coat colours: base, shade, light, cape and cape shade.
pub struct Coat {
    pub name: &'static str,
    pub base: Rgba,
    shade: Rgba,
    light: Rgba,
    cape: Rgba,
    cape_shade: Rgba,
}

const fn coat(name: &'static str, c: [u32; 5]) -> Coat {
    Coat {
        name,
        base: color(c[0]),
        shade: color(c[1]),
        light: color(c[2]),
        cape: color(c[3]),
        cape_shade: color(c[4]),
    }
}

pub const COATS: [Coat; look::COATS as usize] = [
    coat(
        "Azul tormenta",
        [0x2f58a8, 0x1f3a74, 0x4d7fd6, 0x27468a, 0x172b58],
    ),
    coat(
        "Carmesí",
        [0xa8333f, 0x742030, 0xd65a58, 0x8a2838, 0x581828],
    ),
    coat(
        "Verde bosque",
        [0x2f7a4e, 0x1f5238, 0x54a86c, 0x266440, 0x163c2a],
    ),
    coat("Noche", [0x3a3452, 0x262038, 0x5c547c, 0x2e2844, 0x1c182c]),
    coat("Marfil", [0xd8ccb4, 0xa89a86, 0xf6eed8, 0xc0b29a, 0x887a68]),
    coat(
        "Púrpura",
        [0x6c3ea0, 0x4a2872, 0x9a66cc, 0x583286, 0x381e5a],
    ),
    coat("Ocre", [0xb47a2c, 0x80521c, 0xdca848, 0x946224, 0x603e16]),
    coat(
        "Turquesa",
        [0x23848c, 0x165a66, 0x44b4b4, 0x1c6c76, 0x10444e],
    ),
];

/// Number of body placeholders, numbered from 1.
const BODY_COLORS: usize = 12;
/// Number of the first of the three placeholders of a piece of gear.
const GEAR_PLACEHOLDER: u8 = 0x10;
const CAPE_TRIM: Rgba = rgb(0xcd, 0xc8, 0xe4);
const WOOD: Rgba = rgb(0xae, 0x6a, 0x3e);
const FLAME: Rgba = rgb(0xff, 0x98, 0x38);
const SPARK: Rgba = rgb(0xff, 0xf8, 0xec);
const ROWEL: Rgba = rgb(0xff, 0xd9, 0x5c);

/// The placeholder colours of `palette.ron`, numbered from 1.
fn placeholder(index: u8) -> Rgba {
    rgb(0xff, 0x01, index)
}

/// What each body placeholder becomes for a look.
fn palette_for(look: Look) -> [Rgba; BODY_COLORS] {
    let (_, skin, skin_shade) = SKINS[usize::from(look.skin)];
    let hair = &HAIR_COLORS[usize::from(look.hair_color)];
    let eyes = EYES[usize::from(look.eyes)].1;
    let c = &COATS[usize::from(look.coat)];
    [
        skin,
        skin_shade,
        hair.base,
        hair.shade,
        hair.light,
        c.base,
        c.shade,
        c.light,
        eyes,
        c.cape,
        c.cape_shade,
        mix(skin_shade, rgb(0xc0, 0x38, 0x5c), 0.55),
    ]
}

/// One animation frame of the body.
#[derive(Clone, Copy)]
struct Pose {
    dir: &'static str,
    legs: usize,
    /// Pixels the body sinks while breathing or walking.
    bob: i32,
    tails: usize,
    cape: usize,
}

/// A frame being painted. Body layers sit `MARGIN` pixels in.
struct Painter<'a> {
    source: &'a Atlas,
    body: [Rgba; BODY_COLORS],
    out: Canvas,
}

impl Painter<'_> {
    /// A colour of the layer art, with its placeholder filled in.
    fn resolve(&self, c: Rgba, piece: Option<Colors>) -> Rgba {
        if c[0] != 0xff || c[1] != 0x01 {
            return c;
        }
        let filled = match c[2].checked_sub(GEAR_PLACEHOLDER) {
            Some(i) => piece.and_then(|colors| colors.get(usize::from(i)).copied()),
            None => usize::from(c[2])
                .checked_sub(1)
                .and_then(|i| self.body.get(i).copied()),
        };
        filled.unwrap_or(c)
    }

    /// Paints a layer; `piece` colours the gear placeholders.
    fn layer(&mut self, name: &str, frame: usize, dy: i32, piece: Option<Colors>) {
        let f = self.source.get(name, frame);
        for y in 0..f.h.min(H) {
            for x in 0..f.w.min(W) {
                let c = self.source.pixel(f, x, y);
                if c[3] != 0 {
                    let c = self.resolve(c, piece);
                    self.out.set(x + MARGIN, y + dy, c);
                }
            }
        }
    }

    /// Paints the cape layer in the colours of a piece of armour.
    fn cloak(&mut self, dir: &str, frame: usize, dy: i32, colors: Colors) {
        let f = self.source.get(&format!("hero_cape_{dir}"), frame);
        let [main, shade, trim] = colors;
        for y in 0..f.h.min(H) {
            for x in 0..f.w.min(W) {
                let c = self.source.pixel(f, x, y);
                if c[3] == 0 {
                    continue;
                }
                let c = if c == placeholder(10) {
                    main
                } else if c == placeholder(11) {
                    shade
                } else if c == CAPE_TRIM {
                    trim
                } else {
                    c
                };
                self.out.set(x + MARGIN, y + dy, c);
            }
        }
    }

    /// A pixel in body coordinates.
    fn dot(&mut self, x: i32, y: i32, c: Rgba) {
        self.out.set(x + MARGIN, y, c);
    }

    /// A small block of rows of characters: `h`, `H`, `i` take the piece's
    /// colours, `m` is wood, `f` flame and `w` a spark.
    fn trinket(&mut self, rows: &[&str], x: i32, y: i32, [main, shade, accent]: Colors) {
        for (dy, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                let c = match ch {
                    'h' => main,
                    'H' => shade,
                    'i' => accent,
                    'm' => WOOD,
                    'f' => FLAME,
                    'w' => SPARK,
                    _ => continue,
                };
                self.dot(x + dx as i32, y + dy as i32, c);
            }
        }
    }

    /// Footwear repaints the bottom of the leg layer, so it follows the step.
    fn footwear(&mut self, pose: &Pose, boots: Piece<Footwear>) {
        let [main, shade, accent] = boots.colors;
        let f = self
            .source
            .get(&format!("hero_legs_{}", pose.dir), pose.legs);
        let solid = |x: i32, y: i32| y >= 0 && self.source.pixel(f, x, y)[3] != 0;
        // Rows from the sole of each foot upwards.
        let height = match boots.shape {
            Footwear::Greaves => 6,
            Footwear::Sandals => 3,
            Footwear::Boots | Footwear::Spurs => 3,
        };
        let mut paint = Vec::new();
        for x in 0..f.w.min(W) {
            let Some(sole) = (0..f.h.min(H)).rev().find(|&y| solid(x, y)) else {
                continue;
            };
            for rise in 0..height {
                let y = sole - rise;
                if !solid(x, y) {
                    continue;
                }
                let c = match (boots.shape, rise) {
                    (Footwear::Sandals, 0) => shade,
                    // Straps over bare skin.
                    (Footwear::Sandals, 2) => main,
                    (Footwear::Sandals, _) => self.body[0],
                    (_, 0) => shade,
                    (_, r) if r == height - 1 => accent,
                    _ => main,
                };
                paint.push((x, y, c));
            }
        }
        if boots.shape == Footwear::Spurs {
            // A golden rowel beside each outer heel.
            let low = paint.iter().map(|p| p.1).max().unwrap_or(0);
            let heels = [
                paint.iter().filter(|p| p.1 == low - 1).map(|p| p.0).min(),
                paint.iter().filter(|p| p.1 == low - 1).map(|p| p.0).max(),
            ];
            if let [Some(left), Some(right)] = heels {
                paint.push((left - 1, low - 1, ROWEL));
                paint.push((right + 1, low - 1, ROWEL));
            }
        }
        for (x, y, c) in paint {
            self.dot(x, y, c);
        }
    }

    /// The lamp hangs from the belt, on the side away from the hand.
    fn lamp(&mut self, pose: &Pose, lamp: Piece<LampGear>) {
        let rows: &[&str] = match lamp.shape {
            LampGear::Lantern => &[".HH.", "hhhh", "hwih", "hiih", "HHHH"],
            LampGear::Candle => &[".f..", ".w..", ".w..", "HwH.", "hhh."],
            LampGear::Torch => &[".f..", "fwf.", ".i..", ".m..", ".m.."],
            LampGear::Brazier => &[".ff.", "fwwf", "iiii", "Hhhh", ".HH."],
        };
        let x = match pose.dir {
            "up" => 14,
            "side" => 5,
            _ => 2,
        };
        self.trinket(rows, x, 20 + pose.bob, lamp.colors);
    }

    fn charm(&mut self, pose: &Pose, charm: Piece<Charm>) {
        let (dir, bob) = (pose.dir, pose.bob);
        let colors = charm.colors;
        match charm.shape {
            Charm::Pendant => match dir {
                "down" => self.trinket(
                    &["h....h", ".h..h.", "..hh..", ".hwih.", ".hiih.", "..hh.."],
                    7,
                    13 + bob,
                    colors,
                ),
                "side" => self.trinket(&["h..", ".h.", ".ii", ".wi"], 10, 14 + bob, colors),
                _ => self.trinket(&["h....h", ".hhhh."], 7, 13 + bob, colors),
            },
            Charm::Ring => {
                let x = match dir {
                    "down" => 1,
                    "up" => 16,
                    _ => 8,
                };
                self.trinket(&[".w.", "iii", ".h."], x, 18 + bob, colors);
            }
            Charm::Flask | Charm::Pouch | Charm::Book => {
                let rows: &[&str] = match charm.shape {
                    Charm::Flask => &[".HH.", ".hh.", "hiih", "hwih", ".hh."],
                    Charm::Pouch => &[".mm.", "hmmh", "hhih", "Hhhh", ".HH."],
                    _ => &["iiiH", "iwwH", "iwwH", "iiiH", "hhhH"],
                };
                let x = match dir {
                    "up" => 3,
                    "side" => 7,
                    _ => 13,
                };
                self.trinket(rows, x, 20 + bob, colors);
            }
        }
    }

    /// Paints a held item with its grip column at `hand` (body coordinates),
    /// leaning `lean` pixels sideways per row above the grip.
    fn item(&mut self, piece: Piece<Held>, hand: i32, lean: f32, dy: i32) {
        let f = self.source.get(held_sprite(piece.shape), 0);
        for y in 0..f.h.min(H) {
            let shift = ((GRIP.1 - y) as f32 * lean).round() as i32;
            for x in 0..f.w.min(ITEM_W) {
                let c = self.source.pixel(f, x, y);
                if c[3] != 0 {
                    let c = self.resolve(c, Some(piece.colors));
                    self.dot(hand + shift + x - GRIP.0, y + dy, c);
                }
            }
        }
    }

    /// The weapon slung across the back, its business end over a shoulder.
    fn slung(&mut self, dir: &str, piece: Piece<Held>, bob: i32) {
        // Short weapons ride higher so that they still show.
        let raise = (2 - top_row(self.source, held_sprite(piece.shape))).min(0);
        // Over the shoulder opposite the hand.
        let (column, lean) = match dir {
            "down" => (8, -0.5),
            "up" => (10, 0.5),
            _ => (6, -0.34),
        };
        self.item(piece, column, lean, bob + raise);
    }

    /// The item in the hand, then the fingers that close around it.
    fn in_hand(&mut self, pose: &Pose, piece: Piece<Held>) {
        let (column, lean) = hand_of(pose.dir);
        self.item(piece, column, lean, pose.bob);
        let beside = if pose.dir == "up" {
            column + 1
        } else {
            column - 1
        };
        for (i, x) in [beside, column].into_iter().enumerate() {
            self.dot(x, GRIP.1 - 1 + pose.bob, self.body[0]);
            self.dot(x, GRIP.1 + pose.bob, self.body[i]);
        }
    }
}

/// Column of the gripping hand and how much the item leans there.
fn hand_of(dir: &str) -> (i32, f32) {
    match dir {
        "down" => (18, 0.0),
        "up" => (1, 0.0),
        _ => (10, 0.4),
    }
}

/// First painted row of a sprite's first frame.
fn top_row(atlas: &Atlas, name: &str) -> i32 {
    let f = atlas.get(name, 0);
    (0..f.h)
        .find(|&y| (0..f.w).any(|x| atlas.pixel(f, x, y)[3] != 0))
        .unwrap_or(0)
}

/// Sprite of a held item in `gear.ron`.
fn held_sprite(shape: Held) -> &'static str {
    match shape {
        Held::Spear => "held_spear",
        Held::Trident => "held_trident",
        Held::Sword => "held_sword",
        Held::Dagger => "held_dagger",
        Held::Rapier => "held_rapier",
        Held::Curved => "held_curved",
        Held::Axe => "held_axe",
        Held::Scythe => "held_scythe",
        Held::Mace => "held_mace",
        Held::Staff => "held_staff",
        Held::Bow => "held_bow",
        Held::Whip => "held_whip",
        Held::Quill => "held_quill",
        Held::Hammer => "held_hammer",
        Held::Lantern => "held_lantern",
        Held::Lens => "held_lens",
    }
}

/// Armour drawn over the torso, and armour worn on the head.
fn armor_layers(shape: Armor) -> (Option<&'static str>, Option<&'static str>) {
    match shape {
        Armor::Plate => (Some("plate"), None),
        Armor::Robe => (Some("robe"), None),
        Armor::Vest => (Some("vest"), None),
        Armor::Helmet => (None, Some("helmet")),
        Armor::Hood => (None, Some("hood")),
        Armor::Hat => (None, Some("hat")),
        Armor::Crown => (None, Some("crown")),
        Armor::Mask | Armor::Shield | Armor::Cloak => (None, None),
    }
}

fn compose(source: &Atlas, look: Look, gear: &Gear, pose: Pose, armed: bool) -> Canvas {
    let mut p = Painter {
        source,
        body: palette_for(look),
        out: Canvas::new(FRAME_W, H),
    };
    let (dir, bob, hair, body) = (pose.dir, pose.bob, look.hair, look.body);
    let facing_away = dir == "up";
    let worn = gear.armor;
    let is = |shape: Armor| worn.filter(|a| a.shape == shape);
    let (torso_layer, head_layer) = worn.map_or((None, None), |a| armor_layers(a.shape));
    let colors = worn.map(|a| a.colors);
    // A hood swallows the hair.
    let hooded = is(Armor::Hood).is_some();

    let cape = |p: &mut Painter| {
        if let Some(cloak) = is(Armor::Cloak) {
            p.cloak(dir, pose.cape, bob, cloak.colors);
        } else if look.cape {
            p.layer(&format!("hero_cape_{dir}"), pose.cape, bob, None);
        }
    };
    if !facing_away {
        if let Some(slung) = gear.back {
            p.slung(dir, slung, bob);
        }
        cape(&mut p);
        if !hooded {
            p.layer(&format!("hero_back{hair}_{dir}"), 0, bob, None);
        }
    }
    p.layer(&format!("hero_legs_{dir}"), pose.legs, 0, None);
    if let Some(boots) = gear.boots {
        p.footwear(&pose, boots);
    }
    p.layer(&format!("hero_tails{body}_{dir}"), pose.tails, bob, None);
    p.layer(&format!("hero_torso{body}_{dir}"), 0, bob, None);
    if let Some(name) = torso_layer {
        p.layer(&format!("gear_{name}_{dir}"), 0, bob, colors);
    }
    if let Some(charm) = gear.charm {
        p.charm(&pose, charm);
    }
    p.layer(&format!("hero_head_{dir}"), 0, bob, None);
    if facing_away {
        cape(&mut p);
    } else {
        if body == 1 {
            p.layer(&format!("hero_face_{dir}"), 0, bob, None);
        }
        if look.brand {
            p.layer(&format!("hero_brand_{dir}"), 0, bob, None);
        }
        if let Some(mask) = is(Armor::Mask) {
            p.layer(&format!("gear_mask_{dir}"), 0, bob, Some(mask.colors));
        }
    }
    if !hooded {
        p.layer(&format!("hero_hair{hair}_{dir}"), 0, bob, None);
    }
    if let Some(name) = head_layer {
        p.layer(&format!("gear_{name}_{dir}"), 0, bob, colors);
    }
    if facing_away && let Some(slung) = gear.back {
        p.slung(dir, slung, bob);
    }
    if let Some(shield) = is(Armor::Shield) {
        p.layer(&format!("gear_shield_{dir}"), 0, bob, Some(shield.colors));
    }
    if let Some(lamp) = gear.lamp {
        p.lamp(&pose, lamp);
    }
    if armed {
        p.in_hand(&pose, gear.held);
    }
    p.out
}

/// The held item on its own, upright.
fn lone_item(source: &Atlas, piece: Piece<Held>) -> Canvas {
    let mut painter = Painter {
        source,
        body: palette_for(Look::default()),
        out: Canvas::new(ITEM_W, H),
    };
    // `item` works in body coordinates, so undo the margin.
    painter.item(piece, GRIP.0 - MARGIN, 0.0, 0);
    painter.out
}

/// Composes every animation frame of the hero.
pub fn build(source: &Atlas, look: Look, gear: &Gear) -> Atlas {
    let look = look.sanitized();
    let mut sprites = vec![("held".to_string(), vec![lone_item(source, gear.held)])];
    for dir in ["down", "up", "side"] {
        for (anim, legs, bobs, tails, capes) in [
            (
                "idle",
                [0, 0, 0, 0],
                [0, 0, 1, 1],
                [0, 0, 0, 0],
                [0, 0, 1, 1],
            ),
            (
                "walk",
                [1, 0, 2, 0],
                [1, 0, 1, 0],
                [1, 0, 1, 0],
                [1, 2, 1, 2],
            ),
        ] {
            for (prefix, armed) in [("player", true), ("bare", false)] {
                let frames = (0..4)
                    .map(|i| {
                        let pose = Pose {
                            dir,
                            legs: legs[i],
                            bob: bobs[i],
                            tails: tails[i],
                            cape: capes[i],
                        };
                        compose(source, look, gear, pose, armed)
                    })
                    .collect();
                sprites.push((format!("{prefix}_{anim}_{dir}"), frames));
            }
        }
    }
    Atlas::from_frames(sprites, source.outline)
}

/// Draws the head and shoulders of the hero, for portraits. `x` is the left of
/// the body, 22 pixels wide at scale 1.
pub fn portrait(canvas: &mut Canvas, hero: &Atlas, x: i32, y: i32, scale: f32) {
    let f = hero.get("bare_idle_down", 0);
    let head = Frame {
        x: f.x + MARGIN,
        w: (f.w - 2 * MARGIN).max(0),
        h: f.h.min(17),
        ..f
    };
    canvas.sprite(
        hero,
        head,
        x,
        y,
        Blit {
            scale,
            ..Default::default()
        },
    );
}

/// Where the eyes are inside an outlined frame, for the glow of stored light.
pub fn eye_pixels(dir: &str) -> &'static [(i32, i32)] {
    match dir {
        "down" => &[(12, 9), (17, 9)],
        "side" => &[(17, 9)],
        _ => &[],
    }
}

/// The grip of the lone held item inside its outlined frame.
pub const ITEM_GRIP: (i32, i32) = (GRIP.0 + 1, GRIP.1 + 1);

/// First painted row of the lone held item inside its outlined frame.
pub fn held_top(hero: &Atlas) -> i32 {
    top_row(hero, "held")
}

/// Where the tip of the held item is inside an outlined frame, for its glow.
pub fn item_tip(dir: &str, top: i32) -> (i32, i32) {
    let (column, lean) = hand_of(dir);
    // Two rows below the tip, in layer coordinates.
    let row = top + 1;
    let shift = ((GRIP.1 - row) as f32 * lean).round() as i32;
    (column + shift + MARGIN + 1, row + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::babel::seeds::Dice;

    fn source() -> Atlas {
        Atlas::built_in().expect("atlas")
    }

    fn pixels(atlas: &Atlas, name: &str, index: usize) -> Vec<Rgba> {
        let f = atlas.get(name, index);
        (0..f.h)
            .flat_map(|y| (0..f.w).map(move |x| (x, y)))
            .map(|(x, y)| atlas.pixel(f, x, y))
            .collect()
    }

    /// The idle hero from the three sides.
    fn seen(source: &Atlas, look: Look, gear: &Gear) -> [Vec<Rgba>; 3] {
        let hero = build(source, look, gear);
        ["down", "side", "up"].map(|d| pixels(&hero, &format!("player_idle_{d}"), 0))
    }

    const TINT: Colors = [
        rgb(0x12, 0xc8, 0x40),
        rgb(0x0a, 0x70, 0x24),
        rgb(0xf0, 0x20, 0xd0),
    ];

    fn piece<S>(shape: S) -> Piece<S> {
        Piece {
            shape,
            colors: TINT,
            glow: None,
        }
    }

    /// Every shape of every slot, one at a time over the starting gear.
    fn variations() -> Vec<(String, Gear)> {
        let base = Gear::starting();
        let mut out = Vec::new();
        for &s in Held::ALL {
            let gear = Gear {
                held: piece(s),
                ..base
            };
            out.push((format!("held {s:?}"), gear));
        }
        for &s in Armor::ALL {
            let gear = Gear {
                armor: Some(piece(s)),
                ..base
            };
            out.push((format!("armor {s:?}"), gear));
        }
        for &s in Footwear::ALL {
            let gear = Gear {
                boots: Some(piece(s)),
                ..base
            };
            out.push((format!("boots {s:?}"), gear));
        }
        for &s in LampGear::ALL {
            let gear = Gear {
                lamp: Some(piece(s)),
                ..base
            };
            out.push((format!("lamp {s:?}"), gear));
        }
        for &s in Charm::ALL {
            let gear = Gear {
                charm: Some(piece(s)),
                ..base
            };
            out.push((format!("charm {s:?}"), gear));
        }
        out.push((
            "slung dagger".into(),
            Gear {
                back: Some(piece(Held::Dagger)),
                ..base
            },
        ));
        out
    }

    fn assert_filled(hero: &Atlas, label: &str) {
        for dir in ["down", "up", "side"] {
            for anim in ["idle", "walk"] {
                for i in 0..4 {
                    let name = format!("player_{anim}_{dir}");
                    let f = hero.get(&name, i);
                    assert_eq!((f.w, f.h), (FRAME_W + 2, H + 2), "{name}");
                    assert!(
                        pixels(hero, &name, i)
                            .iter()
                            .all(|c| !(c[0] == 0xff && c[1] == 0x01)),
                        "{label} {name}: placeholder left"
                    );
                }
            }
        }
    }

    #[test]
    fn every_look_composes_with_no_placeholder_left() {
        let source = source();
        let mut dice = Dice::from_u64(5);
        let mut looks = vec![Look::default()];
        for hair in 0..look::HAIRS {
            for body in 0..look::BODIES {
                looks.push(Look {
                    hair,
                    body,
                    ..Look::random(&mut dice)
                });
            }
        }
        for look in looks {
            let hero = build(&source, look, &Gear::starting());
            assert_filled(&hero, &format!("{look:?}"));
        }
    }

    #[test]
    fn every_option_changes_what_is_seen() {
        let source = source();
        let base = Look::default();
        let reference = seen(&source, base, &Gear::starting());
        let mut variants = vec![
            Look {
                cape: !base.cape,
                ..base
            },
            Look {
                brand: !base.brand,
                ..base
            },
        ];
        variants.extend((1..look::BODIES).map(|body| Look { body, ..base }));
        variants.extend((1..look::HAIRS).map(|hair| Look { hair, ..base }));
        variants.extend((1..look::HAIR_COLORS).map(|hair_color| Look { hair_color, ..base }));
        variants.extend((1..look::EYE_COLORS).map(|eyes| Look { eyes, ..base }));
        variants.extend((1..look::COATS).map(|coat| Look { coat, ..base }));
        variants.extend(
            (0..look::SKINS)
                .filter(|&s| s != base.skin)
                .map(|skin| Look { skin, ..base }),
        );
        for variant in variants {
            assert_ne!(
                seen(&source, variant, &Gear::starting()),
                reference,
                "{variant:?} looks the same"
            );
        }
    }

    #[test]
    fn every_piece_of_equipment_shows_and_differs() {
        let source = source();
        let look = Look::default();
        let mut pictures = vec![(
            "starting".to_string(),
            seen(&source, look, &Gear::starting()),
        )];
        for (label, gear) in variations() {
            assert_filled(&build(&source, look, &gear), &label);
            // The starting spear is itself one of the held shapes.
            if label == "held Spear" {
                continue;
            }
            let picture = seen(&source, look, &gear);
            // Shapes must not collapse into one another, nor into nothing.
            for (other, existing) in &pictures {
                assert_ne!(&picture, existing, "{label} looks like {other}");
            }
            pictures.push((label, picture));
        }
    }

    #[test]
    fn the_hand_is_empty_only_in_the_bare_frames() {
        let hero = build(&source(), Look::default(), &Gear::starting());
        assert_ne!(
            pixels(&hero, "player_idle_down", 0),
            pixels(&hero, "bare_idle_down", 0)
        );
        assert!(pixels(&hero, "held", 0).iter().any(|c| c[3] != 0));
    }
}
