use super::actors::DropKind;
use super::coop::{Exit, GUEST, HOST, REVIVE_TIME};
use super::events::{GameEvent, Input};
use super::profile::Profile;
use super::travel::Arrival;
use super::vec::{V2, v2};
use super::{Game, tile_center};
use crate::babel::rarity::Rarity;
use crate::data::{GameData, Resource, Station, dev_assets_dir};
use crate::progression::items;
use crate::progression::player::PlayerData;
use crate::world::floors;
use crate::world::grid::{self, TILE};
use crate::world::hex::{self, Dir};
use crate::world::site::SiteKey;
use std::sync::{Arc, OnceLock};

const DT: f32 = 1.0 / 60.0;

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

/// Two players in the host's home gallery.
fn pair(seed: u64) -> Game {
    let mut g = Game::new(data(), Profile::new(seed));
    g.join(PlayerData::default());
    g.drain_events();
    g.drain_partner_events();
    g
}

fn put(g: &mut Game, slot: u8, pos: V2) {
    g.as_player(slot, |g| {
        g.body.pos = pos;
        g.body.vel = V2::ZERO;
        g.body.door_lock = false;
    });
}

fn body(g: &Game, slot: u8) -> &super::player::Body {
    g.view(slot).expect("player").0
}

fn player(g: &Game, slot: u8) -> &PlayerData {
    g.view(slot).expect("player").1
}

fn run(g: &mut Game, host: Input, guest: Input, seconds: f32) {
    for _ in 0..((seconds / DT) as u32).max(1) {
        g.step(DT, &host, &guest);
    }
}

fn idle(g: &mut Game, seconds: f32) {
    run(g, Input::default(), Input::default(), seconds);
}

fn interact() -> Input {
    Input {
        interact: true,
        ..Default::default()
    }
}

fn door_north() -> (Dir, V2) {
    let d = Dir::ALL
        .into_iter()
        .find(|&d| hex::neighbor(0, 0, d) == (0, -1))
        .expect("north");
    let (x, y) = grid::door_mid(d);
    (d, v2(x * TILE, y * TILE + 4.0))
}

/// Both players beside the stairs of the first floor.
fn at_the_stairs(seed: u64) -> (Game, V2) {
    let mut g = pair(seed);
    let (q, r) = floors::stair_offset(seed, 1);
    g.enter(SiteKey::walk(1, q, r), Arrival::Hub);
    g.enemies.clear();
    let stairs = tile_center(g.site.stairs.expect("stairs"));
    put(&mut g, HOST, stairs + v2(-6.0, 12.0));
    put(&mut g, GUEST, stairs + v2(6.0, 12.0));
    g.drain_events();
    (g, stairs)
}

#[test]
fn the_guest_arrives_beside_the_host_and_both_carry_a_light() {
    let mut g = pair(1);
    assert_eq!(g.players(), 2);
    assert!(body(&g, GUEST).pos.dist(body(&g, HOST).pos) < 40.0);
    idle(&mut g, 0.1);
    assert_eq!(g.lights.iter().filter(|l| l.priority == 10).count(), 2);
    assert_eq!(g.acting(), HOST);
}

#[test]
fn each_player_moves_with_their_own_controls() {
    let mut g = pair(2);
    let (a, b) = (body(&g, HOST).pos, body(&g, GUEST).pos);
    let right = Input {
        movement: v2(1.0, 0.0),
        ..Default::default()
    };
    run(&mut g, Input::default(), right, 0.4);
    assert!(body(&g, HOST).pos.dist(a) < 2.0);
    assert!(body(&g, GUEST).pos.x > b.x + 15.0);
}

#[test]
fn a_doorway_is_crossed_only_when_both_stand_in_it() {
    let mut g = pair(2);
    let home = g.site.key.clone();
    let (d, door) = door_north();
    put(&mut g, GUEST, v2(320.0, 300.0));
    put(&mut g, HOST, door);
    idle(&mut g, 1.0);
    assert_eq!(g.site.key, home, "the host alone must wait");
    assert_eq!(g.votes(Exit::Door(d)), 1);

    // Walking away takes the wish back.
    put(&mut g, HOST, v2(320.0, 300.0));
    idle(&mut g, 0.2);
    assert_eq!(g.votes(Exit::Door(d)), 0);

    put(&mut g, GUEST, door + v2(8.0, 0.0));
    idle(&mut g, 0.5);
    assert_eq!(g.site.key, home, "nor can the guest go alone");

    put(&mut g, HOST, door - v2(8.0, 0.0));
    idle(&mut g, DT);
    assert_eq!(g.site.key, SiteKey::walk(1, 0, -1));
    assert!(body(&g, HOST).pos.dist(body(&g, GUEST).pos) < 40.0);
    assert!(g.votes(Exit::Door(d)) == 0 && g.acting() == HOST);
    // Neither walks straight back through the door they came from.
    idle(&mut g, 0.5);
    assert_eq!(g.site.key, SiteKey::walk(1, 0, -1));
}

#[test]
fn the_stairs_need_both_votes_and_a_vote_can_be_taken_back() {
    let (mut g, _) = at_the_stairs(3);
    let upstairs = g.site.key.clone();
    run(&mut g, interact(), Input::default(), DT);
    assert_eq!(g.site.key, upstairs);
    assert_eq!(g.votes(Exit::Down), 1);
    assert!(g.prompt().is_some_and(|p| p.contains("1/2")));

    run(&mut g, interact(), Input::default(), DT);
    assert_eq!(g.votes(Exit::Down), 0, "pressing again takes it back");

    run(&mut g, Input::default(), interact(), DT);
    assert_eq!(g.site.key, upstairs);
    run(&mut g, interact(), Input::default(), DT);
    assert_eq!(g.site.depth, 2);
}

#[test]
fn walking_away_from_the_stairs_withdraws_the_vote() {
    let (mut g, stairs) = at_the_stairs(4);
    run(&mut g, interact(), Input::default(), DT);
    put(&mut g, HOST, stairs + v2(0.0, 90.0));
    idle(&mut g, 0.1);
    assert_eq!(g.votes(Exit::Down), 0);
    run(&mut g, Input::default(), interact(), DT);
    assert_eq!(g.site.depth, 1);
}

#[test]
fn a_fallen_partner_is_carried_out_by_the_one_standing() {
    let (mut g, _) = at_the_stairs(5);
    g.as_player(GUEST, |g| g.lose_hp(1.0e6));
    put(&mut g, GUEST, v2(200.0, 300.0));
    assert!(body(&g, GUEST).downed);
    run(&mut g, interact(), Input::default(), DT);
    assert_eq!(g.site.depth, 2);
    assert!(!body(&g, GUEST).downed && player(&g, GUEST).hp > 0.0);
}

#[test]
fn staying_beside_a_fallen_partner_raises_them_without_any_loss() {
    let mut g = pair(6);
    g.as_player(GUEST, |g| {
        g.profile.player.add_resource(Resource::Paper, 40);
        g.lose_hp(1.0e6);
    });
    let spot = body(&g, GUEST).pos;
    put(&mut g, HOST, spot + v2(90.0, 0.0));
    idle(&mut g, REVIVE_TIME + 1.0);
    assert!(body(&g, GUEST).downed, "nobody was near");
    put(&mut g, HOST, spot + v2(14.0, 0.0));
    idle(&mut g, REVIVE_TIME + 0.2);
    assert!(!body(&g, GUEST).downed && body(&g, GUEST).dead <= 0.0);
    assert!(player(&g, GUEST).hp > 0.0);
    assert_eq!(player(&g, GUEST).resource(Resource::Paper), 40);
}

#[test]
fn when_both_fall_they_wake_together_at_the_lamp() {
    let (mut g, _) = at_the_stairs(7);
    g.profile.player.add_resource(Resource::Paper, 40);
    for slot in [HOST, GUEST] {
        g.as_player(slot, |g| g.lose_hp(1.0e6));
    }
    idle(&mut g, 4.0);
    assert!(g.site.key.is_home());
    for slot in [HOST, GUEST] {
        assert!(!body(&g, slot).downed && body(&g, slot).dead <= 0.0);
        assert!(player(&g, slot).hp > 50.0);
    }
    assert_eq!(player(&g, HOST).resource(Resource::Paper), 40);
    let events = g.drain_events();
    assert!(!events.iter().any(|e| matches!(e, GameEvent::PlayerDied)));
}

#[test]
fn what_one_picks_up_both_receive() {
    let mut g = pair(8);
    put(&mut g, HOST, v2(180.0, 300.0));
    put(&mut g, GUEST, v2(420.0, 300.0));
    // A pile and an item at the guest's feet, far from the host.
    g.spawn_drop(v2(420.0, 300.0), DropKind::Resource(Resource::Ink, 7));
    let item = items::from_words(&g.data.clone(), 900, "daga", None, Rarity::Rare);
    g.spawn_item_drop(v2(422.0, 300.0), item);
    idle(&mut g, 3.0);
    assert!(g.drops.is_empty());
    for slot in [HOST, GUEST] {
        let p = player(&g, slot);
        assert_eq!(p.resource(Resource::Ink), 7, "player {slot}");
        let has = p.inventory.iter().chain(p.equipment.iter().flatten());
        assert_eq!(has.filter(|i| i.words == ["daga"]).count(), 1);
    }
    assert!(g.profile.world.unclaimed.is_empty());
}

#[test]
fn the_world_rewards_both_but_each_earns_their_own_work() {
    let mut g = pair(9);
    g.grant_xp(20);
    assert_eq!((player(&g, HOST).xp, player(&g, GUEST).xp), (20, 20));
    g.as_player(GUEST, |g| g.grant_own_xp(5));
    assert_eq!((player(&g, HOST).xp, player(&g, GUEST).xp), (20, 25));
}

#[test]
fn private_messages_reach_only_whoever_caused_them() {
    let mut g = pair(10);
    g.as_player(GUEST, |g| g.say("solo invitado"));
    g.say("solo anfitrión");
    g.floating(v2(1.0, 1.0), "para los dos", [1, 2, 3]);
    let host = g.drain_events();
    let guest = g.drain_partner_events();
    let says = |events: &[GameEvent], wanted: &str| {
        events
            .iter()
            .any(|e| matches!(e, GameEvent::Message(m) if m == wanted))
    };
    assert!(says(&host, "solo anfitrión") && !says(&host, "solo invitado"));
    assert!(says(&guest, "solo invitado") && !says(&guest, "solo anfitrión"));
    for events in [&host, &guest] {
        let floating = events
            .iter()
            .filter(|e| matches!(e, GameEvent::Floating { .. }))
            .count();
        assert_eq!(floating, 1);
    }
}

#[test]
fn a_journey_from_a_menu_takes_both_or_nobody() {
    let (mut g, _) = at_the_stairs(11);
    g.light_return_lamp();
    g.profile.player.add_resource(Resource::Oil, 99);
    g.light_return_lamp();
    g.travel_home();
    put(&mut g, GUEST, v2(150.0, 300.0));
    put(&mut g, HOST, v2(450.0, 300.0));
    assert!(!g.travel_to_lamp(0));
    assert!(g.catalog_travel("lobo").is_err());
    let beside = body(&g, HOST).pos + v2(20.0, 0.0);
    put(&mut g, GUEST, beside);
    assert!(g.travel_to_lamp(0));
}

#[test]
fn the_chest_belongs_to_the_host() {
    let mut g = pair(12);
    g.profile.world.stations.push((Station::Chest, 0));
    g.profile.world.storage.insert(Resource::Paper, 30);
    g.as_player(GUEST, |g| g.withdraw(Resource::Paper, 30));
    assert_eq!(player(&g, GUEST).resource(Resource::Paper), 0);
    assert_eq!(g.profile.world.storage[&Resource::Paper], 30);
    g.withdraw(Resource::Paper, 10);
    assert_eq!(player(&g, HOST).resource(Resource::Paper), 10);
}

#[test]
fn creatures_go_after_whoever_is_closest() {
    let (mut g, _) = at_the_stairs(13);
    let found = (1..40).any(|i| {
        g.enter(SiteKey::walk(3, i, 2), Arrival::Hub);
        !g.enemies.is_empty()
    });
    assert!(found, "a gallery with creatures");
    g.enemies.truncate(1);
    // The middle of a gallery is open ground.
    let spot = tile_center(g.site.hub);
    g.enemies[0].pos = spot;
    put(&mut g, HOST, spot + v2(200.0, 0.0));
    put(&mut g, GUEST, spot + v2(20.0, 0.0));
    // Long enough for any kind of creature to close in and strike.
    for _ in 0..600 {
        g.step(DT, &Input::default(), &Input::default());
        if player(&g, GUEST).hp < 100.0 {
            break;
        }
    }
    assert!(player(&g, GUEST).hp < 100.0, "it ignored the guest");
    assert!((player(&g, HOST).hp - 100.0).abs() < 0.5);
}

#[test]
fn leaving_hands_back_the_guest_and_leaves_the_host_playing_alone() {
    let mut g = pair(14);
    g.as_player(GUEST, |g| g.profile.player.add_resource(Resource::Paper, 3));
    g.lose_hp(1.0e6);
    let guest = g.leave().expect("guest");
    assert_eq!(guest.resource(Resource::Paper), 3);
    assert_eq!(g.players(), 1);
    assert!(!g.body.downed && g.profile.player.hp > 0.0);
    let (_, door) = door_north();
    put(&mut g, HOST, door);
    idle(&mut g, DT);
    assert_eq!(g.site.key, SiteKey::walk(1, 0, -1), "alone again");
}

#[test]
fn either_player_opens_the_way_down() {
    use crate::data::ToolKind;
    let mut g = pair(4);
    let (q, r) = floors::stair_offset(4, 3);
    g.enter(SiteKey::walk(3, q, r), Arrival::Hub);
    assert_eq!(g.stairs_require(), Some((4, 1)));
    g.as_player(GUEST, |g| {
        g.profile.player.tools[ToolKind::Knife.index()] = 1;
        g.profile.player.tools[ToolKind::Hammer.index()] = 1;
    });
    assert_eq!(g.stairs_require(), None, "the guest's tools count");
    // One tool each is not the same as one player with both.
    g.as_player(GUEST, |g| {
        g.profile.player.tools[ToolKind::Hammer.index()] = 0
    });
    g.profile.player.tools[ToolKind::Hammer.index()] = 1;
    assert_eq!(g.stairs_require(), Some((4, 1)));
}
