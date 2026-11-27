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
    g.profile.player.active_tool = tool;
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
    let held = g.nodes.iter().find(|n| n.id == id).expect("node").kind;
    g.profile.player.active_tool = g.data.node(held).tool;
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
fn dying_on_a_hazard_keeps_the_death_penalty() {
    let mut g = game(20);
    let spikes = data().biome_index("oxido");
    let (key, tile) = (1..600)
        .find_map(|i| {
            let key = SiteKey::walk(3, i % 23 - 11, i / 23 - 13);
            let s = crate::world::site::generate(&g.data, 20, &key);
            if s.recipe.biome != spikes {
                return None;
            }
            let tile = s
                .grid
                .positions()
                .find(|&p| s.grid.at(p) == crate::world::grid::Tile::Hazard)?;
            Some((key, tile))
        })
        .expect("a rust gallery with spikes");
    g.enter(key, Arrival::Hub);
    g.enemies.clear();
    g.profile.player.add_resource(Resource::Paper, 50);
    g.profile.player.hp = 1.0;
    g.body.pos = tile_center(tile);
    g.body.invulnerable = 0.0;
    step(&mut g, &Input::default(), 0.1);
    assert!(g.body.dead > 0.0, "spikes should kill");
    assert_eq!(g.profile.player.resource(Resource::Paper), 40);
    assert!(
        g.profile.world.death_bag.is_some(),
        "bag was picked up at once"
    );
    assert!(g.profile.player.hp <= 0.0);
    assert!(!g.travel_to_lamp(0));
    g.profile.player.relics.push("sello_catalogo".into());
    g.profile.player.add_consumable(Consumable::Index, 20);
    assert!(g.catalog_travel("sol").is_err(), "no travel while falling");
}

#[test]
fn presses_during_hitstop_are_not_lost() {
    let mut g = game(21);
    g.profile.player.add_consumable(Consumable::Tonic, 1);
    let tonics = g.profile.player.consumable(Consumable::Tonic);
    g.profile.player.hp = 10.0;
    g.hitstop = 0.05;
    g.update(
        1.0 / 60.0,
        &Input {
            tonic: true,
            ..Default::default()
        },
    );
    assert_eq!(g.profile.player.consumable(Consumable::Tonic), tonics);
    step(&mut g, &Input::default(), 0.2);
    assert_eq!(g.profile.player.consumable(Consumable::Tonic), tonics - 1);
}

#[test]
fn word_items_left_behind_are_never_lost() {
    let mut g = game(22);
    let item = crate::progression::items::from_words(
        &g.data,
        900,
        "daga",
        None,
        crate::babel::rarity::Rarity::Epic,
    );
    g.spawn_item_drop(g.body.pos + v2(200.0, 0.0), item);
    // Closing the game and reopening keeps the reward.
    let reopened = Game::new(data(), g.profile.clone());
    assert!(reopened.profile.world.unclaimed.is_empty());
    assert!(
        reopened
            .profile
            .player
            .equipment
            .iter()
            .flatten()
            .chain(&reopened.profile.player.inventory)
            .any(|i| i.uid == 900)
    );
    // Picking it up normally does not duplicate it later.
    while let Some(d) = g.drops.pop() {
        g.pick_up(d);
    }
    g.travel_home();
    let copies = g
        .profile
        .player
        .equipment
        .iter()
        .flatten()
        .chain(&g.profile.player.inventory)
        .filter(|i| i.uid == 900)
        .count();
    assert_eq!(copies, 1);
}

#[test]
fn evicted_return_lamps_can_be_lit_again() {
    let mut g = game(23);
    g.profile.player.add_resource(Resource::Oil, 999);
    for i in 0..=super::travel::MAX_LAMPS as i32 {
        g.enter(SiteKey::walk(2, 20 + i, 0), Arrival::Hub);
        g.light_return_lamp();
    }
    assert_eq!(g.profile.world.lamps.len(), super::travel::MAX_LAMPS);
    g.enter(SiteKey::walk(2, 20, 0), Arrival::Hub);
    assert!(g.delta().return_lamp.is_none());
    g.light_return_lamp();
    assert!(g.delta().return_lamp.is_some());
}

#[test]
fn infinite_guardians_never_get_weaker_than_the_final_boss() {
    let data = data();
    let hp_and_damage = |floor: u32| {
        let (a, b) = floors::stair_offset(5, floor);
        let site = crate::world::site::generate(&data, 5, &SiteKey::walk(floor, a, b));
        let spawn = site.guardian.expect("guardian");
        let boss = super::actors::make_guardian(&data, 1, floor, &spawn);
        (boss.max_hp, boss.damage)
    };
    let mut previous = hp_and_damage(30);
    for floor in [35, 40, 45, 60, 90] {
        let now = hp_and_damage(floor);
        // Word modifiers vary a little, the base never drops.
        assert!(
            now.0 > previous.0 * 0.85,
            "floor {floor}: {now:?} {previous:?}"
        );
        assert!(
            now.1 > previous.1 * 0.85,
            "floor {floor}: {now:?} {previous:?}"
        );
        previous = now;
    }
    assert!(hp_and_damage(60).0 > hp_and_damage(30).0 * 2.0);
}

#[test]
fn a_dead_player_never_regains_health() {
    let mut g = game(24);
    g.lose_hp(1.0e9);
    assert!(g.body.dead > 0.0);
    g.grant_xp(100_000);
    assert!(g.profile.player.level > 1);
    assert!(g.profile.player.hp <= 0.0);
    g.use_tonic();
    assert!(g.profile.player.hp <= 0.0);
}

#[test]
fn remote_trips_keep_their_difficulty_and_return_the_bag() {
    let mut g = game(25);
    g.profile.player.relics.push("sello_catalogo".into());
    g.profile.player.add_consumable(Consumable::Index, 90);
    g.enter(SiteKey::walk(10, 2, 2), Arrival::Hub);
    g.catalog_travel("lobo sombrio").expect("travel");
    let first = g.site.key.id();
    assert_eq!(g.site.depth, 12);
    g.catalog_travel("hielo laberinto").expect("travel again");
    assert_eq!(g.site.depth, 12, "chaining must not raise difficulty");
    g.profile.player.add_resource(Resource::Iron, 50);
    g.lose_hp(1.0e9);
    let bag = g.profile.world.death_bag.clone().expect("bag");
    assert_eq!(bag.site, SiteKey::walk(10, 2, 2));
    g.respawn();
    g.enter(bag.site.clone(), Arrival::At(v2(bag.x, bag.y)));
    g.body.pos = v2(bag.x, bag.y);
    g.check_death_bag();
    assert_eq!(g.profile.player.resource(Resource::Iron), 50);
    // The same phrase at another difficulty is a different site.
    let SiteKey::Remote { address, .. } = ({
        g.enter(SiteKey::walk(19, 2, 2), Arrival::Hub);
        g.catalog_travel("lobo sombrio").expect("travel");
        g.site.key.clone()
    }) else {
        panic!("remote");
    };
    assert!(!address.is_empty());
    assert_ne!(g.site.key.id(), first);
}

#[test]
fn split_echoes_must_fall_before_a_site_counts_as_cleared() {
    let mut g = game(26);
    g.enter(SiteKey::walk(4, 2, -1), Arrival::Hub);
    g.enemies.retain(|e| !e.illusion);
    g.enemies.truncate(1);
    let e = &mut g.enemies[0];
    e.flags = vec![crate::data::ModFlag::SplitOnDeath];
    e.shield = false;
    let (uid, hp) = (e.uid, e.hp);
    g.apply_damage_to_enemy(uid, hp + 1.0, super::events::NumberKind::Enemy);
    assert_eq!(g.enemies.len(), 2);
    assert!(!g.delta().cleared);
    let echoes: Vec<(u32, f32)> = g.enemies.iter().map(|e| (e.uid, e.hp)).collect();
    for (uid, hp) in echoes {
        g.apply_damage_to_enemy(uid, hp + 1.0, super::events::NumberKind::Enemy);
    }
    assert!(g.delta().cleared);
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

/// A node of this kind placed by the test next to the player.
fn place_node(g: &mut Game, kind: crate::data::NodeKind, offset: V2) -> u32 {
    let id = 90_000 + g.nodes.len() as u32;
    let mut node = g.nodes[0].clone();
    node.id = id;
    node.kind = kind;
    node.pos = g.body.pos + offset;
    node.hp = g.data.node(kind).hits as f32;
    g.nodes.push(node);
    id
}

fn health_of(g: &Game, id: u32) -> Option<f32> {
    g.nodes.iter().find(|n| n.id == id).map(|n| n.hp)
}

#[test]
fn bookshelves_belong_to_the_hammer() {
    use crate::data::{NodeKind, ToolKind};
    let g = game(3);
    assert_eq!(g.data.node(NodeKind::Shelf).tool, ToolKind::Hammer);
    // The knife keeps things of its own.
    for kind in [NodeKind::Vine, NodeKind::InkWell, NodeKind::SealedCodex] {
        assert_eq!(g.data.node(kind).tool, ToolKind::Knife, "{kind:?}");
    }
}

#[test]
fn a_node_only_breaks_with_its_tool_in_hand() {
    use crate::data::{NodeKind, ToolKind};
    for (kind, right, wrong) in [
        (NodeKind::Shelf, ToolKind::Hammer, ToolKind::Knife),
        (NodeKind::Vine, ToolKind::Knife, ToolKind::Hammer),
        (NodeKind::Rock, ToolKind::Hammer, ToolKind::Lamp),
    ] {
        let mut g = game(3);
        let id = place_node(&mut g, kind, v2(14.0, 0.0));
        let whole = health_of(&g, id);
        g.profile.player.active_tool = wrong;
        g.events.clear();
        for _ in 0..30 {
            g.harvest(id, 1.0);
        }
        assert_eq!(health_of(&g, id), whole, "{kind:?} with {wrong:?}");
        assert!(
            g.events
                .iter()
                .any(|e| matches!(e, GameEvent::Message(m) if m.contains(right.name()))),
            "{kind:?} does not say which tool: {:?}",
            g.events
        );
        g.profile.player.active_tool = right;
        for _ in 0..30 {
            g.harvest(id, 1.0);
        }
        assert_eq!(health_of(&g, id), None, "{kind:?} with {right:?}");
    }
}

#[test]
fn each_hammer_upgrade_breaks_faster_and_gives_one_more_up_to_five() {
    use crate::data::{NodeKind, ToolKind};
    let mut g = game(3);
    let id = place_node(&mut g, NodeKind::Shelf, v2(14.0, 0.0));
    let total = |g: &Game, kind| g.node_yield(id, kind).iter().map(|y| y.1).sum::<u32>();
    let base = total(&g, NodeKind::Shelf);
    let vine = total(&g, NodeKind::Vine);
    let mut hits = Vec::new();
    for tier in 0..=5u8 {
        g.profile.player.tools[ToolKind::Hammer.index()] = tier;
        assert_eq!(
            total(&g, NodeKind::Shelf),
            base + u32::from(tier),
            "tier {tier}"
        );
        // What the knife gathers does not depend on the hammer.
        assert_eq!(total(&g, NodeKind::Vine), vine);

        g.profile.player.active_tool = ToolKind::Hammer;
        let shelf = place_node(&mut g, NodeKind::Shelf, v2(14.0, 0.0));
        let mut blows = 0;
        while health_of(&g, shelf).is_some() {
            g.harvest(shelf, 1.0);
            blows += 1;
            assert!(blows < 20);
        }
        hits.push(blows);
    }
    assert!(hits.windows(2).all(|w| w[1] <= w[0]), "{hits:?}");
    assert!(hits[5] < hits[0], "{hits:?}");
    // The extra is shared between what the shelf gives.
    g.profile.player.tools[ToolKind::Hammer.index()] = 5;
    let best = g.node_yield(id, NodeKind::Shelf);
    g.profile.player.tools[ToolKind::Hammer.index()] = 0;
    let first = g.node_yield(id, NodeKind::Shelf);
    let extra: Vec<u32> = best.iter().zip(&first).map(|(a, b)| a.1 - b.1).collect();
    assert_eq!(extra, [3, 2]);
}

#[test]
fn a_swing_goes_for_what_the_held_tool_can_break() {
    use crate::data::{NodeKind, ToolKind};
    let mut g = game(3);
    g.enemies.clear();
    g.nodes.truncate(1);
    g.nodes[0].pos = v2(-500.0, -500.0);
    // The vine is nearer, but the hammer is in hand.
    let vine = place_node(&mut g, NodeKind::Vine, v2(10.0, 0.0));
    let shelf = place_node(&mut g, NodeKind::Shelf, v2(17.0, 0.0));
    let (whole_vine, whole_shelf) = (health_of(&g, vine), health_of(&g, shelf));
    g.profile.player.active_tool = ToolKind::Hammer;
    let swing = Input {
        attack_pressed: true,
        aim_dir: Some(v2(1.0, 0.0)),
        ..Default::default()
    };
    g.update(1.0 / 60.0, &swing);
    step(&mut g, &Input::default(), 0.5);
    assert_eq!(health_of(&g, vine), whole_vine);
    assert!(health_of(&g, shelf) < whole_shelf);
}

#[test]
fn the_first_lesson_says_how_to_get_the_hammer_out() {
    let mut g = game(3);
    g.mark_tutorial(super::tutorial::Step::Move);
    let hint = g.tutorial_hint().expect("hint");
    assert!(
        hint.contains("Martillo") && hint.contains("[tool]"),
        "{hint}"
    );
}

#[test]
fn making_something_once_unpins_it_and_supplies_stay_pinned() {
    use crate::progression::pins::PinResult;
    let mut g = game(5);
    for resource in Resource::ALL {
        g.profile.player.add_resource(resource, 200);
    }
    for id in ["build_binder", "knife_1", "torch"] {
        assert_eq!(g.toggle_pin(id), PinResult::Pinned, "{id}");
    }
    g.craft("build_binder", Some(0)).expect("bench");
    assert_eq!(g.profile.player.pins, ["knife_1", "torch"]);
    g.craft("knife_1", None).expect("knife");
    g.craft("torch", None).expect("torches");
    assert_eq!(g.profile.player.pins, ["torch"]);
}

#[test]
fn what_is_broken_at_home_stays_broken() {
    let mut g = game(6);
    assert!(g.site.key.is_home());
    let (id, kind) = (g.nodes[0].id, g.nodes[0].kind);
    g.profile.player.active_tool = g.data.node(kind).tool;
    for _ in 0..30 {
        g.harvest(id, 1.0);
    }
    assert!(g.nodes.iter().all(|n| n.id != id));
    let left = g.nodes.len();
    g.enter(SiteKey::walk(1, 1, 0), Arrival::Hub);
    g.travel_home();
    assert!(g.nodes.iter().all(|n| n.id != id), "it grew back");
    assert_eq!(g.nodes.len(), left);
    let reopened = Game::new(data(), g.profile.clone());
    assert!(
        reopened.nodes.iter().all(|n| n.id != id),
        "back after loading"
    );
}

/// On the stairs of `floor`, guardian or not, with nothing guarding them.
fn on_the_stairs_of(g: &mut Game, floor: u32) {
    let (q, r) = floors::stair_offset(g.profile.world_seed, floor);
    g.enter(SiteKey::walk(floor, q, r), Arrival::Hub);
    g.enemies.clear();
    let stairs = tile_center(g.site.stairs.expect("stairs"));
    g.body.pos = stairs + v2(0.0, 12.0);
    g.events.clear();
}

#[test]
fn some_floors_ask_for_better_tools() {
    for floor in 1..60 {
        let expected = match floor {
            4 => Some(1),
            8 => Some(2),
            13 => Some(3),
            18 => Some(4),
            _ => None,
        };
        assert_eq!(floors::gate(floor), expected, "floor {floor}");
    }
}

#[test]
fn stairs_stay_shut_without_both_tools() {
    use crate::data::ToolKind;
    let mut g = game(12);
    on_the_stairs_of(&mut g, 3);
    assert_eq!(g.stairs_require(), Some((4, 1)));
    let prompt = g.prompt().expect("prompt");
    assert!(
        prompt.contains("Hierro") && prompt.contains("Martillo"),
        "{prompt}"
    );
    assert!(!g.descend());
    assert!(
        g.events.iter().any(
            |e| matches!(e, GameEvent::Message(m) if m.contains("piso 4") && m.contains("Hierro"))
        ),
        "{:?}",
        g.events
    );
    g.profile.player.tools[ToolKind::Knife.index()] = 1;
    assert!(!g.descend(), "the knife alone is not enough");
    g.profile.player.tools[ToolKind::Hammer.index()] = 1;
    assert_eq!(g.stairs_require(), None);
    assert_eq!(g.prompt().as_deref(), Some("Bajar al piso 4"));
    assert!(g.descend());
    assert_eq!(g.site.depth, 4);
}

#[test]
fn deeper_saves_only_meet_the_next_gate() {
    let mut g = game(13);
    // Already past the first gate with rustic tools, as an old save may be.
    on_the_stairs_of(&mut g, 6);
    assert_eq!(g.stairs_require(), None);
    assert!(g.descend());
    on_the_stairs_of(&mut g, 7);
    assert_eq!(g.stairs_require(), Some((8, 2)));
}

#[test]
fn an_improvement_made_at_home_is_felt_at_once() {
    use crate::progression::upgrades::{Change, UpgradeError};
    let mut g = game(5);
    for resource in Resource::ALL {
        g.profile.player.add_resource(resource, 100);
    }
    assert_eq!(
        g.upgrade_check("lomo_reforzado"),
        Err(UpgradeError::NeedStation(Station::Binder))
    );
    g.profile.world.stations.push((Station::Binder, 0));
    let before = g.derived.max_hp;
    assert_eq!(
        g.upgrade_check("lomo_reforzado").map(|o| o.change),
        Ok(Change::New)
    );
    assert_eq!(g.upgrade("lomo_reforzado"), Ok(()));
    assert_eq!(g.derived.max_hp, before + 10.0);
    assert!(g.events.iter().any(|e| matches!(e, GameEvent::Autosave)));
}
