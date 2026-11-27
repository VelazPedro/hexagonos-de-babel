//! Puts the pet together from the pieces of `pet.ron`, coloured and shaped as
//! chosen.

use super::atlas::Atlas;
use super::canvas::{Canvas, Rgba, mix, rgb};
use babel_core::progression::pet::{self, PetLook};

pub const FRAME_W: i32 = 24;
pub const FRAME_H: i32 = 19;
/// Frames of each animation.
const FRAMES: usize = 4;

/// Base, light and shade of each fur.
const FURS: [(&str, [Rgba; 3]); pet::FURS as usize] = [
    (
        "Canela",
        [
            rgb(0xae, 0x60, 0x26),
            rgb(0xcd, 0x80, 0x3a),
            rgb(0x7c, 0x3f, 0x1c),
        ],
    ),
    (
        "Negro",
        [
            rgb(0x3c, 0x34, 0x44),
            rgb(0x5a, 0x50, 0x66),
            rgb(0x28, 0x22, 0x30),
        ],
    ),
    (
        "Blanco",
        [
            rgb(0xe4, 0xdc, 0xd2),
            rgb(0xff, 0xf8, 0xec),
            rgb(0xb4, 0xaa, 0xb2),
        ],
    ),
    (
        "Gris",
        [
            rgb(0x88, 0x84, 0x98),
            rgb(0xaa, 0xa6, 0xb8),
            rgb(0x5e, 0x5a, 0x6e),
        ],
    ),
    (
        "Chocolate",
        [
            rgb(0x74, 0x42, 0x2e),
            rgb(0x92, 0x58, 0x3c),
            rgb(0x4e, 0x2a, 0x22),
        ],
    ),
    (
        "Arena",
        [
            rgb(0xd6, 0xaa, 0x6c),
            rgb(0xee, 0xcc, 0x90),
            rgb(0xa8, 0x7c, 0x4a),
        ],
    ),
    (
        "Fuego",
        [
            rgb(0xd0, 0x52, 0x1e),
            rgb(0xee, 0x7a, 0x34),
            rgb(0x92, 0x30, 0x1a),
        ],
    ),
];
const EYES: [(&str, Rgba); pet::PET_EYES as usize] = [
    ("Oscuros", rgb(0x24, 0x0e, 0x12)),
    ("Miel", rgb(0x9c, 0x58, 0x14)),
    ("Verdes", rgb(0x30, 0x86, 0x44)),
    ("Celestes", rgb(0x44, 0x9c, 0xde)),
    ("Ámbar", rgb(0xe4, 0xa2, 0x1c)),
];
/// The first entry is no collar; its colour is never painted.
const COLLARS: [(&str, Rgba); pet::COLLARS as usize] = [
    ("Sin collar", rgb(0, 0, 0)),
    ("Rojo", rgb(0xe4, 0x46, 0x4c)),
    ("Azul", rgb(0x48, 0x7c, 0xee)),
    ("Verde", rgb(0x3a, 0xb4, 0x68)),
    ("Violeta", rgb(0xa4, 0x62, 0xe2)),
    ("Turquesa", rgb(0x48, 0xdc, 0xc4)),
];
pub const EAR_NAMES: [&str; pet::EARS as usize] = ["Paradas", "Caídas", "Una y una"];
pub const TAIL_NAMES: [&str; pet::TAILS as usize] = ["Punta oscura", "Punta blanca", "Lisa"];

const INNER_EAR: Rgba = rgb(0xc0, 0x9a, 0x94);
const INNER_EAR_SHADE: Rgba = rgb(0x8a, 0x66, 0x64);
const NOSE: Rgba = rgb(0x24, 0x16, 0x1c);
const MUZZLE_TINT: Rgba = rgb(0x38, 0x1c, 0x1c);
const WHITE_TIP: Rgba = rgb(0xf2, 0xec, 0xe2);
const TAG: Rgba = rgb(0xff, 0xd9, 0x5c);

pub fn fur_name(look: PetLook) -> (&'static str, Rgba) {
    let (name, colors) = FURS[usize::from(look.sanitized().fur)];
    (name, colors[0])
}

pub fn eye_name(look: PetLook) -> (&'static str, Rgba) {
    EYES[usize::from(look.sanitized().eyes)]
}

/// The collar's name and, if there is one, its colour.
pub fn collar_name(look: PetLook) -> (&'static str, Option<Rgba>) {
    let index = usize::from(look.sanitized().collar);
    let (name, color) = COLLARS[index];
    (name, (index > 0).then_some(color))
}

/// What each placeholder of `pet.ron` becomes, numbered from 1.
fn palette_for(look: PetLook) -> [Rgba; 12] {
    let [base, light, shade] = FURS[usize::from(look.fur)].1;
    let tip = match look.tail {
        0 => mix(shade, MUZZLE_TINT, 0.65),
        1 => WHITE_TIP,
        _ => base,
    };
    let muzzle = mix(shade, MUZZLE_TINT, 0.4);
    [
        base,
        light,
        shade,
        INNER_EAR,
        INNER_EAR_SHADE,
        muzzle,
        NOSE,
        EYES[usize::from(look.eyes)].1,
        tip,
        base,
        COLLARS[usize::from(look.collar)].1,
        TAG,
    ]
}

/// How a piece is painted.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Normal,
    /// The far side, a shade darker.
    Far,
    /// An ear from behind shows no inside.
    Back,
}

struct Painter<'a> {
    source: &'a Atlas,
    colors: [Rgba; 12],
    out: Canvas,
}

impl Painter<'_> {
    fn piece(&mut self, name: &str, frame: usize, (x, y): (i32, i32), mirror: bool, tone: Tone) {
        let f = self.source.get(name, frame);
        for py in 0..f.h {
            for px in 0..f.w {
                let from = if mirror { f.w - 1 - px } else { px };
                let c = self.source.pixel(f, from, py);
                if c[3] == 0 {
                    continue;
                }
                let color = if c[0] == 0xff && c[1] == 0x02 {
                    let mut index = usize::from(c[2]);
                    index = match (tone, index) {
                        // Light to base, base and inside to their shades.
                        (Tone::Far, 1) => 3,
                        (Tone::Far, 2) => 1,
                        (Tone::Far, 4) => 5,
                        (Tone::Back, 4) => 1,
                        (Tone::Back, 5) => 3,
                        _ => index,
                    };
                    self.colors
                        .get(index.saturating_sub(1))
                        .copied()
                        .unwrap_or(c)
                } else {
                    c
                };
                self.out.set(x + px, y + py, color);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Anim {
    Idle,
    Walk,
    Sit,
}

/// Which of the two ears hang down.
fn flops(look: PetLook) -> [bool; 2] {
    match look.ears {
        0 => [false, false],
        1 => [true, true],
        _ => [false, true],
    }
}

fn compose(source: &Atlas, look: PetLook, dir: &str, anim: Anim, frame: usize) -> Canvas {
    let mut p = Painter {
        source,
        colors: palette_for(look),
        out: Canvas::new(FRAME_W, FRAME_H),
    };
    let walking = anim == Anim::Walk;
    let sitting = anim == Anim::Sit;
    // The head dips while breathing and with every other step.
    let bob = match anim {
        Anim::Walk => i32::from(frame % 2 == 1),
        _ => i32::from(frame >= 2),
    };
    let legs = if walking { [1, 0, 2, 0][frame] } else { 0 };
    // The tail never rests: quicker on the move, slower when sitting.
    let wag = if walking {
        frame % 2
    } else {
        usize::from(frame >= 2)
    };
    let [left_flops, right_flops] = flops(look);
    let collared = look.collar > 0;
    match dir {
        "side" => {
            let head_y = 4 + bob;
            // The far ear peeks from behind the head.
            if right_flops {
                p.piece("pet_ear_flop_side", 0, (18, head_y + 1), false, Tone::Far);
            } else {
                p.piece("pet_ear_side", 0, (17, head_y - 4), false, Tone::Far);
            }
            if sitting {
                p.piece("pet_tail_side", wag, (0, 11), false, Tone::Normal);
                p.piece("pet_sit_side", 0, (5, 10), false, Tone::Normal);
            } else {
                p.piece("pet_tail_side", wag, (0, 4), false, Tone::Normal);
                p.piece("pet_legs_side", legs, (4, 15), false, Tone::Normal);
                p.piece("pet_body_side", 0, (4, 10), false, Tone::Normal);
            }
            p.piece("pet_head_side", 0, (12, head_y), false, Tone::Normal);
            if left_flops {
                p.piece(
                    "pet_ear_flop_side",
                    0,
                    (11, head_y + 1),
                    false,
                    Tone::Normal,
                );
            } else {
                p.piece("pet_ear_side", 0, (11, head_y - 4), false, Tone::Normal);
            }
            if collared {
                p.piece("pet_collar_side", 0, (13, head_y + 6), false, Tone::Normal);
            }
        }
        _ => {
            let back = dir == "up";
            let head_y = 4 + bob;
            let tone = if back { Tone::Back } else { Tone::Normal };
            if !back {
                let tail_y = if sitting { 12 } else { 9 };
                p.piece("pet_tail_front", wag, (15, tail_y), false, Tone::Normal);
            }
            for (flop, x, mirror) in [(left_flops, 4, false), (right_flops, 14, true)] {
                if !flop {
                    p.piece("pet_ear_front", 0, (x, head_y - 4), mirror, tone);
                }
            }
            if sitting {
                let name = if back { "pet_sit_up" } else { "pet_sit_down" };
                p.piece(name, 0, (7, 12), false, Tone::Normal);
            } else {
                p.piece("pet_legs_front", legs, (9, 15), false, Tone::Normal);
                if back {
                    p.piece("pet_body_up", 0, (7, 11 + bob), false, Tone::Normal);
                } else {
                    p.piece("pet_chest_down", 0, (8, 11 + bob), false, Tone::Normal);
                }
            }
            let head = if back { "pet_head_up" } else { "pet_head_down" };
            p.piece(head, 0, (7, head_y), false, Tone::Normal);
            // Hanging ears fall over the sides of the head.
            for (flop, x, mirror) in [(left_flops, 4, false), (right_flops, 14, true)] {
                if flop {
                    p.piece("pet_ear_flop", 0, (x, head_y), mirror, Tone::Normal);
                }
            }
            if collared {
                let (name, y) = if back {
                    ("pet_collar_up", head_y + 7)
                } else {
                    ("pet_collar_down", head_y + 10)
                };
                p.piece(name, 0, (9, y), false, Tone::Normal);
            }
            if back {
                // From behind it rises over the rump, to the left.
                let tail_y = if sitting { 12 } else { 10 };
                p.piece("pet_tail_front", wag, (4, tail_y), true, Tone::Normal);
            }
        }
    }
    p.out
}

/// Composes every animation frame of a pet: `pet_{idle,walk,sit}_{down,up,side}`.
pub fn build(source: &Atlas, look: PetLook) -> Atlas {
    let look = look.sanitized();
    let mut sprites = Vec::new();
    for dir in ["down", "up", "side"] {
        for (name, anim) in [
            ("idle", Anim::Idle),
            ("walk", Anim::Walk),
            ("sit", Anim::Sit),
        ] {
            let frames = (0..FRAMES)
                .map(|i| compose(source, look, dir, anim, i))
                .collect();
            sprites.push((format!("pet_{name}_{dir}"), frames));
        }
    }
    Atlas::from_frames(sprites, source.outline)
}

/// A pet ready to be drawn, with the look it was made for.
pub struct PetArt {
    pub look: PetLook,
    pub atlas: Atlas,
}

impl PetArt {
    pub fn new(source: &Atlas, look: PetLook) -> PetArt {
        PetArt {
            look,
            atlas: build(source, look),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Atlas {
        Atlas::built_in().expect("atlas")
    }

    fn every_frame(source: &Atlas, look: PetLook) -> Vec<Rgba> {
        let art = build(source, look);
        let mut all = Vec::new();
        for dir in ["down", "up", "side"] {
            for anim in ["idle", "walk", "sit"] {
                for i in 0..FRAMES {
                    let f = art.get(&format!("pet_{anim}_{dir}"), i);
                    for y in 0..f.h {
                        for x in 0..f.w {
                            all.push(art.pixel(f, x, y));
                        }
                    }
                }
            }
        }
        all
    }

    #[test]
    fn no_placeholder_is_left_in_any_pet() {
        let source = source();
        let mut dice = babel_core::babel::seeds::Dice::from_u64(4);
        let mut looks = vec![PetLook::default()];
        looks.extend((0..60).map(|_| PetLook::random(&mut dice)));
        for look in looks {
            let pixels = every_frame(&source, look);
            assert!(
                !pixels
                    .iter()
                    .any(|c| c[3] != 0 && c[0] == 0xff && c[1] <= 0x02),
                "{look:?}"
            );
        }
    }

    #[test]
    fn every_pet_option_changes_what_is_seen() {
        let source = source();
        let plain = every_frame(&source, PetLook::default());
        type Change = fn(&mut PetLook, u8);
        let options: [(u8, Change); 5] = [
            (pet::FURS, |l, v| l.fur = v),
            (pet::EARS, |l, v| l.ears = v),
            (pet::TAILS, |l, v| l.tail = v),
            (pet::PET_EYES, |l, v| l.eyes = v),
            (pet::COLLARS, |l, v| l.collar = v),
        ];
        for (index, (count, change)) in options.into_iter().enumerate() {
            let mut seen = vec![plain.clone()];
            for value in 1..count {
                let mut look = PetLook::default();
                change(&mut look, value);
                let pixels = every_frame(&source, look);
                assert!(!seen.contains(&pixels), "option {index} value {value}");
                seen.push(pixels);
            }
        }
    }

    /// With `HEXBABEL_PICTURES` set, writes a large sheet to look at.
    #[test]
    fn the_pet_can_be_looked_over() {
        let Some(dir) = std::env::var_os("HEXBABEL_PICTURES") else {
            return;
        };
        let source = source();
        let looks = [
            PetLook::default(),
            PetLook {
                ears: 1,
                fur: 1,
                tail: 1,
                eyes: 3,
                collar: 1,
            },
            PetLook {
                ears: 2,
                fur: 5,
                tail: 2,
                eyes: 1,
                collar: 2,
            },
        ];
        let scale = 7;
        let (cell_w, cell_h) = ((FRAME_W + 4) * scale, (FRAME_H + 4) * scale);
        let mut sheet = Canvas::new(cell_w * 6, cell_h * 3 * looks.len() as i32);
        sheet.clear(rgb(0x6a, 0x62, 0x80));
        for (row, look) in looks.into_iter().enumerate() {
            let art = build(&source, look);
            for (d, facing) in ["down", "side", "up"].into_iter().enumerate() {
                let shots = [
                    ("idle", 0),
                    ("idle", 2),
                    ("walk", 0),
                    ("walk", 1),
                    ("walk", 2),
                    ("sit", 0),
                ];
                for (col, (anim, i)) in shots.into_iter().enumerate() {
                    let f = art.get(&format!("pet_{anim}_{facing}"), i);
                    sheet.sprite(
                        &art,
                        f,
                        col as i32 * cell_w + scale,
                        (row * 3 + d) as i32 * cell_h + scale,
                        super::super::canvas::Blit {
                            scale: scale as f32,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        let path = std::path::Path::new(&dir).join("pet-sheet.png");
        crate::debug::save_png(&sheet, &path).expect("save");
    }
}
