//! What each station improvement does while playing.

use super::Game;
use super::actors::{Projectile, ShotKind};
use super::coop::GUEST;
use super::events::Input;
use super::profile::Profile;
use super::travel::Arrival;
use super::vec::{V2, v2};
use crate::data::{Consumable, GameData, NodeKind, Resource, Station, ToolKind, dev_assets_dir};
use crate::progression::player::PlayerData;
use crate::progression::upgrades::Fitted;
use crate::world::grid::{Pos, TILE};
use crate::world::site::SiteKey;
use std::sync::{Arc, OnceLock};

const DT: f32 = 1.0 / 60.0;

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

fn game() -> Game {
    Game::new(data(), Profile::new(21))
}

fn fit(g: &mut Game, target: &str, id: &str, level: u8) {
    let fitted = Fitted {
        id: id.into(),
        level,
    };
    g.profile.player.upgrades.insert(target.into(), fitted);
    g.refresh();
}

fn total(g: &Game, kind: NodeKind) -> u32 {
    g.node_yield(7, kind).iter().map(|y| y.1).sum()
}

#[test]
fn the_quarry_hammer_brings_more_up_to_five_extra() {
    let mut g = game();
    let bare = total(&g, NodeKind::Shelf);
    g.profile.player.tools[ToolKind::Hammer.index()] = 1;
    assert_eq!(total(&g, NodeKind::Shelf), bare + 1);
    fit(&mut g, "hammer", "cantero", 2);
    assert_eq!(total(&g, NodeKind::Shelf), bare + 3);
    // A golden hammer already gives all five there are.
    g.profile.player.tools[ToolKind::Hammer.index()] = 5;
    assert_eq!(total(&g, NodeKind::Shelf), bare + 5);
    // What the knife cuts is none of the hammer's business.
    let vine = total(&g, NodeKind::Vine);
    g.profile.player.upgrades.clear();
    g.refresh();
    assert_eq!(total(&g, NodeKind::Vine), vine);
}

/// How much one blow takes from a fresh node of a kind.
fn blow(g: &mut Game, kind: NodeKind) -> f32 {
    let mut node = g.nodes[0].clone();
    node.id = 90_001;
    node.kind = kind;
    node.hp = 1000.0;
    g.profile.player.active_tool = g.data.node(kind).tool;
    g.nodes.retain(|n| n.id != node.id);
    g.nodes.push(node);
    g.harvest(90_001, 1.0);
    1000.0 - g.nodes.iter().find(|n| n.id == 90_001).expect("node").hp
}

#[test]
fn the_dry_blow_breaks_faster_with_the_hammer_only() {
    let mut g = game();
    let (rock, vine) = (blow(&mut g, NodeKind::Rock), blow(&mut g, NodeKind::Vine));
    fit(&mut g, "hammer", "golpe_seco", 3);
    assert!((blow(&mut g, NodeKind::Rock) - rock * 1.5).abs() < 1e-3);
    assert_eq!(blow(&mut g, NodeKind::Vine), vine);
}

/// A game ready to throw pages.
fn thrower() -> Game {
    let mut g = game();
    g.profile.player.skills.insert("c_paginas".into());
    g.profile.player.add_consumable(Consumable::PageAmmo, 500);
    g.refresh();
    g
}

/// Throws one page and returns it with the energy it cost.
fn throw(g: &mut Game) -> (Projectile, f32) {
    g.projectiles.clear();
    g.body.ranged_cooldown = 0.0;
    g.profile.player.energy = 50.0;
    let input = Input {
        ranged: true,
        aim_dir: Some(v2(1.0, 0.0)),
        ..Default::default()
    };
    g.update(DT, &input);
    let page = *g
        .projectiles
        .iter()
        .find(|p| p.kind == ShotKind::Page)
        .expect("a page in the air");
    (page, 50.0 - g.profile.player.energy)
}

#[test]
fn heavy_pages_hit_harder_and_light_paper_tires_less() {
    let (mut plain, mut better) = (thrower(), thrower());
    let (page, cost) = throw(&mut plain);
    assert!((page.damage - plain.derived.damage * 0.75).abs() < 1e-3);
    fit(&mut better, "pages", "paginas_pesadas", 3);
    let (heavy, _) = throw(&mut better);
    assert!((heavy.damage - better.derived.damage * 1.05).abs() < 1e-3);
    fit(&mut better, "pages", "papel_liviano", 3);
    let (_, light) = throw(&mut better);
    assert!((cost - light - 3.0).abs() < 1e-3, "{cost} and {light}");
}

#[test]
fn burning_pages_sometimes_catch_fire() {
    let mut g = thrower();
    let burning = |g: &mut Game| (0..100).filter(|_| throw(g).0.burn).count();
    assert_eq!(burning(&mut g), 0);
    fit(&mut g, "pages", "paginas_encendidas", 3);
    let caught = burning(&mut g);
    assert!((20..=52).contains(&caught), "{caught} of 100");
}

/// A gallery with one creature that can be hit, standing at `pos`.
fn with_a_creature(g: &mut Game) -> u32 {
    for i in 1..40 {
        g.enter(SiteKey::walk(1, i, 3), Arrival::Hub);
        if !g.enemies.is_empty() {
            break;
        }
    }
    g.enemies.truncate(1);
    let e = &mut g.enemies[0];
    e.lit = true;
    e.shield = false;
    e.illusion = false;
    e.flags.clear();
    e.grace = 0.0;
    e.hp = 500.0;
    e.max_hp = 500.0;
    e.uid
}

#[test]
fn a_burning_page_sets_what_it_hits_on_fire() {
    for burn in [false, true] {
        let mut g = thrower();
        with_a_creature(&mut g);
        let pos = g.enemies[0].pos;
        let (mut page, _) = {
            let mut other = thrower();
            throw(&mut other)
        };
        page.pos = pos;
        page.vel = V2::ZERO;
        page.burn = burn;
        g.projectiles = vec![page];
        g.update_projectiles(DT);
        assert!(g.enemies[0].hp < 500.0, "the page did not arrive");
        assert_eq!(g.enemies[0].status.burn > 0.0, burn);
    }
}

/// Plants a torch under the player's feet and returns where it is.
fn torch_here(g: &mut Game) -> V2 {
    let tile = Pos::from_world(g.body.pos.x, g.body.pos.y);
    g.delta_mut().torches.push(tile);
    super::tile_center(tile)
}

fn torch_light(g: &mut Game, at: V2) -> f32 {
    g.collect_lights();
    g.lights
        .iter()
        .find(|l| l.priority == 7 && l.pos == at)
        .expect("torch light")
        .radius
}

#[test]
fn tall_torches_light_farther() {
    let mut g = game();
    let at = torch_here(&mut g);
    assert!((torch_light(&mut g, at) - 4.6 * TILE).abs() < 1e-3);
    fit(&mut g, "torches", "antorcha_alta", 3);
    assert!((torch_light(&mut g, at) - 4.6 * TILE * 1.45).abs() < 1e-3);
}

#[test]
fn a_partners_torch_improvement_serves_everyone() {
    let mut g = game();
    let mut visitor = PlayerData::default();
    let tall = Fitted {
        id: "antorcha_alta".into(),
        level: 1,
    };
    visitor.upgrades.insert("torches".into(), tall);
    g.join(visitor);
    let at = torch_here(&mut g);
    assert!((torch_light(&mut g, at) - 4.6 * TILE * 1.15).abs() < 1e-3);
}

#[test]
fn warm_torches_heal_whoever_stands_beside_them() {
    let mut g = game();
    let at = torch_here(&mut g);
    g.profile.player.hp = 40.0;
    g.update_torches(1.0);
    assert_eq!(g.profile.player.hp, 40.0, "nothing fitted");
    fit(&mut g, "torches", "antorcha_calida", 3);
    g.update_torches(2.0);
    assert!((g.profile.player.hp - 43.0).abs() < 1e-3);
    // Never above the limit, and not from far away.
    g.profile.player.hp = g.derived.max_hp - 0.1;
    g.update_torches(1.0);
    assert_eq!(g.profile.player.hp, g.derived.max_hp);
    g.profile.player.hp = 40.0;
    g.body.pos = at + v2(6.0 * TILE, 0.0);
    g.update_torches(1.0);
    assert_eq!(g.profile.player.hp, 40.0);
}

#[test]
fn the_fallen_are_not_warmed_by_torches() {
    let mut g = game();
    g.join(PlayerData::default());
    torch_here(&mut g);
    fit(&mut g, "torches", "antorcha_calida", 3);
    g.as_player(GUEST, |g| {
        g.profile.player.hp = 0.0;
        g.body.dead = 5.0;
    });
    g.update_torches(2.0);
    assert_eq!(g.view(GUEST).expect("guest").1.hp, 0.0);
}

#[test]
fn fierce_torches_burn_what_comes_near() {
    let mut g = game();
    with_a_creature(&mut g);
    let at = torch_here(&mut g);
    g.enemies[0].pos = at + v2(TILE, 0.0);
    let after = |g: &mut Game, seconds: f32| {
        for _ in 0..(seconds / DT).round() as u32 {
            g.update_torches(DT);
        }
        g.enemies[0].hp
    };
    assert_eq!(after(&mut g, 2.0), 500.0, "nothing fitted");
    fit(&mut g, "torches", "antorcha_brava", 3);
    assert_eq!(after(&mut g, 2.05), 492.0);
    g.enemies[0].pos = at + v2(8.0 * TILE, 0.0);
    assert_eq!(after(&mut g, 2.0), 492.0, "too far away");
}

/// Drinks a tonic from 1 health and no energy; returns what came back.
fn drink(g: &mut Game) -> (f32, f32) {
    g.profile.player.hp = 1.0;
    g.profile.player.energy = 0.0;
    g.profile.player.add_consumable(Consumable::Tonic, 1);
    g.use_tonic();
    (g.profile.player.hp - 1.0, g.profile.player.energy)
}

#[test]
fn thick_and_lively_tonics_give_more() {
    let mut g = game();
    let max = g.derived.max_hp;
    assert_eq!(drink(&mut g), (max * 0.45, 0.0));
    fit(&mut g, "tonic", "tonico_espeso", 3);
    assert!((drink(&mut g).0 - max * 0.67).abs() < 1e-3);
    fit(&mut g, "tonic", "tonico_vivo", 2);
    assert_eq!(drink(&mut g), (max * 0.45, 50.0));
}

#[test]
fn good_batches_bring_more_tonics_and_ink() {
    let mut g = game();
    for resource in Resource::ALL {
        g.profile.player.add_resource(resource, 100);
    }
    g.profile.world.stations.push((Station::Alembic, 0));
    let made = |g: &mut Game, id: &str| {
        let tonics = g.profile.player.consumable(Consumable::Tonic);
        let ink = g.profile.player.resource(Resource::Ink);
        g.craft(id, None).expect(id);
        (
            g.profile.player.consumable(Consumable::Tonic) - tonics,
            g.profile.player.resource(Resource::Ink).saturating_sub(ink),
        )
    };
    assert_eq!(made(&mut g, "tonic").0, 2);
    assert_eq!(made(&mut g, "ink").1, 3);
    assert_eq!(made(&mut g, "ink_paper").1, 2);
    fit(&mut g, "tonic", "tonico_rendidor", 3);
    assert_eq!(made(&mut g, "tonic").0, 4);
    fit(&mut g, "ink", "tinta_abundante", 2);
    assert_eq!(made(&mut g, "ink").1, 5);
    assert_eq!(made(&mut g, "ink_paper").1, 4);
}
