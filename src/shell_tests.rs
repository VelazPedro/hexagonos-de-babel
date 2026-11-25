//! Headless integration tests: the whole shell driven without a window, using
//! only keyboard/pad style input.

use crate::audio::Mixer;
use crate::input::Snapshot;
use crate::render::atlas::Atlas;
use crate::render::pipeline::Frame;
use crate::shell::{AppState, Shell};
use crate::ui::state::Screen;
use crate::ui::{Rect, adjuster_zones, next_focus};
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

/// Widget index of "Continuar": two preview buttons and eight choices first.
const LOOK_CONTINUE: usize = 10;

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
    // Rows start after "Girar" and "Al azar": body first, then skin, hair.
    let step_right = |rig: &mut Rig, row: usize| {
        rig.shell.set_focus(2 + row);
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
    match babel_core::save::load(&rig.shell.save_path) {
        babel_core::save::LoadOutcome::Loaded(profile) => {
            assert_eq!(profile.player.look.coat, designed.coat + 1);
        }
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_skill_has_its_own_icon() {
    let data = GameData::load(&assets()).expect("data");
    let atlas = Atlas::load(&assets()).expect("atlas");
    let mut seen = BTreeSet::new();
    for skill in &data.skills {
        let name = crate::ui::skills::skill_icon(&skill.id);
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
