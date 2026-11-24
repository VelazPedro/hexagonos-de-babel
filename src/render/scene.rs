//! Draws the current site into the albedo and emissive layers, y-sorted, with
//! height kept apart from ground position.

use super::atlas::Atlas;
use super::canvas::{Blit, Canvas, Rgba, mix, rgb, with_alpha};
use super::fx::{Fx, Particle};
use super::tiles::{NIGHT, SiteArt, T, WALL_RISE, noise};
use babel_core::babel::rarity::Rarity;
use babel_core::data::{Behavior, ModFlag, Station};
use babel_core::game::actors::{Ai, DropKind, Enemy, ShotKind, ZoneKind};
use babel_core::game::vec::{V2, v2};
use babel_core::game::{Game, tile_center};
use babel_core::progression::player::PlayerData;
use babel_core::world::grid::{HEIGHT, Pos, WIDTH};
use babel_core::world::site::{HOME_LAMP, HOME_PLOTS};

pub const VIEW_W: i32 = 480;
pub const VIEW_H: i32 = 270;
/// One extra pixel on each side hides the sub-pixel camera offset.
pub const SCENE_W: i32 = VIEW_W + 2;
pub const SCENE_H: i32 = VIEW_H + 2;

/// Smoothed camera holding the world position of the view's top-left corner.
#[derive(Default, Clone, Copy)]
pub struct Camera {
    pub pos: V2,
    pub shake: V2,
}

impl Camera {
    fn target(game: &Game, aim: V2) -> V2 {
        let site_w = (WIDTH * T) as f32;
        let site_h = (HEIGHT * T) as f32;
        let focus = game.body.pos + aim * 14.0;
        v2(
            (focus.x - VIEW_W as f32 / 2.0).clamp(-28.0, site_w - VIEW_W as f32 + 28.0),
            (focus.y - VIEW_H as f32 / 2.0).clamp(-34.0, site_h - VIEW_H as f32 + 30.0),
        )
    }

    pub fn snap(&mut self, game: &Game) {
        self.pos = Camera::target(game, V2::ZERO);
    }

    pub fn follow(&mut self, game: &Game, aim: V2, dt: f32) {
        let target = Camera::target(game, aim);
        let t = 1.0 - (-dt * 9.0).exp();
        self.pos += (target - self.pos) * t;
    }

    /// Whole-pixel origin the scene is drawn from.
    pub fn origin(&self) -> (i32, i32) {
        let p = self.pos + self.shake;
        (p.x.floor() as i32, p.y.floor() as i32)
    }

    /// Fraction applied when the scene is upscaled, never to the world.
    pub fn fraction(&self) -> V2 {
        let p = self.pos + self.shake;
        v2(p.x - p.x.floor(), p.y - p.y.floor())
    }

    /// World position under a point of the 480×270 view.
    pub fn to_world(self, view: V2) -> V2 {
        view + self.pos + self.shake
    }

    /// View position (for unlit overlays) of a world point.
    pub fn to_view(self, world: V2) -> V2 {
        world - self.pos - self.shake
    }
}

pub struct Layers<'a> {
    pub scene: &'a mut Canvas,
    pub emissive: &'a mut Canvas,
}

#[derive(Clone, Copy)]
enum Item {
    Wall(usize),
    Node(usize),
    Chest(usize),
    Lamp(usize),
    Torch(usize),
    ReturnLamp,
    Station(Station, u8),
    Enemy(usize),
    Player,
    Drop(usize),
    Shot(usize),
    Flame(usize),
}

/// Sprite direction and mirroring for a facing vector.
fn facing_of(facing: V2) -> (&'static str, bool) {
    if facing.x.abs() > facing.y.abs() {
        ("side", facing.x < 0.0)
    } else if facing.y < 0.0 {
        ("up", false)
    } else {
        ("down", false)
    }
}

fn color3(c: [u8; 3]) -> Rgba {
    rgb(c[0], c[1], c[2])
}

/// Canonical objects shimmer through the spectrum.
pub fn rarity_color(r: Rarity, time: f32) -> Rgba {
    if r == Rarity::Canonical {
        let wave = |phase: f32| (200.0 + 55.0 * (time * 3.0 + phase).sin()) as u8;
        rgb(wave(0.0), wave(2.1), wave(4.2))
    } else {
        color3(r.color())
    }
}

struct Painter<'a> {
    atlas: &'a Atlas,
    ox: i32,
    oy: i32,
    time: f32,
}

impl Painter<'_> {
    fn sx(&self, wx: f32) -> i32 {
        wx.round() as i32 - self.ox
    }

    fn sy(&self, wy: f32) -> i32 {
        wy.round() as i32 - self.oy
    }

    /// Draws a sprite standing on `(wx, wy)`.
    fn stand(&self, c: &mut Canvas, name: &str, frame: usize, wx: f32, wy: f32, opts: Blit) {
        let f = self.atlas.get(name, frame);
        let w = (f.w as f32 * opts.scale).round() as i32;
        let h = (f.h as f32 * opts.scale).round() as i32;
        c.sprite(self.atlas, f, self.sx(wx) - w / 2, self.sy(wy) - h, opts);
    }

    fn shadow(&self, c: &mut Canvas, pos: V2, rx: f32, z: f32) {
        // Higher objects cast smaller, fainter shadows.
        let shrink = 1.0 / (1.0 + z / 22.0);
        c.ellipse(
            pos.x - self.ox as f32,
            pos.y - self.oy as f32,
            rx * shrink,
            rx * 0.42 * shrink,
            [26, 16, 40, (92.0 * (0.5 + shrink * 0.5)) as u8],
        );
    }

    fn glow(&self, e: &mut Canvas, pos: V2, radius: f32, color: Rgba, alpha: u8) {
        e.glow(
            pos.x - self.ox as f32,
            pos.y - self.oy as f32,
            radius,
            with_alpha(color, alpha),
        );
    }

    fn telegraph(&self, c: &mut Canvas, game: &Game, e: &Enemy) {
        let Ai::Windup {
            t,
            total,
            dir,
            target,
        } = e.ai
        else {
            return;
        };
        let progress = 1.0 - (t / total.max(0.01)).clamp(0.0, 1.0);
        let warn = [255, 86, 86, (80.0 + progress * 150.0) as u8];
        let to_screen = |p: V2| (p.x - self.ox as f32, p.y - self.oy as f32);
        let line = |c: &mut Canvas, length: f32| {
            let a = to_screen(e.pos);
            let side = dir.perp() * 4.0;
            for offset in [side, side * -1.0] {
                let b = to_screen(e.pos + dir * (length * progress) + offset);
                c.line(
                    (a.0 + offset.x) as i32,
                    (a.1 + offset.y) as i32,
                    b.0 as i32,
                    b.1 as i32,
                    warn,
                );
            }
        };
        let circle = |c: &mut Canvas, center: V2, radius: f32| {
            let (x, y) = to_screen(center);
            c.ring(x, y, radius, radius * 0.8, warn);
            c.ellipse(
                x,
                y,
                radius * progress,
                radius * 0.8 * progress,
                [255, 86, 86, 52],
            );
        };
        if e.boss.is_some() {
            circle(c, e.pos, e.radius + 8.0);
            if game
                .enemies
                .iter()
                .any(|b| b.uid == e.uid && matches!(b.ai, Ai::Windup { .. }))
            {
                line(c, 90.0);
            }
            return;
        }
        match e.behavior {
            Behavior::Charger => line(c, 100.0),
            Behavior::Patroller => line(c, 112.0),
            Behavior::Beast => line(c, 60.0),
            Behavior::Burner => circle(c, target, 26.0),
            Behavior::Ambusher => circle(c, target, 12.0),
            Behavior::Tank => circle(c, e.pos, 30.0 + e.radius),
            Behavior::Thrower => line(c, 46.0),
            Behavior::Erratic | Behavior::Phantom | Behavior::Swarm => {
                circle(c, e.pos, e.radius + 6.0);
            }
        }
    }

    /// Mirrors one standing sprite below its feet, only over water.
    fn reflect(&self, c: &mut Canvas, art: &SiteArt, name: &str, frame: usize, at: V2, flip: bool) {
        let f = self.atlas.get(name, frame);
        let (left, feet) = (self.sx(at.x) - f.w / 2, self.sy(at.y));
        for y in 0..f.h {
            let ripple = ((self.time * 5.0 + y as f32 * 0.9).sin() * 1.1).round() as i32;
            for x in 0..f.w {
                let sx = if flip { f.w - 1 - x } else { x };
                let pixel = self.atlas.pixel(f, sx, f.h - 1 - y);
                if pixel[3] == 0 {
                    continue;
                }
                let (px, py) = (left + x + ripple, feet + y - 2);
                let tile = Pos::new((px + self.ox).div_euclid(T), (py + self.oy).div_euclid(T));
                if art.hazard_tiles.contains(&tile) {
                    let tinted = mix(pixel, rgb(80, 180, 235), 0.4);
                    c.put(px, py, with_alpha(tinted, (130 - y * 4).max(30) as u8));
                }
            }
        }
    }

    /// Puddles reflect the player, creatures and lamps.
    fn reflections(&self, c: &mut Canvas, art: &SiteArt, game: &Game) {
        let b = &game.body;
        let (dir, flip) = facing_of(b.facing);
        self.reflect(
            c,
            art,
            &format!("player_idle_{dir}"),
            (self.time * 3.5) as usize,
            b.pos + v2(0.0, 5.0),
            flip,
        );
        for e in game.enemies.iter().filter(|e| e.visible()) {
            self.reflect(
                c,
                art,
                &e.sprite,
                (e.anim * 6.0) as usize,
                e.pos + v2(0.0, 5.0),
                e.facing.x < -0.15,
            );
        }
        for lamp in &game.site.lamps {
            let (x, y) = lamp.center();
            self.reflect(c, art, "lamp", 0, v2(x, y + 5.0), false);
        }
    }

    /// Small head ornament that tells word variants of an archetype apart.
    fn accessory(&self, c: &mut Canvas, kind: u32, x: i32, y: i32, alpha: u8) {
        let (pixels, color): (&[(i32, i32)], Rgba) = match kind % 4 {
            // Horns.
            1 => (
                &[(-5, 0), (-4, 1), (-4, 2), (4, 0), (3, 1), (3, 2)],
                [248, 234, 200, 255],
            ),
            // A ribbon bookmark.
            2 => (
                &[(1, -1), (2, -1), (3, -1), (2, 0), (2, 1), (3, -2), (1, -2)],
                [238, 87, 76, 255],
            ),
            // A feather crest.
            3 => (
                &[(0, 1), (0, 0), (0, -1), (1, -2), (-1, -2), (2, -1), (1, 0)],
                [72, 220, 196, 255],
            ),
            _ => return,
        };
        for &(dx, dy) in pixels {
            for (ox, oy) in [(-1, 0), (1, 0), (0, -1)] {
                if !pixels.contains(&(dx + ox, dy + oy)) {
                    c.put(x + dx + ox, y + dy + oy, [26, 16, 40, alpha]);
                }
            }
        }
        for &(dx, dy) in pixels {
            c.put(x + dx, y + dy, with_alpha(color, alpha));
        }
    }

    fn enemy(&self, layers: &mut Layers, e: &Enemy, fx: &mut Fx, emit: bool) {
        let flying = matches!(
            e.behavior,
            Behavior::Erratic | Behavior::Swarm | Behavior::Phantom
        ) && e.boss.is_none();
        let hover = if flying {
            6.0 + (e.anim * 5.0).sin() * 2.0
        } else {
            0.0
        };
        let hidden = matches!(e.ai, Ai::Hidden);
        let speed = if matches!(e.ai, Ai::Attack { .. }) {
            12.0
        } else {
            6.0
        };
        let frame = if hidden { 0 } else { (e.anim * speed) as usize };
        let windup = matches!(e.ai, Ai::Windup { .. });
        // Trembling during the windup reinforces the telegraph.
        let jitter = if windup {
            ((self.time * 50.0).sin() * 1.2).round()
        } else {
            0.0
        };
        let squash = if windup { 1.06 } else { 1.0 };
        let mut alpha = 255u8;
        if hidden {
            alpha = 120;
        }
        if e.illusion {
            // Reflections are pale and waver.
            alpha = (150.0 + (self.time * 6.0 + e.anim).sin() * 40.0) as u8;
        }
        if e.behavior == Behavior::Phantom || e.has(ModFlag::PhaseWalls) {
            alpha = alpha.min(190);
        }
        if e.grace > 0.0 && (self.time * 14.0).sin() > 0.0 {
            alpha = alpha.min(140);
        }
        let opts = Blit {
            flip: e.facing.x < -0.15,
            alpha,
            tint: e.tint.map(|t| (color3(t), 0.38)),
            solid: (e.flash > 0.0).then_some([255, 255, 255, 255]),
            variant: if e.boss.is_some() { 0 } else { e.variant },
            scale: e.scale * squash,
            sway: 0,
        };
        let feet = e.pos.y + (e.radius * 0.7).min(8.0) - e.z - hover;
        if e.has(ModFlag::Glows) && emit {
            let c = e.tint.map_or(rgb(200, 220, 255), color3);
            self.glow(
                layers.emissive,
                e.pos - v2(0.0, 7.0 + e.z),
                e.radius * 2.1,
                c,
                70,
            );
        }
        self.stand(layers.scene, &e.sprite, frame, e.pos.x + jitter, feet, opts);
        if e.boss.is_none() && !hidden {
            let body = self.atlas.get(&e.sprite, frame);
            let top = self.sy(feet) - (body.h as f32 * opts.scale) as i32;
            self.accessory(
                layers.scene,
                e.variant / 6,
                self.sx(e.pos.x + jitter),
                top + 1,
                alpha,
            );
        }
        if e.elite.is_some() {
            self.glow(
                layers.emissive,
                e.pos + v2(0.0, 2.0),
                e.radius * 1.7,
                e.elite
                    .as_ref()
                    .map_or(rgb(255, 255, 255), |x| rarity_color(x.rarity, self.time)),
                60,
            );
        }
        if e.shield {
            let (x, y) = (
                e.pos.x - self.ox as f32,
                e.pos.y - self.oy as f32 - 6.0 - e.z,
            );
            layers
                .emissive
                .ring(x, y, e.radius + 5.0, e.radius + 5.0, [255, 250, 210, 190]);
        }
        if e.has(ModFlag::Armored) {
            let (x, y) = (e.pos.x - self.ox as f32, e.pos.y - self.oy as f32 + 3.0);
            layers
                .scene
                .ring(x, y, e.radius + 3.0, e.radius * 0.45, [190, 204, 226, 210]);
        }
        // Auras tell modifiers and statuses apart at a glance.
        let mut auras: Vec<([u8; 3], f32, bool)> = Vec::new();
        for flag in &e.flags {
            let aura = match flag {
                ModFlag::BurnTrail | ModFlag::BurnOnHit => Some(([255, 140, 40], 34.0, true)),
                ModFlag::DarkOnly => Some(([70, 40, 120], -6.0, false)),
                ModFlag::PoisonOnHit => Some(([140, 230, 70], 20.0, false)),
                ModFlag::SlowOnHit => Some(([200, 240, 255], -14.0, true)),
                ModFlag::Regenerates => Some(([110, 255, 160], 26.0, true)),
                ModFlag::Lifesteal => Some(([220, 40, 90], 10.0, false)),
                ModFlag::Shock => Some(([140, 240, 255], 0.0, true)),
                ModFlag::ExtraLoot => Some(([255, 220, 80], 18.0, true)),
                ModFlag::DrainsEnergy => Some(([180, 70, 210], 14.0, false)),
                ModFlag::Explodes => Some(([255, 200, 60], 44.0, true)),
                ModFlag::Thorns => Some(([200, 150, 90], -4.0, false)),
                ModFlag::Dodges => Some(([140, 240, 220], 4.0, false)),
                ModFlag::Enrages if e.enraged() => Some(([255, 60, 40], 40.0, true)),
                ModFlag::HeavyKnockback => Some(([150, 130, 150], -20.0, false)),
                _ => None,
            };
            auras.extend(aura);
        }
        if e.status.burn > 0.0 {
            auras.push(([255, 150, 40], 48.0, true));
        }
        if e.status.poison > 0.0 {
            auras.push(([150, 240, 60], 22.0, false));
        }
        if e.status.slow > 0.0 {
            auras.push(([180, 230, 255], -10.0, false));
        }
        if emit {
            for (color, rise, glow) in auras {
                if fx.random() < 0.10 {
                    let dx = fx.spread(e.radius);
                    let dy = fx.spread(4.0);
                    let drift = fx.spread(6.0);
                    fx.spawn(Particle {
                        pos: e.pos + v2(dx, dy),
                        z: 6.0 + e.z,
                        vel: v2(drift, 0.0),
                        vz: rise,
                        color,
                        life: 0.5,
                        max_life: 0.5,
                        glow,
                        gravity: 0.0,
                    });
                }
            }
        }
    }

    fn player(&self, layers: &mut Layers, game: &Game, ghost: bool) {
        let b = &game.body;
        let (dir, flip) = facing_of(b.facing);
        let (name, frame) = if b.moving {
            (format!("player_walk_{dir}"), (b.anim * 11.0) as usize)
        } else {
            (format!("player_idle_{dir}"), (self.time * 3.5) as usize)
        };
        let blink = b.invulnerable > 0.0 && b.dash_time <= 0.0 && (self.time * 22.0).sin() > 0.3;
        let mut opts = Blit {
            flip,
            alpha: if blink { 110 } else { 255 },
            solid: (b.flash > 0.0).then_some([255, 255, 255, 255]),
            ..Default::default()
        };
        if b.status.poison > 0.0 {
            opts.tint = Some((rgb(120, 230, 80), 0.3));
        }
        if b.status.slow > 0.0 {
            opts.tint = Some((rgb(150, 210, 255), 0.35));
        }
        if ghost {
            opts.solid = Some([170, 220, 255, 90]);
            self.stand(layers.scene, &name, frame, b.pos.x, b.pos.y + 5.0, opts);
            return;
        }
        if b.dead > 0.0 {
            let sink = (1.6 - b.dead).max(0.0);
            opts.alpha = (255.0 * (1.0 - sink / 1.6).clamp(0.0, 1.0)) as u8;
            opts.solid = Some([180, 150, 230, 255]);
        }
        if b.dash_time > 0.0 {
            for k in 1..4 {
                let trail = Blit {
                    alpha: 90 - k * 22,
                    solid: Some([150, 220, 255, 255]),
                    ..opts
                };
                let p = b.pos - b.dash_dir * (f32::from(k) * 7.0);
                self.stand(layers.scene, &name, frame, p.x, p.y + 5.0, trail);
            }
        }
        let swing_behind = b.swing.is_some_and(|s| s.dir.y < -0.3);
        if swing_behind {
            self.swing(layers, game);
        }
        self.stand(layers.scene, &name, frame, b.pos.x, b.pos.y + 5.0, opts);
        if !swing_behind {
            self.swing(layers, game);
        }
        if b.charge > 0.2 {
            let full = (b.charge / 0.55).min(1.0);
            let (x, y) = (b.pos.x - self.ox as f32, b.pos.y - self.oy as f32);
            let color = if full >= 1.0 {
                [255, 232, 120, 230]
            } else {
                [255, 255, 255, 120]
            };
            layers
                .emissive
                .ring(x, y, 6.0 + full * 8.0, 3.0 + full * 4.0, color);
        }
    }

    /// The weapon arc: a crescent sweeping along the attack direction.
    fn swing(&self, layers: &mut Layers, game: &Game) {
        let Some(s) = game.body.swing else {
            return;
        };
        let progress = (s.t / s.duration).clamp(0.0, 1.0);
        let spin = s.charged || s.step == 3;
        let radius = if s.charged { 34.0 } else { 21.0 };
        let sweep = if spin { std::f32::consts::TAU } else { 2.4 };
        let handed = if s.step % 2 == 0 { 1.0 } else { -1.0 };
        let base = s.dir.angle() - handed * sweep / 2.0;
        let head = (progress * 1.8).min(1.0);
        let center = game.body.pos + v2(0.0, -6.0);
        let color: Rgba = match (s.charged, s.step) {
            (true, _) => [255, 224, 110, 235],
            (_, 2 | 3) => [255, 196, 140, 225],
            _ => [236, 250, 255, 200],
        };
        let samples = (sweep * radius * 0.8) as i32;
        for i in 0..samples {
            let u = i as f32 / samples as f32;
            if u > head || u < head - 0.65 {
                continue;
            }
            let angle = base + handed * sweep * u;
            let age = (head - u) / 0.65;
            let width = (4.0 * (1.0 - age)).max(1.0) as i32;
            for k in 0..width {
                let p = center + V2::from_angle(angle) * (radius - k as f32);
                let a = (f32::from(color[3]) * (1.0 - age) * (1.0 - progress * 0.5)) as u8;
                layers
                    .emissive
                    .put(self.sx(p.x), self.sy(p.y), with_alpha(color, a));
            }
        }
    }
}

fn station_sprite(s: Station) -> &'static str {
    match s {
        Station::Binder => "station_binder",
        Station::Forge => "station_forge",
        Station::Alembic => "station_alembic",
        Station::Scriptorium => "station_scriptorium",
        Station::Chest => "station_chest",
    }
}

pub fn drop_icon(kind: &DropKind) -> String {
    use babel_core::data::{Consumable, Slot};
    match kind {
        DropKind::Resource(r, _) => format!("res_{}", format!("{r:?}").to_lowercase()),
        DropKind::Consumable(c, _) => match c {
            Consumable::Torch => "icon_torch",
            Consumable::Tonic => "icon_tonic",
            Consumable::PageAmmo => "icon_ammo",
            Consumable::Index => "icon_index",
        }
        .into(),
        DropKind::Item(i) => match i.slot {
            Slot::Weapon => "icon_weapon",
            Slot::Armor => "icon_armor",
            Slot::Boots => "icon_boots",
            Slot::Lamp => "icon_lantern",
            Slot::Charm => "icon_charm",
        }
        .into(),
    }
}

/// Renders the whole world. `advance` is false while the game is frozen so no
/// new cosmetic particles pile up behind menus.
pub fn draw(
    layers: &mut Layers,
    atlas: &Atlas,
    art: &SiteArt,
    game: &Game,
    camera: &Camera,
    fx: &mut Fx,
    time: f32,
    advance: bool,
) {
    let (ox, oy) = camera.origin();
    let p = Painter {
        atlas,
        ox,
        oy,
        time,
    };
    layers.emissive.clear([0; 4]);
    layers.scene.clear(NIGHT);
    // Faint far galleries beyond the hexagon.
    for y in 0..SCENE_H {
        for x in 0..SCENE_W {
            let wx = x + ox / 2;
            let wy = y + oy / 2;
            if noise(0xBABE, wx / 3, wy / 3).is_multiple_of(190) && wx % 3 == 0 {
                layers.scene.set(x, y, [52, 36, 78, 255]);
            }
        }
    }
    // Ground layer.
    for y in 0..SCENE_H {
        for x in 0..SCENE_W {
            let c = art.ground.get(x + ox, y + oy);
            if c[3] != 0 {
                layers.scene.put(x, y, c);
            }
        }
    }
    let hazard_frame = (time * 5.0) as usize % art.hazard_frames.len().max(1);
    for tile in &art.hazard_tiles {
        let (hx, hy) = (tile.x * T - ox, tile.y * T - oy);
        if hx < -T || hy < -T || hx > SCENE_W || hy > SCENE_H {
            continue;
        }
        if let Some(frame) = art.hazard_frames.get(hazard_frame) {
            for y in 0..T {
                for x in 0..T {
                    let c = frame.get(x, y);
                    if c[3] == 0 {
                        continue;
                    }
                    layers.scene.put(hx + x, hy + y, c);
                    if art.hazard == babel_core::data::Hazard::Embers && c[0] == 255 && c[1] > 150 {
                        layers.emissive.put(hx + x, hy + y, c);
                    }
                }
            }
        }
    }

    let biome = &game.data.biomes[game.site.recipe.biome];
    for d in &game.site.decor {
        let Some(kind) = biome.decor.get(d.kind as usize) else {
            continue;
        };
        let (cx, cy) = d.pos.center();
        let jx = f32::from(d.variant) * 2.0 - 3.0;
        let sway = if matches!(kind.as_str(), "grass" | "flower" | "moss") {
            ((time * 1.8 + d.pos.x as f32 * 0.7).sin() * 1.2).round() as i32
        } else {
            0
        };
        p.stand(
            layers.scene,
            &format!("decor_{kind}"),
            d.variant as usize,
            cx + jx,
            cy + 5.0,
            Blit {
                alpha: if kind == "rug" { 190 } else { 235 },
                flip: d.variant % 2 == 1,
                sway,
                ..Default::default()
            },
        );
    }

    let delta = game.delta();
    if let Some(s) = game.site.stairs {
        let c = tile_center(s);
        p.stand(
            layers.scene,
            "stairs_down",
            0,
            c.x,
            c.y + 8.0,
            Blit::default(),
        );
        p.glow(layers.emissive, c, 12.0, rgb(150, 120, 255), 60);
        if game.stairs_locked() {
            p.stand(
                layers.scene,
                "icon_lock",
                0,
                c.x,
                c.y + 4.0,
                Blit::default(),
            );
        }
    }
    if game.site.key.is_remote() {
        let c = tile_center(game.site.hub);
        p.stand(
            layers.scene,
            "portal",
            (time * 5.0) as usize,
            c.x,
            c.y + 7.0,
            Blit::default(),
        );
        p.glow(layers.emissive, c, 20.0, rgb(150, 220, 255), 90);
    } else if let Some(s) = delta.stairs_up {
        let c = tile_center(s);
        p.stand(
            layers.scene,
            "stairs_up",
            0,
            c.x,
            c.y + 8.0,
            Blit::default(),
        );
    }
    if game.site.key.is_home() {
        for (i, &plot) in HOME_PLOTS.iter().enumerate() {
            if !game.profile.world.stations.iter().any(|s| s.1 == i as u8) {
                let c = tile_center(plot);
                let pulse = (190.0 + (time * 2.4 + i as f32).sin() * 50.0) as u8;
                // Unlit, so building spots read in any light.
                p.stand(
                    layers.emissive,
                    "plot",
                    0,
                    c.x,
                    c.y + 7.0,
                    Blit {
                        alpha: pulse,
                        ..Default::default()
                    },
                );
                p.stand(
                    layers.emissive,
                    "icon_hammer",
                    0,
                    c.x,
                    c.y + 4.0,
                    Blit {
                        alpha: pulse,
                        ..Default::default()
                    },
                );
            }
        }
    }
    if let Some(bag) = &game.profile.world.death_bag
        && bag.site.id() == game.site.key.id()
    {
        let bob = (time * 3.0).sin() * 1.5;
        p.shadow(layers.scene, v2(bag.x, bag.y), 7.0, 0.0);
        p.stand(
            layers.scene,
            "death_bag",
            0,
            bag.x,
            bag.y + 3.0 + bob,
            Blit::default(),
        );
        p.glow(
            layers.emissive,
            v2(bag.x, bag.y - 3.0),
            10.0,
            rgb(255, 210, 90),
            90,
        );
    }

    // Ground telegraphs for zones.
    for z in &game.zones {
        let (x, y) = (z.pos.x - ox as f32, z.pos.y - oy as f32);
        if z.delay > 0.0 {
            let progress = 1.0 - (z.delay / z.total_delay.max(0.01)).clamp(0.0, 1.0);
            layers
                .emissive
                .ring(x, y, z.radius, z.radius * 0.8, [255, 92, 80, 215]);
            layers.scene.ellipse(
                x,
                y,
                z.radius * progress,
                z.radius * 0.8 * progress,
                [255, 80, 70, 70],
            );
        } else {
            match z.kind {
                ZoneKind::Fire => {
                    layers
                        .scene
                        .ellipse(x, y, z.radius, z.radius * 0.8, [255, 110, 30, 96]);
                }
                ZoneKind::Slam | ZoneKind::Explosion => {
                    let grow = 1.0 - (z.ttl / 0.25).clamp(0.0, 1.0);
                    let r = z.radius * (0.5 + grow * 0.5);
                    let color = if z.kind == ZoneKind::Explosion {
                        [255, 200, 90, 235]
                    } else {
                        [232, 214, 255, 220]
                    };
                    layers.emissive.ring(x, y, r, r * 0.8, color);
                    layers.emissive.ring(x, y, r - 2.0, r * 0.8 - 2.0, color);
                }
            }
        }
    }
    for e in game.enemies.iter().filter(|e| e.visible()) {
        p.telegraph(layers.scene, game, e);
    }

    if art.hazard == babel_core::data::Hazard::Water {
        p.reflections(layers.scene, art, game);
    }

    // Shadows stay on the ground whatever the height above them.
    p.shadow(layers.scene, game.body.pos + v2(0.0, 4.0), 7.5, game.body.z);
    for e in game.enemies.iter().filter(|e| e.visible()) {
        let flying = matches!(
            e.behavior,
            Behavior::Erratic | Behavior::Swarm | Behavior::Phantom
        ) && e.boss.is_none();
        let z = e.z + if flying { 6.0 } else { 0.0 };
        p.shadow(
            layers.scene,
            e.pos + v2(0.0, (e.radius * 0.6).min(7.0)),
            e.radius * 1.15,
            z,
        );
    }
    for n in &game.nodes {
        p.shadow(layers.scene, n.pos + v2(0.0, 6.0), 9.0, 0.0);
    }
    for c in &game.chests {
        p.shadow(layers.scene, c.pos + v2(0.0, 6.0), 9.0, 0.0);
    }
    for d in &game.drops {
        p.shadow(layers.scene, d.pos + v2(0.0, 2.0), 5.0, d.z);
    }
    for s in &game.projectiles {
        p.shadow(layers.scene, s.pos + v2(0.0, 2.0), 3.5, s.z);
    }

    // Y-sorted actors and tall objects.
    let visible = |wx: f32, wy: f32| {
        wx > ox as f32 - 48.0
            && wx < (ox + SCENE_W) as f32 + 48.0
            && wy > oy as f32 - 48.0
            && wy < (oy + SCENE_H) as f32 + 64.0
    };
    let mut order: Vec<(f32, Item)> = Vec::with_capacity(256);
    for (i, (tile, _)) in art.walls.iter().enumerate() {
        let (cx, cy) = tile.center();
        if visible(cx, cy) {
            order.push(((tile.y + 1) as f32 * T as f32 - 3.0, Item::Wall(i)));
        }
    }
    order.extend(
        game.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| visible(n.pos.x, n.pos.y))
            .map(|(i, n)| (n.pos.y + 6.0, Item::Node(i))),
    );
    order.extend(
        game.chests
            .iter()
            .enumerate()
            .map(|(i, c)| (c.pos.y + 6.0, Item::Chest(i))),
    );
    order.extend(
        game.site
            .lamps
            .iter()
            .enumerate()
            .map(|(i, l)| (l.center().1 + 3.0, Item::Lamp(i))),
    );
    order.extend(
        delta
            .torches
            .iter()
            .enumerate()
            .map(|(i, t)| (t.center().1 + 2.0, Item::Torch(i))),
    );
    if let Some(l) = game.lamp_here() {
        order.push((l.y + 4.0, Item::ReturnLamp));
    }
    if game.site.key.is_home() {
        for &(station, plot) in &game.profile.world.stations {
            if let Some(&tile) = HOME_PLOTS.get(plot as usize) {
                order.push((tile.center().1 + 6.0, Item::Station(station, plot)));
            }
        }
    }
    order.extend(
        game.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.visible())
            .map(|(i, e)| (e.pos.y + (e.radius * 0.6).min(7.0), Item::Enemy(i))),
    );
    order.push((game.body.pos.y + 4.0, Item::Player));
    order.extend(
        game.drops
            .iter()
            .enumerate()
            .map(|(i, d)| (d.pos.y + 2.0, Item::Drop(i))),
    );
    order.extend(
        game.projectiles
            .iter()
            .enumerate()
            .map(|(i, s)| (s.pos.y + 3.0, Item::Shot(i))),
    );
    order.extend(
        game.zones
            .iter()
            .enumerate()
            .filter(|(_, z)| z.kind == ZoneKind::Fire && z.delay <= 0.0)
            .map(|(i, z)| (z.pos.y + 4.0, Item::Flame(i))),
    );
    order.sort_by(|a, b| a.0.total_cmp(&b.0));

    let player_key = game.body.pos.y + 4.0;
    let mut player_hidden = false;
    for &(key, item) in &order {
        match item {
            Item::Wall(i) => {
                let (tile, sprite) = &art.walls[i];
                let (wx, wy) = (tile.x * T - ox, tile.y * T - WALL_RISE - oy);
                for y in 0..sprite.h {
                    for x in 0..sprite.w {
                        let c = sprite.get(x, y);
                        if c[3] != 0 {
                            layers.scene.put(wx + x, wy + y, c);
                            // Walls in front also hide what glows behind them.
                            layers.emissive.set(wx + x, wy + y, [0; 4]);
                        }
                    }
                }
                let head = game.body.pos - v2(0.0, 6.0);
                if key > player_key
                    && (head.x - (tile.x * T + 8) as f32).abs() < 12.0
                    && head.y > (tile.y * T - WALL_RISE) as f32
                    && head.y < ((tile.y + 1) * T) as f32
                {
                    player_hidden = true;
                }
            }
            Item::Node(i) => {
                let n = &game.nodes[i];
                let def = game.data.node(n.kind);
                let shake = if n.shake > 0.0 {
                    ((n.shake * 90.0).sin() * 2.0).round()
                } else {
                    0.0
                };
                let sway = if matches!(def.sprite.as_str(), "node_vine" | "node_goldvine") {
                    ((time * 1.6 + n.pos.x * 0.05).sin() * 1.4).round() as i32
                } else {
                    0
                };
                let locked = game.profile.player.tool_tier(def.tool) < def.min_tier;
                p.stand(
                    layers.scene,
                    &def.sprite,
                    0,
                    n.pos.x + shake,
                    n.pos.y + 9.0,
                    Blit {
                        sway,
                        solid: (n.shake > 0.14 && !locked).then_some([255, 255, 255, 255]),
                        ..Default::default()
                    },
                );
                if locked {
                    // A clear padlock marks nodes that need a better tool.
                    p.stand(
                        layers.emissive,
                        "icon_lock",
                        0,
                        n.pos.x + 6.0,
                        n.pos.y - 4.0,
                        Blit {
                            alpha: 215,
                            ..Default::default()
                        },
                    );
                }
                if matches!(
                    def.sprite.as_str(),
                    "node_mirror" | "node_vitral" | "node_goldvine" | "node_codex"
                ) {
                    let sparkle = (time * 2.0 + n.pos.x).sin().max(0.0);
                    p.glow(
                        layers.emissive,
                        n.pos - v2(2.0, 6.0),
                        4.0,
                        rgb(255, 255, 240),
                        (sparkle * 150.0) as u8,
                    );
                }
            }
            Item::Chest(i) => {
                let c = &game.chests[i];
                let name = if c.open { "chest_open" } else { "chest" };
                p.stand(
                    layers.scene,
                    name,
                    0,
                    c.pos.x,
                    c.pos.y + 8.0,
                    Blit::default(),
                );
                if !c.open {
                    p.glow(
                        layers.emissive,
                        c.pos - v2(0.0, 4.0),
                        11.0,
                        rgb(255, 214, 90),
                        (60.0 + (time * 3.0).sin() * 30.0) as u8,
                    );
                }
            }
            Item::Lamp(i) => {
                let (x, y) = game.site.lamps[i].center();
                let sway = ((time * 1.3 + x * 0.11).sin() * 1.1).round() as i32;
                p.stand(
                    layers.scene,
                    "lamp",
                    (time * 6.0 + x) as usize,
                    x,
                    y + 5.0,
                    Blit {
                        sway,
                        ..Default::default()
                    },
                );
                let lit = if game.darkness > 0.0 { 30 } else { 115 };
                p.glow(
                    layers.emissive,
                    v2(x + sway as f32, y - 8.0),
                    7.0,
                    rgb(255, 214, 120),
                    lit,
                );
            }
            Item::Torch(i) => {
                let (x, y) = delta.torches[i].center();
                p.stand(
                    layers.scene,
                    "torch",
                    (time * 9.0 + x) as usize,
                    x,
                    y + 5.0,
                    Blit::default(),
                );
                p.glow(layers.emissive, v2(x, y - 6.0), 7.0, rgb(255, 170, 60), 150);
                if advance && fx.random() < 0.05 {
                    fx.burst(v2(x, y), 10.0, [255, 190, 80], 1, 10.0);
                }
            }
            Item::ReturnLamp => {
                if let Some(l) = game.lamp_here() {
                    p.stand(
                        layers.scene,
                        "return_lamp",
                        (time * 4.0) as usize,
                        l.x,
                        l.y + 7.0,
                        Blit::default(),
                    );
                    p.glow(
                        layers.emissive,
                        l - v2(0.0, 7.0),
                        9.0,
                        rgb(150, 245, 255),
                        130,
                    );
                }
                let _ = HOME_LAMP;
            }
            Item::Station(station, plot) => {
                if let Some(&tile) = HOME_PLOTS.get(plot as usize) {
                    let c = tile_center(tile);
                    p.shadow(layers.scene, c + v2(0.0, 6.0), 13.0, 0.0);
                    p.stand(
                        layers.scene,
                        station_sprite(station),
                        0,
                        c.x,
                        c.y + 9.0,
                        Blit::default(),
                    );
                    if station == Station::Forge {
                        p.glow(
                            layers.emissive,
                            c + v2(-3.0, 0.0),
                            6.0,
                            rgb(255, 150, 40),
                            110,
                        );
                    }
                }
            }
            Item::Enemy(i) => p.enemy(layers, &game.enemies[i], fx, advance),
            Item::Player => p.player(layers, game, false),
            Item::Drop(i) => {
                let d = &game.drops[i];
                let rarity = d.rarity();
                let bob = if d.z <= 0.1 {
                    (time * 4.0 + d.pos.x).sin() * 1.2 + 1.2
                } else {
                    0.0
                };
                let at = v2(d.pos.x, d.pos.y + 4.0 - d.z - bob);
                if matches!(d.kind, DropKind::Item(_)) {
                    let color = rarity_color(rarity, time);
                    let radius = 8.0 + rarity.tier() as f32 * 2.0;
                    p.glow(layers.emissive, at - v2(0.0, 5.0), radius, color, 150);
                    if rarity >= Rarity::Rare {
                        // A beam makes valuable loot readable from afar.
                        let x = p.sx(at.x);
                        let top = p.sy(at.y) - 14 - rarity.tier() as i32 * 5;
                        for y in top..p.sy(at.y) - 8 {
                            let fade = (y - top) as f32 / 20.0;
                            layers.emissive.put(
                                x,
                                y,
                                with_alpha(color, (fade.min(1.0) * 150.0) as u8),
                            );
                        }
                    }
                }
                p.stand(
                    layers.scene,
                    &drop_icon(&d.kind),
                    0,
                    at.x,
                    at.y,
                    Blit::default(),
                );
            }
            Item::Shot(i) => {
                let s = &game.projectiles[i];
                let at = v2(s.pos.x, s.pos.y - s.z);
                let (color, size) = match s.kind {
                    ShotKind::Page => (rgb(255, 246, 214), 3),
                    ShotKind::Ink => (rgb(190, 120, 255), 4),
                    ShotKind::Spark => (rgb(140, 240, 255), 2),
                    ShotKind::Flame => (rgb(255, 150, 40), 4),
                };
                p.glow(
                    layers.emissive,
                    at,
                    7.0,
                    if s.from_player {
                        rgb(190, 235, 255)
                    } else {
                        rgb(255, 120, 120)
                    },
                    110,
                );
                let (x, y) = (p.sx(at.x), p.sy(at.y));
                let flip = ((time * 18.0) as i32) % 2;
                layers.emissive.fill(
                    x - size / 2,
                    y - size / 2 + flip,
                    size,
                    (size - flip).max(2),
                    color,
                );
                layers
                    .scene
                    .fill(x - size / 2, y - size / 2, size, size, color);
            }
            Item::Flame(i) => {
                let z = &game.zones[i];
                let flames = (z.radius / 4.0) as i32;
                for k in 0..flames {
                    let h = noise(0xF1A, k, i as i32);
                    let angle = (h % 628) as f32 / 100.0;
                    let r = (h >> 10) % 100;
                    let at = z.pos + V2::from_angle(angle) * (z.radius * r as f32 / 115.0);
                    let flicker = (time * 14.0 + k as f32).sin();
                    let height = (5.0 + flicker * 2.5) as i32;
                    let (x, y) = (p.sx(at.x), p.sy(at.y));
                    for step in 0..height {
                        let t = step as f32 / height.max(1) as f32;
                        let color = mix(rgb(255, 230, 120), rgb(230, 70, 40), t);
                        let half = ((1.0 - t) * 2.0) as i32;
                        layers.emissive.fill(
                            x - half,
                            y - step,
                            half * 2 + 1,
                            1,
                            with_alpha(color, 230),
                        );
                    }
                }
            }
        }
    }
    if player_hidden {
        p.player(layers, game, true);
    }

    for part in &fx.particles {
        let x = (part.pos.x.round() as i32) - ox;
        let y = ((part.pos.y - part.z).round() as i32) - oy;
        let fade = (part.life / part.max_life.max(0.01)).clamp(0.0, 1.0);
        let color = with_alpha(color3(part.color), (255.0 * fade.min(0.8) / 0.8) as u8);
        let size = if fade > 0.55 { 2 } else { 1 };
        layers.scene.fill(x, y, size, size, color);
        if part.glow {
            layers.emissive.fill(x, y, size, size, color);
        }
    }
}

/// Names the tool shown in the HUD, matching the player's tier.
pub fn tool_icon(player: &PlayerData) -> &'static str {
    use babel_core::data::ToolKind;
    match player.active_tool {
        ToolKind::Knife => "icon_knife",
        ToolKind::Hammer => "icon_hammer",
        ToolKind::Lamp => "icon_lamp",
        ToolKind::Lens => "icon_lens",
    }
}
