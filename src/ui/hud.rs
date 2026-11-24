//! Heads-up display and unlit overlays anchored to the world.

use super::{BAD, DIM, FAINT, GOLD, HEIGHT, INK, Ui, WIDTH, rarity_rgba, rect};
use crate::render::canvas::{Blit, Rgba, hex, mix, rgb, with_alpha};
use crate::render::fx::{BANNER_TIME, Fx, TOAST_TIME};
use crate::render::scene::{Camera, rarity_color, tool_icon};
use crate::settings::{Action, Settings};
use babel_core::babel::rarity::Rarity;
use babel_core::data::{Consumable, RecipeOutput, ToolKind, Unlock, tier_name};
use babel_core::game::Game;
use babel_core::game::vec::v2;
use babel_core::progression::player::xp_to_next;
use babel_core::world::floors;
use babel_core::world::hex::{Dir, distance, neighbor};
use babel_core::world::site::SiteKey;

/// How controls are named in hints, depending on the last device used.
pub fn control_label(settings: &Settings, action: Action, pad: bool) -> String {
    if pad {
        match action {
            Action::Up | Action::Down | Action::Left | Action::Right => "stick".into(),
            Action::Attack => "X".into(),
            Action::Dash => "B".into(),
            Action::Interact => "A".into(),
            Action::Ranged => "R2".into(),
            Action::Torch => "↓".into(),
            Action::Tonic => "L2".into(),
            Action::Lamp => "↑".into(),
            Action::Read => "Y".into(),
            Action::NextTool => "R1".into(),
            Action::Inventory => "←".into(),
            Action::Map => "→".into(),
            Action::Skills | Action::Glossary | Action::Catalog => "Start".into(),
        }
    } else {
        match action {
            Action::Attack => format!("clic / {}", settings.key(action)),
            Action::Ranged => format!("clic der. / {}", settings.key(action)),
            _ => settings.key(action).to_string(),
        }
    }
}

fn expand_hint(hint: &str, settings: &Settings, pad: bool) -> String {
    let moves = if pad {
        "el stick".to_string()
    } else {
        format!(
            "{}{}{}{}",
            settings.key(Action::Up),
            settings.key(Action::Left),
            settings.key(Action::Down),
            settings.key(Action::Right)
        )
    };
    let mut out = hint.replace("[move]", &moves);
    for action in Action::ALL {
        let tag = action.tag();
        if !tag.is_empty() {
            out = out.replace(
                &format!("[{tag}]"),
                &format!("[{}]", control_label(settings, action, pad)),
            );
        }
    }
    out
}

fn bars(ui: &mut Ui, game: &Game) {
    let p = &game.profile.player;
    ui.canvas.fill(3, 3, 138, 40, [26, 16, 40, 170]);
    ui.icon("icon_heart", 6, 5);
    ui.bar(
        rect(20, 7, 82, 6),
        p.hp / game.derived.max_hp.max(1.0),
        rgb(238, 82, 90),
    );
    ui.text(
        &format!(
            "{}/{}",
            p.hp.ceil() as i32,
            game.derived.max_hp.ceil() as i32
        ),
        106,
        6,
        INK,
    );
    ui.icon("icon_energy", 7, 17);
    ui.bar(
        rect(20, 19, 82, 4),
        p.energy / game.derived.max_energy.max(1.0),
        rgb(90, 214, 255),
    );
    ui.text(&format!("Nv {}", p.level), 7, 31, GOLD);
    ui.bar(
        rect(36, 33, 66, 3),
        p.xp as f32 / xp_to_next(p.level) as f32,
        rgb(255, 214, 90),
    );
    if p.skill_points > 0 {
        let color = if (ui.time * 4.0).sin() > 0.0 {
            GOLD
        } else {
            INK
        };
        ui.text(&format!("+{}", p.skill_points), 108, 31, color);
    }

    // Statuses with clear icons.
    let mut x = 6;
    for (active, icon, label) in [
        (game.body.status.burn, "icon_burn", "Quemado"),
        (game.body.status.poison, "icon_poison", "Veneno"),
        (game.body.status.slow, "icon_slow", "Lento"),
    ] {
        if active > 0.0 {
            ui.icon(icon, x, 46);
            x = ui.text(label, x + 11, 47, INK) + 6;
        }
    }
}

fn belt(ui: &mut Ui, game: &Game, settings: &Settings, pad: bool) {
    let p = &game.profile.player;
    let y = HEIGHT - 28;
    ui.canvas.fill(3, y - 2, 150, 27, [26, 16, 40, 175]);
    ui.canvas.frame(5, y, 23, 23, rgb(128, 92, 190));
    let frame = ui.atlas.get(tool_icon(p), 0);
    ui.canvas.sprite(
        ui.atlas,
        frame,
        5,
        y,
        Blit {
            scale: 2.0,
            ..Default::default()
        },
    );
    let tool = p.active_tool;
    ui.text(tool.name(), 32, y + 2, INK);
    ui.text(tier_name(p.tool_tier(tool)), 32, y + 12, DIM);
    let mut x = 92;
    let mut slot = |ui: &mut Ui, icon: &str, count: u32, action: Action| {
        ui.icon(icon, x, y + 1);
        ui.text(
            &count.to_string(),
            x + 2,
            y + 15,
            if count > 0 { INK } else { FAINT },
        );
        let _ = action;
        x += 20;
    };
    slot(
        ui,
        "icon_tonic",
        p.consumable(Consumable::Tonic),
        Action::Tonic,
    );
    slot(
        ui,
        "icon_torch",
        p.consumable(Consumable::Torch),
        Action::Torch,
    );
    if game.derived.has(Unlock::RangedPages) {
        slot(
            ui,
            "icon_ammo",
            p.consumable(Consumable::PageAmmo),
            Action::Ranged,
        );
    }
    if tool == ToolKind::Lens {
        let key = control_label(settings, Action::Read, pad);
        ui.text(&format!("[{key}] leer"), 6, y - 12, DIM);
    }
}

/// A tiny hexagon: 11×9 pixels.
fn mini_hex(ui: &mut Ui, cx: i32, cy: i32, color: Rgba) {
    for (row, inset) in [3, 2, 1, 0, 0, 0, 1, 2, 3].into_iter().enumerate() {
        ui.canvas.fill(
            cx - 5 + inset,
            cy - 4 + row as i32,
            11 - inset * 2,
            1,
            color,
        );
    }
}

fn minimap(ui: &mut Ui, game: &Game) {
    let area = rect(WIDTH - 82, 3, 79, 70);
    ui.canvas
        .fill(area.x, area.y, area.w, area.h, [26, 16, 40, 232]);
    ui.canvas
        .frame(area.x, area.y, area.w, area.h, [128, 92, 190, 160]);
    let (cx, cy) = area.center();
    let biome_name = &game.data.biomes[game.site.recipe.biome].name;
    match game.site.key {
        SiteKey::Walk { floor, q, r } => {
            let place = |dq: i32, dr: i32| (cx + dq * 12, cy + dr * 11 + dq * 11 / 2);
            for dq in -3..=3 {
                for dr in -3..=3 {
                    let (nq, nr) = (q + dq, r + dr);
                    if distance((q, r), (nq, nr)) > 3 {
                        continue;
                    }
                    let (x, y) = place(dq, dr);
                    if x < area.x + 6
                        || x > area.x + area.w - 6
                        || y < area.y + 5
                        || y > area.y + area.h - 5
                    {
                        continue;
                    }
                    let id = SiteKey::walk(floor, nq, nr).id();
                    match game.profile.world.visited.get(&id) {
                        Some(v) => {
                            let biome = &game.data.biomes[v.biome.min(game.data.biomes.len() - 1)];
                            mini_hex(ui, x, y, mix(hex(&biome.floor[0]), hex(&biome.trim), 0.15));
                            if v.stairs {
                                ui.canvas.fill(x - 2, y - 1, 5, 1, rgb(40, 20, 60));
                                ui.canvas.fill(x - 1, y + 1, 3, 1, rgb(40, 20, 60));
                            }
                            for d in Dir::ALL {
                                if v.exits[d.index()] {
                                    let (mq, mr) = neighbor(0, 0, d);
                                    let (tx, ty) = place(dq + mq, dr + mr);
                                    ui.canvas.put((x + tx) / 2, (y + ty) / 2, INK);
                                }
                            }
                        }
                        None => mini_hex(ui, x, y, [74, 56, 108, 150]),
                    }
                    if game.profile.world.lamps.iter().any(|l| l.key.id() == id) {
                        ui.canvas.fill(x + 2, y - 3, 3, 3, rgb(140, 245, 255));
                    }
                }
            }
            let blink = if (ui.time * 4.0).sin() > 0.0 {
                INK
            } else {
                GOLD
            };
            ui.canvas.fill(cx - 2, cy - 2, 5, 5, rgb(26, 16, 40));
            ui.canvas.fill(cx - 1, cy - 1, 3, 3, blink);
            if game.derived.has(Unlock::Compass) {
                let (sq, sr) = floors::nearest_stairs(game.profile.world_seed, floor, q, r);
                if (sq, sr) != (q, r) {
                    let (x, y) = place(sq - q, sr - r);
                    let d = v2((x - cx) as f32, (y - cy) as f32).norm();
                    for k in 0..4 {
                        let at = d * (24.0 + k as f32 * 2.0);
                        ui.canvas
                            .fill(cx + at.x as i32, cy + at.y as i32, 2, 2, GOLD);
                    }
                }
            }
            ui.right(&format!("Piso {floor}"), WIDTH - 4, 76, INK);
        }
        SiteKey::Remote { difficulty, .. } => {
            ui.centered("Sitio", cx, cy - 10, DIM);
            ui.centered("remoto", cx, cy + 2, DIM);
            ui.right(&format!("Catálogo · {difficulty}"), WIDTH - 4, 76, INK);
        }
    }
    ui.right(biome_name, WIDTH - 4, 86, DIM);
}

fn boss_bar(ui: &mut Ui, game: &Game) {
    let Some(boss) = game.enemies.iter().find(|e| e.boss.is_some()) else {
        return;
    };
    let w = 220;
    let x = (WIDTH - w) / 2;
    let y = HEIGHT - 46;
    ui.canvas.fill(x - 4, y - 14, w + 8, 24, [26, 16, 40, 205]);
    ui.centered(&boss.name, WIDTH / 2, y - 11, rgb(255, 190, 190));
    ui.bar(rect(x, y, w, 6), boss.hp / boss.max_hp, rgb(224, 64, 108));
    if let Some(state) = &boss.boss
        && let Some(def) = game.data.enemies.guardians.get(state.index)
    {
        for phase in def.phases.iter().skip(1) {
            let px = x + (w as f32 * phase.below) as i32;
            ui.canvas.fill(px, y - 1, 1, 8, INK);
        }
    }
}

fn world_overlays(ui: &mut Ui, game: &Game, camera: &Camera) {
    for e in game
        .enemies
        .iter()
        .filter(|e| e.visible() && e.boss.is_none())
    {
        let at = camera.to_view(e.pos);
        let top = at.y as i32 - (16.0 * e.scale) as i32 - 8 - e.z as i32;
        let x = at.x as i32;
        if e.hurt_timer > 0.0 || e.elite.is_some() {
            ui.bar(rect(x - 9, top, 18, 2), e.hp / e.max_hp, rgb(240, 90, 90));
        }
        if let Some(elite) = &e.elite {
            let color = rarity_color(elite.rarity, ui.time);
            // Keep the name on screen and clear of the status panel.
            let half = ui.font.width(&e.name) / 2 + 3;
            let nx = x.clamp(half, WIDTH - half);
            let mut ny = (top - 11).clamp(4, HEIGHT - 40);
            if nx - half < 142 && ny < 46 {
                ny = 46;
            }
            if (-20..WIDTH + 20).contains(&x) && (-20..HEIGHT + 20).contains(&top) {
                ui.centered(&e.name, nx, ny, color);
            }
        }
    }
}

fn floating(ui: &mut Ui, fx: &Fx, camera: &Camera) {
    for f in &fx.floats {
        let at = camera.to_view(f.pos);
        let fade = ((1.0 - f.age / f.life) * 3.0).clamp(0.0, 1.0);
        let color = [f.color[0], f.color[1], f.color[2], (255.0 * fade) as u8];
        // A quick pop when the number appears.
        let rise = if f.age < 0.08 { 2 } else { 0 };
        ui.centered(&f.text, at.x as i32, at.y as i32 - rise, color);
    }
}

/// Near a locked node, say which tool is needed and what it is made of.
fn locked_nodes(ui: &mut Ui, game: &Game, camera: &Camera) {
    let nearest = game
        .nodes
        .iter()
        .filter(|n| {
            let def = game.data.node(n.kind);
            game.profile.player.tool_tier(def.tool) < def.min_tier
                && n.pos.dist(game.body.pos) < 46.0
        })
        .min_by(|a, b| {
            a.pos
                .dist(game.body.pos)
                .total_cmp(&b.pos.dist(game.body.pos))
        });
    let Some(node) = nearest else {
        return;
    };
    let def = game.data.node(node.kind);
    let material = game
        .data
        .recipes
        .iter()
        .find(|r| r.output == RecipeOutput::Tool(def.tool, def.min_tier))
        .and_then(|r| r.cost.first())
        .map_or(String::new(), |c| format!(" · hace falta {}", c.0.name()));
    let text = format!("{} {}{material}", def.tool.name(), tier_name(def.min_tier));
    let at = camera.to_view(node.pos);
    let half = ui.font.width(&text) / 2 + 4;
    let x = (at.x as i32).clamp(half, WIDTH - half);
    let y = (at.y as i32 - 26).clamp(48, HEIGHT - 50);
    ui.canvas
        .fill(x - half, y - 2, half * 2, 12, [26, 16, 40, 220]);
    ui.centered(&text, x, y, BAD);
}

/// Everything shown while playing, below any open screen.
pub fn draw(ui: &mut Ui, game: &Game, fx: &Fx, camera: &Camera, settings: &Settings, pad: bool) {
    world_overlays(ui, game, camera);
    floating(ui, fx, camera);
    locked_nodes(ui, game, camera);
    bars(ui, game);
    belt(ui, game, settings, pad);
    minimap(ui, game);
    boss_bar(ui, game);
    if let Some(prompt) = game.prompt() {
        let key = control_label(settings, Action::Interact, pad);
        let text = format!("[{key}] {prompt}");
        let w = ui.font.width(&text) + 12;
        ui.canvas
            .fill((WIDTH - w) / 2, HEIGHT - 72, w, 13, [26, 16, 40, 215]);
        ui.centered(&text, WIDTH / 2, HEIGHT - 69, GOLD);
    }
    if let Some(hint) = game.tutorial_hint() {
        let text = expand_hint(hint, settings, pad);
        let lines = ui.font.wrap(&text, 230);
        let h = lines.len() as i32 * 10 + 8;
        let x = WIDTH - 246;
        let y = HEIGHT - h - 4;
        ui.panel(rect(x, y, 242, h));
        for (i, line) in lines.iter().enumerate() {
            ui.centered(line, x + 121, y + 5 + i as i32 * 10, INK);
        }
    }
    if game.darkness > 0.0 {
        ui.centered(
            "La luz se apaga…".replace('…', "...").as_str(),
            WIDTH / 2,
            56,
            BAD,
        );
    }
}

/// Banners, toasts and celebrations, drawn above everything.
pub fn notices(ui: &mut Ui, fx: &Fx) {
    if let Some((rarity, age)) = fx.celebration {
        let fade = (1.0 - age / 1.8).clamp(0.0, 1.0);
        let base = if rarity == Rarity::Canonical {
            rarity_color(rarity, ui.time)
        } else {
            rarity_rgba(rarity)
        };
        for i in 0..18 {
            let a = i as f32 / 18.0 * std::f32::consts::TAU + age * 0.9;
            let (s, c) = a.sin_cos();
            let inner = 46.0 + age * 60.0;
            let outer = inner + 60.0;
            ui.canvas.line(
                WIDTH / 2 + (c * inner) as i32,
                HEIGHT / 2 + (s * inner * 0.6) as i32,
                WIDTH / 2 + (c * outer) as i32,
                HEIGHT / 2 + (s * outer * 0.6) as i32,
                with_alpha(base, (150.0 * fade) as u8),
            );
        }
        ui.title(
            rarity.label(),
            WIDTH / 2,
            HEIGHT / 2 + 34,
            2,
            with_alpha(base, (255.0 * fade.min(0.7) / 0.7) as u8),
        );
    }
    if let Some(banner) = fx.banners.first() {
        let appear = (banner.age * 6.0).min(1.0);
        let fade = ((BANNER_TIME - banner.age) * 2.5).clamp(0.0, 1.0);
        let alpha = (255.0 * appear.min(fade)) as u8;
        let color = [banner.color[0], banner.color[1], banner.color[2], alpha];
        let lines = ui.font.wrap(&banner.subtitle, 320);
        let h = 24 + lines.len() as i32 * 10;
        let y = 40 - ((1.0 - appear) * 10.0) as i32;
        ui.canvas.fill(
            60,
            y,
            360,
            h,
            [26, 16, 40, (alpha as u32 * 200 / 255) as u8],
        );
        ui.canvas.fill(60, y, 360, 1, color);
        ui.canvas.fill(60, y + h - 1, 360, 1, color);
        ui.title(
            &banner.title,
            WIDTH / 2,
            y + 5,
            if ui.font.width(&banner.title) * 2 < 340 {
                2
            } else {
                1
            },
            color,
        );
        for (i, line) in lines.iter().enumerate() {
            ui.centered(
                line,
                WIDTH / 2,
                y + 23 + i as i32 * 10,
                with_alpha(INK, alpha),
            );
        }
    }
    let count = fx.toasts.len() as i32;
    for (i, (text, age)) in fx.toasts.iter().enumerate() {
        let alpha = (((TOAST_TIME - age) * 2.0).clamp(0.0, 1.0) * 255.0) as u8;
        let y = HEIGHT - 44 - (count - i as i32) * 11;
        let w = ui.font.width(text) + 8;
        ui.canvas.fill(
            4,
            y - 2,
            w,
            11,
            [26, 16, 40, (alpha as u32 * 180 / 255) as u8],
        );
        ui.text(text, 8, y, with_alpha(INK, alpha));
    }
}
