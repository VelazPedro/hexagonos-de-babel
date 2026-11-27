//! The character creator: a large living preview and one row per choice, with
//! a tab for the hero and another for the pet. Each has a name.

use super::state::{Cmd, Screen, UiData, keyboard, text_field};
use super::{DIM, GOLD, INK, SKY, Ui, WIDTH, rect};
use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, Rgba, rgb, with_alpha};
use crate::render::hero::{BODY_NAMES, COATS, EYES, HAIR_COLORS, HAIR_NAMES, SKINS};
use crate::render::pet as art;
use babel_core::babel::seeds::Dice;
use babel_core::progression::look::{self, Look};
use babel_core::progression::pet::{self, NAME_LIMIT, PetLook, name_char};

const HERO_SCALE: f32 = 5.0;
const PET_SCALE: f32 = 5.0;
/// The first row of choices and the distance between rows.
const FIRST_ROW: i32 = 72;
const ROW: i32 = 18;

/// What the creator edits. Names are kept as typed in [`UiData`].
pub struct Choices {
    pub look: Look,
    pub pet: PetLook,
}

/// The sprites the preview shows.
pub struct Sprites<'a> {
    pub hero: &'a Atlas,
    pub pet: &'a Atlas,
}

fn cycle(value: u8, change: i32, count: u8) -> u8 {
    (i32::from(value) + change).rem_euclid(i32::from(count)) as u8
}

/// Adds one typed character to a name, if a name may have it there.
fn push_name(draft: &mut String, c: char) {
    let doubled_space = c == ' ' && (draft.is_empty() || draft.ends_with(' '));
    if name_char(c) && !doubled_space && draft.chars().count() < NAME_LIMIT {
        draft.push(c);
    }
}

/// The on-screen keyboard only has small letters: names start in capitals.
fn capitalise_last(draft: &mut String) {
    let Some(last) = draft.pop() else {
        return;
    };
    if draft.is_empty() || draft.ends_with(' ') {
        draft.extend(last.to_uppercase());
    } else {
        draft.push(last);
    }
}

/// A hexagonal pedestal with a pool of light.
fn pedestal(ui: &mut Ui, cx: i32, cy: i32, tint: Rgba) {
    ui.canvas
        .glow(cx as f32, (cy - 56) as f32, 78.0, with_alpha(tint, 150));
    for (half, rows, color) in [
        (60, 8, rgb(44, 28, 74)),
        (52, 6, rgb(78, 52, 122)),
        (40, 4, rgb(112, 80, 170)),
    ] {
        for row in -rows..=rows {
            let row: i32 = row;
            let w = half - row.abs() * 2;
            ui.canvas.fill(cx - w, cy + row, w * 2, 1, color);
        }
    }
}

/// Drifting motes of stored light around the preview.
fn motes(ui: &mut Ui, cx: i32, cy: i32) {
    for i in 0..22 {
        let seed = i as f32 * 1.618;
        let life = (ui.time * (0.22 + (i % 5) as f32 * 0.04) + seed).fract();
        let x = cx + ((seed * 7.3).sin() * 52.0 + (ui.time + seed).sin() * 4.0) as i32;
        let y = cy - (life * 128.0) as i32;
        let alpha = ((life * std::f32::consts::PI).sin() * 210.0) as u8;
        ui.canvas.put(x, y, [196, 232, 255, alpha]);
        if i % 4 == 0 {
            ui.canvas.put(x, y - 1, [196, 232, 255, alpha / 2]);
        }
    }
}

fn preview(ui: &mut Ui, sprites: &Sprites, data: &UiData) {
    let (cx, floor) = (96, 200);
    pedestal(ui, cx, floor, rgb(90, 130, 230));
    let (dir, flip) = [
        ("down", false),
        ("side", false),
        ("up", false),
        ("side", true),
    ][usize::from(data.preview_turn % 4)];
    // Stand for a while, then take a few steps.
    let walking = ui.time % 7.0 > 4.8;
    let (atlas, scale, name, fps) = if data.creator_pet {
        // The pet also sits down now and then.
        let (anim, fps) = match ui.time % 9.0 {
            t if t < 3.5 => ("idle", 3.0),
            t if t < 6.5 => ("sit", 2.0),
            _ => ("walk", 8.0),
        };
        (sprites.pet, PET_SCALE, format!("pet_{anim}_{dir}"), fps)
    } else {
        let (anim, fps) = if walking {
            ("walk", 7.0)
        } else {
            ("idle", 3.0)
        };
        (
            sprites.hero,
            HERO_SCALE,
            format!("player_{anim}_{dir}"),
            fps,
        )
    };
    let frame = atlas.animate(&name, ui.time, fps);
    let w = (frame.w as f32 * scale) as i32;
    let h = (frame.h as f32 * scale) as i32;
    ui.canvas.sprite(
        atlas,
        frame,
        cx - w / 2,
        floor - h + 4,
        Blit {
            flip,
            scale,
            ..Default::default()
        },
    );
    motes(ui, cx, floor);
    let draft = if data.creator_pet {
        &data.pet_name
    } else {
        &data.hero_name
    };
    let shown = pet::clean_name(draft);
    ui.centered(&shown, cx, 38, INK);
}

fn hero_rows(ui: &mut Ui, chosen: &mut Look) {
    let row = |i: i32| rect(196, FIRST_ROW + i * ROW, 268, 17);
    let mut value = chosen.sanitized();
    let change = ui.selector(row(0), "Cuerpo", BODY_NAMES[usize::from(value.body)], None);
    value.body = cycle(value.body, change, look::BODIES);
    let skin = &SKINS[usize::from(value.skin)];
    let change = ui.selector(row(1), "Piel", skin.0, Some(skin.1));
    value.skin = cycle(value.skin, change, look::SKINS);
    let change = ui.selector(row(2), "Peinado", HAIR_NAMES[usize::from(value.hair)], None);
    value.hair = cycle(value.hair, change, look::HAIRS);
    let color = &HAIR_COLORS[usize::from(value.hair_color)];
    let change = ui.selector(row(3), "Color de pelo", color.name, Some(color.base));
    value.hair_color = cycle(value.hair_color, change, look::HAIR_COLORS);
    let eyes = &EYES[usize::from(value.eyes)];
    let change = ui.selector(row(4), "Ojos", eyes.0, Some(eyes.1));
    value.eyes = cycle(value.eyes, change, look::EYE_COLORS);
    let coat = &COATS[usize::from(value.coat)];
    // The coat names are long, so the swatch alone sits beside them.
    let short = coat.name.split(' ').next().unwrap_or(coat.name);
    let change = ui.selector(row(5), "Abrigo", short, Some(coat.base));
    value.coat = cycle(value.coat, change, look::COATS);
    let on = |b: bool| if b { "Sí" } else { "No" };
    if ui.selector(row(6), "Capa", on(value.cape), None) != 0 {
        value.cape = !value.cape;
    }
    if ui.selector(row(7), "Marca en la frente", on(value.brand), None) != 0 {
        value.brand = !value.brand;
    }
    *chosen = value;
}

fn pet_rows(ui: &mut Ui, chosen: &mut PetLook) {
    let row = |i: i32| rect(196, FIRST_ROW + i * ROW, 268, 17);
    let mut value = chosen.sanitized();
    let (name, swatch) = art::fur_name(value);
    let change = ui.selector(row(0), "Pelaje", name, Some(swatch));
    value.fur = cycle(value.fur, change, pet::FURS);
    let ears = art::EAR_NAMES[usize::from(value.ears)];
    let change = ui.selector(row(1), "Orejas", ears, None);
    value.ears = cycle(value.ears, change, pet::EARS);
    let tail = art::TAIL_NAMES[usize::from(value.tail)];
    let change = ui.selector(row(2), "Cola", tail, None);
    value.tail = cycle(value.tail, change, pet::TAILS);
    let (name, swatch) = art::eye_name(value);
    let change = ui.selector(row(3), "Ojos", name, Some(swatch));
    value.eyes = cycle(value.eyes, change, pet::PET_EYES);
    let (name, swatch) = art::collar_name(value);
    let change = ui.selector(row(4), "Collar", name, swatch);
    value.collar = cycle(value.collar, change, pet::COLLARS);
    *chosen = value;
    ui.centered("Te acompaña desde el primer paso.", 330, 174, DIM);
    ui.centered("Aprende con tus puntos de habilidad.", 330, 186, DIM);
}

/// The two tabs at the top of the choices.
fn tabs(ui: &mut Ui, data: &mut UiData) {
    for (i, label) in ["Personaje", "Mascota"].into_iter().enumerate() {
        let r = rect(190 + i as i32 * 141, 30, 139, 15);
        let active = data.creator_pet == (i == 1);
        if ui.button(r, label) && !active {
            data.creator_pet = i == 1;
            data.show_keys = false;
        }
        if active {
            ui.canvas.fill(r.x + 4, r.y + r.h - 2, r.w - 8, 2, GOLD);
        }
    }
    if ui.input.tab != 0 {
        data.creator_pet = !data.creator_pet;
    }
}

/// `creating` is true on the way to a new game, false when changing the
/// current one.
pub fn draw(
    ui: &mut Ui,
    sprites: &Sprites,
    chosen: &mut Choices,
    data: &mut UiData,
    creating: bool,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(215);
    ui.heading("Tu personaje", 6);

    // A physical keyboard always types into the name of the open tab.
    let draft = if data.creator_pet {
        &mut data.pet_name
    } else {
        &mut data.hero_name
    };
    for &c in &ui.input.letters {
        push_name(draft, c);
    }
    if ui.input.backspace {
        draft.pop();
    }

    if data.show_keys {
        on_screen_keys(ui, data);
        return;
    }

    ui.card(rect(10, 30, 172, 218), SKY);
    preview(ui, sprites, data);
    if ui.icon_button(rect(20, 224, 74, 17), "ui_turn", "Girar") {
        data.preview_turn = (data.preview_turn + 1) % 4;
    }
    if ui.icon_button(rect(98, 224, 74, 17), "ui_dice", "Al azar") {
        let mut dice = Dice::from_u64(u64::from(ui.time.to_bits()));
        if data.creator_pet {
            chosen.pet = PetLook::random(&mut dice);
        } else {
            chosen.look = Look::random(&mut dice);
        }
    }

    tabs(ui, data);
    ui.card(rect(190, 47, 280, 171), rgb(190, 150, 255));
    let (draft, placeholder) = if data.creator_pet {
        (&data.pet_name, "Nombre de tu mascota")
    } else {
        (&data.hero_name, "Tu nombre")
    };
    text_field(ui, rect(196, 51, 200, 17), draft, placeholder);
    if ui.button(rect(400, 51, 64, 17), "Teclado") {
        data.show_keys = true;
        ui.focus.index = 0;
    }
    if data.creator_pet {
        pet_rows(ui, &mut chosen.pet);
    } else {
        hero_rows(ui, &mut chosen.look);
    }

    let label = if creating { "Continuar" } else { "Listo" };
    if ui.primary_button(rect(262, 223, 136, 19), label) {
        if creating {
            cmds.push(Cmd::Push(Screen::NewGame));
        } else {
            cmds.push(Cmd::Pop);
        }
    }
    if creating && ui.button(rect(194, 223, 58, 19), "Volver") {
        cmds.push(Cmd::Pop);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[("← →", "cambiar"), (back, "volver")]);
}

/// Letters to click or pick with a gamepad.
fn on_screen_keys(ui: &mut Ui, data: &mut UiData) {
    let (draft, what) = if data.creator_pet {
        (&mut data.pet_name, "Nombre de tu mascota")
    } else {
        (&mut data.hero_name, "Tu nombre")
    };
    ui.centered(what, WIDTH / 2, 40, DIM);
    text_field(ui, rect(WIDTH / 2 - 100, 54, 200, 20), draft, "Sin nombre");
    let before = draft.chars().count();
    keyboard(ui, draft, 86, NAME_LIMIT, false);
    if draft.chars().count() > before {
        if draft.ends_with([',', '.']) || draft.ends_with("  ") || draft == " " {
            draft.pop();
        } else {
            capitalise_last(draft);
        }
    }
    if ui.primary_button(rect(WIDTH / 2 - 68, 168, 136, 19), "Listo") {
        data.show_keys = false;
        ui.focus.index = 0;
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "volver")]);
}
