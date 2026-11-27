use super::Game;
use super::actors::DropKind;
use super::coop::{GUEST, HOST};
use super::events::{GameEvent, Input};
use super::profile::Profile;
use super::travel::Arrival;
use super::vec::{V2, v2};
use crate::data::{GameData, Resource, dev_assets_dir};
use crate::progression::player::PlayerData;
use crate::progression::stats;
use crate::world::grid::{TILE, Tile};
use crate::world::site::SiteKey;
use std::sync::{Arc, OnceLock};

const DT: f32 = 1.0 / 60.0;

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

fn learnt(skills: &[&str]) -> PlayerData {
    let mut p = PlayerData::default();
    p.pet.skills = skills.iter().map(|s| s.to_string()).collect();
    p
}

/// A game in the home gallery, emptied, with a pet that knows `skills`.
fn game(skills: &[&str]) -> Game {
    let mut profile = Profile::new(5);
    profile.player = learnt(skills);
    let mut g = Game::new(data(), profile);
    g.enemies.clear();
    g.drops.clear();
    g.drain_events();
    g
}

/// A spot the pet can stand on, `offset` away from the player.
fn near(g: &Game, offset: V2) -> V2 {
    g.safe_position(g.body.pos + offset)
}

fn pet_runs(g: &mut Game, seconds: f32) {
    for _ in 0..(seconds / DT) as u32 {
        g.update_pet(DT);
    }
}

/// One enemy taken from a nearby gallery, brought `offset` from the player.
fn foe(g: &mut Game, offset: V2) -> u32 {
    let home = g.body.pos;
    let mut found = None;
    for q in 1..40 {
        g.enter(SiteKey::walk(2, q, 0), Arrival::Hub);
        if let Some(e) = g.enemies.first() {
            found = Some(e.clone());
            break;
        }
    }
    let mut e = found.expect("an enemy somewhere");
    g.enter(SiteKey::walk(1, 0, 0), Arrival::At(home));
    g.enemies.clear();
    g.drops.clear();
    e.pos = near(g, offset);
    e.lit = true;
    e.shield = false;
    e.illusion = false;
    e.ai = super::actors::Ai::Chase;
    e.hp = 500.0;
    e.max_hp = 500.0;
    let uid = e.uid;
    g.enemies.push(e);
    g.drain_events();
    uid
}

fn health(g: &Game, uid: u32) -> Option<f32> {
    g.enemies.iter().find(|e| e.uid == uid).map(|e| e.hp)
}

fn lay_drop(g: &mut Game, at: V2) {
    g.spawn_drop(at, DropKind::Resource(Resource::ALL[0], 3));
    let d = g.drops.last_mut().expect("drop");
    (d.pos, d.vel, d.z, d.vz, d.age) = (at, V2::ZERO, 0.0, 0.0, 1.0);
}

#[test]
fn without_skills_it_only_follows() {
    let mut g = game(&[]);
    let uid = foe(&mut g, v2(30.0, 0.0));
    let at = near(&g, v2(-30.0, 0.0));
    lay_drop(&mut g, at);
    pet_runs(&mut g, 3.0);
    assert_eq!(health(&g, uid), Some(500.0));
    assert_eq!(g.drops.len(), 1);
    g.body.pos = near(&g, v2(0.0, 60.0));
    pet_runs(&mut g, 3.0);
    assert!(g.body.pet.pos.dist(g.body.pos) < 30.0);
    assert!(!g.body.pet.moving, "it settles once it arrives");
}

#[test]
fn fangs_bite_only_what_is_lit() {
    let mut g = game(&["m_mordisco"]);
    let uid = foe(&mut g, v2(34.0, 0.0));
    g.enemies[0].lit = false;
    pet_runs(&mut g, 3.0);
    assert_eq!(health(&g, uid), Some(500.0));
    g.enemies[0].lit = true;
    pet_runs(&mut g, 3.0);
    let after = health(&g, uid).expect("alive");
    assert!(after < 500.0);
    // A bite is a fraction of a blow, and there is a pause between them.
    let bite = (g.derived.damage * g.derived.pet.bite).round().max(1.0);
    assert!(500.0 - after <= bite * 3.0, "{after} with bites of {bite}");
}

#[test]
fn a_shield_and_an_illusion_are_left_alone() {
    let mut g = game(&["m_mordisco"]);
    let uid = foe(&mut g, v2(30.0, 0.0));
    g.enemies[0].shield = true;
    pet_runs(&mut g, 2.0);
    assert!(g.enemies[0].shield);
    assert_eq!(health(&g, uid), Some(500.0));
    g.enemies[0].shield = false;
    g.enemies[0].illusion = true;
    pet_runs(&mut g, 2.0);
    assert_eq!(g.enemies.len(), 1);
}

#[test]
fn a_bite_can_finish_an_enemy_and_pays_as_usual() {
    let mut g = game(&["m_mordisco"]);
    let uid = foe(&mut g, v2(30.0, 0.0));
    g.enemies[0].hp = 1.0;
    let xp = (g.profile.player.level, g.profile.player.xp);
    pet_runs(&mut g, 2.0);
    assert_eq!(health(&g, uid), None);
    assert!((g.profile.player.level, g.profile.player.xp) > xp);
}

#[test]
fn the_nose_brings_back_a_far_drop() {
    let mut g = game(&["m_buscar"]);
    let at = near(&g, v2(70.0, 0.0));
    lay_drop(&mut g, at);
    let had = g.profile.player.resource(Resource::ALL[0]);
    pet_runs(&mut g, 3.0);
    assert!(g.drops.is_empty());
    assert_eq!(g.profile.player.resource(Resource::ALL[0]), had + 3);
    // Beyond its range it does not wander off.
    let far = near(&g, v2(0.0, 7.5 * TILE));
    lay_drop(&mut g, far);
    pet_runs(&mut g, 3.0);
    assert_eq!(g.drops.len(), 1);
}

#[test]
fn digging_helps_with_what_the_owner_is_harvesting() {
    for (skills, helps) in [(&[][..], false), (&["m_buscar", "m_escarbar"][..], true)] {
        let mut g = game(skills);
        let id = g.nodes[0].id;
        let spot = g.nodes[0].pos;
        g.profile.player.active_tool = g.data.node(g.nodes[0].kind).tool;
        g.body.pos = g.safe_position(spot + v2(0.0, 18.0));
        g.body.pet.pos = g.body.pos;
        g.nodes[0].hp = 100.0;
        g.harvest(id, 1.0);
        g.drops.clear();
        g.hitstop = 0.0;
        let after_owner = g.nodes[0].hp;
        g.drain_events();
        pet_runs(&mut g, 2.0);
        assert_eq!(g.nodes[0].hp < after_owner, helps, "{skills:?}");
        assert_eq!(g.hitstop, 0.0, "the pet never freezes the game");
        assert!(!g.events.iter().any(|e| matches!(e, GameEvent::Shake(_))));
        // It forgets once the owner has stopped for a while.
        pet_runs(&mut g, 2.0);
        let rested = g.nodes[0].hp;
        pet_runs(&mut g, 2.0);
        assert_eq!(g.nodes[0].hp, rested);
    }
}

#[test]
fn it_never_stays_lost() {
    let mut g = game(&[]);
    g.body.pet.pos = g.body.pos + v2(400.0, 300.0);
    g.update_pet(DT);
    assert!(g.body.pet.pos.dist(g.body.pos) < 40.0);
    // Walled in: after a moment of pushing, it reappears.
    let cell = crate::world::grid::Pos::from_world(g.body.pos.x + 80.0, g.body.pos.y);
    for dx in -2i32..=2 {
        for dy in -2i32..=2 {
            let wall = dx.abs() == 2 || dy.abs() == 2;
            let tile = if wall { Tile::Wall } else { Tile::Floor };
            g.site.grid.set(cell.x + dx, cell.y + dy, tile);
        }
    }
    g.body.pet.pos = super::tile_center(cell);
    pet_runs(&mut g, 4.0);
    assert!(g.body.pet.pos.dist(g.body.pos) < 30.0);
}

#[test]
fn it_arrives_beside_its_owner_in_every_gallery() {
    let mut g = game(&[]);
    for q in 0..6 {
        g.enter(SiteKey::walk(1, q, 1), Arrival::Hub);
        assert!(g.body.pet.pos.dist(g.body.pos) < 70.0, "gallery {q}");
        assert!(
            !super::physics::blocked(
                &g.site.grid,
                &[],
                g.body.pet.pos,
                3.0,
                super::physics::Mobility::Ground
            ),
            "inside a wall in gallery {q}"
        );
    }
}

#[test]
fn the_pet_sits_beside_a_fallen_owner() {
    let mut g = game(&["m_mordisco"]);
    let uid = foe(&mut g, v2(30.0, 0.0));
    g.body.dead = 5.0;
    g.body.pet.pos = near(&g, v2(-50.0, 0.0));
    pet_runs(&mut g, 3.0);
    assert!(g.body.pet.pos.dist(g.body.pos) < 24.0);
    assert_eq!(health(&g, uid), Some(500.0));
}

#[test]
fn each_player_has_their_own_pet() {
    let mut g = game(&[]);
    g.join(learnt(&["m_mordisco"]));
    assert_eq!(g.view(HOST).expect("host").2.pet.bite, 0.0);
    assert!(g.view(GUEST).expect("guest").2.pet.bite > 0.0);
    let guest_pet = |g: &Game| g.view(GUEST).expect("guest").0.pet.pos;
    assert!(g.body.pet.pos.dist(g.body.pos) < 40.0);
    assert!(guest_pet(&g).dist(g.view(GUEST).expect("guest").0.pos) < 40.0);
    // Both are moved by the ordinary step, each after its own owner.
    g.as_player(GUEST, |g| {
        g.body.pos = g.safe_position(g.body.pos + v2(60.0, 20.0))
    });
    for _ in 0..240 {
        g.step(DT, &Input::default(), &Input::default());
    }
    let owner = g.view(GUEST).expect("guest").0.pos;
    assert!(guest_pet(&g).dist(owner) < 30.0);
    let derived = stats::derive(&g.data, &g.profile.player);
    assert_eq!(derived.pet.bite, 0.0);
}

/// Neither hidden behind its owner nor standing on them.
fn in_sight(pet: V2, owner: V2) -> bool {
    (pet.x - owner.x).abs() >= 15.0 || pet.y - owner.y >= 10.0
}

#[test]
fn it_waits_where_it_can_be_seen() {
    let mut g = game(&[]);
    assert!(in_sight(g.body.pet.pos, g.body.pos), "on arrival");
    for facing in [v2(0.0, 1.0), v2(0.0, -1.0), v2(1.0, 0.0), v2(-1.0, 0.0)] {
        // Starting right underneath, it steps out to one side.
        g.body.facing = facing;
        g.body.pet = super::pet::PetBody::beside(g.body.pos + v2(1.0, -3.0));
        g.body.pet.still = 5.0;
        pet_runs(&mut g, 2.0);
        let (pet, owner) = (g.body.pet.pos, g.body.pos);
        assert!(in_sight(pet, owner), "{facing:?}: {:?}", pet - owner);
        assert!(pet.dist(owner) < 30.0 && !g.body.pet.moving, "{facing:?}");
    }
}

#[test]
fn two_pets_start_apart_from_both_players() {
    let mut g = game(&[]);
    g.join(PlayerData::default());
    let spot = |g: &Game, slot| {
        let body = g.view(slot).expect("player").0;
        (body.pos, body.pet.pos)
    };
    let (host, host_pet) = spot(&g, HOST);
    let (guest, guest_pet) = spot(&g, GUEST);
    for pet in [host_pet, guest_pet] {
        assert!(in_sight(pet, host) && in_sight(pet, guest), "{pet:?}");
    }
    assert!(host_pet.dist(guest_pet) > 24.0);
}

#[test]
fn spending_on_the_pet_also_answers_the_level_up_hint() {
    let mut g = game(&[]);
    g.profile.player.skill_points = 2;
    // The workbench has its own sign until it is built.
    g.profile
        .world
        .stations
        .push((crate::data::Station::Binder, 0));
    use super::tutorial::Step;
    for step in [
        Step::Move,
        Step::Harvest,
        Step::Build,
        Step::Read,
        Step::Explore,
        Step::Lamp,
        Step::Stairs,
    ] {
        g.mark_tutorial(step);
    }
    assert!(g.tutorial_hint().is_some_and(|h| h.contains("[skills]")));
    assert!(g.learn_pet_skill("m_trote"));
    assert!(g.tutorial_hint().is_none());
}
