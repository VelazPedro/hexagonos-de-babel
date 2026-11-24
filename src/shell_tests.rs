//! Headless integration tests: the whole shell driven without a window, using
//! only keyboard/pad style input.

use crate::audio::Mixer;
use crate::input::Snapshot;
use crate::render::atlas::Atlas;
use crate::render::pipeline::Frame;
use crate::shell::{AppState, Shell};
use crate::ui::state::Screen;
use crate::ui::{Rect, next_focus};
use babel_core::data::GameData;
use babel_core::game::ai::PlayerHit;
use babel_core::game::vec::{V2, v2};
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

    fn back(&mut self) {
        let mut s = Snapshot::default();
        s.ui.back = true;
        s.hot.pause = true;
        self.step(&s);
        self.idle(2);
    }

    fn start_new_game(&mut self) {
        assert_eq!(self.shell.top(), Some(Screen::Title));
        self.press(0);
        assert_eq!(self.shell.top(), Some(Screen::NewGame));
        // 40 keys, then erase and clear, then "Comenzar".
        self.press(42);
        assert_eq!(self.shell.mode(), AppState::InGame);
    }
}

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
    }
    for screen in [
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
        Screen::NewGame,
        Screen::Credits,
    ] {
        rig.shell.screens.clear();
        rig.shell.push(screen);
        rig.idle(if screen == Screen::Credits { 100 } else { 3 });
        let widgets = rig.shell.widgets();
        assert!(!widgets.is_empty(), "{screen:?} has nothing to focus");
        let lost = unreachable(&widgets, rig.shell.focus_index());
        assert!(
            lost.is_empty(),
            "{screen:?}: widgets {lost:?} of {} cannot be reached",
            widgets.len()
        );
        for w in &widgets {
            assert!(
                w.x >= 0 && w.y >= 0 && w.x + w.w <= 480 && w.y + w.h <= 270,
                "{screen:?}: {w:?} is off screen"
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
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
