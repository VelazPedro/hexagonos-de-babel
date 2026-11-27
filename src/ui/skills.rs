//! The skill trees, the hero's and the pet's: hexagonal icon nodes, one branch
//! per column. Everything is visible and plannable from the start; only the
//! focused node explains itself. Both are paid with the same points.

use super::state::{Cmd, Screen, UiData, tabs};
use super::{DIM, FAINT, GOLD, GOOD, INK, Rect, Ui, WIDTH, rect};
use crate::render::canvas::{Blit, Rgba, mix, rgb, with_alpha};
use babel_core::data::{Branch, GameData, PetBranch, PetPerk};
use babel_core::game::Game;
use babel_core::progression::stats::describe;
use std::collections::BTreeSet;

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

const NODE_W: i32 = 28;
const NODE_H: i32 = 23;
const ROW_H: i32 = 32;
const FIRST_ROW: i32 = 66;

fn pet_color(b: PetBranch) -> Rgba {
    match b {
        PetBranch::Fang => rgb(255, 130, 120),
        PetBranch::Nose => rgb(255, 196, 96),
        PetBranch::Bond => rgb(150, 220, 140),
    }
}

/// A node of either tree.
struct Entry {
    id: String,
    name: String,
    description: String,
    bonus: String,
    requires: Vec<String>,
    column: usize,
    color: Rgba,
    /// Marked with a star: it unlocks something new.
    mechanic: bool,
    area: Rect,
}

/// A branch heading.
struct Heading {
    name: &'static str,
    color: Rgba,
    center: i32,
}

/// Where a node sits: `side` is -1, 0 or 1 within its column.
fn spot(center: i32, side: i32, row: u8) -> Rect {
    let cx = center + side * 24;
    let cy = FIRST_ROW + i32::from(row) * ROW_H;
    rect(cx - NODE_W / 2, cy - NODE_H / 2, NODE_W, NODE_H)
}

fn column_center(column: usize, columns: usize) -> i32 {
    let width = (WIDTH - 4) / columns as i32;
    2 + column as i32 * width + width / 2
}

fn hero_tree(data: &GameData) -> (Vec<Heading>, Vec<Entry>) {
    let headings = Branch::ALL
        .into_iter()
        .enumerate()
        .map(|(i, b)| Heading {
            name: b.name(),
            color: branch_color(b),
            center: column_center(i, Branch::ALL.len()),
        })
        .collect();
    let entries = data
        .skills
        .iter()
        .map(|s| {
            let column = Branch::ALL.iter().position(|&b| b == s.branch).unwrap_or(0);
            // A row with a single node is centred in its branch column.
            let side = if s.row == 0 {
                0
            } else {
                i32::from(s.col) * 2 - 1
            };
            let bonuses: Vec<String> = s.stats.iter().map(|&(s, v)| describe(s, v)).collect();
            Entry {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                bonus: bonuses.join("   "),
                requires: s.requires.clone(),
                column,
                color: branch_color(s.branch),
                mechanic: s.unlock.is_some(),
                area: spot(column_center(column, Branch::ALL.len()), side, s.row),
            }
        })
        .collect();
    (headings, entries)
}

fn pet_tree(data: &GameData) -> (Vec<Heading>, Vec<Entry>) {
    let columns = PetBranch::ALL.len();
    let headings = PetBranch::ALL
        .into_iter()
        .enumerate()
        .map(|(i, b)| Heading {
            name: b.name(),
            color: pet_color(b),
            center: column_center(i, columns),
        })
        .collect();
    let entries = data
        .pet_skills
        .iter()
        .map(|s| {
            let column = PetBranch::ALL
                .iter()
                .position(|&b| b == s.branch)
                .unwrap_or(0);
            let mut bonuses: Vec<String> = s.stats.iter().map(|&(s, v)| describe(s, v)).collect();
            if !bonuses.is_empty() {
                bonuses.push("para vos".into());
            }
            Entry {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                bonus: bonuses.join("   "),
                requires: s.requires.clone(),
                column,
                color: pet_color(s.branch),
                mechanic: s
                    .perks
                    .iter()
                    .any(|p| matches!(p.0, PetPerk::Fetch | PetPerk::Dig | PetPerk::Pounce))
                    || (s.row == 0 && !s.perks.is_empty()),
                area: spot(column_center(column, columns), 0, s.row),
            }
        })
        .collect();
    (headings, entries)
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

fn state_of(owned: &BTreeSet<String>, entry: &Entry) -> State {
    if owned.contains(&entry.id) {
        State::Owned
    } else if entry.requires.iter().all(|r| owned.contains(r)) {
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

fn links(ui: &mut Ui, entries: &[Entry], owned_ids: &BTreeSet<String>) {
    for entry in entries {
        let color = entry.color;
        let state = state_of(owned_ids, entry);
        let parents = entries.iter().filter(|p| entry.requires.contains(&p.id));
        for parent in parents {
            let path = link_path(parent.area, entry.area);
            let owned = owned_ids.contains(&parent.id);
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

fn node(ui: &mut Ui, entry: &Entry, state: State, focused: bool) {
    let r = entry.area;
    let color = entry.color;
    let pulse = (ui.time * 4.0).sin() * 0.5 + 0.5;
    let (edge, fill) = match state {
        State::Owned => (color, mix(ui.theme.cell, color, 0.45)),
        State::Available => (
            mix(rgb(150, 112, 50), GOLD, pulse),
            if focused {
                ui.theme.cell_hot
            } else {
                ui.theme.cell
            },
        ),
        State::Locked => (
            ui.theme.frame,
            if focused {
                ui.theme.cell
            } else {
                ui.theme.well
            },
        ),
    };
    if state == State::Owned {
        let (cx, cy) = r.center();
        ui.canvas
            .glow(cx as f32, cy as f32, 21.0, with_alpha(color, 130));
    }
    let outline = ui.theme.frame_dark;
    hexagon(ui, r, 1, outline);
    hexagon(ui, r, 0, edge);
    hexagon(ui, r, -2, fill);
    let lift = i32::from(focused);
    let opts = match state {
        State::Locked => Blit {
            tint: Some((ui.theme.frame, 0.72)),
            ..Default::default()
        },
        _ => Blit::default(),
    };
    let (cx, cy) = r.center();
    ui.icon_at(&skill_icon(&entry.id), cx, cy - lift, opts);
    if entry.mechanic {
        // A star marks skills that unlock a new mechanic.
        let (x, y) = (r.x + r.w - 5, r.y + 2);
        for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            ui.canvas.put(x + dx, y + dy, GOLD);
        }
        ui.canvas.put(x, y, INK);
    }
}

fn headers(ui: &mut Ui, headings: &[Heading], entries: &[Entry], owned: &BTreeSet<String>) {
    for (i, heading) in headings.iter().enumerate() {
        let all = entries.iter().filter(|e| e.column == i);
        let total = all.clone().count() as i32;
        let have = all.filter(|e| owned.contains(&e.id)).count() as i32;
        ui.centered(heading.name, heading.center, 34, heading.color);
        // One pip per skill of the branch.
        let left = heading.center - total * 7 / 2;
        for k in 0..total {
            let fill = if k < have {
                heading.color
            } else {
                ui.theme.cell
            };
            ui.canvas.fill(left + k * 7, 44, 5, 3, fill);
        }
    }
}

fn detail(ui: &mut Ui, entries: &[Entry], entry: &Entry, owned: &BTreeSet<String>, points: u32) {
    let color = entry.color;
    ui.card(rect(4, 214, 472, 42), color);
    let icon = Blit {
        scale: 2.0,
        ..Default::default()
    };
    ui.icon_at(&skill_icon(&entry.id), 26, 235, icon);
    let end = ui.text(&entry.name, 48, 220, color);
    if entry.mechanic {
        ui.text("· mecánica nueva", end + 4, 220, GOLD);
    }
    ui.text(&entry.description, 48, 232, INK);
    ui.text(&entry.bonus, 48, 244, GOOD);

    match state_of(owned, entry) {
        State::Owned => ui.right("✓ aprendida", 468, 220, GOOD),
        State::Available if points > 0 => {
            let key = ui.glyph("Enter", "A");
            let w = super::keycap_width(ui.font, key);
            let label = "aprender";
            let x = 468 - ui.font.width(label) - w - 4;
            ui.keycap(key, x, 220);
            ui.text(label, x + w + 4, 220, GOLD);
        }
        State::Available => ui.right("sin puntos", 468, 220, DIM),
        State::Locked => {
            let missing: Vec<&str> = entry
                .requires
                .iter()
                .filter(|r| !owned.contains(*r))
                .filter_map(|r| entries.iter().find(|e| &e.id == r))
                .map(|e| e.name.as_str())
                .collect();
            ui.right(&format!("antes: {}", missing.join(" y ")), 468, 244, FAINT);
        }
    }
}

/// The ring of light that spreads from a skill just learned.
fn burst(ui: &mut Ui, entries: &[Entry], data: &UiData) {
    let Some((id, at)) = &data.learned else {
        return;
    };
    let age = ui.time - at;
    let Some(entry) = entries.iter().find(|e| &e.id == id).filter(|_| age < 0.7) else {
        return;
    };
    let (cx, cy) = entry.area.center();
    let fade = ((1.0 - age / 0.7) * 255.0) as u8;
    let color = with_alpha(entry.color, fade);
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

/// Switches between the two trees.
fn whose(ui: &mut Ui, game: &Game, data: &mut UiData) {
    let pet = &game.profile.player.pet.name;
    let pet_label = if pet.is_empty() { "Mascota" } else { pet };
    for (i, label) in ["Personaje", pet_label].into_iter().enumerate() {
        let r = rect(WIDTH / 2 - 100 + i as i32 * 102, 20, 98, 12);
        let active = data.skill_pet == (i == 1);
        if ui.button(r, label) && !active {
            data.skill_pet = i == 1;
            data.learned = None;
        }
        if active {
            ui.canvas.fill(r.x + 4, r.y + r.h - 2, r.w - 8, 2, GOLD);
        }
    }
}

pub fn draw(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(232);
    tabs(ui, Screen::Skills, game.catalog_unlocked(), cmds);
    let points = game.profile.player.skill_points;
    let label = format!("{points} puntos");
    let label = if points == 1 { "1 punto".into() } else { label };
    ui.text(&label, 8, 20, if points > 0 { GOLD } else { FAINT });
    whose(ui, game, data);
    let showing_pet = data.skill_pet;
    let (headings, entries) = if showing_pet {
        pet_tree(&game.data)
    } else {
        hero_tree(&game.data)
    };
    let owned = if showing_pet {
        game.profile.player.pet.skills.clone()
    } else {
        game.profile.player.skills.clone()
    };
    ui.right(
        &format!("{}/{}", owned.len(), entries.len()),
        WIDTH - 8,
        20,
        DIM,
    );
    headers(ui, &headings, &entries, &owned);
    links(ui, &entries, &owned);
    let mut shown: Option<&Entry> = None;
    let mut learn: Option<String> = None;
    for entry in &entries {
        let response = ui.focusable(entry.area);
        node(ui, entry, state_of(&owned, entry), response.focused);
        if response.focused {
            shown = Some(entry);
        }
        if response.activated {
            learn = Some(entry.id.clone());
        }
    }
    if let Some(id) = learn {
        let learned = if showing_pet {
            game.learn_pet_skill(&id)
        } else {
            game.learn_skill(&id)
        };
        if learned {
            data.learned = Some((id, ui.time));
        }
    }
    burst(ui, &entries, data);
    let points = game.profile.player.skill_points;
    match shown {
        Some(entry) => detail(ui, &entries, entry, &owned, points),
        None => ui.centered("Elegí un hexágono para ver qué hace.", WIDTH / 2, 232, DIM),
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "cerrar")]);
}
