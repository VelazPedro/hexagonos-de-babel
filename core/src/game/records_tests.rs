//! What is counted while playing.

use super::Game;
use super::actors::{Drop, DropKind};
use super::coop::{GUEST, HOST};
use super::events::{Input, NumberKind};
use super::profile::Profile;
use super::sync::{Publisher, State};
use super::travel::Arrival;
use super::vec::v2;
use crate::data::{GameData, NodeKind, Resource, Station, ToolKind, dev_assets_dir};
use crate::progression::player::PlayerData;
use crate::progression::records::Tally;
use crate::world::site::SiteKey;
use std::sync::{Arc, OnceLock};

const DT: f32 = 1.0 / 60.0;

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

fn game() -> Game {
    Game::new(data(), Profile::new(31))
}

fn pair() -> Game {
    let mut g = game();
    g.join(PlayerData::default());
    g
}

fn tally(g: &Game, slot: u8) -> Tally {
    g.view(slot).expect("player").1.records.tally
}

#[test]
fn a_new_game_starts_on_its_first_hexagon() {
    let g = game();
    let t = tally(&g, HOST);
    assert!(g.profile.player.records.begun);
    assert_eq!((t.hexagons, t.deepest_floor, t.play_seconds), (1, 1, 0));
}

#[test]
fn an_older_save_starts_from_what_its_world_knew() {
    let mut played = game();
    played.enter(SiteKey::walk(1, 4, 0), Arrival::Hub);
    played.enter(SiteKey::walk(3, 1, 1), Arrival::Hub);
    let mut old = played.profile.clone();
    old.player.records = Default::default();
    old.world.play_seconds = 5_000.4;
    old.world.guardians_defeated.extend([5, 10]);
    old.begin_records();
    let t = old.player.records.tally;
    assert_eq!(
        (t.play_seconds, t.deepest_floor, t.hexagons, t.guardians),
        (5_000, 3, 3, 2)
    );
    assert_eq!((t.enemies, t.books, t.broken), (0, 0, 0));
    // Only once.
    old.world.play_seconds = 9_999.0;
    old.begin_records();
    assert_eq!(old.player.records.tally.play_seconds, 5_000);
}

#[test]
fn new_hexagons_and_depth_are_counted_once() {
    let mut g = pair();
    let before = tally(&g, HOST).hexagons;
    g.enter(SiteKey::walk(1, 4, 0), Arrival::Hub);
    g.enter(SiteKey::walk(1, 0, 0), Arrival::Hub);
    g.enter(SiteKey::walk(1, 4, 0), Arrival::Hub);
    for slot in [HOST, GUEST] {
        let t = tally(&g, slot);
        assert_eq!(t.hexagons - tally_start(slot, before), 1, "slot {slot}");
    }
    g.enter(SiteKey::walk(6, 1, 1), Arrival::Hub);
    assert_eq!(tally(&g, HOST).deepest_floor, 6);
    assert_eq!(tally(&g, GUEST).deepest_floor, 6);
}

/// The partner joins after the first hexagon was counted.
fn tally_start(slot: u8, host: u32) -> u32 {
    if slot == HOST { host } else { 0 }
}

#[test]
fn time_adds_up_while_the_world_runs() {
    let mut g = pair();
    for _ in 0..(17.0 / DT) as u32 {
        g.update(DT, &Input::default());
    }
    for slot in [HOST, GUEST] {
        let seconds = tally(&g, slot).play_seconds;
        assert!((10..=17).contains(&seconds), "{seconds} for slot {slot}");
    }
}

#[test]
fn breaking_crafting_and_improving_are_counted() {
    let mut g = game();
    for resource in Resource::ALL {
        g.profile.player.add_resource(resource, 200);
    }
    let mut node = g.nodes[0].clone();
    (node.id, node.kind, node.hp) = (90_001, NodeKind::Vine, 1.0);
    g.nodes.push(node);
    g.profile.player.active_tool = ToolKind::Knife;
    g.harvest(90_001, 5.0);
    g.craft("build_binder", Some(0)).expect("build");
    g.craft("knife_1", None).expect("craft");
    assert!(g.craft("knife_1", None).is_err());
    g.upgrade("lomo_reforzado").expect("improve");
    assert!(g.upgrade("filo_igneo").is_err(), "no forge");
    let t = tally(&g, HOST);
    assert_eq!((t.broken, t.crafted, t.improved), (1, 2, 1));
}

#[test]
fn gathered_materials_are_counted() {
    let mut g = pair();
    let mut drop = Drop {
        pos: g.body.pos,
        vel: v2(0.0, 0.0),
        z: 0.0,
        vz: 0.0,
        age: 1.0,
        kind: DropKind::Resource(Resource::Paper, 7),
        magnet: false,
    };
    g.pick_up(drop.clone());
    drop.kind = DropKind::Resource(Resource::Iron, 2);
    g.pick_up(drop);
    // What one picks up, both have.
    assert_eq!(tally(&g, HOST).materials, 9);
    assert_eq!(tally(&g, GUEST).materials, 9);
}

#[test]
fn defeats_count_enemies_elites_and_damage() {
    let mut g = pair();
    for i in 1..60 {
        g.enter(SiteKey::walk(1, i, 3), Arrival::Hub);
        if !g.enemies.is_empty() {
            break;
        }
    }
    let (uid, health) = (g.enemies[0].uid, g.enemies[0].hp);
    g.enemies[0].illusion = false;
    g.apply_damage_to_enemy(uid, health + 500.0, NumberKind::Enemy);
    let t = tally(&g, HOST);
    assert_eq!(t.enemies, 1);
    // Only what there was to take.
    assert_eq!(t.damage_dealt, health.round() as u64);
    assert_eq!(tally(&g, GUEST).enemies, 1, "both were there");
    assert_eq!(tally(&g, GUEST).damage_dealt, 0, "only one struck");
}

#[test]
fn falls_and_damage_taken_are_counted() {
    let mut g = game();
    g.profile.player.hp = 50.0;
    g.lose_hp(20.0);
    assert_eq!(tally(&g, HOST).damage_taken, 20);
    g.lose_hp(500.0);
    let t = tally(&g, HOST);
    assert_eq!((t.damage_taken, t.falls), (50, 1));
    // The fallen take nothing more.
    g.lose_hp(10.0);
    assert_eq!(tally(&g, HOST).damage_taken, 50);
}

#[test]
fn reading_is_counted_for_the_reader() {
    let mut g = pair();
    g.enter(SiteKey::walk(1, 4, 0), Arrival::Hub);
    g.read_book();
    g.read_book();
    let t = tally(&g, HOST);
    assert_eq!((t.books, t.pages), (1, 1));
    assert_eq!(tally(&g, GUEST).books, 0);
}

#[test]
fn a_canonical_phrase_is_counted_once() {
    let mut g = game();
    let feistel = crate::babel::feistel::Feistel::new(31);
    let key = crate::world::site::key_for_phrase(&feistel, "lampara bajo mar", 3).expect("key");
    g.enter(key, Arrival::Hub);
    assert!(!g.site.analysis.canons.is_empty(), "the phrase is there");
    g.read_book();
    g.read_book();
    assert_eq!(tally(&g, HOST).canons, 1);
    assert_eq!(g.profile.player.records.found_canons.len(), 1);
}

fn sent(state: &State) -> State {
    let bytes = postcard::to_stdvec(state).expect("encode");
    postcard::from_bytes(&bytes).expect("decode")
}

#[test]
fn a_blow_does_not_resend_the_guests_whole_data() {
    let mut g = pair();
    let mut publisher = Publisher::default();
    let first = publisher.capture(&mut g).expect("state");
    assert!(first.guest.is_some());
    g.as_player(GUEST, |g| {
        let t = &mut g.profile.player.records.tally;
        t.damage_dealt += 12;
        t.play_seconds += 5;
    });
    let next = publisher.capture(&mut g).expect("state");
    assert!(next.guest.is_none(), "everything was sent again");
    assert_eq!(next.guest_tally, tally(&g, GUEST));
    assert_eq!(tally(&g, GUEST).damage_dealt, 12, "left as it was");
}

#[test]
fn the_guests_records_arrive() {
    let mut g = pair();
    let mut publisher = Publisher::default();
    let first = publisher.capture(&mut g).expect("state");
    let mut mirror = Game::mirror(data(), 31, sent(&first)).expect("mirror");
    g.as_player(GUEST, |g| g.read_book());
    let state = publisher.capture(&mut g).expect("state");
    mirror.apply_state(sent(&state)).expect("apply");
    assert_eq!(mirror.profile.player.records.tally, tally(&g, GUEST));
    assert_eq!(mirror.profile.player.records.opened.len(), 1);
    // The host's long list of books does not travel.
    assert!(state.host.records.opened.is_empty());
}

#[test]
fn building_on_a_plot_counts_as_crafting_only_for_who_did_it() {
    let mut g = pair();
    g.profile.player.add_resource(Resource::Paper, 20);
    g.profile.player.add_resource(Resource::Stone, 20);
    g.craft("build_binder", Some(0)).expect("build");
    assert_eq!(g.stations_built().len(), 1);
    assert!(g.stations_built().contains(&Station::Binder));
    assert_eq!(tally(&g, HOST).crafted, 1);
    assert_eq!(tally(&g, GUEST).crafted, 0);
}
