//! Heads-up display and unlit overlays anchored to the world. It stays quiet:
//! numbers, names and places appear when they change and then fade away.

use super::{BAD, DIM, GOLD, HEIGHT, INK, SKY, Ui, WIDTH, keycap_width, rarity_rgba, rect};
use crate::render::atlas::Atlas;
use crate::render::canvas::{Blit, Rgba, hex, mix, rgb, with_alpha};
use crate::render::fx::{BANNER_TIME, Fx, TOAST_TIME};
use crate::render::hero;
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

const SHADE: Rgba = [24, 14, 40, 188];
/// How long a passing label stays before fading, in seconds.
const LINGER: f32 = 3.0;

/// What the HUD remembers between frames to know what has just changed.
#[derive(Default)]
pub struct HudState {
    site: String,
    entered: f32,
    tool: Option<ToolKind>,
    tool_changed: f32,
    hp: f32,
    hp_changed: f32,
    /// Health that trails behind after a blow.
    trail: f32,
    hint: String,
    hint_since: f32,
}

impl HudState {
    pub fn update(&mut self, game: &Game, time: f32, dt: f32) {
        let site = game.site.key.id();
        if site != self.site {
            self.site = site;
            self.entered = time;
        }
        let player = &game.profile.player;
        if self.tool != Some(player.active_tool) {
            self.tool = Some(player.active_tool);
            self.tool_changed = time;
        }
        if (player.hp - self.hp).abs() > 0.5 {
            self.hp_changed = time;
            self.hp = player.hp;
        }
        let share = player.hp / game.derived.max_hp.max(1.0);
        self.trail = if share >= self.trail {
            share
        } else {
            (self.trail - dt * 0.45).max(share)
        };
        let hint = game.tutorial_hint().unwrap_or_default();
        if hint != self.hint {
            self.hint = hint.to_string();
            self.hint_since = time;
        }
    }
}

/// 1 while fresh, fading to 0 after `LINGER` seconds.
fn freshness(time: f32, since: f32) -> f32 {
    ((since + LINGER - time) * 2.0).clamp(0.0, 1.0)
}

fn faded(color: Rgba, amount: f32) -> Rgba {
    with_alpha(color, (f32::from(color[3]) * amount.clamp(0.0, 1.0)) as u8)
}

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

/// The short form that fits on a keycap.
fn keycap_label(settings: &Settings, action: Action, pad: bool) -> String {
    if pad {
        control_label(settings, action, pad)
    } else {
        let key = settings.key(action);
        if key == "Espacio" {
            "Esp".into()
        } else {
            key.to_string()
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

fn status_panel(ui: &mut Ui, game: &Game, state: &HudState, settings: &Settings, hero: &Atlas) {
    let p = &game.profile.player;
    // Portrait with the level on it.
    ui.canvas.fill(4, 4, 26, 24, SHADE);
    hero::portrait(ui.canvas, hero, 6, 5, 1.0);
    ui.canvas.frame(4, 4, 26, 24, rgb(128, 92, 190));
    let level = p.level.to_string();
    let w = ui.font.width(&level) + 5;
    ui.canvas.fill(30 - w, 21, w, 10, rgb(40, 24, 64));
    ui.text(&level, 33 - w, 22, GOLD);

    let share = p.hp / game.derived.max_hp.max(1.0);
    let low = share < 0.35;
    ui.bar(rect(35, 7, 84, 6), state.trail, rgb(255, 226, 200));
    let filled = (84.0 * share.clamp(0.0, 1.0)).round() as i32;
    let pulse = if low {
        (ui.time * 6.0).sin() * 0.5 + 0.5
    } else {
        0.0
    };
    let red = mix(rgb(232, 72, 86), rgb(255, 150, 110), pulse);
    ui.canvas.fill(35, 7, filled, 6, red);
    ui.canvas.fill(35, 7, filled, 1, [255, 255, 255, 100]);
    ui.bar(
        rect(35, 16, 70, 3),
        p.energy / game.derived.max_energy.max(1.0),
        rgb(110, 214, 255),
    );
    ui.bar(
        rect(35, 22, 56, 1),
        p.xp as f32 / xp_to_next(p.level) as f32,
        rgb(255, 214, 90),
    );
    let numbers = if low {
        1.0
    } else {
        freshness(ui.time, state.hp_changed)
    };
    if numbers > 0.0 {
        let text = format!(
            "{}/{}",
            p.hp.ceil() as i32,
            game.derived.max_hp.ceil() as i32
        );
        ui.text(&text, 123, 6, faded(INK, numbers));
    }

    let mut y = 34;
    if p.skill_points > 0 {
        // Invites to spend points without covering anything.
        let key = keycap_label(settings, Action::Skills, ui.pad);
        let bob = i32::from((ui.time * 4.0).sin() > 0.4);
        let end = 6 + keycap_width(ui.font, &key);
        ui.keycap(&key, 6, y + 1 - bob);
        ui.icon("ui_tree", end + 2, y - 3 - bob);
        ui.text(&format!("+{}", p.skill_points), end + 17, y + 1 - bob, GOLD);
        y += 15;
    }
    let mut x = 6;
    for (left, icon) in [
        (game.body.status.burn, "icon_burn"),
        (game.body.status.poison, "icon_poison"),
        (game.body.status.slow, "icon_slow"),
    ] {
        if left > 0.0 {
            ui.canvas.fill(x - 2, y - 1, 15, 16, SHADE);
            ui.icon(icon, x, y);
            let w = (11.0 * (left / 4.0).min(1.0)) as i32;
            ui.canvas.fill(x, y + 13, w, 1, INK);
            x += 18;
        }
    }
}

fn belt(ui: &mut Ui, game: &Game, state: &HudState, settings: &Settings) {
    let p = &game.profile.player;
    let y = HEIGHT - 29;
    ui.canvas.fill(4, y, 25, 25, SHADE);
    ui.canvas.frame(4, y, 25, 25, rgb(150, 112, 214));
    let frame = ui.atlas.get(tool_icon(p), 0);
    let scale = Blit {
        scale: 2.0,
        ..Default::default()
    };
    ui.canvas.sprite(ui.atlas, frame, 5, y + 2, scale);
    let next = keycap_label(settings, Action::NextTool, ui.pad);
    ui.keycap(&next, 4, y - 10);

    let tool = p.active_tool;
    let named = freshness(ui.time, state.tool_changed);
    if named > 0.0 {
        let text = format!("{} {}", tool.name(), tier_name(p.tool_tier(tool)));
        ui.text(
            &text,
            8 + keycap_width(ui.font, &next),
            y - 10,
            faded(INK, named),
        );
    }
    // Only what is actually carried takes room on the belt.
    let mut x = 34;
    let mut slots = vec![
        ("icon_tonic", p.consumable(Consumable::Tonic), Action::Tonic),
        ("icon_torch", p.consumable(Consumable::Torch), Action::Torch),
    ];
    if game.derived.has(Unlock::RangedPages) {
        let ammo = p.consumable(Consumable::PageAmmo);
        slots.push(("icon_ammo", ammo, Action::Ranged));
    }
    for (icon, count, action) in slots {
        if count == 0 {
            continue;
        }
        ui.canvas.fill(x, y + 5, 22, 20, SHADE);
        ui.icon_at(icon, x + 10, y + 13, Blit::default());
        let text = count.to_string();
        ui.right(&text, x + 21, y + 17, INK);
        let key = match action {
            Action::Ranged if !ui.pad => settings.key(action).to_string(),
            _ => keycap_label(settings, action, ui.pad),
        };
        let w = keycap_width(ui.font, &key);
        ui.keycap(&key, x + 11 - w / 2, y - 1);
        x += 25;
    }
    if tool == ToolKind::Lens {
        let key = keycap_label(settings, Action::Read, ui.pad);
        let end = x + 3;
        ui.keycap(&key, end, y + 13);
        ui.text("leer", end + keycap_width(ui.font, &key) + 3, y + 13, DIM);
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

fn minimap(ui: &mut Ui, game: &Game, state: &HudState) {
    let area = rect(WIDTH - 70, 4, 66, 54);
    ui.canvas.fill(area.x, area.y, area.w, area.h, SHADE);
    ui.canvas
        .frame(area.x, area.y, area.w, area.h, [128, 92, 190, 140]);
    let (cx, cy) = area.center();
    let named = freshness(ui.time, state.entered);
    let biome_name = &game.data.biomes[game.site.recipe.biome].name;
    let floor_label = match game.site.key {
        SiteKey::Walk { floor, q, r } => {
            let place = |dq: i32, dr: i32| (cx + dq * 12, cy + dr * 11 + dq * 11 / 2);
            for dq in -2..=2 {
                for dr in -2..=2 {
                    let (nq, nr) = (q + dq, r + dr);
                    if distance((q, r), (nq, nr)) > 2 {
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
                        None => mini_hex(ui, x, y, [74, 56, 108, 140]),
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
                        let at = d * (18.0 + k as f32 * 2.0);
                        ui.canvas
                            .fill(cx + at.x as i32, cy + at.y as i32, 2, 2, GOLD);
                    }
                }
            }
            format!("Piso {floor}")
        }
        SiteKey::Remote { difficulty, .. } => {
            ui.centered("Sitio", cx, cy - 10, DIM);
            ui.centered("remoto", cx, cy + 2, DIM);
            format!("Catálogo {difficulty}")
        }
    };
    ui.right(&floor_label, WIDTH - 5, area.y + area.h + 3, INK);
    if named > 0.0 {
        // The gallery's name slides in on arrival, then leaves.
        let slide = ((1.0 - (ui.time - state.entered) * 4.0).max(0.0) * 30.0) as i32;
        ui.right(
            biome_name,
            WIDTH - 5 + slide,
            area.y + area.h + 13,
            faded(SKY, named),
        );
    }
}

fn boss_bar(ui: &mut Ui, game: &Game) {
    let Some(boss) = game.enemies.iter().find(|e| e.boss.is_some()) else {
        return;
    };
    let w = 220;
    let x = (WIDTH - w) / 2;
    let y = HEIGHT - 46;
    ui.canvas.fill(x - 4, y - 14, w + 8, 24, SHADE);
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
        // An elite's name shows once it is close enough to matter.
        if let Some(elite) = &e.elite
            && e.pos.dist(game.body.pos) < 130.0
        {
            let color = rarity_color(elite.rarity, ui.time);
            // Keep the name on screen and clear of the status panel.
            let half = ui.font.width(&e.name) / 2 + 3;
            let nx = x.clamp(half, WIDTH - half);
            let mut ny = (top - 11).clamp(4, HEIGHT - 40);
            if nx - half < 124 && ny < 34 {
                ny = 34;
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
                && n.pos.dist(game.body.pos) < 34.0
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
    let half = ui.font.width(&text) / 2 + 12;
    let x = (at.x as i32).clamp(half, WIDTH - half);
    let y = (at.y as i32 - 30).clamp(48, HEIGHT - 50);
    ui.canvas.fill(x - half, y - 3, half * 2, 13, SHADE);
    ui.icon("icon_lock", x - half + 3, y - 2);
    ui.centered(&text, x + 6, y, BAD);
}

fn prompt(ui: &mut Ui, game: &Game, settings: &Settings) {
    let Some(prompt) = game.prompt() else {
        return;
    };
    let key = keycap_label(settings, Action::Interact, ui.pad);
    let cap = keycap_width(ui.font, &key);
    let w = cap + ui.font.width(&prompt) + 16;
    let x = (WIDTH - w) / 2;
    let y = HEIGHT - 68 + i32::from((ui.time * 4.0).sin() > 0.5);
    ui.canvas.fill(x, y - 4, w, 15, SHADE);
    ui.canvas.fill(x + 2, y + 11, w - 4, 1, GOLD);
    ui.keycap(&key, x + 5, y);
    ui.text(&prompt, x + cap + 10, y, GOLD);
}

fn tutorial(ui: &mut Ui, game: &Game, state: &HudState, settings: &Settings) {
    let Some(hint) = game.tutorial_hint() else {
        return;
    };
    let text = expand_hint(hint, settings, ui.pad);
    let lines = ui.font.wrap(&text, 190);
    let h = lines.len() as i32 * 10 + 10;
    // Each new step slides up into place.
    let arrive = ((ui.time - state.hint_since) * 3.5).clamp(0.0, 1.0);
    let drop = ((1.0 - arrive) * (1.0 - arrive) * 24.0) as i32;
    let x = WIDTH - 210;
    let y = HEIGHT - h - 5 + drop;
    ui.card(rect(x, y, 206, h), GOLD);
    for (i, line) in lines.iter().enumerate() {
        ui.centered(line, x + 103, y + 6 + i as i32 * 10, INK);
    }
}

/// Everything shown while playing, below any open screen.
pub fn draw(
    ui: &mut Ui,
    game: &Game,
    fx: &Fx,
    camera: &Camera,
    settings: &Settings,
    state: &HudState,
    hero: &Atlas,
) {
    world_overlays(ui, game, camera);
    floating(ui, fx, camera);
    locked_nodes(ui, game, camera);
    status_panel(ui, game, state, settings, hero);
    belt(ui, game, state, settings);
    minimap(ui, game, state);
    boss_bar(ui, game);
    prompt(ui, game, settings);
    tutorial(ui, game, state, settings);
    if game.darkness > 0.0 {
        ui.centered("La luz se apaga...", WIDTH / 2, 56, BAD);
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
            [26, 16, 40, (u32::from(alpha) * 200 / 255) as u8],
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
        let y = HEIGHT - 50 - (count - i as i32) * 12;
        let w = ui.font.width(text) + 10;
        // New messages slide in from the edge.
        let slide = ((1.0 - age * 6.0).max(0.0) * 40.0) as i32;
        ui.canvas.fill(
            4 - slide,
            y - 2,
            w,
            11,
            [26, 16, 40, (u32::from(alpha) * 190 / 255) as u8],
        );
        ui.canvas
            .fill(4 - slide, y - 2, 2, 11, with_alpha(GOLD, alpha));
        ui.text(text, 10 - slide, y, with_alpha(INK, alpha));
    }
}
