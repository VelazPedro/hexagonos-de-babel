//! The skill tree: hexagonal icon nodes, one branch per column. Everything is
//! visible and plannable from the start; only the focused node explains itself.

use super::state::{Cmd, Screen, UiData, tabs};
use super::{CELL, CELL_HOT, DIM, FAINT, GOLD, GOOD, INK, Rect, Ui, WIDTH, rect};
use crate::render::canvas::{Blit, Rgba, mix, rgb, with_alpha};
use babel_core::data::{Branch, SkillDef};
use babel_core::game::Game;
use babel_core::progression::stats::describe;

fn branch_color(b: Branch) -> Rgba {
    match b {
        Branch::Reader => rgb(140, 170, 255),
        Branch::Explorer => rgb(110, 230, 190),
        Branch::Artisan => rgb(255, 190, 90),
        Branch::Fighter => rgb(255, 120, 130),
    }
}

/// Name of the sprite drawn inside a skill's node.
pub fn skill_icon(id: &str) -> String {
    format!("skill_{id}")
}

const COLUMN: i32 = 119;
const NODE_W: i32 = 28;
const NODE_H: i32 = 23;
const ROW_H: i32 = 32;
const FIRST_ROW: i32 = 66;

fn column_of(branch: Branch) -> i32 {
    Branch::ALL.iter().position(|&b| b == branch).unwrap_or(0) as i32
}

fn column_center(column: i32) -> i32 {
    2 + column * COLUMN + COLUMN / 2
}

fn area(skill: &SkillDef) -> Rect {
    let center = column_center(column_of(skill.branch));
    // A row with a single node is centred in its branch column.
    let cx = if skill.row == 0 {
        center
    } else {
        center - 24 + i32::from(skill.col) * 48
    };
    let cy = FIRST_ROW + i32::from(skill.row) * ROW_H;
    rect(cx - NODE_W / 2, cy - NODE_H / 2, NODE_W, NODE_H)
}

/// A hexagon with pointed sides, `grow` pixels larger than the node.
fn hexagon(ui: &mut Ui, r: Rect, grow: i32, color: Rgba) {
    let (cx, cy) = r.center();
    let half_h = r.h / 2 + grow;
    let half_w = r.w / 2 + grow;
    for dy in -half_h..=half_h {
        let w = half_w - dy.abs() * 6 / half_h.max(1);
        ui.canvas.fill(cx - w, cy + dy, w * 2, 1, color);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Owned,
    Available,
    Locked,
}

fn state_of(game: &Game, skill: &SkillDef) -> State {
    let player = &game.profile.player;
    if player.skills.contains(&skill.id) {
        State::Owned
    } else if skill.requires.iter().all(|r| player.skills.contains(r)) {
        State::Available
    } else {
        State::Locked
    }
}

/// The three straight pieces of a link, from parent to child.
fn link_path(from: Rect, to: Rect) -> [(i32, i32); 4] {
    let (fx, _) = from.center();
    let (tx, _) = to.center();
    let mid = to.y - 5;
    [(fx, from.y + from.h), (fx, mid), (tx, mid), (tx, to.y)]
}

/// A point at fraction `t` along a polyline.
fn along(path: &[(i32, i32)], t: f32) -> (i32, i32) {
    let length = |a: (i32, i32), b: (i32, i32)| (b.0 - a.0).abs() + (b.1 - a.1).abs();
    let total: i32 = path.windows(2).map(|s| length(s[0], s[1])).sum();
    let mut left = (t * total as f32) as i32;
    for s in path.windows(2) {
        let (a, b) = (s[0], s[1]);
        let here = length(a, b);
        if left <= here {
            let f = left as f32 / here.max(1) as f32;
            return (
                a.0 + ((b.0 - a.0) as f32 * f) as i32,
                a.1 + ((b.1 - a.1) as f32 * f) as i32,
            );
        }
        left -= here;
    }
    path.last().copied().unwrap_or_default()
}

fn links(ui: &mut Ui, game: &Game) {
    for skill in &game.data.skills {
        let color = branch_color(skill.branch);
        let state = state_of(game, skill);
        for parent in skill.requires.iter().filter_map(|r| game.data.skill(r)) {
            let path = link_path(area(parent), area(skill));
            let owned = game.profile.player.skills.contains(&parent.id);
            let line = if owned { color } else { rgb(78, 62, 112) };
            for s in path.windows(2) {
                ui.canvas.line(s[0].0, s[0].1, s[1].0, s[1].1, line);
                if owned && state == State::Owned {
                    // Learned paths are twice as thick.
                    ui.canvas.line(s[0].0 + 1, s[0].1, s[1].0 + 1, s[1].1, line);
                }
            }
            if owned && state == State::Available {
                // A spark runs toward what can be learned next.
                let (x, y) = along(&path, (ui.time * 0.8).fract());
                ui.canvas
                    .glow(x as f32, y as f32, 4.0, with_alpha(INK, 230));
                ui.canvas.put(x, y, INK);
            }
        }
    }
}

fn node(ui: &mut Ui, skill: &SkillDef, state: State, focused: bool) {
    let r = area(skill);
    let color = branch_color(skill.branch);
    let pulse = (ui.time * 4.0).sin() * 0.5 + 0.5;
    let (edge, fill) = match state {
        State::Owned => (color, mix(CELL, color, 0.45)),
        State::Available => (
            mix(rgb(150, 112, 50), GOLD, pulse),
            if focused { CELL_HOT } else { CELL },
        ),
        State::Locked => (
            rgb(78, 62, 112),
            if focused {
                rgb(58, 42, 90)
            } else {
                rgb(34, 22, 56)
            },
        ),
    };
    if state == State::Owned {
        let (cx, cy) = r.center();
        ui.canvas
            .glow(cx as f32, cy as f32, 21.0, with_alpha(color, 130));
    }
    hexagon(ui, r, 1, rgb(18, 8, 32));
    hexagon(ui, r, 0, edge);
    hexagon(ui, r, -2, fill);
    let lift = i32::from(focused);
    let opts = match state {
        State::Locked => Blit {
            tint: Some((rgb(74, 62, 108), 0.72)),
            ..Default::default()
        },
        _ => Blit::default(),
    };
    let (cx, cy) = r.center();
    ui.icon_at(&skill_icon(&skill.id), cx, cy - lift, opts);
    if skill.unlock.is_some() {
        // A star marks skills that unlock a new mechanic.
        let (x, y) = (r.x + r.w - 5, r.y + 2);
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            ui.canvas.put(x + dx, y + dy, GOLD);
        }
        ui.canvas.put(x, y, INK);
    }
}

fn headers(ui: &mut Ui, game: &Game) {
    for (i, branch) in Branch::ALL.into_iter().enumerate() {
        let cx = column_center(i as i32);
        let all: Vec<&SkillDef> = game
            .data
            .skills
            .iter()
            .filter(|s| s.branch == branch)
            .collect();
        let have = all
            .iter()
            .filter(|s| game.profile.player.skills.contains(&s.id))
            .count();
        let color = branch_color(branch);
        ui.centered(branch.name(), cx, 31, color);
        // One pip per skill of the branch.
        let left = cx - all.len() as i32 * 7 / 2;
        for k in 0..all.len() {
            let fill = if k < have { color } else { rgb(64, 48, 98) };
            ui.canvas.fill(left + k as i32 * 7, 42, 5, 3, fill);
        }
    }
}

fn detail(ui: &mut Ui, game: &Game, skill: &SkillDef) {
    let color = branch_color(skill.branch);
    ui.card(rect(4, 214, 472, 42), color);
    let icon = Blit {
        scale: 2.0,
        ..Default::default()
    };
    ui.icon_at(&skill_icon(&skill.id), 26, 235, icon);
    let end = ui.text(&skill.name, 48, 220, color);
    if skill.unlock.is_some() {
        ui.text("· mecánica nueva", end + 4, 220, GOLD);
    }
    ui.text(&skill.description, 48, 232, INK);
    let bonuses: Vec<String> = skill.stats.iter().map(|&(s, v)| describe(s, v)).collect();
    ui.text(&bonuses.join("   "), 48, 244, GOOD);

    let player = &game.profile.player;
    match state_of(game, skill) {
        State::Owned => ui.right("✓ aprendida", 468, 220, GOOD),
        State::Available if player.skill_points > 0 => {
            let key = ui.glyph("Enter", "A");
            let w = super::keycap_width(ui.font, key);
            let label = "aprender";
            let x = 468 - ui.font.width(label) - w - 4;
            ui.keycap(key, x, 220);
            ui.text(label, x + w + 4, 220, GOLD);
        }
        State::Available => ui.right("sin puntos", 468, 220, DIM),
        State::Locked => {
            let missing: Vec<&str> = skill
                .requires
                .iter()
                .filter(|r| !player.skills.contains(*r))
                .filter_map(|r| game.data.skill(r).map(|s| s.name.as_str()))
                .collect();
            ui.right(&format!("antes: {}", missing.join(" y ")), 468, 244, FAINT);
        }
    }
}

/// The ring of light that spreads from a skill just learned.
fn burst(ui: &mut Ui, game: &Game, data: &UiData) {
    let Some((id, at)) = &data.learned else {
        return;
    };
    let age = ui.time - at;
    let Some(skill) = game.data.skill(id).filter(|_| age < 0.7) else {
        return;
    };
    let (cx, cy) = area(skill).center();
    let fade = ((1.0 - age / 0.7) * 255.0) as u8;
    let color = with_alpha(branch_color(skill.branch), fade);
    for k in 0..3 {
        let radius = 12.0 + age * 60.0 + k as f32 * 3.0;
        ui.canvas
            .ring(cx as f32, cy as f32, radius, radius * 0.8, color);
    }
    for i in 0..10 {
        let angle = i as f32 / 10.0 * std::f32::consts::TAU;
        let d = 14.0 + age * 90.0;
        let (x, y) = (cx + (angle.cos() * d) as i32, cy + (angle.sin() * d) as i32);
        ui.canvas.fill(x, y, 2, 2, with_alpha(INK, fade));
    }
}

pub fn draw(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(232);
    tabs(ui, Screen::Skills, game.catalog_unlocked(), cmds);
    let points = game.profile.player.skill_points;
    let label = format!("{points} puntos");
    let label = if points == 1 { "1 punto".into() } else { label };
    ui.text(&label, 8, 20, if points > 0 { GOLD } else { FAINT });
    ui.right(
        &format!(
            "{}/{}",
            game.profile.player.skills.len(),
            game.data.skills.len()
        ),
        WIDTH - 8,
        20,
        DIM,
    );
    headers(ui, game);
    let data_ref = game.data.clone();
    links(ui, game);
    let mut shown: Option<&SkillDef> = None;
    let mut learn: Option<String> = None;
    for skill in &data_ref.skills {
        let response = ui.focusable(area(skill));
        node(ui, skill, state_of(game, skill), response.focused);
        if response.focused {
            shown = Some(skill);
        }
        if response.activated {
            learn = Some(skill.id.clone());
        }
    }
    if let Some(id) = learn
        && game.learn_skill(&id)
    {
        data.learned = Some((id, ui.time));
    }
    burst(ui, game, data);
    match shown {
        Some(skill) => detail(ui, game, skill),
        None => ui.centered("Elegí un hexágono para ver qué hace.", WIDTH / 2, 232, DIM),
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
}
