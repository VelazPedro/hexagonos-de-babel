//! The skill tree: everything is visible and plannable from the start.

use super::state::{Cmd, Screen, UiData, tabs};
use super::{DIM, FAINT, GOLD, GOOD, INK, Ui, WIDTH, rect};
use crate::render::canvas::{Rgba, rgb, with_alpha};
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

const COLUMN: i32 = 119;
const NODE_W: i32 = 55;
const NODE_H: i32 = 22;

fn area(skill: &SkillDef) -> super::Rect {
    let column = Branch::ALL
        .iter()
        .position(|&b| b == skill.branch)
        .unwrap_or(0) as i32;
    // A row with a single node is centred in its branch column.
    let x = 4 + column * COLUMN + i32::from(skill.col) * (NODE_W + 4);
    let x = if skill.row == 0 { x + (NODE_W + 4) / 2 } else { x };
    rect(x + 1, 44 + i32::from(skill.row) * 33, NODE_W, NODE_H)
}

pub fn draw(ui: &mut Ui, game: &mut Game, data: &mut UiData, cmds: &mut Vec<Cmd>) {
    ui.dim_background(225);
    tabs(ui, Screen::Skills, game.catalog_unlocked(), cmds);
    ui.text(
        &format!("Puntos disponibles: {}", game.profile.player.skill_points),
        8,
        21,
        if game.profile.player.skill_points > 0 {
            GOLD
        } else {
            DIM
        },
    );
    ui.right(
        &format!("{}/{} aprendidas", game.profile.player.skills.len(), game.data.skills.len()),
        WIDTH - 8,
        21,
        DIM,
    );
    for (i, branch) in Branch::ALL.into_iter().enumerate() {
        ui.centered(
            branch.name(),
            4 + i as i32 * COLUMN + NODE_W + 2,
            32,
            branch_color(branch),
        );
    }
    let data_ref = game.data.clone();
    // Links first so nodes sit on top.
    for skill in &data_ref.skills {
        let to = area(skill);
        for parent in skill.requires.iter().filter_map(|r| data_ref.skill(r)) {
            let from = area(parent);
            let owned = game.profile.player.skills.contains(&parent.id);
            let color = if owned {
                branch_color(skill.branch)
            } else {
                rgb(86, 70, 120)
            };
            let (fx, _) = from.center();
            let (tx, _) = to.center();
            let mid = to.y - 5;
            ui.canvas.line(fx, from.y + from.h, fx, mid, color);
            ui.canvas.line(fx, mid, tx, mid, color);
            ui.canvas.line(tx, mid, tx, to.y, color);
        }
    }
    let mut shown: Option<&SkillDef> = None;
    let mut learn: Option<&str> = None;
    for skill in &data_ref.skills {
        let r = area(skill);
        let response = ui.focusable(r);
        let owned = game.profile.player.skills.contains(&skill.id);
        let available = game.profile.player.can_learn(&data_ref, &skill.id);
        let color = branch_color(skill.branch);
        let edge = if owned {
            color
        } else if available {
            GOLD
        } else {
            rgb(92, 76, 126)
        };
        ui.cell(r, &response, Some(edge));
        if owned {
            ui.canvas
                .fill(r.x + 1, r.y + 1, r.w - 2, r.h - 2, with_alpha(color, 70));
        }
        let text_color = if owned {
            INK
        } else if available {
            GOLD
        } else {
            FAINT
        };
        let lines = ui.font.wrap(&skill.name, NODE_W - 3);
        let top = r.y + (NODE_H - lines.len().min(2) as i32 * 9) / 2;
        for (i, line) in lines.iter().take(2).enumerate() {
            ui.centered(line, r.x + NODE_W / 2, top + i as i32 * 9, text_color);
        }
        if skill.unlock.is_some() {
            ui.canvas.fill(r.x + r.w - 4, r.y + 2, 2, 2, GOLD);
        }
        if response.focused {
            shown = Some(skill);
        }
        if response.activated {
            learn = Some(&skill.id);
        }
    }
    ui.panel(rect(4, 213, 472, 45));
    if let Some(skill) = shown {
        let end = ui.text(&skill.name, 12, 218, branch_color(skill.branch));
        let status = if game.profile.player.skills.contains(&skill.id) {
            ("aprendida".to_string(), GOOD)
        } else if game.profile.player.can_learn(&data_ref, &skill.id) {
            ("Enter / A para aprender".to_string(), GOLD)
        } else {
            let missing: Vec<&str> = skill
                .requires
                .iter()
                .filter(|r| !game.profile.player.skills.contains(*r))
                .filter_map(|r| data_ref.skill(r).map(|s| s.name.as_str()))
                .collect();
            if missing.is_empty() {
                ("sin puntos".to_string(), DIM)
            } else {
                (format!("requiere {}", missing.join(" y ")), DIM)
            }
        };
        ui.text(&status.0, end + 10, 218, status.1);
        ui.paragraph(&skill.description, 12, 231, 450, INK);
        let bonuses: Vec<String> = skill.stats.iter().map(|&(s, v)| describe(s, v)).collect();
        ui.text(&bonuses.join("  "), 12, 245, GOOD);
    }
    if let Some(id) = learn {
        let id = id.to_string();
        if game.learn_skill(&id) {
            data.message = "Aprendida".into();
        }
    }
    ui.footer("El punto amarillo marca una mecánica nueva · Q/Tab o L1/R1: pestañas");
}
