//! Builds the player's sprites from the layers in `hero.ron`, following the
//! chosen look. The result is a small atlas with the same animation names the
//! scene draws: `player_{idle,walk}_{dir}` and, without the spear (for when it
//! is being swung), `bare_{idle,walk}_{dir}`.

use super::atlas::{Atlas, Frame};
use super::canvas::{Blit, Canvas, Rgba, mix, rgb};
use babel_core::progression::look::{self, Look};

/// Size of every hero layer, before the outline.
const W: i32 = 20;
const H: i32 = 28;

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

/// Name, base, shade and highlight.
pub const HAIR_COLORS: [(&str, Rgba, Rgba, Rgba); look::HAIR_COLORS as usize] = [
    (
        "Negro",
        rgb(0x2b, 0x24, 0x38),
        rgb(0x1c, 0x17, 0x26),
        rgb(0x4a, 0x42, 0x62),
    ),
    (
        "Moreno",
        rgb(0x4a, 0x2e, 0x22),
        rgb(0x32, 0x1f, 0x18),
        rgb(0x6e, 0x46, 0x32),
    ),
    (
        "Castaño",
        rgb(0x7a, 0x4a, 0x2c),
        rgb(0x57, 0x33, 0x1f),
        rgb(0xa2, 0x68, 0x3e),
    ),
    (
        "Cobrizo",
        rgb(0xb4, 0x5a, 0x2a),
        rgb(0x7c, 0x39, 0x1f),
        rgb(0xe0, 0x80, 0x38),
    ),
    (
        "Rubio",
        rgb(0xd9, 0xae, 0x55),
        rgb(0xa8, 0x7f, 0x38),
        rgb(0xf6, 0xd8, 0x88),
    ),
    (
        "Blanco",
        rgb(0xd8, 0xd6, 0xe6),
        rgb(0xa2, 0x9e, 0xbc),
        rgb(0xff, 0xff, 0xff),
    ),
    (
        "Rojo",
        rgb(0xb0, 0x30, 0x3c),
        rgb(0x76, 0x1f, 0x30),
        rgb(0xe0, 0x54, 0x5a),
    ),
    (
        "Azul",
        rgb(0x2c, 0x5a, 0x9e),
        rgb(0x1e, 0x3c, 0x72),
        rgb(0x4e, 0x8c, 0xd8),
    ),
    (
        "Violeta",
        rgb(0x6a, 0x3e, 0x9e),
        rgb(0x4a, 0x2a, 0x72),
        rgb(0x9c, 0x6a, 0xd2),
    ),
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
    const fn color(v: u32) -> Rgba {
        rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
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

/// The placeholder colours of `palette.ron`, in the order of `palette_for`.
fn placeholder(index: u8) -> Rgba {
    rgb(0xff, 0x01, index)
}

/// What each placeholder becomes for a look.
fn palette_for(look: Look) -> [Rgba; 12] {
    let (_, skin, skin_shade) = SKINS[usize::from(look.skin)];
    let (_, hair, hair_shade, hair_light) = HAIR_COLORS[usize::from(look.hair_color)];
    let eyes = EYES[usize::from(look.eyes)].1;
    let c = &COATS[usize::from(look.coat)];
    [
        skin,
        skin_shade,
        hair,
        hair_shade,
        hair_light,
        c.base,
        c.shade,
        c.light,
        eyes,
        c.cape,
        c.cape_shade,
        mix(skin_shade, rgb(0xc0, 0x38, 0x5c), 0.55),
    ]
}

/// The layers of one animation frame, back to front.
struct Pose {
    dir: &'static str,
    legs: usize,
    /// Pixels the body sinks while breathing or walking.
    bob: i32,
    tails: usize,
    cape: usize,
}

fn compose(source: &Atlas, look: Look, pose: &Pose, spear: bool) -> Canvas {
    let colors = palette_for(look);
    let mut out = Canvas::new(W, H);
    let mut put = |name: String, frame: usize, dy: i32| {
        let f = source.get(&name, frame);
        for y in 0..f.h.min(H) {
            for x in 0..f.w.min(W) {
                let mut c = source.pixel(f, x, y);
                if c[3] == 0 {
                    continue;
                }
                if let Some(i) = (0..12u8).find(|&i| c == placeholder(i + 1)) {
                    c = colors[usize::from(i)];
                }
                out.set(x, y + dy, c);
            }
        }
    };
    let dir = pose.dir;
    let (bob, hair, body) = (pose.bob, look.hair, look.body);
    let facing_away = dir == "up";
    if !facing_away {
        if look.cape {
            put(format!("hero_cape_{dir}"), pose.cape, bob);
        }
        put(format!("hero_back{hair}_{dir}"), 0, bob);
    }
    put(format!("hero_legs_{dir}"), pose.legs, 0);
    put(format!("hero_tails{body}_{dir}"), pose.tails, bob);
    put(format!("hero_torso{body}_{dir}"), 0, bob);
    put(format!("hero_head_{dir}"), 0, bob);
    if facing_away {
        if look.cape {
            put("hero_cape_up".into(), pose.cape, bob);
        }
    } else {
        if body == 1 {
            put(format!("hero_face_{dir}"), 0, bob);
        }
        if look.brand {
            put(format!("hero_brand_{dir}"), 0, bob);
        }
    }
    put(format!("hero_hair{hair}_{dir}"), 0, bob);
    if spear {
        put(format!("hero_spear_{dir}"), 0, bob);
    }
    out
}

/// Composes every animation frame of the hero.
pub fn build(source: &Atlas, look: Look) -> Atlas {
    let look = look.sanitized();
    let mut sprites = Vec::new();
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
            for (prefix, spear) in [("player", true), ("bare", false)] {
                let frames = (0..4)
                    .map(|i| {
                        let pose = Pose {
                            dir,
                            legs: legs[i],
                            bob: bobs[i],
                            tails: tails[i],
                            cape: capes[i],
                        };
                        compose(source, look, &pose, spear)
                    })
                    .collect();
                sprites.push((format!("{prefix}_{anim}_{dir}"), frames));
            }
        }
    }
    Atlas::from_frames(sprites, source.outline)
}

/// Draws the head and shoulders of the hero, for portraits.
pub fn portrait(canvas: &mut Canvas, hero: &Atlas, x: i32, y: i32, scale: f32) {
    let f = hero.get("bare_idle_down", 0);
    let head = Frame {
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
        "down" => &[(8, 9), (13, 9)],
        "side" => &[(13, 9)],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::babel::seeds::Dice;
    use std::path::Path;

    fn source() -> Atlas {
        Atlas::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).expect("atlas")
    }

    fn pixels(atlas: &Atlas, name: &str, index: usize) -> Vec<Rgba> {
        let f = atlas.get(name, index);
        (0..f.h)
            .flat_map(|y| (0..f.w).map(move |x| (x, y)))
            .map(|(x, y)| atlas.pixel(f, x, y))
            .collect()
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
            let hero = build(&source, look);
            for dir in ["down", "up", "side"] {
                for anim in ["idle", "walk"] {
                    for i in 0..4 {
                        let name = format!("player_{anim}_{dir}");
                        let f = hero.get(&name, i);
                        assert_eq!((f.w, f.h), (W + 2, H + 2), "{name}");
                        assert!(
                            pixels(&hero, &name, i)
                                .iter()
                                .all(|c| !(c[0] == 0xff && c[1] == 0x01)),
                            "{look:?} {name}: placeholder left"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_option_changes_what_is_seen() {
        let source = source();
        let base = Look::default();
        let seen = |look: Look| {
            let hero = build(&source, look);
            ["down", "side", "up"].map(|d| pixels(&hero, &format!("player_idle_{d}"), 0))
        };
        let reference = seen(base);
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
            assert_ne!(seen(variant), reference, "{variant:?} looks the same");
        }
    }

    #[test]
    fn the_spear_is_only_in_the_armed_frames() {
        let hero = build(&source(), Look::default());
        assert_ne!(
            pixels(&hero, "player_idle_down", 0),
            pixels(&hero, "bare_idle_down", 0)
        );
    }
}
