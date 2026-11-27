//! Two players in the same gallery.
//!
//! The simulation was written around one player: `Game::body`,
//! `profile.player` and `derived`. In co-op the other player's three live in a
//! [`Companion`] and are swapped in while that player acts, so every rule keeps
//! reading the same fields. Without a companion nothing here changes anything.
//!
//! The world (`profile.world`) belongs to the host, player 0, whose own data is
//! in place whenever the simulation is not in the middle of a guest's action.

use super::Game;
use super::events::{GameEvent, Input, Sfx};
use super::player::{Body, Interaction};
use super::travel::Arrival;
use super::vec::{V2, v2};
use crate::progression::player::PlayerData;
use crate::progression::stats::{self, Derived};
use crate::world::grid::entry_pos;
use crate::world::hex::{self, Dir};
use crate::world::site::SiteKey;
use serde::{Deserialize, Serialize};

pub const HOST: u8 = 0;
pub const GUEST: u8 = 1;
/// Seconds a partner has to stay close to raise a fallen player.
pub const REVIVE_TIME: f32 = 2.5;
const REVIVE_RANGE: f32 = 24.0;
/// Health a raised player gets back.
const REVIVE_HEALTH: f32 = 0.35;
/// Between both falling and waking up at the lamp.
const WIPE_DELAY: f32 = 3.0;
/// How close the partner must be to a journey chosen from a menu.
pub const PARTY_RANGE: f32 = 56.0;
/// Guardians hold out longer against two.
const BOSS_HEALTH_FOR_TWO: f32 = 1.6;

/// A way out of the current gallery. Two players leave only together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exit {
    Door(Dir),
    Down,
    Up,
    /// Back from a Catalog destination.
    Portal,
    /// Through a return lamp to the home hexagon.
    Home,
}

impl Exit {
    fn place(self) -> &'static str {
        match self {
            Exit::Door(_) => "en una puerta",
            Exit::Down | Exit::Up => "en la escalera",
            Exit::Portal => "en el portal",
            Exit::Home => "en la lámpara",
        }
    }

    pub fn of(interaction: &Interaction, home: bool) -> Option<Exit> {
        match interaction {
            Interaction::StairsDown => Some(Exit::Down),
            Interaction::StairsUp => Some(Exit::Up),
            Interaction::ReturnPortal => Some(Exit::Portal),
            Interaction::ReturnLamp if !home => Some(Exit::Home),
            _ => None,
        }
    }
}

/// Everything that is the other player's.
pub struct Companion {
    pub body: Body,
    pub player: PlayerData,
    pub derived: Derived,
    /// Presses made while the world was frozen by hitstop.
    pub(crate) buffered: Input,
}

/// Events meant only for whoever caused them.
fn is_private(event: &GameEvent) -> bool {
    match event {
        GameEvent::Message(_)
        | GameEvent::Open(_)
        | GameEvent::LevelUp(_)
        | GameEvent::PlayerDied => true,
        GameEvent::Sound(s) => matches!(
            s,
            Sfx::UiMove
                | Sfx::UiConfirm
                | Sfx::UiBack
                | Sfx::UiError
                | Sfx::Locked
                | Sfx::Craft
                | Sfx::SkillLearn
                | Sfx::ReadPage
                | Sfx::WordFound
                | Sfx::LevelUp
        ),
        _ => false,
    }
}

impl Game {
    pub fn is_coop(&self) -> bool {
        self.companion.is_some()
    }

    pub fn players(&self) -> u8 {
        1 + u8::from(self.companion.is_some())
    }

    /// Whose data is in `body`, `profile.player` and `derived` right now.
    pub fn acting(&self) -> u8 {
        self.slot
    }

    /// Body, data and stats of either player.
    pub fn view(&self, slot: u8) -> Option<(&Body, &PlayerData, &Derived)> {
        if slot == self.slot {
            return Some((&self.body, &self.profile.player, &self.derived));
        }
        let other = self.companion.as_deref()?;
        (slot <= GUEST).then_some((&other.body, &other.player, &other.derived))
    }

    /// The player who is not acting.
    pub fn partner(&self) -> Option<&Companion> {
        self.companion.as_deref()
    }

    /// Players on their feet, with where they are.
    pub(crate) fn standing(&self) -> Vec<(u8, V2)> {
        (0..self.players())
            .filter_map(|slot| {
                let (body, _, _) = self.view(slot)?;
                (body.dead <= 0.0).then_some((slot, body.pos))
            })
            .collect()
    }

    fn swap_players(&mut self) {
        let Some(other) = self.companion.as_deref_mut() else {
            return;
        };
        std::mem::swap(&mut self.body, &mut other.body);
        std::mem::swap(&mut self.profile.player, &mut other.player);
        std::mem::swap(&mut self.derived, &mut other.derived);
        self.slot = 1 - self.slot;
    }

    /// Runs `action` as one of the players. `None` if there is no such player.
    pub fn as_player<R>(&mut self, slot: u8, action: impl FnOnce(&mut Game) -> R) -> Option<R> {
        if slot == self.slot {
            return Some(action(self));
        }
        if self.companion.is_none() || slot > GUEST {
            return None;
        }
        self.flush_events();
        self.swap_players();
        let result = action(self);
        self.flush_events();
        self.swap_players();
        Some(result)
    }

    /// Runs `action` as the other player, if there is one.
    pub(crate) fn for_partner(&mut self, action: impl FnOnce(&mut Game)) {
        if self.companion.is_some() {
            self.as_player(1 - self.slot, action);
        }
    }

    /// Sorts the events not yet sorted between the host's and the guest's.
    pub(crate) fn flush_events(&mut self) {
        let from = self.routed.min(self.events.len());
        if self.companion.is_some() {
            for event in self.events.split_off(from) {
                if !is_private(&event) {
                    self.partner_events.push(event.clone());
                    self.events.push(event);
                } else if self.slot == GUEST {
                    self.partner_events.push(event);
                } else {
                    self.events.push(event);
                }
            }
        }
        self.routed = self.events.len();
    }

    /// What the guest must be shown since the last call.
    pub fn drain_partner_events(&mut self) -> Vec<GameEvent> {
        self.flush_events();
        std::mem::take(&mut self.partner_events)
    }

    /// A second player arrives next to the host. `player` comes from another
    /// computer, so it is put in order first.
    pub fn join(&mut self, player: PlayerData) {
        if self.companion.is_some() {
            return;
        }
        let mut player = player.sanitized(&self.data);
        let derived = stats::derive(&self.data, &player);
        if player.hp < 1.0 {
            player.hp = derived.max_hp;
        }
        player.hp = player.hp.min(derived.max_hp);
        player.energy = player.energy.min(derived.max_energy);
        let at = self.safe_position(self.body.pos + v2(16.0, 0.0));
        let mut body = Body::new(at);
        body.dash_charges = derived.dash_charges;
        for boss in self.enemies.iter_mut().filter(|e| e.boss.is_some()) {
            boss.hp *= BOSS_HEALTH_FOR_TWO;
            boss.max_hp *= BOSS_HEALTH_FOR_TWO;
        }
        self.flush_events();
        self.companion = Some(Box::new(Companion {
            body,
            player,
            derived,
            buffered: Input::default(),
        }));
        self.for_partner(Game::place_pet);
        self.say("Tu compañero se unió");
    }

    /// The second player goes; their data is handed back.
    pub fn leave(&mut self) -> Option<PlayerData> {
        self.flush_events();
        let other = self.companion.take()?;
        self.partner_events.clear();
        self.wipe = 0.0;
        self.body.waiting = None;
        // Alone there is nobody to raise whoever was down.
        if self.body.downed {
            self.get_up(REVIVE_HEALTH);
        }
        for boss in self.enemies.iter_mut().filter(|e| e.boss.is_some()) {
            boss.hp /= BOSS_HEALTH_FOR_TWO;
            boss.max_hp /= BOSS_HEALTH_FOR_TWO;
        }
        Some(other.player)
    }

    pub(crate) fn boss_health_factor(&self) -> f32 {
        if self.is_coop() {
            BOSS_HEALTH_FOR_TWO
        } else {
            1.0
        }
    }

    // ------------------------------------------------------------ ways out

    /// Leaves through `exit` right away. Returns whether the gallery changed.
    pub(crate) fn take_exit(&mut self, exit: Exit) -> bool {
        match exit {
            Exit::Door(d) => {
                let SiteKey::Walk { floor, q, r } = self.site.key else {
                    return false;
                };
                let (nq, nr) = hex::neighbor(q, r, d);
                self.sound(Sfx::Door);
                self.mark_tutorial(super::tutorial::Step::Explore);
                self.enter(SiteKey::walk(floor, nq, nr), Arrival::Door(d.opposite()));
                true
            }
            Exit::Down => self.descend(),
            Exit::Up => self.ascend(),
            Exit::Portal => self.return_from_catalog(),
            Exit::Home => {
                self.travel_home();
                true
            }
        }
    }

    /// The acting player wants to leave. Alone, they do. In company they wait
    /// until the partner wants the same.
    pub(crate) fn want_exit(&mut self, exit: Exit) -> bool {
        let helper_up = self.companion.as_ref().is_some_and(|c| !c.body.downed);
        if !helper_up {
            return self.take_exit(exit);
        }
        if self.body.waiting == Some(exit) {
            // Pressing again takes the vote back; a doorway is just stood in.
            if !matches!(exit, Exit::Door(_)) {
                self.body.waiting = None;
            }
            return false;
        }
        self.body.waiting = Some(exit);
        let agreed = self.partner().is_some_and(|c| c.body.waiting == Some(exit));
        if !agreed {
            self.say(format!("Esperando a tu compañero {}", exit.place()));
            self.for_partner(|g| g.say(format!("Tu compañero te espera {}", exit.place())));
        }
        false
    }

    /// Leaves if everyone able to walk wants the same way out.
    pub(crate) fn resolve_exit(&mut self) -> bool {
        let Some(other) = self.companion.as_deref() else {
            return false;
        };
        let (a, b) = (&self.body, &other.body);
        let chosen = match (a.downed, b.downed) {
            (false, false) if a.waiting == b.waiting => a.waiting,
            (false, true) => a.waiting,
            (true, false) => b.waiting,
            _ => None,
        };
        chosen.is_some_and(|exit| self.take_exit(exit))
    }

    /// How many of the two are waiting at this way out.
    pub fn votes(&self, exit: Exit) -> usize {
        (0..self.players())
            .filter(|&slot| {
                self.view(slot)
                    .is_some_and(|(body, _, _)| body.waiting == Some(exit))
            })
            .count()
    }

    /// Whether a journey chosen from a menu would take everyone along.
    pub fn party_together(&self) -> bool {
        self.companion
            .as_ref()
            .is_none_or(|c| c.body.downed || c.body.pos.dist(self.body.pos) <= PARTY_RANGE)
    }

    /// Places the partner beside whoever arrived, after changing gallery.
    pub(crate) fn bring_partner(&mut self, arrival: Arrival) {
        if self.companion.is_none() {
            return;
        }
        let wanted = match arrival {
            // Side by side along the doorway they came through.
            Arrival::Door(d) => {
                let door = super::tile_center(entry_pos(d));
                let inward = (super::tile_center(self.site.hub) - door).norm();
                self.body.pos + v2(-inward.y, inward.x) * 14.0
            }
            _ => self.body.pos + v2(16.0, 0.0),
        };
        let at = self.safe_position(wanted);
        let facing = self.body.facing;
        self.for_partner(|g| {
            let fallen = g.body.downed;
            let status = g.body.status;
            g.body = Body::new(at);
            g.body.facing = facing;
            g.body.status = status;
            g.body.dash_charges = g.derived.dash_charges;
            if fallen {
                g.get_up(REVIVE_HEALTH);
            }
            g.place_pet();
        });
        for e in &mut self.enemies {
            if (e.pos - at).len() < 64.0 {
                e.grace = e.grace.max(1.8);
            }
        }
    }

    // ------------------------------------------------------------- falling

    /// A player on their feet again.
    pub(crate) fn get_up(&mut self, health: f32) {
        self.body.downed = false;
        self.body.dead = 0.0;
        self.body.revive = 0.0;
        self.body.invulnerable = 2.0;
        self.body.status.clear();
        self.profile.player.hp = self.derived.max_hp * health;
        self.events.push(GameEvent::Particles {
            pos: self.body.pos,
            z: 8.0,
            color: [150, 255, 190],
            count: 26,
            speed: 70.0,
        });
        self.sound(Sfx::Heal);
    }

    /// Falling with a partner: lie down until raised. Nothing is lost.
    pub(crate) fn fall_in_company(&mut self) {
        self.profile.player.hp = 0.0;
        self.body.dead = 1.6;
        self.body.downed = true;
        self.body.revive = 0.0;
        self.body.swing = None;
        self.body.waiting = None;
        self.body.status.clear();
        self.sound(Sfx::Death);
        self.events.push(GameEvent::Shake(6.0));
        if self.partner().is_some_and(|c| !c.body.downed) {
            self.say("Caíste: tu compañero puede levantarte");
            self.for_partner(|g| g.say("Tu compañero cayó: quedate a su lado"));
        } else {
            self.wipe = WIPE_DELAY;
            self.events.push(GameEvent::Banner {
                title: "Cayeron los dos".into(),
                subtitle: "Volverán a la última lámpara".into(),
                color: [255, 90, 110],
            });
        }
    }

    /// Raises fallen players, or wakes both at the lamp. Returns true when the
    /// gallery changed.
    pub(crate) fn update_downed(&mut self, dt: f32) -> bool {
        if self.companion.is_none() {
            return false;
        }
        if self.wipe > 0.0 {
            self.wipe -= dt;
            if self.wipe <= 0.0 {
                self.wipe = 0.0;
                self.respawn();
                return true;
            }
            return false;
        }
        let standing = self.standing();
        for slot in 0..self.players() {
            self.as_player(slot, |g| {
                if !g.body.downed {
                    return;
                }
                let pos = g.body.pos;
                let helped = standing.iter().any(|&(_, p)| p.dist(pos) <= REVIVE_RANGE);
                g.body.revive = if helped {
                    g.body.revive + dt
                } else {
                    (g.body.revive - dt).max(0.0)
                };
                if g.body.revive >= REVIVE_TIME {
                    g.get_up(REVIVE_HEALTH);
                }
            });
        }
        false
    }
}
