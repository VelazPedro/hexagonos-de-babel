use super::Game;
use super::actors::Ai;
use super::coop::{GUEST, HOST};
use super::events::{GameEvent, Input, ScreenRequest};
use super::profile::Profile;
use super::sync::{Action, Publisher, State, tidy_events, tidy_input};
use super::travel::Arrival;
use super::vec::v2;
use crate::babel::rarity::Rarity;
use crate::data::{GameData, Resource, Slot, dev_assets_dir};
use crate::progression::items;
use crate::progression::pet::PetLook;
use crate::progression::player::PlayerData;
use crate::world::site::{SiteKey, key_for_phrase};
use std::sync::{Arc, OnceLock};

const DT: f32 = 1.0 / 60.0;
const SEED: u64 = 77;

fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| Arc::new(GameData::load(&dev_assets_dir()).expect("data")))
        .clone()
}

/// A guest with something in the bag.
fn visitor() -> PlayerData {
    let data = data();
    let mut player = PlayerData::default();
    for (i, word) in ["daga", "espada", "yelmo"].into_iter().enumerate() {
        let item = items::from_words(&data, i as u64 + 1, word, None, Rarity::Rare);
        player.receive_item(item);
    }
    player.next_uid = 20;
    player.add_resource(Resource::Paper, 12);
    player.gain_xp(500);
    player
}

fn host() -> Game {
    let mut g = Game::new(data(), Profile::new(SEED));
    g.join(visitor());
    g
}

/// Through the wire and back.
fn sent(state: &State) -> State {
    let bytes = postcard::to_stdvec(state).expect("encode");
    postcard::from_bytes(&bytes).expect("decode")
}

fn mirror_of(host: &mut Game, publisher: &mut Publisher) -> Game {
    publisher.resend();
    let state = publisher.capture(host).expect("state");
    Game::mirror(data(), SEED, sent(&state)).expect("mirror")
}

/// A gallery with creatures, a few steps into the fight.
fn busy_host() -> Game {
    let mut g = host();
    for q in 1..40 {
        g.enter(SiteKey::walk(4, q, 1), Arrival::Hub);
        if g.enemies.len() >= 2 && !g.nodes.is_empty() {
            break;
        }
    }
    for _ in 0..120 {
        g.step(DT, &Input::default(), &Input::default());
    }
    g
}

fn assert_same(host: &Game, mirror: &Game) {
    assert_eq!(mirror.site.key, host.site.key);
    assert_eq!(mirror.acting(), GUEST);
    for slot in [HOST, GUEST] {
        let (a, ad, _) = host.view(slot).expect("host side");
        let (b, bd, _) = mirror.view(slot).expect("mirror side");
        assert_eq!(a.pos, b.pos, "player {slot}");
        assert_eq!(ad.level, bd.level);
        assert_eq!(ad.equipment, bd.equipment);
    }
    assert_eq!(mirror.profile.player, *host.view(GUEST).expect("guest").1);
    assert_eq!(mirror.enemies.len(), host.enemies.len());
    for (a, b) in host.enemies.iter().zip(&mirror.enemies) {
        assert_eq!((a.uid, a.pos, a.lit, a.ai), (b.uid, b.pos, b.lit, b.ai));
    }
    assert_eq!(mirror.nodes.len(), host.nodes.len());
    assert_eq!(mirror.lights.len(), mirror.lights.len());
    assert_eq!(mirror.delta(), host.delta());
}

#[test]
fn the_mirror_shows_what_the_host_has() {
    let mut host = busy_host();
    assert!(!host.enemies.is_empty());
    let before = host.profile.clone();
    let mirror = mirror_of(&mut host, &mut Publisher::default());
    assert_eq!(host.profile, before, "capturing leaves the host as it was");
    assert!(mirror.is_mirror() && !host.is_mirror());
    assert_same(&host, &mirror);
    assert!(!mirror.lights.is_empty());
}

#[test]
fn only_what_changed_is_sent_again() {
    let mut host = busy_host();
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut host, &mut publisher);
    host.step(DT, &Input::default(), &Input::default());
    let next = publisher.capture(&mut host).expect("state");
    assert!(next.guest.is_none() && next.world.is_none());
    let size = postcard::to_stdvec(&next).expect("encode").len();
    assert!(size < 24 * 1024, "a routine state takes {size} bytes");
    mirror.apply_state(sent(&next)).expect("apply");
    assert_same(&host, &mirror);

    host.as_player(GUEST, |g| g.profile.player.add_resource(Resource::Ink, 4));
    let changed = publisher.capture(&mut host).expect("state");
    assert!(changed.guest.is_some() && changed.world.is_none());
    mirror.apply_state(sent(&changed)).expect("apply");
    assert_eq!(mirror.profile.player.resource(Resource::Ink), 4);
}

#[test]
fn the_mirror_follows_the_host_into_another_gallery() {
    let mut host = busy_host();
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut host, &mut publisher);
    host.enter(SiteKey::walk(2, 5, 5), Arrival::Hub);
    mirror
        .apply_state(sent(&publisher.capture(&mut host).expect("state")))
        .expect("apply");
    assert_same(&host, &mirror);
    assert_eq!(mirror.profile.world.visited, host.profile.world.visited);
}

#[test]
fn what_the_guest_does_in_menus_is_repeated_by_the_host() {
    let mut host = host();
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut host, &mut publisher);
    let before = mirror.profile.player.clone();
    let hosts_own = host.profile.player.clone();
    assert!(!before.inventory.is_empty());
    mirror.equip(0);
    mirror.unequip(Slot::ALL[0]);
    mirror.swap_inventory(0, 5);
    mirror.recycle(0);
    let skill = mirror
        .data
        .skills
        .iter()
        .find(|s| s.requires.is_empty())
        .map(|s| s.id.clone())
        .expect("a first skill");
    mirror.learn_skill(&skill);
    let actions = mirror.take_actions();
    assert!(actions.len() >= 4, "{actions:?}");
    for action in actions {
        let bytes = postcard::to_stdvec(&action).expect("encode");
        host.apply_action(postcard::from_bytes(&bytes).expect("decode"));
    }
    assert_eq!(*host.view(GUEST).expect("guest").1, mirror.profile.player);
    assert_ne!(mirror.profile.player, before);
    assert_eq!(host.acting(), HOST);
    assert_eq!(host.profile.player, hosts_own, "the host's own");
}

#[test]
fn an_action_about_an_item_that_is_gone_does_nothing() {
    let mut host = host();
    let before = host.view(GUEST).expect("guest").1.clone();
    host.apply_action(Action::Equip(999_999));
    host.apply_action(Action::Recycle(999_999));
    host.apply_action(Action::LampTravel(u32::MAX));
    host.apply_action(Action::Unequip(Slot::ALL[4]));
    host.apply_action(Action::CatalogTravel("x".repeat(10_000)));
    assert_eq!(
        host.view(GUEST).expect("guest").1.inventory,
        before.inventory
    );
}

#[test]
fn the_guest_asks_to_travel_and_only_the_host_moves() {
    let mut host = host();
    host.profile.world.lamps.push(super::profile::LampRecord {
        key: SiteKey::walk(2, 1, 1),
        pos: host.site.hub,
        label: "una".into(),
    });
    let mut mirror = mirror_of(&mut host, &mut Publisher::default());
    assert!(mirror.travel_to_lamp(0));
    assert!(mirror.site.key.is_home(), "the mirror waits for the host");
    assert_eq!(mirror.take_actions(), vec![Action::LampTravel(0)]);
    host.apply_action(Action::LampTravel(0));
    assert_eq!(host.site.key, SiteKey::walk(2, 1, 1));
    assert_eq!(host.acting(), HOST);
}

#[test]
fn a_catalog_destination_travels_too() {
    let mut host = host();
    let feistel = crate::babel::feistel::Feistel::new(SEED);
    let key = key_for_phrase(&feistel, "lobo sombrio", 3).expect("key");
    assert!(key.is_remote());
    host.enter(key, Arrival::Hub);
    let mirror = mirror_of(&mut host, &mut Publisher::default());
    assert_same(&host, &mirror);
}

#[test]
fn impossible_states_are_rejected() {
    let mut host = busy_host();
    let mut publisher = Publisher::default();
    let good = publisher.capture(&mut host).expect("state");
    assert!(good.check(&host.data).is_ok());
    let spoil: [fn(&mut State); 11] = [
        // An attack drawn along an enormous vector would freeze the game.
        |s| {
            s.enemies[0].ai = Ai::Windup {
                t: 1.0,
                total: 1.0,
                dir: v2(1.0e30, 0.0),
                target: v2(0.0, 0.0),
            }
        },
        |s| s.enemies[0].radius = 1.0e6,
        |s| s.delta.torches = vec![crate::world::grid::Pos::new(i32::MAX, 0)],
        |s| {
            if let Some(world) = &mut s.world {
                world.visited.insert(
                    "x".into(),
                    super::profile::VisitRecord {
                        floor: 1,
                        q: i32::MIN,
                        r: i32::MAX,
                        biome: 0,
                        stairs: false,
                        exits: [false; 6],
                    },
                );
            }
        },
        |s| s.enemies[0].archetype = 9999,
        |s| s.enemies[0].modifiers = vec![usize::MAX],
        |s| s.enemies[0].word = Some((u32::MAX, Rarity::Common)),
        |s| s.guest_body.pos = v2(f32::NAN, 0.0),
        |s| s.key = SiteKey::walk(u32::MAX, 0, 0),
        |s| s.key = SiteKey::walk(1, i32::MIN, 0),
        |s| {
            let node = s.nodes[0].clone();
            s.nodes = vec![node; 10_000];
        },
    ];
    for (i, spoil) in spoil.into_iter().enumerate() {
        let mut bad = sent(&good);
        spoil(&mut bad);
        assert!(bad.check(&host.data).is_err(), "case {i}");
    }
    // And a mirror keeps what it had.
    let mut mirror = Game::mirror(host.data.clone(), SEED, sent(&good)).expect("mirror");
    let mut bad = sent(&good);
    bad.enemies[0].archetype = 9999;
    assert!(mirror.apply_state(bad).is_err());
    assert_eq!(mirror.enemies.len(), host.enemies.len());
}

#[test]
fn damaged_bytes_never_panic() {
    let mut host = busy_host();
    let state = Publisher::default().capture(&mut host).expect("state");
    let bytes = postcard::to_stdvec(&state).expect("encode");
    let mut dice = crate::babel::seeds::Dice::from_u64(9);
    for _ in 0..3000 {
        let mut damaged = bytes.clone();
        for _ in 0..1 + dice.below(8) {
            let at = dice.index(damaged.len());
            damaged[at] ^= 1 << dice.below(8);
        }
        damaged.truncate(1 + dice.index(damaged.len()));
        if let Ok(read) = postcard::from_bytes::<State>(&damaged)
            && read.check(&host.data).is_ok()
        {
            // Whatever passes must be usable.
            let _ = Game::mirror(host.data.clone(), SEED, read);
        }
    }
}

#[test]
fn received_controls_and_events_are_brought_within_limits() {
    let wild = Input {
        movement: v2(500.0, f32::NAN),
        aim_at: Some(v2(f32::INFINITY, 0.0)),
        cycle_tool: i32::MAX,
        ..Default::default()
    };
    let tidy = tidy_input(wild);
    assert!(tidy.movement.len() <= 1.0 && tidy.aim_at.is_none());
    assert_eq!(tidy.cycle_tool, 1);

    let events = tidy_events(vec![
        GameEvent::Message("á".repeat(100_000)),
        GameEvent::Particles {
            pos: v2(1.0, 1.0),
            z: 0.0,
            color: [0; 3],
            count: u16::MAX,
            speed: 1.0,
        },
        GameEvent::Open(ScreenRequest::Build(250)),
        GameEvent::Shake(f32::NAN),
    ]);
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], GameEvent::Message(m) if m.chars().count() < 300));
    assert!(matches!(events[1], GameEvent::Particles { count, .. } if count <= 150));
    assert!(matches!(events[2], GameEvent::Shake(a) if a == 0.0));
    assert_eq!(tidy_events(vec![GameEvent::Autosave; 100_000]).len(), 600);
}

#[test]
fn the_mirror_shows_both_pets() {
    let mut g = host();
    g.profile.player.name = "Anfitriona".into();
    g.profile.player.pet.name = "Canela".into();
    g.as_player(GUEST, |g| {
        g.profile.player.pet.name = "Tinto".into();
        g.profile.player.pet.look.fur = 3;
        g.body.pet.pos = g.body.pos + v2(-20.0, 4.0);
    });
    let mut publisher = Publisher::default();
    let mirror = mirror_of(&mut g, &mut publisher);
    assert_eq!(mirror.profile.player.pet.name, "Tinto");
    assert_eq!(mirror.profile.player.pet.look.fur, 3);
    let (other_body, other, _) = mirror.view(HOST).expect("host");
    assert_eq!(
        (other.name.as_str(), other.pet.name.as_str()),
        ("Anfitriona", "Canela")
    );
    assert_eq!(other_body.pet.pos, g.body.pet.pos);
    let theirs = g.view(GUEST).expect("guest").0.pet.pos;
    assert_eq!(mirror.body.pet.pos, theirs);
}

#[test]
fn a_guests_pet_actions_change_only_the_guest() {
    let mut g = host();
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut g, &mut publisher);
    let points = mirror.profile.player.skill_points;
    assert!(points > 0);

    mirror.set_name("  Vera  ");
    mirror.set_pet(
        "Tinto\u{7}",
        PetLook {
            fur: 2,
            ..PetLook::default()
        },
    );
    assert!(mirror.learn_pet_skill("m_mordisco"));
    let actions = mirror.take_actions();
    assert_eq!(actions.len(), 3);
    for action in actions {
        let bytes = postcard::to_stdvec(&action).expect("encode");
        g.apply_action(postcard::from_bytes(&bytes).expect("decode"));
    }

    let guest = g.view(GUEST).expect("guest").1;
    assert_eq!(
        (guest.name.as_str(), guest.pet.name.as_str()),
        ("Vera", "Tinto")
    );
    assert_eq!(guest.pet.look.fur, 2);
    assert!(guest.pet.skills.contains("m_mordisco"));
    assert_eq!(guest.skill_points, points - 1);
    assert!(g.view(GUEST).expect("guest").2.pet.bite > 0.0);
    assert_eq!(g.profile.player.name, "");
    assert!(g.profile.player.pet.skills.is_empty());

    // An oversized text is ignored outright.
    g.apply_action(Action::SetName("x".repeat(10_000)));
    assert_eq!(g.view(GUEST).expect("guest").1.name, "Vera");
}

#[test]
fn a_state_with_an_impossible_pet_is_rejected() {
    let mut g = host();
    let mut publisher = Publisher::default();
    let good = publisher.capture(&mut g).expect("state");
    let data = data();
    assert!(good.check(&data).is_ok());
    let spoil: [fn(&mut State); 4] = [
        |s| s.host_body.pet.pos.x = f32::NAN,
        |s| s.guest_body.pet.facing = v2(1.0e30, 0.0),
        |s| s.host_body.pet.anim = f32::INFINITY,
        |s| s.guest_body.pet.bite = f32::NAN,
    ];
    for (i, change) in spoil.into_iter().enumerate() {
        let mut bad = good.clone();
        change(&mut bad);
        assert!(bad.check(&data).is_err(), "case {i}");
    }
}

#[test]
fn a_guests_pins_are_theirs_and_reach_the_host() {
    let mut g = host();
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut g, &mut publisher);
    mirror.toggle_pin("knife_1");
    assert!(mirror.profile.player.pinned("knife_1"), "seen at once");
    let actions = mirror.take_actions();
    assert_eq!(actions.len(), 1);
    for action in actions {
        let bytes = postcard::to_stdvec(&action).expect("encode");
        let back = postcard::from_bytes(&bytes).expect("decode");
        g.apply_action(back);
    }
    let theirs = g.view(GUEST).expect("guest").1;
    assert!(theirs.pinned("knife_1"));
    assert!(g.profile.player.pins.is_empty(), "not the host's");
}

#[test]
fn a_guests_improvements_change_only_the_guest() {
    let mut g = host();
    g.profile
        .world
        .stations
        .push((crate::data::Station::Binder, 0));
    g.as_player(GUEST, |g| {
        for resource in Resource::ALL {
            g.profile.player.add_resource(resource, 100);
        }
        g.refresh();
    });
    let mut publisher = Publisher::default();
    let mut mirror = mirror_of(&mut g, &mut publisher);
    assert_eq!(mirror.upgrade("lomo_reforzado"), Ok(()));
    let actions = mirror.take_actions();
    assert_eq!(actions.len(), 1);
    for action in actions {
        let bytes = postcard::to_stdvec(&action).expect("encode");
        g.apply_action(postcard::from_bytes(&bytes).expect("decode"));
    }
    let theirs = g.view(GUEST).expect("guest").1;
    assert!(theirs.upgrades.contains_key("cover"));
    assert!(g.profile.player.upgrades.is_empty(), "not the host's");
    // An oversized text is ignored outright.
    g.apply_action(Action::Upgrade("x".repeat(10_000)));
    assert_eq!(g.view(GUEST).expect("guest").1.upgrades.len(), 1);
}
