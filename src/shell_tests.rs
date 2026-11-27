//! Headless integration tests: the whole shell driven without a window, using
//! only keyboard/pad style input.

use crate::audio::Mixer;
use crate::input::Snapshot;
use crate::render::atlas::Atlas;
use crate::render::pipeline::Frame;
use crate::shell::{AppState, Shell};
use crate::ui::state::Screen;
use crate::ui::{Rect, adjuster_zones, next_focus};
use babel_core::data::{GameData, Resource};
use babel_core::game::Game;
use babel_core::game::ai::PlayerHit;
use babel_core::game::coop::{GUEST, HOST};
use babel_core::game::profile::Profile;
use babel_core::game::vec::{V2, v2};
use babel_core::world::grid::TILE;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Rig {
    shell: Shell,
    frame: Frame,
    mixer: Mixer,
}

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hexbabel-shell-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

impl Rig {
    fn new(dir: PathBuf) -> Rig {
        let data = Arc::new(GameData::load(&assets()).expect("data"));
        let atlas = Atlas::load(&assets()).expect("atlas");
        let mut rig = Rig {
            shell: Shell::new(data, atlas, dir),
            frame: Frame::default(),
            mixer: Mixer::default(),
        };
        rig.idle(2);
        rig
    }

    fn step(&mut self, snapshot: &Snapshot) {
        self.shell
            .frame(1.0 / 60.0, snapshot, &mut self.frame, &mut self.mixer);
    }

    fn idle(&mut self, frames: u32) {
        for _ in 0..frames {
            self.step(&Snapshot::default());
        }
    }

    /// Focuses widget `index` of the top screen and presses confirm.
    fn press(&mut self, index: usize) {
        self.shell.set_focus(index);
        let mut s = Snapshot::default();
        s.ui.confirm = true;
        self.step(&s);
        self.idle(2);
    }

    fn click(&mut self, x: i32, y: i32) {
        let mut s = Snapshot::default();
        s.ui.mouse = Some(v2(x as f32, y as f32));
        s.ui.mouse_moved = true;
        s.ui.click = true;
        self.step(&s);
        self.idle(1);
    }

    fn back(&mut self) {
        let mut s = Snapshot::default();
        s.ui.back = true;
        s.hot.pause = true;
        self.step(&s);
        self.idle(2);
    }

    /// Title → character creator → seed.
    fn open_seed_screen(&mut self) {
        assert_eq!(self.shell.top(), Some(Screen::Title));
        self.press(0);
        assert_eq!(self.shell.top(), Some(Screen::Look));
        self.press(LOOK_CONTINUE);
        assert_eq!(self.shell.top(), Some(Screen::NewGame));
    }

    fn start_new_game(&mut self) {
        self.open_seed_screen();
        // "Comenzar aventura" while the keyboard is folded.
        self.press(0);
        assert_eq!(self.shell.mode(), AppState::InGame);
    }

    fn assert_navigable(&mut self, name: &str) {
        let widgets = self.shell.widgets();
        assert!(!widgets.is_empty(), "{name} has nothing to focus");
        let lost = unreachable(&widgets, self.shell.focus_index());
        assert!(
            lost.is_empty(),
            "{name}: widgets {lost:?} of {} cannot be reached",
            widgets.len()
        );
        for (i, a) in widgets.iter().enumerate() {
            for b in &widgets[i + 1..] {
                let apart =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{name}: {a:?} overlaps {b:?}");
            }
        }
        for w in &widgets {
            assert!(
                w.x >= 0 && w.y >= 0 && w.x + w.w <= 480 && w.y + w.h <= 270,
                "{name}: {w:?} is off screen"
            );
        }
    }
}

/// Widget index of "Continuar" on the hero's tab: two preview buttons, two
/// tabs, the keyboard button and eight choices come first.
const LOOK_CONTINUE: usize = 13;
const LOOK_PET_TAB: usize = 3;
const LOOK_FIRST_ROW: usize = 5;
/// The same button on the pet's tab, which has five choices.
const PET_CONTINUE: usize = 10;

/// Every widget can be reached with directional presses alone.
fn unreachable(widgets: &[Rect], start: usize) -> Vec<usize> {
    let mut seen = BTreeSet::from([start.min(widgets.len().saturating_sub(1))]);
    let mut stack: Vec<usize> = seen.iter().copied().collect();
    while let Some(at) = stack.pop() {
        for dir in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = next_focus(widgets, at, dir);
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    (0..widgets.len()).filter(|i| !seen.contains(i)).collect()
}

#[test]
fn a_whole_session_works_with_keys_only() {
    let dir = temp_dir("session");
    let mut rig = Rig::new(dir.clone());
    assert_eq!(rig.shell.mode(), AppState::MainMenu);
    rig.start_new_game();
    rig.shell.flush_saves();
    assert!(rig.shell.save_path.exists(), "new games save at once");
    let seed = rig.shell.game.as_ref().expect("game").profile.world_seed;

    let start = rig.shell.game.as_ref().expect("game").body.pos;
    let mut walk = Snapshot::default();
    walk.game.movement = v2(1.0, 0.0);
    for _ in 0..30 {
        rig.step(&walk);
    }
    assert!(rig.shell.game.as_ref().expect("game").body.pos.x > start.x + 10.0);

    let mut inventory = Snapshot::default();
    inventory.hot.inventory = true;
    rig.step(&inventory);
    assert_eq!(rig.shell.top(), Some(Screen::Inventory));
    assert_eq!(rig.shell.mode(), AppState::Paused);
    let frozen = rig.shell.game.as_ref().expect("game").body.pos;
    for _ in 0..10 {
        rig.step(&walk);
    }
    assert_eq!(rig.shell.game.as_ref().expect("game").body.pos, frozen);
    rig.back();
    assert_eq!(rig.shell.mode(), AppState::InGame);

    let mut pause = Snapshot::default();
    pause.hot.pause = true;
    rig.step(&pause);
    rig.idle(2);
    assert_eq!(rig.shell.top(), Some(Screen::Pause));
    let last = rig.shell.widgets().len() - 1;
    rig.press(last);
    assert_eq!(rig.shell.mode(), AppState::MainMenu);

    // Continue is now the first button.
    rig.press(0);
    assert_eq!(rig.shell.mode(), AppState::InGame);
    assert_eq!(
        rig.shell.game.as_ref().expect("game").profile.world_seed,
        seed
    );

    if let Some(game) = rig.shell.game.as_mut() {
        game.body.invulnerable = 0.0;
        game.hurt_player(PlayerHit::plain(1.0e9, V2::ZERO));
    }
    rig.idle(150);
    assert_eq!(rig.shell.top(), Some(Screen::GameOver));
    assert_eq!(rig.shell.mode(), AppState::GameOver);
    rig.press(0);
    assert_eq!(rig.shell.mode(), AppState::InGame);
    assert!(rig.shell.game.as_ref().expect("game").profile.player.hp > 1.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn typing_a_seed_and_pressing_enter_uses_exactly_that_seed() {
    let dir = temp_dir("seed");
    let mut rig = Rig::new(dir.clone());
    rig.open_seed_screen();
    // Even with the cursor left on another button.
    rig.shell.set_focus(2);
    for c in ['4', '2'] {
        let mut s = Snapshot::default();
        s.ui.typed = vec![c];
        rig.step(&s);
    }
    let mut enter = Snapshot::default();
    enter.ui.confirm = true;
    rig.step(&enter);
    assert_eq!(
        rig.shell.game.as_ref().expect("started").profile.world_seed,
        42
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_screen_is_fully_navigable_without_a_mouse() {
    let dir = temp_dir("navigation");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let data = rig.shell.data.clone();
    let profile = crate::debug::showcase_profile(&data, 7, 12);
    rig.shell.ephemeral = true;
    rig.shell
        .visit(profile, babel_core::world::site::SiteKey::walk(1, 0, 0));
    if let Some(game) = rig.shell.game.as_mut() {
        game.profile.player.relics.push("sello_catalogo".into());
        // Enough items to make the Scriptorium list scroll.
        for i in 0..20 {
            let item = babel_core::progression::items::from_words(
                &data,
                2000 + i,
                "daga",
                None,
                babel_core::babel::rarity::Rarity::Rare,
            );
            game.profile.player.receive_item(item);
        }
    }
    for screen in [
        Screen::Coop,
        Screen::Options,
        Screen::Controls,
        Screen::Pause,
        Screen::Look,
        Screen::Inventory,
        Screen::Skills,
        Screen::Glossary,
        Screen::Map,
        Screen::Catalog,
        Screen::Reader,
        Screen::Crafting,
        Screen::Build(0),
        Screen::Chest,
        Screen::Lamps,
        Screen::Scriptorium,
        Screen::Stats,
        Screen::NewGame,
        Screen::Credits,
    ] {
        // Tabs and folding keyboards change what is on screen.
        let variants: usize = match screen {
            Screen::Inventory | Screen::Controls => 3,
            Screen::Catalog | Screen::NewGame => 2,
            Screen::Crafting => 2,
            _ => 1,
        };
        for variant in 0..variants {
            rig.shell.screens.clear();
            rig.shell.push(screen);
            rig.shell.ui_data.bag_tab = variant;
            rig.shell.ui_data.controls_tab = variant;
            rig.shell.ui_data.craft_tab = variant.min(1);
            rig.shell.ui_data.show_keys = variant == 1;
            rig.idle(if screen == Screen::Credits { 100 } else { 3 });
            rig.assert_navigable(&format!("{screen:?} #{variant}"));
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clicking_the_arrows_and_the_bar_sets_the_volume() {
    let dir = temp_dir("volume");
    let mut rig = Rig::new(dir.clone());
    rig.shell.screens.clear();
    rig.shell.push(Screen::Options);
    rig.idle(20);
    let row = rig.shell.widgets()[0];
    let zones = adjuster_zones(row);
    let middle_y = row.y + row.h / 2;
    let before = rig.shell.settings.master;
    rig.click(zones.less.x + 2, middle_y);
    assert_eq!(rig.shell.settings.master, before - 1, "left arrow lowers");
    rig.click(zones.less.x + zones.less.w - 2, row.y + 2);
    assert_eq!(
        rig.shell.settings.master,
        before - 2,
        "its whole box lowers"
    );
    rig.click(zones.more.x + 2, middle_y);
    assert_eq!(rig.shell.settings.master, before - 1, "right arrow raises");
    rig.click(zones.middle.x + 3, middle_y);
    assert_eq!(rig.shell.settings.master, 1, "first segment of the bar");
    rig.click(zones.middle.x + zones.middle.w - 2, middle_y);
    assert_eq!(rig.shell.settings.master, 10, "last segment of the bar");
    // The label is not a control.
    rig.click(row.x + 10, middle_y);
    assert_eq!(rig.shell.settings.master, 10);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn arrow_keys_adjust_a_slider_without_leaving_it() {
    let dir = temp_dir("slider-keys");
    let mut rig = Rig::new(dir.clone());
    rig.shell.screens.clear();
    rig.shell.push(Screen::Options);
    rig.idle(3);
    rig.shell.set_focus(1);
    let before = rig.shell.settings.music;
    let mut left = Snapshot::default();
    left.ui.nav = (-1, 0);
    rig.step(&left);
    rig.idle(1);
    assert_eq!(rig.shell.settings.music, before - 1);
    assert_eq!(rig.shell.focus_index(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_designed_character_is_the_one_that_plays_and_is_saved() {
    let dir = temp_dir("character");
    let mut rig = Rig::new(dir.clone());
    rig.press(0);
    assert_eq!(rig.shell.top(), Some(Screen::Look));
    // Body first, then skin and hair.
    let step_right = |rig: &mut Rig, row: usize| {
        rig.shell.set_focus(LOOK_FIRST_ROW + row);
        let mut s = Snapshot::default();
        s.ui.nav = (1, 0);
        rig.step(&s);
        rig.idle(1);
    };
    step_right(&mut rig, 0);
    step_right(&mut rig, 2);
    step_right(&mut rig, 2);
    let designed = rig.shell.ui_data.look;
    assert_eq!((designed.body, designed.hair), (1, 2));
    rig.press(LOOK_CONTINUE);
    rig.press(0);
    assert_eq!(rig.shell.mode(), AppState::InGame);
    let playing = rig.shell.game.as_ref().expect("game").profile.player.look;
    assert_eq!(playing, designed);
    rig.shell.flush_saves();
    match babel_core::save::load(&rig.shell.save_path) {
        babel_core::save::LoadOutcome::Loaded(profile) => {
            assert_eq!(profile.player.look, designed);
        }
        other => panic!("{other:?}"),
    }

    // It can be changed later from the pause menu, and is saved again.
    rig.shell.push(Screen::Pause);
    rig.shell.push(Screen::Look);
    rig.idle(2);
    step_right(&mut rig, 5);
    rig.back();
    assert_eq!(rig.shell.top(), Some(Screen::Pause));
    rig.shell.flush_saves();
    match babel_core::save::load(&rig.shell.save_path) {
        babel_core::save::LoadOutcome::Loaded(profile) => {
            assert_eq!(profile.player.look.coat, designed.coat + 1);
        }
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn changing_equipment_changes_the_hero_on_screen() {
    use babel_core::babel::rarity::Rarity;
    use babel_core::data::{Slot, ToolKind};
    use babel_core::progression::items::from_words;

    let dir = temp_dir("gear");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.idle(2);
    let data = rig.shell.data.clone();
    let mut drawn = vec![rig.shell.hero_pixels().to_vec()];
    let mut expect_new = |rig: &mut Rig, what: &str| {
        rig.idle(2);
        let now = rig.shell.hero_pixels().to_vec();
        assert_ne!(Some(&now), drawn.last(), "{what} did not change the hero");
        drawn.push(now);
    };
    for (uid, word) in ["hacha", "yelmo", "greba", "pebetero", "grial"]
        .into_iter()
        .enumerate()
    {
        let item = from_words(&data, 900 + uid as u64, word, None, Rarity::Rare);
        let game = rig.shell.game.as_mut().expect("game");
        game.profile.player.receive_item(item);
        expect_new(&mut rig, word);
    }
    let game = rig.shell.game.as_mut().expect("game");
    assert!(game.unequip(Slot::Armor));
    expect_new(&mut rig, "taking the helmet off");
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.tools[ToolKind::Knife.index()] = 4;
    expect_new(&mut rig, "a better knife");
    let mut cycle = Snapshot::default();
    cycle.game.cycle_tool = 1;
    rig.step(&cycle);
    expect_new(&mut rig, "switching tools");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_skill_has_its_own_icon() {
    let data = GameData::load(&assets()).expect("data");
    let atlas = Atlas::load(&assets()).expect("atlas");
    let mut seen = BTreeSet::new();
    let hero = data.skills.iter().map(|s| &s.id);
    for id in hero.chain(data.pet_skills.iter().map(|s| &s.id)) {
        let name = crate::ui::skills::skill_icon(id);
        assert!(atlas.has(&name), "{name} is missing");
        let f = atlas.get(&name, 0);
        let pixels: Vec<_> = (0..f.h)
            .flat_map(|y| (0..f.w).map(move |x| (x, y)))
            .map(|(x, y)| atlas.pixel(f, x, y))
            .collect();
        assert!(seen.insert(pixels), "{name} repeats another icon");
    }
}

#[test]
fn a_corrupt_save_shows_a_notice_instead_of_crashing() {
    let dir = temp_dir("corrupt");
    std::fs::write(dir.join("save.ron"), "((( not a save").expect("write");
    std::fs::write(dir.join("options.ron"), "garbage").expect("write");
    let mut rig = Rig::new(dir.clone());
    assert!(!rig.shell.has_save);
    assert_eq!(rig.shell.mode(), AppState::MainMenu);
    rig.start_new_game();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_screen_survives_a_fresh_profile() {
    let dir = temp_dir("fresh");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    for screen in [
        Screen::Look,
        Screen::Inventory,
        Screen::Skills,
        Screen::Glossary,
        Screen::Map,
        Screen::Reader,
        Screen::Crafting,
        Screen::Chest,
        Screen::Lamps,
        Screen::Scriptorium,
        Screen::Ending,
    ] {
        rig.shell.screens.clear();
        rig.shell.push(screen);
        // Hammer every input at once.
        for i in 0..40 {
            let mut s = Snapshot::default();
            s.ui.nav = [(1, 0), (0, 1), (-1, 0), (0, -1)][i % 4];
            s.ui.tab = i32::from(i % 13 == 0);
            s.ui.alt = i % 7 == 0;
            s.ui.scroll = i32::from(i % 5 == 0);
            rig.step(&s);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Two whole games on this computer, talking over a real connection.
struct Duo {
    host: Rig,
    guest: Rig,
    dirs: [PathBuf; 2],
}

impl Drop for Duo {
    fn drop(&mut self) {
        self.host.shell.flush_saves();
        self.guest.shell.flush_saves();
        for dir in &self.dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn loaded(path: &Path) -> Profile {
    match babel_core::save::load(path) {
        babel_core::save::LoadOutcome::Loaded(p) => *p,
        other => panic!("{other:?}"),
    }
}

impl Duo {
    fn alone(name: &str) -> Duo {
        let dirs = [
            temp_dir(&format!("{name}-host")),
            temp_dir(&format!("{name}-guest")),
        ];
        let host = Rig::new(dirs[0].clone());
        let guest = Rig::new(dirs[1].clone());
        Duo { host, guest, dirs }
    }

    /// Runs both games side by side.
    fn run(&mut self, host: &Snapshot, guest: &Snapshot, frames: u32) {
        for _ in 0..frames {
            self.host.step(host);
            self.guest.step(guest);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    fn idle(&mut self, frames: u32) {
        self.run(&Snapshot::default(), &Snapshot::default(), frames);
    }

    /// Runs until something becomes true; the network takes its time.
    fn until(&mut self, what: &str, mut done: impl FnMut(&Duo) -> bool) {
        let started = std::time::Instant::now();
        while !done(self) {
            assert!(started.elapsed().as_secs() < 20, "waiting for {what}");
            self.idle(1);
        }
    }

    fn together(name: &str) -> Duo {
        let mut duo = Duo::alone(name);
        duo.host.start_new_game();
        let code = duo.host.shell.open_local_room().expect("room");
        duo.guest.shell.join_invitation(&code);
        duo.until("the guest", |d| {
            d.guest.shell.mode() == AppState::InGame && d.host.shell.mode() == AppState::InGame
        });
        duo.idle(10);
        duo
    }

    fn host_game(&self) -> &Game {
        self.host.shell.game.as_ref().expect("host game")
    }

    fn guest_game(&self) -> &Game {
        self.guest.shell.game.as_ref().expect("guest game")
    }
}

fn paused() -> Snapshot {
    let mut s = Snapshot::default();
    s.hot.pause = true;
    s
}

#[test]
fn a_friend_joins_with_the_code_and_each_moves_their_own_character() {
    let mut duo = Duo::together("join");
    assert_eq!(duo.host_game().players(), 2);
    assert!(duo.guest_game().is_mirror());
    assert_eq!(duo.guest_game().acting(), GUEST);
    assert_eq!(duo.guest_game().site.key, duo.host_game().site.key);

    let guest_before = duo.guest_game().body.pos;
    let host_before = duo.host_game().body.pos;
    let mut right = Snapshot::default();
    right.game.movement = v2(1.0, 0.0);
    duo.run(&Snapshot::default(), &right, 40);
    duo.idle(20);
    let on_host = duo.host_game().view(GUEST).expect("guest").0.pos;
    assert!(
        on_host.x > guest_before.x + 10.0,
        "the host moved the guest"
    );
    assert!(duo.guest_game().body.pos.dist(on_host) < 3.0, "and said so");
    assert!(duo.host_game().body.pos.dist(host_before) < 2.0);
}

#[test]
fn the_world_stops_only_when_both_pause() {
    let mut duo = Duo::together("pause");
    let clock = |d: &Duo| d.host_game().profile.world.play_seconds;
    duo.run(&paused(), &Snapshot::default(), 1);
    duo.idle(10);
    assert_eq!(duo.host.shell.top(), Some(Screen::Pause));
    let before = clock(&duo);
    duo.idle(30);
    assert!(
        clock(&duo) > before + 0.2,
        "one pause does not stop the world"
    );

    duo.run(&Snapshot::default(), &paused(), 1);
    duo.until("both pauses", |d| {
        d.host.shell.coop_status().is_some_and(|c| c.stopped())
            && d.guest.shell.coop_status().is_some_and(|c| c.stopped())
    });
    let stopped = clock(&duo);
    duo.idle(30);
    assert_eq!(clock(&duo), stopped, "both paused: the world waits");
    duo.host.assert_navigable("pause in company");

    // Either of them going on starts it again.
    duo.run(&Snapshot::default(), &paused(), 1);
    duo.until("the clock", |d| clock(d) > stopped + 0.1);
    assert_eq!(duo.guest.shell.mode(), AppState::InGame);
}

#[test]
fn the_guest_progress_is_saved_on_their_disk_and_their_world_is_untouched() {
    let mut duo = Duo::alone("progress");
    // The guest has a game of their own first.
    duo.guest.start_new_game();
    duo.guest.shell.save();
    duo.guest.shell.flush_saves();
    let own = loaded(&duo.guest.shell.save_path);
    duo.host.start_new_game();
    let code = duo.host.shell.open_local_room().expect("room");
    duo.guest.shell.join_invitation(&code);
    duo.until("the visit", |d| {
        d.guest.shell.game.as_ref().is_some_and(Game::is_mirror)
    });
    let host_seed = duo.host_game().profile.world_seed;

    if let Some(game) = duo.host.shell.game.as_mut() {
        game.as_player(GUEST, |g| {
            g.profile.player.add_resource(Resource::Paper, 33);
            g.grant_own_xp(40);
        });
    }
    duo.until("the news", |d| {
        d.guest_game().profile.player.resource(Resource::Paper) == 33
    });
    // Saving during the visit must not write the host's world.
    duo.guest.shell.save();
    duo.guest.shell.leave_room();
    duo.guest.shell.flush_saves();
    assert!(duo.guest.shell.game.is_none());
    assert_eq!(duo.guest.shell.top(), Some(Screen::Title));
    let kept = loaded(&duo.guest.shell.save_path);
    assert_eq!(kept.world_seed, own.world_seed);
    assert_ne!(kept.world_seed, host_seed);
    assert_eq!(kept.world, own.world, "their own world is as they left it");
    assert_eq!(kept.player.resource(Resource::Paper), 33);
    assert!(kept.player.xp > own.player.xp || kept.player.level > own.player.level);

    duo.until("the host alone", |d| d.host_game().players() == 1);
    assert_eq!(duo.host.shell.mode(), AppState::InGame);
}

#[test]
fn the_host_leaving_sends_the_guest_home_with_their_progress() {
    let mut duo = Duo::together("host-leaves");
    duo.host.shell.leave_room();
    duo.until("the title", |d| d.guest.shell.game.is_none());
    assert_eq!(duo.guest.shell.top(), Some(Screen::Title));
    duo.guest.shell.flush_saves();
    assert!(duo.guest.shell.save_path.exists());
    assert_eq!(duo.host_game().players(), 1);
}

#[test]
fn both_walk_into_the_next_gallery_together() {
    let mut duo = Duo::together("door");
    let home = duo.host_game().site.key.clone();
    let (x, y) = babel_core::world::grid::door_mid(babel_core::world::hex::Dir::ALL[0]);
    let door = v2(x * TILE, y * TILE);
    let inward = (v2(320.0, 272.0) - door).norm();
    let place = |duo: &mut Duo, slot: u8| {
        if let Some(game) = duo.host.shell.game.as_mut() {
            game.as_player(slot, |g| {
                g.body.pos = door + inward * 6.0;
                g.body.door_lock = false;
            });
        }
    };
    place(&mut duo, HOST);
    duo.idle(30);
    assert_eq!(duo.host_game().site.key, home, "one is not enough");
    place(&mut duo, GUEST);
    duo.until("the door", |d| {
        d.host_game().site.key != home && d.guest_game().site.key == d.host_game().site.key
    });
}

#[test]
fn a_wrong_code_explains_itself_and_changes_nothing() {
    let mut duo = Duo::alone("wrong-code");
    duo.guest.shell.join_invitation("nada que ver");
    duo.idle(2);
    assert!(!duo.guest.shell.ui_data.coop_error.is_empty());
    assert!(duo.guest.shell.game.is_none());
    duo.guest.assert_navigable("multiplayer menu");

    duo.host.start_new_game();
    let code = duo.host.shell.open_local_room().expect("room");
    assert_eq!(duo.host.shell.top(), Some(Screen::Coop));
    duo.idle(2);
    duo.host.assert_navigable("open room");
    // A code for the same room with another secret.
    let mut forged = crate::net::code::RoomCode::parse(&code).expect("code");
    forged.secret[2] ^= 0x55;
    duo.guest.shell.screens.pop();
    duo.guest.shell.join_invitation(&forged.text());
    duo.until("the refusal", |d| {
        !d.guest.shell.ui_data.coop_error.is_empty()
    });
    assert!(duo.guest.shell.game.is_none());
    assert_eq!(duo.host_game().players(), 1);
    duo.guest.picture("coop-menu-error");
    // The longest message has to fit too.
    duo.guest.shell.ui_data.coop_error = crate::net::Failure::DirectBlocked.message();
    duo.guest.picture("coop-menu-blocked");
}

#[test]
fn the_waiting_room_tells_each_situation_apart() {
    use crate::net::code::RoomCode;
    use crate::net::reach::Reach;
    let mut rig = Rig::new(temp_dir("reach"));
    rig.start_new_game();
    rig.shell.open_local_room().expect("room");
    let seen = |room: RoomCode| RoomCode {
        address: std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::new(93, 184, 216, 34),
            room.address.port(),
        ),
        udp: 50_000,
        secret: room.secret,
    };
    type Outcome = Box<dyn Fn(RoomCode) -> Reach>;
    let cases: [(&str, Outcome); 4] = [
        ("searching", Box::new(|_| Reach::Searching)),
        ("open", Box::new(move |r| Reach::Open(seen(r)))),
        ("direct", Box::new(move |r| Reach::Direct(seen(r)))),
        ("offline", Box::new(|_| Reach::Offline)),
    ];
    for (name, outcome) in cases {
        rig.shell.pretend_reach(outcome);
        rig.idle(3);
        rig.assert_navigable(name);
        // `HEXBABEL_PICTURES=folder cargo test` keeps what each looks like.
        if let Some(dir) = std::env::var_os("HEXBABEL_PICTURES") {
            let path = Path::new(&dir).join(format!("coop-{name}.png"));
            crate::debug::save_png(&rig.frame.ui, &path).expect("picture");
        }
    }
}

impl Rig {
    /// Types on a physical keyboard, one character per frame.
    fn write(&mut self, text: &str) {
        for c in text.chars() {
            let mut s = Snapshot::default();
            s.ui.letters = vec![c];
            self.step(&s);
        }
    }

    fn picture(&mut self, name: &str) {
        if let Some(dir) = std::env::var_os("HEXBABEL_PICTURES") {
            self.idle(30);
            let path = Path::new(&dir).join(format!("{name}.png"));
            crate::debug::save_png(&self.frame.ui, &path).expect("save");
        }
    }
}

#[test]
fn names_typed_on_the_creator_reach_the_new_game() {
    let dir = temp_dir("names");
    let mut rig = Rig::new(dir.clone());
    rig.press(0);
    assert_eq!(rig.shell.top(), Some(Screen::Look));
    rig.assert_navigable("creator");
    // Doubled and leading spaces, and what a name may not have, never land.
    rig.write("  José  Ñ!<>andú_xxxxxxxxxxxx");
    assert_eq!(rig.shell.ui_data.hero_name, "José Ñandúxx");
    for _ in 0..2 {
        let mut s = Snapshot::default();
        s.ui.backspace = true;
        rig.step(&s);
    }
    rig.picture("creator-hero");
    rig.press(LOOK_PET_TAB);
    rig.assert_navigable("creator, pet tab");
    rig.write("Canela ");
    rig.picture("creator-pet");
    rig.press(PET_CONTINUE);
    assert_eq!(rig.shell.top(), Some(Screen::NewGame));
    rig.press(0);
    let player = &rig.shell.game.as_ref().expect("game").profile.player;
    assert_eq!(player.name, "José Ñandú");
    assert_eq!(player.pet.name, "Canela");
    rig.picture("play-with-names");
    // And they are in the save.
    rig.shell.flush_saves();
    let text = std::fs::read_to_string(&rig.shell.save_path).expect("save");
    assert!(text.contains("Canela") && text.contains("José Ñandú"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_on_screen_keyboard_writes_names_with_a_capital() {
    let dir = temp_dir("keys");
    let mut rig = Rig::new(dir.clone());
    rig.press(0);
    // "Teclado" sits after the preview buttons and the tabs.
    rig.press(4);
    rig.assert_navigable("name keyboard");
    rig.press(0);
    rig.press(1);
    assert_eq!(rig.shell.ui_data.hero_name, "Ab");
    rig.picture("creator-keys");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_pet_tree_is_learned_with_the_heros_points() {
    let dir = temp_dir("pet-tree");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.skill_points = 3;
    game.profile.player.pet.name = "Canela".into();
    let mut open = Snapshot::default();
    open.hot.skills = true;
    rig.step(&open);
    rig.idle(30);
    assert_eq!(rig.shell.top(), Some(Screen::Skills));
    rig.assert_navigable("hero tree");
    rig.picture("tree-hero");
    // The second of the two buttons shows the pet's tree.
    rig.click(290, 24);
    assert!(rig.shell.ui_data.skill_pet);
    rig.idle(10);
    rig.assert_navigable("pet tree");
    // The first node of the first column.
    rig.click(81, 66);
    let player = &rig.shell.game.as_ref().expect("game").profile.player;
    assert!(
        player.pet.skills.contains("m_mordisco"),
        "{:?}",
        player.pet.skills
    );
    assert_eq!(player.skill_points, 2);
    assert!(player.skills.is_empty());
    rig.click(81, 98);
    rig.picture("tree-pet");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn names_can_be_hidden() {
    let dir = temp_dir("hidden");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let drawn = |rig: &mut Rig, hero: &str, pet: &str, show: bool| {
        rig.shell.settings.show_names = show;
        let game = rig.shell.game.as_mut().expect("game");
        game.profile.player.name = hero.into();
        game.profile.player.pet.name = pet.into();
        // Far apart, so that neither label gives way to the other.
        game.body.pet.pos = game.body.pos + v2(-90.0, 0.0);
        game.enemies.clear();
        rig.shell.frozen = true;
        rig.step(&Snapshot::default());
        // The tutorial card and the place name move on their own at the
        // right and bottom edges, where no label of this scene is.
        let row = rig.frame.ui.w as usize * 4;
        let rows = rig.frame.ui.px[..row * 200].chunks(row);
        let left: Vec<u8> = rows.flat_map(|r| &r[..420 * 4]).copied().collect();
        left
    };
    let bare = drawn(&mut rig, "", "", true);
    let named = drawn(&mut rig, "Vera", "Canela", true);
    let hidden = drawn(&mut rig, "Vera", "Canela", false);
    assert_ne!(bare, named);
    assert_eq!(bare, hidden);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn whetstones_are_made_from_the_crafting_screen() {
    let atlas = Atlas::load(&assets()).expect("atlas");
    assert!(atlas.has("ui_whetstone"), "the whetstone has no icon");
    let dir = temp_dir("whetstone");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    for resource in Resource::ALL {
        game.profile.player.add_resource(resource, 100);
    }
    game.craft("build_binder", Some(0)).expect("bench");
    // The stones are the forge's work.
    game.craft("build_forge", Some(1)).expect("forge");
    let before = game.derived.damage;
    rig.shell.push(Screen::Crafting);
    rig.idle(2);
    // Two tabs, then the next knife, hammer, lamp and lens: the stone is last.
    const STONE: usize = 6;
    rig.shell.set_focus(STONE);
    rig.idle(2);
    rig.picture("crafting-whetstone");
    rig.press(STONE);
    let game = rig.shell.game.as_ref().expect("game");
    assert_eq!(game.profile.player.whetstones, 1);
    assert_eq!(game.derived.damage, before + 3.0);
    rig.assert_navigable("crafting with whetstones");
    let _ = std::fs::remove_dir_all(dir);
}

/// Widget index of "Conectar" in the waiting room: copy and paste come first.
const WAITING_CONNECT: usize = 2;

impl Rig {
    /// Types a reply code in the waiting room and confirms it with the button.
    fn type_reply(&mut self, text: &str) {
        if self.shell.top() != Some(Screen::Coop) {
            self.shell.push(Screen::Coop);
            self.idle(2);
        }
        self.shell.ui_data.reply.clear();
        for c in text.chars() {
            let mut s = Snapshot::default();
            s.ui.raw = vec![c];
            self.step(&s);
        }
        self.press(WAITING_CONNECT);
    }
}

/// A host whose front door is closed and a guest who has a reply to send.
fn knocking_pair(name: &str) -> (Duo, String) {
    let mut duo = Duo::alone(name);
    duo.host.start_new_game();
    let code = duo.host.shell.open_direct_room().expect("room");
    duo.guest.shell.join_direct(&code);
    duo.until("the reply code", |d| d.guest.shell.reply_code().is_some());
    duo.idle(3);
    (duo, code)
}

#[test]
fn two_homes_play_through_the_reply_code() {
    let (mut duo, _) = knocking_pair("reply");
    let reply = duo.guest.shell.reply_code().expect("reply");
    duo.guest.assert_navigable("guest with a reply code");
    duo.guest.picture("coop-reply");
    duo.host.assert_navigable("host waiting for a reply");
    duo.host.picture("coop-waiting-direct");
    assert_eq!(duo.host_game().players(), 1, "not before pasting it");
    duo.host.type_reply(&reply.to_lowercase());
    duo.until("both in the game", |d| {
        d.guest.shell.mode() == AppState::InGame && d.host.shell.mode() == AppState::InGame
    });
    duo.idle(10);
    assert_eq!(duo.host_game().players(), 2);
    assert!(duo.guest_game().is_mirror());
    let before = duo.guest_game().view(HOST).expect("host").0.pos;
    let mut right = Snapshot::default();
    right.game.movement = v2(1.0, 0.0);
    duo.run(&right, &Snapshot::default(), 40);
    duo.until("the host to be seen moving", |d| {
        d.guest_game().view(HOST).expect("host").0.pos.x > before.x + 10.0
    });
}

#[test]
fn wrong_things_in_the_reply_field_are_explained() {
    use crate::net::code::ReplyCode;
    use crate::net::secure::PunchKeys;
    let (mut duo, room) = knocking_pair("wrong-reply");
    let other = PunchKeys::derive(&[0; 6]);
    let stray = ReplyCode::new("127.0.0.1:9".parse().expect("address"), &other).text();
    for (typed, expected) in [
        ("", "Ese no es un código de respuesta."),
        ("HOLA", "Ese no es un código de respuesta."),
        (
            room.as_str(),
            "Ese es un código de sala: acá va el de respuesta.",
        ),
        (stray.as_str(), "Esa respuesta es de otra sala."),
    ] {
        duo.host.type_reply(typed);
        assert_eq!(duo.host.shell.ui_data.coop_error, expected, "{typed}");
        assert!(duo.host.shell.room_code().is_some(), "the room stays open");
    }
    duo.host.picture("coop-wrong-reply");
    // The right one still works afterwards.
    let reply = duo.guest.shell.reply_code().expect("reply");
    duo.host.type_reply(&reply);
    duo.until("the guest", |d| d.guest.shell.mode() == AppState::InGame);
}

#[test]
fn leaving_while_waiting_for_the_reply_cancels_cleanly() {
    let (mut duo, _) = knocking_pair("give-up");
    duo.guest.back();
    duo.idle(5);
    assert_eq!(duo.guest.shell.reply_code(), None);
    assert_eq!(duo.guest.shell.mode(), AppState::MainMenu);
    assert!(duo.host.shell.room_code().is_some());
    assert_eq!(duo.host_game().players(), 1);
}

#[test]
fn every_explanation_fits_its_panel() {
    use crate::render::font::Font;
    use crate::ui::craft::{BUILDING_TEXT, CRAFTING_TEXT, guide_height};
    use babel_core::data::RecipeOutput;
    use babel_core::progression::guide::explain;
    use babel_core::progression::player::PlayerData;
    let data = GameData::load(&assets()).expect("data");
    let font = Font::default();
    // The longest texts come from players who lack the most, or have the most.
    let mut veteran = PlayerData {
        whetstones: 5,
        tools: [4, 4, 4, 4],
        ..PlayerData::default()
    };
    for amount in veteran.resources.values_mut() {
        *amount = 9999;
    }
    let mut stocked = PlayerData::default();
    for amount in stocked.consumables.values_mut() {
        *amount = 9999;
    }
    for player in [PlayerData::default(), veteran, stocked] {
        for recipe in &data.recipes {
            if recipe.output == RecipeOutput::ReturnLamp {
                continue;
            }
            let guide = explain(&data, &player, recipe);
            let (width, room) = match recipe.output {
                RecipeOutput::Build(_) => BUILDING_TEXT,
                _ => CRAFTING_TEXT,
            };
            let height = guide_height(&font, &guide, width);
            assert!(height <= room, "{}: {height} of {room} pixels", recipe.id);
        }
    }
}

#[test]
fn the_explanations_are_on_screen_before_building_or_crafting() {
    let dir = temp_dir("explanations");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    for resource in Resource::ALL {
        game.profile.player.add_resource(resource, 100);
    }
    rig.shell.push(Screen::Build(0));
    for (focus, name) in [
        (0, "build-binder"),
        (3, "build-scriptorium"),
        (4, "build-chest"),
    ] {
        rig.shell.set_focus(focus);
        rig.idle(3);
        rig.picture(name);
    }
    rig.back();
    let game = rig.shell.game.as_mut().expect("game");
    game.craft("build_binder", Some(0)).expect("bench");
    rig.shell.push(Screen::Crafting);
    rig.idle(2);
    // The next lens, and in the other tab the torches, which need a skill.
    rig.shell.set_focus(5);
    rig.idle(3);
    rig.picture("craft-lens");
    rig.shell.ui_data.craft_tab = 1;
    rig.shell.set_focus(2);
    rig.idle(3);
    rig.picture("craft-torch");
    rig.assert_navigable("crafting materials");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn taking_out_the_hammer_lets_the_player_break_a_bookshelf() {
    use babel_core::data::{NodeKind, ToolKind};
    let dir = temp_dir("hammer");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    let shelf = game
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Shelf)
        .map(|n| (n.id, n.pos))
        .expect("home has bookshelves");
    game.body.pos = shelf.1 + v2(0.0, 18.0);
    game.nodes.retain(|n| n.id == shelf.0);
    rig.idle(40);
    rig.picture("hint-hammer");
    let mut swing = Snapshot::default();
    swing.game.attack_pressed = true;
    swing.game.aim_dir = Some(v2(0.0, -1.0));
    let health = |rig: &Rig| {
        let nodes = &rig.shell.game.as_ref().expect("game").nodes;
        nodes.iter().find(|n| n.id == shelf.0).map(|n| n.hp)
    };
    let whole = health(&rig);
    rig.step(&swing);
    rig.idle(40);
    assert_eq!(health(&rig), whole, "the knife does not break bookshelves");
    let mut next_tool = Snapshot::default();
    next_tool.game.cycle_tool = 1;
    rig.step(&next_tool);
    rig.idle(2);
    let held = rig
        .shell
        .game
        .as_ref()
        .expect("game")
        .profile
        .player
        .active_tool;
    assert_eq!(held, ToolKind::Hammer, "one press takes out the hammer");
    rig.step(&swing);
    rig.idle(40);
    assert!(health(&rig) < whole);
    let _ = std::fs::remove_dir_all(dir);
}

impl Rig {
    /// The secondary press on widget `index`: X, or the right mouse button.
    fn press_alt(&mut self, index: usize) {
        self.shell.set_focus(index);
        self.idle(1);
        let mut s = Snapshot::default();
        s.ui.alt = true;
        self.step(&s);
        self.idle(2);
    }

    fn pin_text(&self) -> Vec<String> {
        crate::ui::hud::pin_lines(self.shell.game.as_ref().expect("game"))
            .into_iter()
            .map(|line| line.0)
            .collect()
    }

    fn pins(&self) -> Vec<String> {
        self.shell
            .game
            .as_ref()
            .expect("game")
            .profile
            .player
            .pins
            .clone()
    }
}

#[test]
fn a_pinned_recipe_follows_the_player_while_they_gather() {
    let dir = temp_dir("pins");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.add_resource(Resource::Paper, 6);
    game.profile.player.add_resource(Resource::Stone, 4);
    game.craft("build_binder", Some(0)).expect("bench");
    assert!(rig.pin_text().is_empty(), "nothing pinned yet");

    rig.shell.push(Screen::Crafting);
    rig.idle(2);
    // Two tabs, then the knife.
    rig.press_alt(2);
    assert_eq!(rig.pins(), ["knife_1"]);
    rig.press_alt(2);
    assert!(rig.pins().is_empty(), "the same press lets it go");
    rig.press_alt(2);
    rig.picture("craft-pinned");
    assert_eq!(
        rig.shell.game.as_ref().expect("game").profile.player.tools,
        [0; 4]
    );
    rig.back();
    assert_eq!(
        rig.pin_text(),
        ["Cuchilla de Hierro", "Hierro: faltan 4", "Cuero: faltan 3"]
    );

    // Gathering is counted at once, and what is in the chest is mentioned.
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.add_resource(Resource::Iron, 3);
    game.profile.world.storage.insert(Resource::Leather, 2);
    rig.idle(2);
    assert_eq!(
        rig.pin_text(),
        [
            "Cuchilla de Hierro",
            "Hierro: falta 1",
            "Cuero: faltan 3 (2 en el cofre)"
        ]
    );
    rig.picture("hud-pins");
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.add_resource(Resource::Iron, 1);
    game.profile.player.add_resource(Resource::Leather, 3);
    rig.idle(2);
    assert_eq!(rig.pin_text(), ["Cuchilla de Hierro", "¡Ya tenés todo!"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn buildings_can_be_pinned_and_no_more_than_three_things() {
    let dir = temp_dir("pins-build");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.shell.push(Screen::Build(0));
    rig.idle(2);
    for row in [0, 1, 2] {
        rig.press_alt(row);
    }
    assert_eq!(rig.pins(), ["build_binder", "build_forge", "build_alembic"]);
    rig.press_alt(4);
    assert_eq!(rig.pins().len(), 3);
    assert!(
        rig.shell.ui_data.message.contains('3'),
        "says why: {}",
        rig.shell.ui_data.message
    );
    rig.picture("build-pinned");
    rig.back();
    let lines = rig.pin_text();
    assert!(lines.contains(&"Forja".to_string()), "{lines:?}");
    rig.picture("hud-three-pins");
    let _ = std::fs::remove_dir_all(dir);
}

/// Index of the third tab, and of the first improvement under it.
const IMPROVE_TAB: usize = 2;
const FIRST_IMPROVEMENT: usize = 3;

/// A new game at home with the three stations that improve things.
fn workshop(name: &str) -> (Rig, PathBuf) {
    use babel_core::data::Station;
    let dir = temp_dir(name);
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    for resource in Resource::ALL {
        game.profile.player.add_resource(resource, 100);
    }
    for (plot, station) in [Station::Binder, Station::Forge, Station::Alembic]
        .into_iter()
        .enumerate()
    {
        game.profile.world.stations.push((station, plot as u8));
    }
    game.profile.player.tools = [1; 4];
    game.refresh();
    (rig, dir)
}

impl Rig {
    /// Walks up to the station on a plot and presses the interact button.
    fn use_station(&mut self, plot: usize) {
        use babel_core::game::tile_center;
        use babel_core::world::site::HOME_PLOTS;
        let game = self.shell.game.as_mut().expect("game");
        game.body.pos = tile_center(HOME_PLOTS[plot]) + v2(0.0, 18.0);
        let mut s = Snapshot::default();
        s.game.interact = true;
        self.step(&s);
        self.idle(3);
    }

    fn upgrade_on(&self, thing: &str) -> Option<(String, u8)> {
        let player = &self.shell.game.as_ref().expect("game").profile.player;
        player.upgrades.get(thing).map(|f| (f.id.clone(), f.level))
    }
}

#[test]
fn using_a_station_offers_what_it_improves() {
    use babel_core::data::Station;
    let (mut rig, dir) = workshop("bench-tabs");
    for (plot, station, rows) in [
        (0, Station::Binder, 9),
        (1, Station::Forge, 12),
        (2, Station::Alembic, 6),
    ] {
        rig.use_station(plot);
        assert_eq!(rig.shell.top(), Some(Screen::Crafting), "{station:?}");
        assert_eq!(rig.shell.ui_data.bench, Some(station));
        rig.shell.ui_data.craft_tab = IMPROVE_TAB;
        rig.idle(3);
        // Three tabs, one row for each choice and the button that closes.
        assert_eq!(rig.shell.widgets().len(), 3 + rows + 1, "{station:?}");
        rig.assert_navigable(&format!("{station:?} improvements"));
        rig.shell.set_focus(FIRST_IMPROVEMENT);
        rig.idle(3);
        rig.picture(&format!("improve-{station:?}"));
        rig.back();
        assert_eq!(rig.shell.top(), None);
    }
    // Opened any other way there is no third tab to be left on.
    rig.shell.push(Screen::Crafting);
    rig.idle(3);
    assert_eq!(rig.shell.ui_data.bench, None);
    assert!(rig.shell.ui_data.craft_tab < IMPROVE_TAB);
    rig.assert_navigable("crafting without a station");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_improvement_is_chosen_raised_and_changed_from_its_station() {
    let (mut rig, dir) = workshop("bench-use");
    rig.use_station(1);
    rig.press(IMPROVE_TAB);
    assert_eq!(rig.shell.ui_data.craft_tab, IMPROVE_TAB);
    // The knife comes first, with its choices in the order of the data.
    rig.press(FIRST_IMPROVEMENT);
    assert_eq!(rig.upgrade_on("knife"), Some(("filo_igneo".into(), 1)));
    rig.shell.set_focus(FIRST_IMPROVEMENT);
    rig.idle(3);
    // The next level asks for a better knife, and the screen says so.
    rig.picture("improve-needs-tool");
    rig.press(FIRST_IMPROVEMENT);
    assert_eq!(rig.upgrade_on("knife"), Some(("filo_igneo".into(), 1)));
    assert!(
        rig.shell.ui_data.message.contains("Plata"),
        "{}",
        rig.shell.ui_data.message
    );
    rig.press(FIRST_IMPROVEMENT + 1);
    assert_eq!(rig.upgrade_on("knife"), Some(("filo_sediento".into(), 1)));
    rig.shell.set_focus(FIRST_IMPROVEMENT);
    rig.idle(3);
    rig.picture("improve-change");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_improvement_explanation_fits_its_panel() {
    use crate::render::font::Font;
    use crate::ui::craft::{CRAFTING_TEXT, guide_height};
    use babel_core::progression::player::PlayerData;
    use babel_core::progression::upgrades::{Fitted, explain};
    let data = GameData::load(&assets()).expect("data");
    let font = Font::default();
    let (width, room) = CRAFTING_TEXT;
    for upgrade in &data.upgrades.upgrades {
        // Nothing fitted, this one at each level, and each other one.
        let others = data
            .upgrades
            .upgrades
            .iter()
            .filter(|u| u.target == upgrade.target);
        let mut states: Vec<Option<Fitted>> = vec![None];
        for other in others {
            for level in 1..=3 {
                states.push(Some(Fitted {
                    id: other.id.clone(),
                    level,
                }));
            }
        }
        for fitted in states {
            for tools in [[0; 4], [5; 4]] {
                let mut player = PlayerData {
                    tools,
                    ..PlayerData::default()
                };
                if let Some(fitted) = fitted.clone() {
                    player.upgrades.insert(upgrade.target.clone(), fitted);
                }
                let guide = explain(&data, &player, &upgrade.id).expect(&upgrade.id);
                let height = guide_height(&font, &guide, width);
                assert!(
                    height <= room,
                    "{} with {fitted:?}: {height} of {room} pixels",
                    upgrade.id
                );
            }
        }
    }
}

/// Every screen, the crafting one at each station too, and the game itself.
fn every_view() -> Vec<(String, Option<Screen>, Option<babel_core::data::Station>)> {
    use babel_core::data::Station;
    let mut out = vec![("Juego".to_string(), None, None)];
    for screen in [
        Screen::Title,
        Screen::NewGame,
        Screen::Look,
        Screen::Coop,
        Screen::Options,
        Screen::Controls,
        Screen::Pause,
        Screen::Inventory,
        Screen::Skills,
        Screen::Glossary,
        Screen::Map,
        Screen::Catalog,
        Screen::Reader,
        Screen::Crafting,
        Screen::Build(0),
        Screen::Chest,
        Screen::Lamps,
        Screen::Scriptorium,
        Screen::Stats,
        Screen::GameOver,
        Screen::Ending,
        Screen::Credits,
    ] {
        out.push((format!("{screen:?}"), Some(screen), None));
    }
    for station in [Station::Binder, Station::Forge, Station::Alembic] {
        out.push((
            format!("{station:?}"),
            Some(Screen::Crafting),
            Some(station),
        ));
    }
    out
}

/// A well stocked game in which every screen has something to show.
fn showcase(rig: &mut Rig) {
    rig.start_new_game();
    let data = rig.shell.data.clone();
    let profile = crate::debug::showcase_profile(&data, 7, 12);
    rig.shell.ephemeral = true;
    rig.shell
        .visit(profile, babel_core::world::site::SiteKey::walk(1, 0, 0));
    if let Some(game) = rig.shell.game.as_mut() {
        game.profile.player.relics.push("sello_catalogo".into());
        for resource in Resource::ALL {
            game.profile.player.add_resource(resource, 40);
        }
        game.profile.player.tools = [1; 4];
        game.profile.player.pins = vec!["knife_2".into(), "hammer_2".into()];
        game.refresh();
    }
}

impl Rig {
    fn show(&mut self, screen: Option<Screen>, bench: Option<babel_core::data::Station>) {
        self.shell.screens.clear();
        if let Some(screen) = screen {
            self.shell.push(screen);
            self.shell.ui_data.bench = bench;
            if bench.is_some() {
                self.shell.ui_data.craft_tab = 2;
            } else {
                self.shell.ui_data.craft_tab = 0;
            }
        }
        if bench.is_some() {
            // On the first improvement, so that its details are shown.
            self.shell.set_focus(3);
        }
        // The Credits take a while to show anything.
        self.idle(if screen == Some(Screen::Credits) {
            100
        } else {
            30
        });
        self.shell.fx.toasts.clear();
        self.shell.fx.banners.clear();
    }
}

#[test]
#[ignore = "timing: run in release with --nocapture"]
fn interface_drawing_times() {
    let dir = temp_dir("timing");
    let mut rig = Rig::new(dir.clone());
    showcase(&mut rig);
    let mut worst = (0.0f64, String::new());
    for (name, screen, bench) in every_view() {
        // The first frame paints whatever art the screen keeps.
        rig.shell.screens.clear();
        if let Some(screen) = screen {
            rig.shell.push(screen);
            rig.shell.ui_data.bench = bench;
        }
        rig.idle(1);
        let first = rig.shell.ui_time.as_secs_f64() * 1000.0;
        rig.show(screen, bench);
        let mut times: Vec<f64> = (0..120)
            .map(|_| {
                rig.idle(1);
                rig.shell.ui_time.as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let median = times[times.len() / 2];
        println!("{name:<14} {median:.3} ms (primer cuadro {first:.2} ms)");
        if median > worst.0 {
            worst = (median, name);
        }
    }
    println!("peor: {} {:.3} ms", worst.1, worst.0);
    assert!(worst.0 < 4.0, "{} takes {:.2} ms", worst.1, worst.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_view_can_be_pictured() {
    let dir = temp_dir("gallery");
    let mut rig = Rig::new(dir.clone());
    showcase(&mut rig);
    for (name, screen, bench) in every_view() {
        rig.show(screen, bench);
        rig.picture(&format!("ver-{name}"));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn each_station_row_wears_its_own_colour() {
    let dir = temp_dir("build-colours");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.shell.push(Screen::Build(0));
    // On "Cancelar", so that no row is under the cursor.
    rig.shell.set_focus(5);
    rig.idle(30);
    // The left border of each of the five rows.
    let edges: BTreeSet<[u8; 4]> = (0..5)
        .map(|row| rig.frame.ui.get(28, 58 + row * 24))
        .collect();
    assert_eq!(edges.len(), 5, "{edges:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pause_buttons_wear_where_they_lead() {
    let dir = temp_dir("pause-colours");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.shell.push(Screen::Pause);
    rig.idle(30);
    // The top border of Inventario, Habilidades, Glosario and Mapa.
    let edges: BTreeSet<[u8; 4]> = (0..4)
        .map(|column| rig.frame.ui.get(52 + column * 76 + 36, 94))
        .collect();
    assert_eq!(edges.len(), 4, "{edges:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_credits_do_not_run_into_their_button() {
    let dir = temp_dir("credits-band");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.shell.push(Screen::Credits);
    // Long enough for the button to appear, and checked while lines pass.
    for wait in [100, 60, 60, 60, 120] {
        rig.idle(wait);
        // Beside the button, where long lines would cross.
        let base = rig.frame.ui.get(150, 244);
        for y in 238..256 {
            for x in 150..178 {
                assert_eq!(rig.frame.ui.get(x, y), base, "({x}, {y})");
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// How many pixels the interface covers while playing, with three pins.
fn hud_cover(name: &str) -> (usize, usize) {
    let dir = temp_dir(name);
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.pins = vec![
        "build_binder".into(),
        "build_forge".into(),
        "build_alembic".into(),
    ];
    rig.idle(90);
    rig.picture(name);
    let painted = |x: i32, y: i32| rig.frame.ui.get(x, y)[3] > 0;
    let all = (0..270)
        .flat_map(|y| (0..480).map(move |x| (x, y)))
        .filter(|&(x, y)| painted(x, y))
        .count();
    let middle = (60..200)
        .flat_map(|y| (140..340).map(move |x| (x, y)))
        .filter(|&(x, y)| painted(x, y))
        .count();
    let _ = std::fs::remove_dir_all(dir);
    (all, middle)
}

#[test]
fn the_hud_takes_no_more_room_than_before() {
    // Measured before it was framed: a new game with three things pinned.
    const BEFORE: usize = 25_048;
    let (all, middle) = hud_cover("hud-room");
    assert!(all <= BEFORE * 103 / 100, "{all} of {BEFORE}");
    assert_eq!(middle, 0, "nothing in the middle of the view");
}

#[test]
fn the_pieces_of_the_hud_are_framed() {
    use crate::ui::theme::Place;
    let dir = temp_dir("hud-frames");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    let game = rig.shell.game.as_mut().expect("game");
    game.profile.player.pins = vec!["build_binder".into()];
    rig.idle(90);
    let edge = Place::Hud.theme().edge;
    for (name, x, y) in [
        ("portrait", 17, 4),
        ("tool in hand", 16, 241),
        ("tonics", 36, 246),
        ("minimap", 443, 4),
        ("pinned list", 20, 73),
    ] {
        assert_eq!(rig.frame.ui.get(x, y), edge, "{name}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn statistics_open_from_the_pause_menu() {
    let dir = temp_dir("stats");
    let mut rig = Rig::new(dir.clone());
    rig.start_new_game();
    rig.shell.push(Screen::Pause);
    rig.idle(3);
    // After the button to go on, the tiles; "Estadísticas" comes before
    // "Salir", which stays last.
    let tiles = rig.shell.widgets().len() - 1;
    rig.press(tiles - 1);
    assert_eq!(rig.shell.top(), Some(Screen::Stats));
    rig.assert_navigable("statistics");
    rig.picture("statistics");
    rig.back();
    assert_eq!(rig.shell.top(), Some(Screen::Pause));
    let _ = std::fs::remove_dir_all(dir);
}
