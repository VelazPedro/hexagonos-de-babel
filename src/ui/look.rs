//! The character creator: a large living preview and one row per choice.

use super::state::{Cmd, Screen, UiData};
use super::{DIM, GOLD, INK, SKY, Ui, WIDTH, rect};
use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, Rgba, rgb, with_alpha};
use crate::render::hero::{BODY_NAMES, COATS, EYES, HAIR_COLORS, HAIR_NAMES, SKINS};
use babel_core::babel::seeds::Dice;
use babel_core::progression::look::{self, Look};

const PREVIEW_SCALE: f32 = 5.0;

fn cycle(value: u8, change: i32, count: u8) -> u8 {
    (i32::from(value) + change).rem_euclid(i32::from(count)) as u8
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

fn preview(ui: &mut Ui, hero: &Atlas, turn: u8) {
    let (cx, floor) = (96, 200);
    pedestal(ui, cx, floor, rgb(90, 130, 230));
    let (dir, flip) = [
        ("down", false),
        ("side", false),
        ("up", false),
        ("side", true),
    ][usize::from(turn % 4)];
    // Stand for a while, then take a few steps.
    let walking = ui.time % 7.0 > 4.8;
    let (anim, fps) = if walking {
        ("walk", 7.0)
    } else {
        ("idle", 3.0)
    };
    let frame = hero.animate(&format!("player_{anim}_{dir}"), ui.time, fps);
    let w = (frame.w as f32 * PREVIEW_SCALE) as i32;
    let h = (frame.h as f32 * PREVIEW_SCALE) as i32;
    ui.canvas.sprite(
        hero,
        frame,
        cx - w / 2,
        floor - h + 4,
        Blit {
            flip,
            scale: PREVIEW_SCALE,
            ..Default::default()
        },
    );
    motes(ui, cx, floor);
}

/// `creating` is true on the way to a new game, false when changing the look
/// of the current one.
pub fn draw(
    ui: &mut Ui,
    hero: &Atlas,
    chosen: &mut Look,
    data: &mut UiData,
    creating: bool,
    cmds: &mut Vec<Cmd>,
) {
    ui.dim_background(215);
    ui.title("Tu personaje", WIDTH / 2, 6, 2, GOLD);
    ui.card(rect(10, 30, 172, 218), SKY);
    preview(ui, hero, data.preview_turn);
    if ui.icon_button(rect(20, 224, 74, 17), "ui_turn", "Girar") {
        data.preview_turn = (data.preview_turn + 1) % 4;
    }
    if ui.icon_button(rect(98, 224, 74, 17), "ui_dice", "Al azar") {
        let mut dice = Dice::from_u64(u64::from(ui.time.to_bits()));
        *chosen = Look::random(&mut dice);
    }

    ui.card(rect(190, 30, 280, 180), rgb(190, 150, 255));
    let row = |i: i32| rect(196, 36 + i * 21, 268, 19);
    let mut value = chosen.sanitized();
    let change = ui.selector(row(0), "Cuerpo", BODY_NAMES[usize::from(value.body)], None);
    value.body = cycle(value.body, change, look::BODIES);
    let skin = &SKINS[usize::from(value.skin)];
    let change = ui.selector(row(1), "Piel", skin.0, Some(skin.1));
    value.skin = cycle(value.skin, change, look::SKINS);
    let change = ui.selector(row(2), "Peinado", HAIR_NAMES[usize::from(value.hair)], None);
    value.hair = cycle(value.hair, change, look::HAIRS);
    let color = &HAIR_COLORS[usize::from(value.hair_color)];
    let change = ui.selector(row(3), "Color de pelo", color.0, Some(color.1));
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

    let label = if creating { "Continuar" } else { "Listo" };
    if ui.primary_button(rect(262, 218, 136, 19), label) {
        if creating {
            cmds.push(Cmd::Push(Screen::NewGame));
        } else {
            cmds.push(Cmd::Pop);
        }
    }
    if creating {
        if ui.button(rect(194, 218, 58, 19), "Volver") {
            cmds.push(Cmd::Pop);
        }
        ui.centered("Podés cambiarlo después desde la pausa.", 330, 243, DIM);
    } else {
        ui.centered("Los cambios se ven al instante.", 330, 243, INK);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[("← →", "cambiar"), (back, "volver")]);
}
