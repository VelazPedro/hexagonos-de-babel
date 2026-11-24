use super::events::{GameEvent, Input};
use super::profile::Profile;
use super::travel::Arrival;
use super::vec::{V2, v2};
use super::{Game, tile_center};
use crate::data::{Consumable, GameData, Resource, Station, Unlock, dev_assets_dir};
use crate::world::floors;
use crate::world::grid::TILE;
use crate::world::hex::Dir;
use crate::world::site::{HOME_PLOTS, SiteKey};
use std::sync::{Arc, OnceLock};

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

fn game(seed: u64) -> Game {
    Game::new(data(), Profile::new(seed))
}

fn step(g: &mut Game, input: &Input, seconds: f32) {
    let frames = (seconds * 60.0) as u32;
    for _ in 0..frames.max(1) {
        g.update(1.0 / 60.0, input);
    }
}

#[test]
fn walking_moves_and_walls_stop_the_player() {
    let mut g = game(1);
    let start = g.body.pos;
    let right = Input {
        movement: v2(1.0, 0.0),
        ..Default::default()
    };
    step(&mut g, &right, 0.5);
    assert!(g.body.pos.x > start.x + 20.0);
    step(&mut g, &right, 8.0);
    assert!(matches!(g.site.key, SiteKey::Walk { .. }));
}

#[test]
fn walking_through_a_doorway_loads_the_neighbour() {
    let mut g = game(2);
    // Stand just inside the northern doorway and walk out.
    g.body.pos = v2(20.0 * TILE, 4.0 * TILE);
    g.body.door_lock = false;
    let north = Input {
        movement: v2(0.0, -1.0),
        ..Default::default()
    };
    for _ in 0..240 {
        g.update(1.0 / 60.0, &north);
        if g.site.key != SiteKey::walk(1, 0, 0) {
            break;
        }
    }
    assert_eq!(g.site.key, SiteKey::walk(1, 0, -1));
    assert!(g.delta().opened_doors[Dir::S.index()]);
    assert!(g.events.iter().any(|e| matches!(e, GameEvent::Autosave)));
}

#[test]
fn harvesting_yields_drops_that_can_be_picked_up() {
    let mut g = game(3);
    let node = g.nodes[0].clone();
    let expected = g.node_yield(node.id, node.kind);
    assert_eq!(expected, g.node_yield(node.id, node.kind));
    let tool = g.data.node(node.kind).tool;
    assert_eq!(g.data.node(node.kind).min_tier, 0, "{tool:?}");
    for _ in 0..20 {
        g.harvest(node.id, 1.0);
    }
    assert!(g.nodes.iter().all(|n| n.id != node.id));
    assert!(!g.drops.is_empty());
    while let Some(d) = g.drops.pop() {
        g.pick_up(d);
    }
    for (r, n) in expected {
        assert!(g.profile.player.resource(r) >= n);
    }
    assert!(g.profile.player.xp > 0);
}

#[test]
fn locked_nodes_explain_the_requirement() {
    let mut g = game(4);
    g.enter(SiteKey::walk(9, 1, 1), Arrival::Hub);
    let silver = g
        .nodes
        .iter()
        .find(|n| n.kind == crate::data::NodeKind::SilverVein)
        .map(|n| n.id)
        .expect("silver is guaranteed on floor 9");
    g.events.clear();
    g.harvest(silver, 1.0);
    assert!(g.events.iter().any(
        |e| matches!(e, GameEvent::Message(m) if m.contains("Martillo") && m.contains("Latón"))
    ));
    assert!(g.nodes.iter().any(|n| n.id == silver));
}

#[test]
fn building_and_crafting_at_home() {
    let mut g = game(5);
    g.profile.player.add_resource(Resource::Paper, 40);
    g.profile.player.add_resource(Resource::Stone, 40);
    g.profile.player.add_resource(Resource::Iron, 40);
    g.profile.player.add_resource(Resource::Leather, 40);
    assert!(g.craft("build_binder", Some(0)).is_ok());
    assert!(g.craft("build_forge", Some(0)).is_err(), "plot taken");
    assert!(g.craft("knife_1", None).is_ok());
    assert_eq!(g.profile.world.stations, vec![(Station::Binder, 0)]);
    g.body.pos = tile_center(HOME_PLOTS[1]) + v2(0.0, 18.0);
    assert!(g.prompt().is_some_and(|p| p.contains("Construir")));
}

#[test]
fn combat_defeats_enemies_and_grants_experience() {
    let mut g = game(6);
    g.enter(SiteKey::walk(4, 2, -1), Arrival::Hub);
    let Some((uid, hp)) = g.enemies.first().map(|e| (e.uid, e.hp)) else {
        panic!("expected enemies on floor 4");
    };
    let xp_before = (g.profile.player.level, g.profile.player.xp);
    let from = g.enemies[0].pos + v2(-10.0, 0.0);
    g.enemies[0].lit = true;
    g.enemies[0].shield = false;
    g.enemies[0].flags.clear();
    assert!(g.strike_enemy(uid, hp * 3.0, from, 100.0, false));
    assert!(g.enemies.iter().all(|e| e.uid != uid));
    assert!((g.profile.player.level, g.profile.player.xp) > xp_before);
    assert!(!g.delta().defeated_enemies.is_empty());
}

#[test]
fn shadow_creatures_cannot_be_hurt_outside_the_light() {
    let mut g = game(7);
    g.enter(SiteKey::walk(4, 2, -1), Arrival::Hub);
    let uid = g.enemies[0].uid;
    let hp = g.enemies[0].hp;
    g.enemies[0].lit = false;
    assert!(!g.strike_enemy(uid, 999.0, V2::ZERO, 10.0, false));
    assert_eq!(g.enemies[0].hp, hp);
}

#[test]
fn death_leaves_a_recoverable_bag_with_a_fifth() {
    let mut g = game(8);
    g.profile.player.add_resource(Resource::Iron, 50);
    g.lose_hp(10_000.0);
    assert!(g.body.dead > 0.0);
    assert_eq!(g.profile.player.resource(Resource::Iron), 40);
    let bag = g.profile.world.death_bag.clone().expect("bag");
    assert_eq!(bag.resources.get(&Resource::Iron), Some(&10));
    g.respawn();
    assert!(g.profile.player.hp > 0.0);
    g.body.pos = v2(bag.x, bag.y);
    g.check_death_bag();
    assert_eq!(g.profile.player.resource(Resource::Iron), 50);
    assert!(g.profile.world.death_bag.is_none());
}

#[test]
fn return_lamps_set_the_respawn_point() {
    let mut g = game(9);
    g.enter(SiteKey::walk(2, 3, 0), Arrival::Hub);
    g.profile.player.add_resource(Resource::Oil, 10);
    g.light_return_lamp();
    assert!(g.delta().return_lamp.is_some());
    assert!(g.profile.player.resource(Resource::Oil) < 10);
    g.travel_home();
    assert!(g.site.key.is_home());
    assert!(g.travel_to_lamp(0));
    assert_eq!(g.site.key, SiteKey::walk(2, 3, 0));
    g.travel_home();
    g.lose_hp(10_000.0);
    g.respawn();
    assert_eq!(g.site.key, SiteKey::walk(2, 3, 0));
}

#[test]
fn guardians_lock_the_stairs_and_grant_a_fixed_relic() {
    let mut g = game(10);
    let (a, b) = floors::stair_offset(10, 5);
    g.enter(SiteKey::walk(5, a, b), Arrival::Hub);
    assert!(g.stairs_locked());
    assert!(!g.descend());
    let boss = g.enemies.iter().find(|e| e.boss.is_some()).expect("boss");
    let (uid, hp) = (boss.uid, boss.hp);
    g.apply_damage_to_enemy(uid, hp + 1.0, super::events::NumberKind::Enemy);
    assert!(g.profile.player.has_relic("botas_dobles"));
    assert_eq!(g.derived.dash_charges, 2);
    assert!(!g.stairs_locked());
    assert!(g.descend());
    assert_eq!(g.site.depth, 6);
}

#[test]
fn the_final_guardian_ends_the_story_and_opens_infinite_mode() {
    let mut g = game(11);
    let (a, b) = floors::stair_offset(11, 30);
    g.enter(SiteKey::walk(30, a, b), Arrival::Hub);
    let boss = g.enemies.iter().find(|e| e.boss.is_some()).expect("boss");
    let (uid, hp) = (boss.uid, boss.hp);
    g.apply_damage_to_enemy(uid, hp + 1.0, super::events::NumberKind::Enemy);
    assert!(g.profile.world.finished);
    assert!(g.events.iter().any(|e| matches!(e, GameEvent::Ending)));
    assert!(g.descend());
    assert_eq!(g.site.depth, 31);
}

#[test]
fn catalog_travel_costs_indices_and_returns() {
    let mut g = game(12);
    assert!(g.catalog_travel("lobo sombrio").is_err(), "sealed");
    g.profile.player.relics.push("sello_catalogo".into());
    assert!(g.catalog_travel("lobo sombrio").is_err(), "no indices");
    g.profile.player.add_consumable(Consumable::Index, 20);
    g.catalog_travel("¡Lobo sombrío!").expect("travel");
    assert!(g.site.key.is_remote());
    assert!(g.site.book.inscription.starts_with("lobo sombrio"));
    assert_eq!(g.site.depth, 3);
    assert!(g.profile.player.consumable(Consumable::Index) < 20);
    g.body.pos = tile_center(g.site.hub);
    assert!(g.return_from_catalog());
    assert!(g.site.key.is_home());
}

#[test]
fn saved_changes_persist_across_visits() {
    let mut g = game(13);
    g.enter(SiteKey::walk(3, 1, 0), Arrival::Hub);
    let id = g
        .nodes
        .iter()
        .find(|n| g.data.node(n.kind).min_tier == 0)
        .map(|n| n.id);
    let id = id.expect("harvestable node");
    for _ in 0..30 {
        g.harvest(id, 1.0);
    }
    g.travel_home();
    g.enter(SiteKey::walk(3, 1, 0), Arrival::Hub);
    assert!(g.nodes.iter().all(|n| n.id != id));
    let reopened = Game::new(data(), g.profile.clone());
    assert!(reopened.nodes.iter().all(|n| n.id != id));
}

#[test]
fn every_archetype_survives_a_long_brawl() {
    let mut g = game(14);
    g.enter(SiteKey::walk(12, 4, 4), Arrival::Hub);
    g.enemies.clear();
    let hub = tile_center(g.site.hub);
    for archetype in 0..g.data.enemies.archetypes.len() {
        let spawn = crate::world::site::EnemySpawn {
            id: 100 + archetype as u32,
            archetype,
            word: None,
            modifiers: vec![archetype % g.data.enemies.modifiers.len(), 6],
            elite: None,
            pos: g.site.hub,
        };
        let mut e = super::actors::make_enemy(&g.data, 500 + archetype as u32, 12, &spawn);
        e.pos = hub + V2::from_angle(archetype as f32) * 48.0;
        e.grace = 0.0;
        g.enemies.push(e);
    }
    g.derived.unlocks.insert(Unlock::Dash);
    let mut hurt = false;
    for frame in 0..1800 {
        g.profile.player.hp = g.derived.max_hp;
        let input = Input {
            movement: V2::from_angle(frame as f32 * 0.03),
            attack_pressed: frame % 12 == 0,
            dash: frame % 90 == 0,
            ..Default::default()
        };
        g.update(1.0 / 60.0, &input);
        hurt |= g.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::Number {
                    kind: super::events::NumberKind::Player,
                    ..
                }
            )
        });
        g.events.clear();
    }
    assert!(hurt, "enemies never landed a hit");
}

#[test]
fn mirror_galleries_conjure_harmless_reflections() {
    let mut g = game(16);
    let mirror = data().biome_index("espejo");
    let found = (1..400).find_map(|i| {
        let key = SiteKey::walk(6, i % 23 - 11, i / 23 - 9);
        let s = crate::world::site::generate(&g.data, 16, &key);
        (s.recipe.biome == mirror && !s.enemies.is_empty()).then_some(key)
    });
    g.enter(found.expect("a mirror gallery nearby"), Arrival::Hub);
    let decoy = g.enemies.iter().find(|e| e.illusion).expect("decoy").uid;
    let xp = g.profile.player.xp;
    let hp = g.profile.player.hp;
    g.body.invulnerable = 0.0;
    g.hurt_player(super::ai::PlayerHit::plain(0.0, V2::ZERO));
    assert_eq!(g.profile.player.hp, hp);
    for e in &mut g.enemies {
        e.lit = true;
    }
    assert!(g.strike_enemy(decoy, 1.0, V2::ZERO, 0.0, true));
    assert!(g.enemies.iter().all(|e| e.uid != decoy));
    assert_eq!(g.profile.player.xp, xp);
}

#[test]
fn every_guardian_phase_runs() {
    for floor in [5u32, 10, 15, 20, 25, 30] {
        let mut g = game(15);
        let (a, b) = floors::stair_offset(15, floor);
        g.enter(SiteKey::walk(floor, a, b), Arrival::Hub);
        let mut attacked = false;
        for frame in 0..2400 {
            g.profile.player.hp = g.derived.max_hp;
            if let Some(boss) = g.enemies.iter_mut().find(|e| e.boss.is_some())
                && frame % 600 == 599
            {
                boss.hp = boss.max_hp * (1.0 - (frame / 600 + 1) as f32 * 0.24);
            }
            g.update(1.0 / 60.0, &Input::default());
            attacked |= !g.projectiles.is_empty() || !g.zones.is_empty();
            g.events.clear();
        }
        assert!(attacked, "guardian of floor {floor} never attacked");
    }
}
