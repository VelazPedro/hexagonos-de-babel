//! Player body: movement, dash, combo, charged strike, ranged pages,
//! harvesting and context interactions.

use super::actors::{Projectile, ShotKind, Status};
use super::coop::Exit;
use super::events::{GameEvent, Input, ScreenRequest, Sfx};
use super::pet::PetBody;
use super::physics::{self, Mobility};
use super::vec::{V2, v2};
use super::{Game, tile_center};
use crate::babel::seeds::Dice;
use crate::data::{
    Consumable, Hazard, NodeKind, Perk, Resource, Stat, ToolKind, Unlock, tier_name,
};
use crate::progression::stats::{self, Derived};
use crate::world::grid::{self, Pos, TILE, Tile};
use crate::world::hex::Dir;
use crate::world::site::{HOME_LAMP, HOME_PLOTS, SiteKey};
use serde::{Deserialize, Serialize};

pub const PLAYER_RADIUS: f32 = 5.0;
const COMBO_WINDOW: f32 = 0.45;
pub const CHARGE_TIME: f32 = 0.55;
const DASH_TIME: f32 = 0.17;
const DASH_SPEED: f32 = 265.0;
const DASH_COST: f32 = 18.0;
const CHARGE_COST: f32 = 25.0;
/// How hard each of these hits when held, next to the knife.
pub(crate) const HAMMER_DAMAGE: f32 = 0.85;
pub(crate) const LAMP_DAMAGE: f32 = 0.65;
/// A thrown page: its share of the player's damage and its price.
pub(crate) const PAGE_DAMAGE: f32 = 0.75;
pub(crate) const PAGE_ENERGY: f32 = 6.0;
/// Share of full health a tonic gives back.
pub(crate) const TONIC_HEAL: f32 = 0.45;

/// The share of the player's damage a thrown page deals.
pub(crate) fn page_share(derived: &Derived) -> f32 {
    PAGE_DAMAGE + derived.perk(Perk::PageDamage)
}

/// What throwing a page costs.
pub(crate) fn page_energy(derived: &Derived) -> f32 {
    (PAGE_ENERGY - derived.perk(Perk::PageThrift)).max(1.0)
}

/// The share of their health a tonic gives back.
pub(crate) fn tonic_share(derived: &Derived) -> f32 {
    TONIC_HEAL + derived.perk(Perk::TonicHeal)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Swing {
    /// Seconds since the swing started.
    pub t: f32,
    pub duration: f32,
    /// 0..=3 combo step.
    pub step: u8,
    pub charged: bool,
    pub dir: V2,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub pos: V2,
    pub vel: V2,
    pub knock: V2,
    pub facing: V2,
    pub z: f32,
    pub moving: bool,
    pub anim: f32,
    pub swing: Option<Swing>,
    pub combo: u8,
    pub combo_timer: f32,
    /// Seconds the attack button has been held.
    pub charge: f32,
    pub dash_time: f32,
    pub dash_dir: V2,
    pub dash_charges: u8,
    pub dash_recharge: f32,
    pub invulnerable: f32,
    pub flash: f32,
    pub status: Status,
    pub ranged_cooldown: f32,
    pub step_timer: f32,
    pub hazard_timer: f32,
    pub second_wind_used: bool,
    /// Positive while the death animation plays.
    pub dead: f32,
    /// Door transitions wait until the player has left the doorway.
    pub door_lock: bool,
    /// Lying down in co-op, until the partner raises them.
    pub downed: bool,
    /// Seconds of help received while down.
    pub revive: f32,
    /// In co-op, the way out this player wants to take.
    pub waiting: Option<Exit>,
    /// The pet walking with this player.
    pub pet: PetBody,
}

impl Body {
    pub fn new(pos: V2) -> Body {
        Body {
            pos,
            vel: V2::ZERO,
            knock: V2::ZERO,
            facing: v2(0.0, 1.0),
            z: 0.0,
            moving: false,
            anim: 0.0,
            swing: None,
            combo: 0,
            combo_timer: 0.0,
            charge: 0.0,
            dash_time: 0.0,
            dash_dir: v2(0.0, 1.0),
            dash_charges: 1,
            dash_recharge: 0.0,
            invulnerable: 1.0,
            flash: 0.0,
            status: Status::default(),
            ranged_cooldown: 0.0,
            step_timer: 0.0,
            hazard_timer: 0.0,
            second_wind_used: false,
            dead: 0.0,
            door_lock: true,
            downed: false,
            revive: 0.0,
            waiting: None,
            pet: PetBody::beside(pos),
        }
    }
}

/// Something the interact button would act on.
#[derive(Clone, Debug, PartialEq)]
pub enum Interaction {
    OpenChest(usize),
    StairsDown,
    StairsUp,
    /// The portal back from a Catalog destination.
    ReturnPortal,
    ReturnLamp,
    Station(u8),
    EmptyPlot(u8),
}

fn node_sound(kind: NodeKind) -> Sfx {
    match kind {
        NodeKind::Shelf | NodeKind::SealedCodex | NodeKind::InkWell => Sfx::HarvestWood,
        NodeKind::Vine | NodeKind::GoldVine => Sfx::HarvestPlant,
        NodeKind::Rock | NodeKind::AshPile | NodeKind::SaltCrust | NodeKind::BonePile => {
            Sfx::HarvestStone
        }
        NodeKind::Railing | NodeKind::BrassVein | NodeKind::SilverVein => Sfx::HarvestMetal,
        NodeKind::BrokenLamp | NodeKind::Mirror | NodeKind::VitralShard => Sfx::HarvestGlass,
    }
}

impl Game {
    fn aim_direction(&self, input: &Input) -> V2 {
        if let Some(d) = input.aim_dir.filter(|d| d.len() > 0.35) {
            return d.norm();
        }
        if let Some(target) = input.aim_at {
            let d = target - (self.body.pos + v2(0.0, -6.0));
            if d.len() > 2.0 {
                return d.norm();
            }
        }
        if input.movement.len() > 0.2 {
            input.movement.norm()
        } else {
            self.body.facing
        }
    }

    /// Returns true when the player moved to another site.
    pub(crate) fn update_player(&mut self, dt: f32, input: &Input) -> bool {
        self.tick_player_status(dt);
        if self.body.dead > 0.0 {
            return false;
        }
        let max_energy = self.derived.max_energy;
        let max_hp = self.derived.max_hp;
        let regen = 17.0 * (1.0 + self.derived.stat(Stat::EnergyRegen));
        let hp_regen = self.derived.stat(Stat::HpRegen);
        {
            let p = &mut self.profile.player;
            p.energy = (p.energy + regen * dt).min(max_energy);
            p.hp = (p.hp + hp_regen * dt).min(max_hp);
        }
        let b = &mut self.body;
        b.invulnerable = (b.invulnerable - dt).max(0.0);
        b.flash = (b.flash - dt).max(0.0);
        b.ranged_cooldown = (b.ranged_cooldown - dt).max(0.0);
        b.combo_timer = (b.combo_timer - dt).max(0.0);
        if b.combo_timer <= 0.0 && b.swing.is_none() {
            b.combo = 0;
        }
        let max_charges = self.derived.dash_charges;
        if b.dash_charges < max_charges {
            b.dash_recharge -= dt;
            if b.dash_recharge <= 0.0 {
                b.dash_charges += 1;
                b.dash_recharge =
                    0.85 * (1.0 - self.derived.stats.get(Stat::DashCooldown).min(0.6));
            }
        }
        b.dash_charges = b.dash_charges.min(max_charges);

        if input.cycle_tool != 0 {
            let i = self.profile.player.active_tool.index() as i32;
            let next = (i + input.cycle_tool.signum()).rem_euclid(4) as usize;
            self.profile.player.active_tool = ToolKind::ALL[next];
            self.sound(Sfx::UiMove);
        }

        self.start_dash(input);
        self.move_player(dt, input);
        self.handle_attack(dt, input);
        // Thorns can kill mid-swing: nothing else may happen to a dead player.
        if self.body.dead > 0.0 {
            return false;
        }
        self.handle_ranged(input);
        self.handle_items(input);
        if input.read {
            self.events.push(GameEvent::Open(ScreenRequest::Reader));
        }
        self.forget_distant_wish();
        if input.interact && self.interact() {
            return true;
        }
        self.check_hazard(dt);
        if self.body.dead > 0.0 {
            return false;
        }
        self.check_death_bag();
        self.check_doors()
    }

    fn tick_player_status(&mut self, dt: f32) {
        let (burn, poison) = self.body.status.tick(dt);
        for (amount, kind) in [
            (burn, super::events::NumberKind::Burn),
            (poison, super::events::NumberKind::Poison),
        ] {
            if amount > 0 {
                self.events.push(GameEvent::Number {
                    pos: self.body.pos + v2(0.0, -18.0),
                    amount,
                    kind,
                });
                self.lose_hp(amount as f32);
            }
        }
    }

    fn start_dash(&mut self, input: &Input) {
        if !input.dash || self.body.dash_time > 0.0 {
            return;
        }
        if !self.derived.has(Unlock::Dash) {
            self.warn("Todavía no sabés esquivar (rama Explorador)");
            return;
        }
        if self.body.dash_charges == 0 || self.profile.player.energy < DASH_COST {
            return;
        }
        self.profile.player.energy -= DASH_COST;
        let cooldown = 0.85 * (1.0 - self.derived.stat(Stat::DashCooldown).min(0.6));
        let dir = if input.movement.len() > 0.2 {
            input.movement.norm()
        } else {
            self.body.facing
        };
        let iframes = 0.28 * (1.0 + self.derived.stat(Stat::IFrames) * 0.4);
        let b = &mut self.body;
        if b.dash_charges == self.derived.dash_charges {
            b.dash_recharge = cooldown;
        }
        b.dash_charges -= 1;
        b.dash_dir = dir;
        b.dash_time = DASH_TIME;
        b.invulnerable = b.invulnerable.max(iframes);
        b.swing = None;
        self.sound(Sfx::Dash);
        self.events.push(GameEvent::Particles {
            pos: self.body.pos,
            z: 4.0,
            color: [190, 230, 255],
            count: 10,
            speed: 40.0,
        });
    }

    fn ground_hazard(&self) -> Hazard {
        if physics::tile_at(&self.site.grid, self.body.pos) == Tile::Hazard {
            self.data.biomes[self.site.recipe.biome].hazard
        } else {
            Hazard::None
        }
    }

    fn move_player(&mut self, dt: f32, input: &Input) {
        let hazard = self.ground_hazard();
        let wanted = input.movement.clamp_len(1.0);
        let mut speed = self.derived.speed * self.body.status.speed_factor();
        speed *= match hazard {
            Hazard::Water => 0.65,
            Hazard::Vines | Hazard::Dunes => 0.72,
            _ => 1.0,
        };
        if self.body.swing.is_some() {
            speed *= 0.45;
        } else if self.body.charge > 0.25 && self.derived.has(Unlock::ChargedStrike) {
            speed *= 0.55;
        }
        let ice = hazard == Hazard::Ice;
        let accel = if ice { 190.0 } else { 900.0 };
        let target = wanted * speed;
        // Short acceleration and braking; ice keeps momentum.
        let rate = if wanted.len() < 0.05 && !ice {
            1100.0
        } else {
            accel
        };
        self.body.vel = self.body.vel.approach(target, rate * dt);

        let mut delta = self.body.vel * dt;
        if self.body.dash_time > 0.0 {
            self.body.dash_time -= dt;
            delta = self.body.dash_dir * (DASH_SPEED * dt);
        }
        if self.body.knock.len() > 4.0 {
            delta += self.body.knock * dt;
            self.body.knock = self.body.knock * (1.0 - (dt * 10.0).min(1.0));
        }
        let obstacles = self.obstacles();
        let (pos, _) = physics::move_body(
            &self.site.grid,
            &obstacles,
            self.body.pos,
            delta,
            PLAYER_RADIUS,
            Mobility::Ground,
        );
        self.body.pos = pos;
        self.body.moving = self.body.vel.len() > 12.0;
        if self.body.moving {
            self.body.anim += dt;
            self.body.step_timer -= dt;
            if self.body.step_timer <= 0.0 {
                self.body.step_timer = 0.3;
                let wet = matches!(hazard, Hazard::Water | Hazard::Brine);
                self.sound(if wet { Sfx::StepWater } else { Sfx::Step });
                self.events.push(GameEvent::Particles {
                    pos: self.body.pos,
                    z: 1.0,
                    color: if wet {
                        [170, 240, 255]
                    } else {
                        [210, 190, 170]
                    },
                    count: 2,
                    speed: 14.0,
                });
            }
        }
        if wanted.len() > 0.2 && self.body.swing.is_none() {
            self.body.facing = wanted.norm();
        }
    }

    fn check_hazard(&mut self, dt: f32) {
        self.body.hazard_timer = (self.body.hazard_timer - dt).max(0.0);
        if self.body.dash_time > 0.0 || self.body.hazard_timer > 0.0 {
            return;
        }
        let depth = self.site.depth as f32;
        match self.ground_hazard() {
            Hazard::Embers => {
                self.body.hazard_timer = 0.8;
                self.body.status.ignite(2.0 + depth * 0.25, 1.5);
                self.sound(Sfx::Burn);
            }
            Hazard::Spikes | Hazard::Bones => {
                self.body.hazard_timer = 1.0;
                let amount = 4.0 + depth * 0.5;
                self.events.push(GameEvent::Number {
                    pos: self.body.pos + v2(0.0, -18.0),
                    amount: amount as i32,
                    kind: super::events::NumberKind::Player,
                });
                self.sound(Sfx::Hurt);
                self.lose_hp(amount);
            }
            Hazard::Brine => {
                self.body.hazard_timer = 0.5;
                self.profile.player.energy = (self.profile.player.energy - 9.0).max(0.0);
            }
            Hazard::Gloom => {
                self.body.hazard_timer = 0.5;
                self.body.status.chill(0.6);
            }
            _ => {}
        }
    }

    fn handle_attack(&mut self, dt: f32, input: &Input) {
        let charged_unlocked = self.derived.has(Unlock::ChargedStrike);
        if let Some(mut swing) = self.body.swing {
            swing.t += dt;
            if !swing.resolved && swing.t >= swing.duration * 0.35 {
                swing.resolved = true;
                self.resolve_swing(swing);
            }
            self.body.swing = (swing.t < swing.duration).then_some(swing);
        }
        if input.attack_held && charged_unlocked {
            let before = self.body.charge;
            self.body.charge += dt;
            if before < CHARGE_TIME && self.body.charge >= CHARGE_TIME {
                self.sound(Sfx::Charge);
            }
        }
        let released_charge = !input.attack_held && self.body.charge >= CHARGE_TIME;
        if !input.attack_held {
            let charge = self.body.charge;
            self.body.charge = 0.0;
            if released_charge && charge >= CHARGE_TIME && self.body.swing.is_none() {
                if self.profile.player.energy >= CHARGE_COST {
                    self.profile.player.energy -= CHARGE_COST;
                    self.begin_swing(input, true);
                    return;
                }
                self.warn("Sin energía para el golpe cargado");
            }
        }
        if input.attack_pressed {
            if self.profile.player.active_tool == ToolKind::Lens {
                self.events.push(GameEvent::Open(ScreenRequest::Reader));
                return;
            }
            let ready = self
                .body
                .swing
                .is_none_or(|s| s.resolved && s.t >= s.duration * 0.62);
            if ready {
                self.begin_swing(input, false);
            }
        }
    }

    fn begin_swing(&mut self, input: &Input, charged: bool) {
        let steps = if self.derived.has(Unlock::FourthSlash) {
            4
        } else {
            3
        };
        let step = if charged {
            0
        } else if self.body.combo_timer > 0.0 || self.body.swing.is_some() {
            self.body.combo % steps
        } else {
            0
        };
        self.body.combo = step + 1;
        let dir = self.aim_direction(input);
        self.body.facing = dir;
        let finisher = charged || step + 1 == steps;
        let duration = if charged {
            0.34
        } else if finisher {
            0.30
        } else {
            0.23
        };
        self.body.swing = Some(Swing {
            t: 0.0,
            duration,
            step,
            charged,
            dir,
            resolved: false,
        });
        self.body.combo_timer = duration + COMBO_WINDOW;
        // A small lunge makes each hit feel committed.
        self.body.knock += dir * if finisher { 70.0 } else { 35.0 };
        self.sound(if finisher {
            Sfx::SwingHeavy
        } else {
            Sfx::Swing
        });
    }

    fn resolve_swing(&mut self, swing: Swing) {
        let steps = if self.derived.has(Unlock::FourthSlash) {
            4
        } else {
            3
        };
        let finisher = swing.step + 1 == steps;
        let size = 1.0 + self.derived.stat(Stat::Reach) + self.derived.stat(Stat::Size) * 0.6;
        let spin = swing.charged || swing.step == 3;
        let reach = if swing.charged { 38.0 } else { 25.0 } * size;
        let mut mult = match swing.step {
            0 => 1.0,
            1 => 1.1,
            2 => 1.45,
            _ => 1.7,
        };
        if finisher {
            mult *= 1.0 + self.derived.stat(Stat::ComboDamage);
        }
        if swing.charged {
            mult = 2.7;
        }
        let tool = self.profile.player.active_tool;
        let (tool_damage, tool_knock) = match tool {
            ToolKind::Hammer => (HAMMER_DAMAGE, 1.6),
            ToolKind::Lamp => (LAMP_DAMAGE, 1.0),
            _ => (1.0, 1.0),
        };
        let origin = self.body.pos;
        let in_arc = |target: V2, radius: f32| {
            let d = target - origin;
            let dist = d.len();
            dist <= reach + radius
                && (spin || dist < radius + 7.0 || d.norm().dot(swing.dir) > 0.25)
        };
        let targets: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.z < 14.0 && in_arc(e.pos, e.radius))
            .map(|e| e.uid)
            .collect();
        let knock = if swing.charged || finisher {
            210.0
        } else {
            110.0
        } * tool_knock
            * (1.0 + self.derived.stat(Stat::Knockback));
        let base = self.derived.damage * mult * tool_damage;
        let mut connected = false;
        for uid in targets {
            // Thorns may have ended the swing.
            if self.body.dead > 0.0 {
                return;
            }
            connected |= self.strike_enemy(uid, base, origin, knock, true);
        }
        if connected {
            self.hitstop = if swing.charged || finisher {
                0.075
            } else {
                0.05
            };
            self.events.push(GameEvent::Shake(if swing.charged {
                3.5
            } else if finisher {
                2.4
            } else {
                1.2
            }));
        }
        if self.body.dead > 0.0 {
            return;
        }
        // Harvest the closest node in the arc (a charged swing hits them all).
        let mut hit_nodes: Vec<(usize, f32)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| in_arc(n.pos, 7.0))
            .map(|(i, n)| (i, n.pos.dist(origin)))
            .collect();
        // What the tool in hand can break comes first, then the nearest.
        let useless = |i: usize| self.data.node(self.nodes[i].kind).tool != tool;
        hit_nodes.sort_by(|a, b| useless(a.0).cmp(&useless(b.0)).then(a.1.total_cmp(&b.1)));
        if !swing.charged {
            hit_nodes.truncate(1);
        } else if hit_nodes.iter().any(|&(i, _)| !useless(i)) {
            hit_nodes.retain(|&(i, _)| !useless(i));
        }
        let mut ids: Vec<u32> = hit_nodes.iter().map(|&(i, _)| self.nodes[i].id).collect();
        ids.sort_unstable();
        for id in ids {
            self.harvest(id, if swing.charged { 2.5 } else { 1.0 });
        }
    }

    /// One harvesting hit. Locked nodes explain which tool and material they need.
    pub fn harvest(&mut self, node_id: u32, power: f32) {
        self.strike_node(node_id, power, false);
    }

    /// The pet's share: it neither freezes nor shakes the game.
    pub(crate) fn pet_harvest(&mut self, node_id: u32, power: f32) {
        self.strike_node(node_id, power, true);
    }

    fn strike_node(&mut self, node_id: u32, power: f32, by_pet: bool) {
        let Some(index) = self.nodes.iter().position(|n| n.id == node_id) else {
            return;
        };
        let kind = self.nodes[index].kind;
        let def = self.data.node(kind).clone();
        let tier = self.profile.player.tool_tier(def.tool);
        if tier < def.min_tier {
            self.nodes[index].shake = 0.12;
            let need = format!("{} {}", def.tool.name(), tier_name(def.min_tier));
            self.warn(format!("{}: necesitás {need}", def.name));
            return;
        }
        // The pet helps with whatever its owner is breaking.
        if !by_pet && self.profile.player.active_tool != def.tool {
            self.nodes[index].shake = 0.12;
            let held = match def.tool {
                ToolKind::Knife => "la",
                _ => "el",
            };
            self.warn(format!(
                "{}: sacá {held} {} (cambiá de herramienta)",
                def.name,
                def.tool.name()
            ));
            return;
        }
        if !by_pet {
            self.body.pet.digging = Some((node_id, super::pet::DIG_MEMORY));
        }
        let base = self
            .data
            .items
            .tool_power
            .get(tier as usize)
            .copied()
            .unwrap_or(1) as f32;
        let mut bonus = 1.0 + self.derived.stat(Stat::HarvestPower);
        if def.tool == ToolKind::Hammer {
            bonus += self.derived.perk(Perk::HammerPower);
        }
        let node = &mut self.nodes[index];
        node.hp -= base * bonus * power;
        node.shake = 0.18;
        let pos = node.pos;
        let broken = node.hp <= 0.0;
        let color = def.yields.first().map_or([220, 200, 180], |y| y.0.color());
        self.sound(node_sound(kind));
        self.events.push(GameEvent::Particles {
            pos,
            z: 8.0,
            color,
            count: 6,
            speed: 55.0,
        });
        if !by_pet {
            self.events.push(GameEvent::Shake(0.8));
            self.hitstop = self.hitstop.max(0.025);
        }
        if !broken {
            return;
        }
        self.nodes.remove(index);
        self.count(|t| t.broken += 1);
        self.sound(Sfx::NodeBreak);
        self.events.push(GameEvent::Particles {
            pos,
            z: 8.0,
            color,
            count: 22,
            speed: 95.0,
        });
        for (resource, amount) in self.node_yield(node_id, kind) {
            self.spawn_resource_drop(pos, resource, amount);
        }
        // Gone for good, at home too: what the player clears stays clear.
        self.delta_mut().depleted_nodes.insert(node_id);
        self.mark_tutorial(super::tutorial::Step::Harvest);
        self.grant_xp(def.xp);
    }

    /// Deterministic yield of a node: the same node always gives the same.
    pub fn node_yield(&self, node_id: u32, kind: NodeKind) -> Vec<(Resource, u32)> {
        let def = self.data.node(kind);
        let mut dice = Dice::labelled(&self.site.seed, &format!("node-{node_id}"));
        let mut out = Vec::new();
        let yield_bonus = 1.0 + self.derived.stat(Stat::Yield);
        for (i, &(resource, lo, hi)) in def.yields.iter().enumerate() {
            let rolled = dice.range(lo as i32, hi as i32).max(0) as u32;
            let mut amount = (rolled as f32 * yield_bonus).round() as u32;
            if i == 0 && self.derived.has(Unlock::DeepHarvest) {
                amount += 1;
            }
            if amount > 0 {
                out.push((resource, amount));
            }
        }
        // A better hammer brings more out of everything it breaks, shared
        // between what the thing gives.
        if def.tool == ToolKind::Hammer && !out.is_empty() {
            let tier = self.profile.player.tool_tier(ToolKind::Hammer);
            let extra = stats::hammer_extra(&self.data, &self.derived, tier);
            let kinds = out.len();
            for i in 0..extra as usize {
                out[i % kinds].1 += 1;
            }
        }
        out
    }

    fn handle_ranged(&mut self, input: &Input) {
        if !input.ranged || self.body.ranged_cooldown > 0.0 {
            return;
        }
        if !self.derived.has(Unlock::RangedPages) {
            self.warn("Todavía no sabés lanzar páginas (rama Combatiente)");
            return;
        }
        let energy = page_energy(&self.derived);
        if self.profile.player.energy < energy
            || !self.profile.player.use_consumable(Consumable::PageAmmo, 1)
        {
            self.warn("Sin páginas afiladas o energía");
            return;
        }
        self.profile.player.energy -= energy;
        let burn_chance = self.derived.perk(Perk::PageBurn);
        // Without the improvement the dice are not touched.
        let burn = burn_chance > 0.0 && self.dice.chance(burn_chance);
        let dir = self.aim_direction(input);
        self.body.facing = dir;
        self.body.ranged_cooldown = 0.28;
        self.projectiles.push(Projectile {
            pos: self.body.pos + dir * 9.0,
            vel: dir * 220.0,
            z: 9.0,
            ttl: 1.1,
            damage: self.derived.damage * page_share(&self.derived),
            radius: 4.0,
            from_player: true,
            owner: self.slot,
            kind: ShotKind::Page,
            burn,
            slow: false,
            poison: false,
        });
        self.sound(Sfx::Throw);
    }

    fn handle_items(&mut self, input: &Input) {
        if input.tonic {
            self.use_tonic();
        }
        if input.torch {
            self.place_torch();
        }
        if input.lamp {
            self.light_return_lamp();
        }
    }

    pub fn use_tonic(&mut self) {
        let max = self.derived.max_hp;
        if self.body.dead > 0.0 || self.profile.player.hp >= max - 0.5 {
            return;
        }
        if !self.profile.player.use_consumable(Consumable::Tonic, 1) {
            self.warn("No quedan tónicos (Alambique de tinta)");
            return;
        }
        let heal = max * tonic_share(&self.derived);
        self.profile.player.hp = (self.profile.player.hp + heal).min(max);
        let spirit = self.derived.perk(Perk::TonicSpirit);
        self.profile.player.energy =
            (self.profile.player.energy + spirit).min(self.derived.max_energy);
        self.body.status.clear();
        self.sound(Sfx::Heal);
        self.events.push(GameEvent::Number {
            pos: self.body.pos + v2(0.0, -18.0),
            amount: heal as i32,
            kind: super::events::NumberKind::Heal,
        });
    }

    fn tile_ahead(&self) -> Pos {
        Pos::from_world(self.body.pos.x, self.body.pos.y)
    }

    fn place_torch(&mut self) {
        if !self.derived.has(Unlock::Torches) {
            self.warn("Todavía no sabés plantar antorchas (rama Explorador)");
            return;
        }
        let tile = self.tile_ahead();
        if self.delta().torches.contains(&tile) {
            return;
        }
        if !self.profile.player.use_consumable(Consumable::Torch, 1) {
            self.warn("No quedan antorchas");
            return;
        }
        let delta = self.delta_mut();
        delta.torches.push(tile);
        if delta.torches.len() > 24 {
            delta.torches.remove(0);
        }
        self.sound(Sfx::TorchPlace);
    }

    pub fn interaction(&self) -> Option<Interaction> {
        let p = self.body.pos;
        let near = |t: V2, d: f32| p.dist(t) <= d;
        for (i, c) in self.chests.iter().enumerate() {
            if !c.open && near(c.pos, 22.0) {
                return Some(Interaction::OpenChest(i));
            }
        }
        if let Some(s) = self.site.stairs
            && near(tile_center(s), 20.0)
        {
            return Some(Interaction::StairsDown);
        }
        let delta = self.delta();
        if self.site.key.is_remote() {
            if near(tile_center(self.site.hub), 20.0) {
                return Some(Interaction::ReturnPortal);
            }
        } else if let Some(s) = delta.stairs_up
            && near(tile_center(s), 18.0)
        {
            return Some(Interaction::StairsUp);
        }
        if self.site.key.is_home() {
            for (plot, &tile) in HOME_PLOTS.iter().enumerate() {
                if near(tile_center(tile), 24.0) {
                    let plot = plot as u8;
                    let built = self.profile.world.stations.iter().any(|s| s.1 == plot);
                    return Some(if built {
                        Interaction::Station(plot)
                    } else {
                        Interaction::EmptyPlot(plot)
                    });
                }
            }
            if near(tile_center(HOME_LAMP), 22.0) {
                return Some(Interaction::ReturnLamp);
            }
        } else if let Some(l) = delta.return_lamp
            && near(tile_center(l), 20.0)
        {
            return Some(Interaction::ReturnLamp);
        }
        None
    }

    /// Walking away from a way out takes back the wish to use it.
    fn forget_distant_wish(&mut self) {
        let Some(wish) = self.body.waiting else {
            return;
        };
        // Doorways are handled by `check_doors`.
        if matches!(wish, Exit::Door(_)) {
            return;
        }
        let here = self
            .interaction()
            .and_then(|i| Exit::of(&i, self.site.key.is_home()));
        if here != Some(wish) {
            self.body.waiting = None;
        }
    }

    /// Hint for the interact button.
    pub fn prompt(&self) -> Option<String> {
        let interaction = self.interaction()?;
        let shut = interaction == Interaction::StairsDown && self.stairs_require().is_some();
        let vote =
            Exit::of(&interaction, self.site.key.is_home()).filter(|_| self.is_coop() && !shut);
        let text: String = match interaction {
            Interaction::OpenChest(_) => "Abrir cofre sellado".into(),
            Interaction::StairsDown if self.stairs_locked() => {
                "Escalera sellada por el Guardián".into()
            }
            Interaction::StairsDown => match self.stairs_require() {
                Some((floor, tier)) => format!(
                    "Al piso {floor}: hacen falta Cuchilla y Martillo de {}",
                    tier_name(tier)
                ),
                None => format!("Bajar al piso {}", self.site.depth + 1),
            },
            Interaction::StairsUp => format!("Subir al piso {}", self.site.depth - 1),
            Interaction::ReturnPortal => "Volver del Catálogo".into(),
            Interaction::ReturnLamp if self.site.key.is_home() => "Viajar a una lámpara".into(),
            Interaction::ReturnLamp => "Volver a tu hexágono".into(),
            Interaction::Station(plot) => {
                let name = self
                    .profile
                    .world
                    .stations
                    .iter()
                    .find(|s| s.1 == plot)
                    .map_or("Estación", |s| s.0.name());
                format!("Usar {name}")
            }
            Interaction::EmptyPlot(_) => "Construir una estación".into(),
        };
        Some(match vote {
            Some(exit) => format!("{text} ({}/2 listos)", self.votes(exit)),
            None => text,
        })
    }

    /// Returns true when the interaction moved the player to another site.
    fn interact(&mut self) -> bool {
        let Some(action) = self.interaction() else {
            return false;
        };
        match action {
            Interaction::OpenChest(i) => {
                self.open_chest(i);
                false
            }
            Interaction::StairsDown if self.stairs_locked() => self.descend(),
            // Shut: say why instead of asking the partner to come.
            Interaction::StairsDown if self.stairs_require().is_some() => self.descend(),
            Interaction::StairsDown => self.want_exit(Exit::Down),
            Interaction::StairsUp => self.want_exit(Exit::Up),
            Interaction::ReturnPortal => self.want_exit(Exit::Portal),
            Interaction::ReturnLamp => {
                if self.site.key.is_home() {
                    self.events.push(GameEvent::Open(ScreenRequest::Lamps));
                    false
                } else {
                    self.want_exit(Exit::Home)
                }
            }
            Interaction::Station(plot) => {
                use crate::data::Station;
                let station = self
                    .profile
                    .world
                    .stations
                    .iter()
                    .find(|s| s.1 == plot)
                    .map(|s| s.0);
                let screen = match station {
                    Some(Station::Scriptorium) => ScreenRequest::Scriptorium,
                    Some(Station::Chest) => ScreenRequest::Chest,
                    Some(bench) => ScreenRequest::Crafting(bench),
                    None => return false,
                };
                self.events.push(GameEvent::Open(screen));
                false
            }
            Interaction::EmptyPlot(plot) => {
                self.events
                    .push(GameEvent::Open(ScreenRequest::Build(plot)));
                false
            }
        }
    }

    fn check_doors(&mut self) -> bool {
        let SiteKey::Walk { .. } = self.site.key else {
            return false;
        };
        let delta = self.delta();
        let mut near_door = false;
        let mut standing_in = None;
        for d in Dir::ALL {
            if !(self.site.exits[d.index()] || delta.opened_doors[d.index()]) {
                continue;
            }
            let (mx, my) = grid::door_mid(d);
            let mid = v2(mx * TILE, my * TILE);
            let dist = self.body.pos.dist(mid);
            if dist < 44.0 {
                near_door = true;
            }
            if dist < 25.0 && !self.body.door_lock {
                standing_in = Some(d);
                break;
            }
        }
        match standing_in {
            Some(d) if self.body.waiting == Some(Exit::Door(d)) => return false,
            Some(d) => return self.want_exit(Exit::Door(d)),
            None if matches!(self.body.waiting, Some(Exit::Door(_))) => {
                self.body.waiting = None;
            }
            None => {}
        }
        if !near_door {
            self.body.door_lock = false;
        }
        false
    }
}
