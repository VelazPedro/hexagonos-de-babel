//! The game shell: owns the session, routes input to the simulation or to the
//! open screen, and turns the state into pixels, light and sound.

use crate::audio::{Mixer, Track};
use crate::input::{Hotkeys, Snapshot};
use crate::render::atlas::Atlas;
use crate::render::canvas::Canvas;
use crate::render::font::Font;
use crate::render::fx::Fx;
use crate::render::gear::Gear;
use crate::render::hero;
use crate::render::pet::PetArt;
use crate::render::pipeline::{CompositeSettings, Frame};
use crate::render::scene::{self, Camera, SCENE_H, SCENE_W, VIEW_H, VIEW_W};
use crate::render::tiles::{self, SiteArt};
use crate::saver::Saver;
use crate::settings::Settings;
use crate::ui::coop as coop_ui;
use crate::ui::state::{Cmd, Screen, UiData};
use crate::ui::theme::{ArtCache, Place, place_for, sees_through};
use crate::ui::{Focus, Ui, UiInput, book, craft, hud, inventory, look, menus, skills, stats};
use babel_core::data::{GameData, Station};
use babel_core::game::Game;
use babel_core::game::events::{GameEvent, Input, ScreenRequest, Sfx};
use babel_core::game::profile::{Profile, parse_seed};
use babel_core::game::travel::Arrival;
use babel_core::game::vec::{V2, v2};
use babel_core::progression::look::Look;
use babel_core::progression::pet::{PetLook, clean_name};
use babel_core::save::{self, LoadOutcome};
use babel_core::world::site::SiteKey;
use bevy::math::Vec4;
use bevy::prelude::{Resource, States};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    InGame,
    Paused,
    GameOver,
    Credits,
}

/// The composed hero and the choices it was composed from.
struct HeroArt {
    look: Look,
    gear: Gear,
    atlas: Atlas,
}

impl HeroArt {
    fn new(source: &Atlas, look: Look, gear: Gear) -> HeroArt {
        HeroArt {
            look,
            gear,
            atlas: hero::build(source, look, &gear),
        }
    }
}

/// How long a screen takes to slide into place, in seconds.
const ENTRANCE: f32 = 0.14;

/// Numbers shown by the F3 overlay.
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugStats {
    pub fps: f32,
    pub frame_ms: f32,
    pub engine_entities: usize,
}

#[path = "shell_coop.rs"]
mod coop;

#[derive(Resource)]
pub struct Shell {
    pub data: Arc<GameData>,
    pub atlas: Atlas,
    pub font: Font,
    pub settings: Settings,
    settings_path: PathBuf,
    pub save_path: PathBuf,
    saver: Saver,
    pub game: Option<Game>,
    /// A quiet gallery rendered behind the title menu.
    backdrop: Game,
    art: Option<(String, SiteArt)>,
    /// The player's sprites and what they were composed from.
    hero: HeroArt,
    hud: hud::HudState,
    /// Where a screen is drawn while it slides in.
    layer: Canvas,
    /// Backgrounds and panels already painted.
    art_cache: ArtCache,
    /// How long the last interface took to draw.
    #[cfg(test)]
    pub ui_time: std::time::Duration,
    pub camera: Camera,
    pub fx: Fx,
    pub screens: Vec<Screen>,
    focus: Vec<Focus>,
    pub ui_data: UiData,
    sounds: Vec<Sfx>,
    time: f32,
    notice: Option<String>,
    pub has_save: bool,
    pub quit: bool,
    pub show_debug: bool,
    pub stats: DebugStats,
    autosave: bool,
    death_shown: bool,
    pad: bool,
    /// The files on disk could not be used and must survive a new game.
    unusable_save: bool,
    /// Tool modes never write to the player's save.
    pub ephemeral: bool,
    /// Freezes the simulation (screenshots).
    pub frozen: bool,
    /// Playing alone, or the state of co-op.
    session: coop::Session,
    partner_art: Option<HeroArt>,
    /// The pet shown, and the partner's.
    pet_art: PetArt,
    partner_pet: Option<PetArt>,
    /// While visiting: the guest's own saved game, which their progress goes
    /// back into.
    guest_home: Option<Box<Profile>>,
    /// Keeps rooms on one address (tests and developer flags).
    pub net_bind: Option<std::net::Ipv4Addr>,
    /// Developer and tests: try the direct way between two games on this
    /// computer, without any network.
    pub direct_loopback: bool,
}

fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x5EED, |d| d.as_nanos() as u64);
    babel_core::babel::seeds::Dice::from_u64(nanos ^ u64::from(std::process::id())).next_u64()
}

/// Screen the simulation asked for.
fn requested(request: ScreenRequest) -> Screen {
    match request {
        ScreenRequest::Build(plot) => Screen::Build(plot),
        ScreenRequest::Crafting(_) => Screen::Crafting,
        ScreenRequest::Scriptorium => Screen::Scriptorium,
        ScreenRequest::Chest => Screen::Chest,
        ScreenRequest::Reader => Screen::Reader,
        ScreenRequest::Lamps => Screen::Lamps,
    }
}

impl Shell {
    pub fn new(data: Arc<GameData>, atlas: Atlas, data_dir: PathBuf) -> Shell {
        let settings_path = data_dir.join("options.ron");
        let save_path = data_dir.join("save.ron");
        let settings = Settings::load(&settings_path);
        let outcome = save::load(&save_path);
        let unusable_save = matches!(outcome, LoadOutcome::Failed(_));
        let (has_save, notice) = match outcome {
            LoadOutcome::NoSave => (false, None),
            LoadOutcome::Loaded(_) => (true, None),
            LoadOutcome::Recovered(_, warning) => (true, Some(warning)),
            LoadOutcome::Failed(message) => (false, Some(message)),
        };
        let backdrop = Game::new(data.clone(), Profile::new(0xBABE1));
        let hero = HeroArt::new(&atlas, Look::default(), Gear::starting());
        let pet_art = PetArt::new(&atlas, PetLook::default());
        Shell {
            data,
            atlas,
            font: Font::default(),
            settings,
            settings_path,
            save_path,
            saver: Saver::new(),
            game: None,
            backdrop,
            art: None,
            hero,
            hud: hud::HudState::default(),
            layer: Canvas::new(VIEW_W, VIEW_H),
            art_cache: ArtCache::default(),
            #[cfg(test)]
            ui_time: std::time::Duration::ZERO,
            camera: Camera::default(),
            fx: Fx::new(),
            screens: vec![Screen::Title],
            focus: vec![Focus::default()],
            ui_data: UiData::default(),
            sounds: Vec::new(),
            time: 0.0,
            notice,
            has_save,
            quit: false,
            show_debug: false,
            stats: DebugStats::default(),
            autosave: false,
            death_shown: false,
            unusable_save,
            pad: false,
            ephemeral: false,
            frozen: false,
            session: coop::Session::default(),
            partner_art: None,
            pet_art,
            partner_pet: None,
            guest_home: None,
            net_bind: None,
            direct_loopback: false,
        }
    }

    pub fn top(&self) -> Option<Screen> {
        self.screens.last().copied()
    }

    pub fn mode(&self) -> AppState {
        match (self.game.is_some(), self.top()) {
            (_, Some(Screen::Credits | Screen::Ending)) => AppState::Credits,
            (false, _) => AppState::MainMenu,
            (true, None) => AppState::InGame,
            (true, Some(Screen::GameOver)) => AppState::GameOver,
            (true, Some(_)) => AppState::Paused,
        }
    }

    pub fn push(&mut self, screen: Screen) {
        self.ui_data.reset_for_screen(self.time);
        match screen {
            Screen::Coop => self.ui_data.coop_error.clear(),
            Screen::Reader => {
                if let Some(game) = self.game.as_mut() {
                    self.ui_data.reading = game.read_book();
                    self.ui_data.reader_half = 0;
                }
            }
            Screen::Catalog | Screen::NewGame => self.ui_data.text.clear(),
            Screen::Look => {
                self.ui_data.preview_turn = 0;
                self.ui_data.creator_pet = false;
                // In a game, the names being edited are the ones it has.
                if let Some(game) = &self.game {
                    let player = &game.profile.player;
                    self.ui_data.hero_name.clone_from(&player.name);
                    self.ui_data.pet_name.clone_from(&player.pet.name);
                }
            }
            Screen::Crafting => self.ui_data.bench = None,
            Screen::Skills => self.ui_data.skill_pet = false,
            Screen::Ending => self.ui_data.ending_page = 0,
            Screen::Scriptorium => {
                self.ui_data.script_target = None;
                self.ui_data.script_scroll = 0;
            }
            _ => {}
        }
        self.screens.push(screen);
        let mut focus = Focus::default();
        // Tabbed screens start on their content, past the tab bar.
        let catalog = self.game.as_ref().is_some_and(Game::catalog_unlocked);
        let tabs = 4 + usize::from(catalog);
        focus.index = match screen {
            // On the first equipment slot, skill or button after the tabs. In
            // the Catalog that is "Viajar", so Enter never types a letter.
            Screen::Inventory | Screen::Skills | Screen::Map | Screen::Catalog => tabs,
            // Past the nine category filters, on the first word.
            Screen::Glossary => tabs + 9,
            // On the first choice, past "Girar" and "Al azar".
            Screen::Look => 2,
            // Past the two craft tabs, on the first recipe.
            Screen::Crafting => 2,
            _ => 0,
        };
        self.focus.push(focus);
    }

    /// The crafting screen just opened belongs to the station being used.
    fn at_bench(&mut self, station: Station) {
        self.ui_data.bench = Some(station);
        // It has one tab more, so the first row is one widget further.
        if let Some(focus) = self.focus.last_mut() {
            focus.index = 3;
        }
    }

    fn pop(&mut self) {
        self.screens.pop();
        self.focus.pop();
        self.ui_data.reset_for_screen(self.time);
        if self.game.is_none() && self.screens.is_empty() {
            self.push(Screen::Title);
        }
    }

    fn close_all(&mut self) {
        self.screens.clear();
        self.focus.clear();
        self.ui_data.reset_for_screen(self.time);
    }

    /// Queues the game to be written in the background.
    pub fn save(&mut self) {
        self.autosave = false;
        if self.ephemeral {
            return;
        }
        if let Some(game) = &self.game {
            let profile = if game.is_mirror() {
                // The host's world is never written here.
                self.guest_save(game)
            } else {
                Some(game.profile.clone())
            };
            let Some(profile) = profile else {
                return;
            };
            match self.saver.store(self.save_path.clone(), profile) {
                Ok(()) => self.has_save = true,
                Err(e) => self.fx.toast(format!("No se pudo guardar: {e}")),
            }
        }
    }

    /// Waits for pending saves: before reading them, and before leaving.
    pub fn flush_saves(&self) {
        self.saver.flush();
    }

    pub fn start(&mut self, mut game: Game) {
        if game.profile.player.hp <= 0.0 {
            game.respawn();
        }
        self.fx = Fx::new();
        self.art = None;
        self.hud = hud::HudState::default();
        self.game = Some(game);
        self.death_shown = false;
        self.close_all();
        self.drain_game();
    }

    fn new_game(&mut self, seed: Option<String>) {
        let seed = seed
            .as_deref()
            .and_then(parse_seed)
            .unwrap_or_else(fresh_seed);
        self.notice = None;
        self.flush_saves();
        if self.unusable_save && !self.ephemeral {
            if let Err(e) = save::set_aside(&self.save_path) {
                // Without a safe copy, refuse to overwrite the old files.
                self.notice = Some(format!("No se pudo apartar la partida anterior: {e}"));
                return;
            }
            self.unusable_save = false;
        }
        let mut profile = Profile::new(seed);
        self.dress(&mut profile);
        self.start(Game::new(self.data.clone(), profile));
        self.save();
    }

    /// Gives a new character what was chosen in the creator.
    fn dress(&self, profile: &mut Profile) {
        let chosen = &self.ui_data;
        profile.player.look = chosen.look.sanitized();
        profile.player.name = clean_name(&chosen.hero_name);
        profile.player.pet.look = chosen.pet.sanitized();
        profile.player.pet.name = clean_name(&chosen.pet_name);
    }

    fn continue_game(&mut self) {
        self.flush_saves();
        match save::load(&self.save_path) {
            LoadOutcome::Loaded(profile) => {
                self.start(Game::new(self.data.clone(), *profile));
            }
            LoadOutcome::Recovered(profile, warning) => {
                self.start(Game::new(self.data.clone(), *profile));
                self.fx.toast(warning);
            }
            LoadOutcome::NoSave => self.has_save = false,
            LoadOutcome::Failed(message) => {
                self.unusable_save = true;
                self.has_save = false;
                self.notice = Some(message);
            }
        }
    }

    fn apply(&mut self, command: Cmd) {
        match command {
            Cmd::Push(s) => self.push(s),
            Cmd::Replace(s) => {
                self.screens.pop();
                self.focus.pop();
                self.push(s);
            }
            Cmd::Pop => self.pop(),
            Cmd::CloseAll => self.close_all(),
            Cmd::NewGame(seed) => {
                self.leave_session();
                self.new_game(seed);
            }
            Cmd::Continue => {
                self.leave_session();
                self.continue_game();
            }
            Cmd::HostRoom => self.host_room(),
            Cmd::JoinRoom => self.join_room(),
            Cmd::LeaveRoom => self.leave_session(),
            Cmd::CopyCode => self.copy_code(),
            Cmd::PasteCode => self.paste_code(),
            Cmd::PasteReply => self.paste_reply(),
            Cmd::SendReply => self.send_reply(),
            Cmd::CopyReply => self.copy_reply(),
            Cmd::Quit => {
                self.save();
                self.leave_session();
                self.flush_saves();
                self.quit = true;
            }
            Cmd::ToTitle => {
                // A visit ends at the title by itself.
                let visiting = self.visiting();
                self.leave_session();
                if visiting {
                    return;
                }
                self.game = None;
                self.art = None;
                self.fx = Fx::new();
                self.close_all();
                self.push(Screen::Title);
            }
            Cmd::Respawn => {
                if let Some(game) = self.game.as_mut() {
                    game.respawn();
                }
                self.death_shown = false;
                self.close_all();
                self.drain_game();
            }
            Cmd::SettingsChanged => {
                if !self.ephemeral
                    && let Err(e) = self.settings.store(&self.settings_path)
                {
                    self.fx
                        .toast(format!("No se pudieron guardar las opciones: {e}"));
                }
            }
            Cmd::Save => self.save(),
            Cmd::FinishCredits => {
                self.close_all();
                self.fx.toast("Modo infinito: la escalera sigue.".into());
                self.save();
            }
        }
    }

    /// Routes simulation events to effects, sound and screens.
    fn drain_game(&mut self) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let events = game.drain_events();
        // The guest's mirror only tries things out; what really happened comes
        // from the host.
        if !game.is_mirror() {
            self.handle_events(&events);
        }
    }

    fn handle_events(&mut self, events: &[GameEvent]) {
        let mut open = Vec::new();
        let mut ending = false;
        for event in events {
            self.fx.handle(event);
            match event {
                GameEvent::Sound(s) => self.sounds.push(*s),
                GameEvent::Open(request) => open.push(*request),
                GameEvent::Autosave => self.autosave = true,
                GameEvent::SiteEntered => self.fx.clear_world(),
                GameEvent::Ending => ending = true,
                _ => {}
            }
        }
        for request in open {
            let screen = requested(request);
            if self.top() != Some(screen) && self.screens.len() < 4 {
                self.push(screen);
                if let ScreenRequest::Crafting(station) = request {
                    self.at_bench(station);
                }
            }
        }
        if ending {
            self.close_all();
            self.push(Screen::Ending);
        }
    }

    fn handle_hotkeys(&mut self, hot: Hotkeys, ui: &UiInput) {
        if hot.debug {
            self.show_debug = !self.show_debug;
        }
        if self.game.is_none() {
            if ui.back && self.screens.len() > 1 && self.ui_data.remap.is_none() {
                self.sounds.push(Sfx::UiBack);
                self.pop();
            }
            return;
        }
        let catalog = self.game.as_ref().is_some_and(Game::catalog_unlocked);
        let shortcuts = [
            (hot.inventory, Screen::Inventory),
            (hot.skills, Screen::Skills),
            (hot.glossary, Screen::Glossary),
            (hot.map, Screen::Map),
            (hot.catalog && catalog, Screen::Catalog),
        ];
        // Nothing can be opened while the player is falling.
        let falling = self.game.as_ref().is_some_and(|g| g.body.dead > 0.0);
        let coop = self.game.as_ref().is_some_and(Game::is_coop);
        match self.top() {
            // Lying down in co-op, one can still pause or leave.
            None if falling => {
                if coop && hot.pause {
                    self.push(Screen::Pause);
                }
            }
            None => {
                if hot.pause {
                    self.sounds.push(Sfx::UiConfirm);
                    self.push(Screen::Pause);
                } else if let Some(&(_, screen)) = shortcuts.iter().find(|s| s.0) {
                    self.sounds.push(Sfx::UiConfirm);
                    self.push(screen);
                }
            }
            Some(Screen::GameOver | Screen::Ending | Screen::Credits) => {}
            Some(top) => {
                if self.ui_data.remap.is_some() {
                    return;
                }
                if ui.back || hot.pause {
                    self.sounds.push(Sfx::UiBack);
                    if self.ui_data.held.take().is_none() {
                        self.pop();
                    }
                } else if !top.takes_text()
                    && let Some(&(_, screen)) = shortcuts.iter().find(|s| s.0)
                {
                    // D-pad shortcuts share buttons with menu navigation.
                    if !self.pad {
                        if screen == top {
                            self.close_all();
                        } else {
                            self.apply(Cmd::Replace(screen));
                        }
                    }
                }
            }
        }
    }

    /// Advances one frame.
    pub fn frame(&mut self, dt: f32, snapshot: &Snapshot, frame: &mut Frame, mixer: &mut Mixer) {
        self.time += dt;
        self.pad = snapshot.pad;
        self.net_receive(dt);
        let screens_before = self.screens.len();
        let top_before = self.top();
        self.handle_hotkeys(snapshot.hot, &snapshot.ui);
        let changed = screens_before != self.screens.len() || top_before != self.top();

        // Alone, the world runs only while nothing covers it. With a partner
        // it runs until both pause; whoever has a menu open just stands still.
        let playing = self.screens.is_empty() && self.game.is_some() && !changed;
        let coop = self.coop_status();
        let mut input = Input::default();
        if playing {
            input = snapshot.game;
            input.aim_at = snapshot.aim_view.map(|v| self.camera.to_world(v));
        }
        let simulate = !self.frozen && !self.visiting() && coop.map_or(playing, |c| !c.stopped());
        if simulate {
            let partner = self.take_guest_controls();
            if let Some(game) = self.game.as_mut() {
                game.step(dt, &input, &partner);
            }
        }
        let mut aim = V2::ZERO;
        if playing && let Some(game) = &self.game {
            if let Some(target) = input.aim_at {
                aim = ((target - game.body.pos) * (1.0 / 160.0)).clamp_len(1.0);
            } else if let Some(d) = input.aim_dir {
                aim = d;
            }
        }
        self.drain_game();
        let dead = self
            .game
            .as_ref()
            .is_some_and(|g| !g.is_coop() && g.body.dead > 0.0 && g.body.dead <= 0.001);
        if dead && !self.death_shown {
            self.death_shown = true;
            self.close_all();
            self.push(Screen::GameOver);
        }
        let fx_running = playing
            || self.top() == Some(Screen::GameOver)
            || self.coop_status().is_some_and(|c| !c.stopped());
        if !self.frozen {
            self.fx.update(if fx_running { dt } else { dt * 0.35 });
        }
        self.fx.shake_enabled = self.settings.screen_shake;

        self.render_world(dt, aim, frame, playing);
        // A screen opened by this same input must not react to it.
        let mut ui_input = snapshot.ui.clone();
        if changed || (playing && !self.screens.is_empty()) {
            ui_input.confirm = false;
            ui_input.click = false;
            ui_input.release = false;
            ui_input.tab = 0;
            ui_input.back = false;
            ui_input.typed.clear();
            ui_input.raw.clear();
            ui_input.letters.clear();
            ui_input.paste = false;
            ui_input.nav = (0, 0);
        }
        #[cfg(test)]
        let started = std::time::Instant::now();
        let commands = self.draw_ui(&ui_input, &mut frame.ui, frame.lights.count);
        #[cfg(test)]
        {
            self.ui_time = started.elapsed();
        }
        for command in commands {
            self.apply(command);
            self.drain_game();
        }
        // So that what a menu just changed is already drawn next frame.
        self.refresh_hero();
        self.net_send(&input);
        if self.autosave {
            self.save();
        }
        if let Some(error) = self.saver.failure() {
            self.fx.toast(format!("No se pudo guardar: {error}"));
        }
        if let Some(game) = &self.game {
            self.hud.update(game, self.time, dt);
        }

        self.update_music(mixer);
        mixer.master = f32::from(self.settings.master) / 10.0;
        mixer.music = f32::from(self.settings.music) / 10.0;
        mixer.effects = f32::from(self.settings.effects) / 10.0;
        for sfx in self.sounds.drain(..) {
            mixer.play(sfx);
        }
    }

    fn update_music(&self, mixer: &mut Mixer) {
        let game = self.game.as_ref().unwrap_or(&self.backdrop);
        let fighting = self.game.is_some() && game.enemies.iter().any(|e| e.boss.is_some());
        mixer.track = if fighting {
            Track::Guardian
        } else {
            let biome = &self.data.biomes[game.site.recipe.biome];
            Track::Biome {
                id: biome.id.clone(),
                root: biome.music_root,
                minor: biome.music_minor,
            }
        };
    }

    /// What to draw: the game's hero with what it wears, or the one being
    /// designed. The character creator shows the body without equipment, so
    /// that no helmet or hood hides a choice.
    fn shown_hero(&self) -> (Look, Gear) {
        match &self.game {
            Some(game) if self.top() == Some(Screen::Look) => {
                (game.profile.player.look, Gear::starting())
            }
            Some(game) => (
                game.profile.player.look,
                Gear::of(&self.data, &game.profile.player),
            ),
            None => (self.ui_data.look, Gear::starting()),
        }
    }

    /// The pet to draw: the game's, or the one being designed.
    fn shown_pet(&self) -> PetLook {
        match &self.game {
            Some(game) => game.profile.player.pet.look,
            None => self.ui_data.pet,
        }
    }

    /// Redraws the hero and the pet only when something about them has changed.
    fn refresh_hero(&mut self) {
        let pet = self.shown_pet();
        if self.pet_art.look != pet {
            self.pet_art = PetArt::new(&self.atlas, pet);
        }
        let (look, gear) = self.shown_hero();
        if self.hero.look != look || self.hero.gear != gear {
            self.hero = HeroArt::new(&self.atlas, look, gear);
        }
    }

    fn render_world(&mut self, dt: f32, aim: V2, frame: &mut Frame, playing: bool) {
        self.refresh_hero();
        self.refresh_partner();
        let title = self.game.is_none();
        if title {
            // The backdrop breathes: lamps flicker and the camera drifts.
            self.backdrop.update(dt, &Input::default());
            self.backdrop.events.clear();
        }
        let game = self.game.as_ref().unwrap_or(&self.backdrop);
        let doors: String = game
            .delta()
            .opened_doors
            .iter()
            .map(|&d| if d { '1' } else { '0' })
            .collect();
        let key = format!("{}:{}:{doors}", game.profile.world_seed, game.site.key.id());
        if self.art.as_ref().is_none_or(|a| a.0 != key) {
            let biome = &self.data.biomes[game.site.recipe.biome];
            self.art = Some((key, tiles::build(&game.site, biome)));
            self.camera.snap(game);
            frame.lights.invalidate();
        }
        let Some((_, art)) = &self.art else {
            return;
        };
        self.camera.shake = if self.fx.shake > 0.05 {
            let amount = self.fx.shake;
            v2(self.fx.spread(amount), self.fx.spread(amount))
        } else {
            V2::ZERO
        };
        if title {
            let drift = v2(
                (self.time * 0.11).sin() * 70.0,
                (self.time * 0.083).cos() * 46.0,
            );
            self.camera.pos = v2(320.0 - VIEW_W as f32 / 2.0, 272.0 - VIEW_H as f32 / 2.0) + drift;
        } else {
            self.camera.follow(game, aim, dt);
        }
        let mut layers = scene::Layers {
            scene: &mut frame.scene,
            emissive: &mut frame.emissive,
        };
        scene::draw(
            &mut layers,
            (&self.atlas, &self.hero.atlas, &self.hero.gear),
            self.partner_art.as_ref().map(|a| (&a.atlas, &a.gear)),
            (
                &self.pet_art.atlas,
                self.partner_pet.as_ref().map(|a| &a.atlas),
            ),
            art,
            game,
            &self.camera,
            &mut self.fx,
            self.time,
            playing,
        );
        let origin = self.camera.origin();
        // Point lights matter less where the gallery is already bright.
        let strength = (1.12 - game.ambient() * 0.72).clamp(0.38, 1.0);
        let mut lights = game.lights.clone();
        for light in &mut lights {
            light.color = light.color.map(|c| c * strength);
        }
        frame
            .lights
            .build(&game.site.grid, lights, origin, self.time);
        frame.fraction = self.camera.fraction();

        let level = game.ambient();
        // Perceptual curve: dim sites read as dim without turning black.
        let effective = 0.035 + level.powf(1.6) * 0.92;
        let tint = game.site.recipe.tint;
        let (flash, amount) = self
            .fx
            .flash
            .map_or(([0u8; 3], 0.0), |(c, s)| (c, s.clamp(0.0, 0.6)));
        let linear = |v: u8| (f32::from(v) / 255.0).powf(2.2);
        frame.settings = CompositeSettings {
            ambient: Vec4::new(
                tint[0] * effective,
                tint[1] * effective,
                tint[2] * effective,
                0.62,
            ),
            flash: Vec4::new(
                linear(flash[0]),
                linear(flash[1]),
                linear(flash[2]),
                amount * 0.55,
            ),
            params: Vec4::new(1.0 / SCENE_W as f32, 1.0 / SCENE_H as f32, 0.55, self.time),
        };
    }

    fn draw_ui(&mut self, input: &UiInput, canvas: &mut Canvas, lights: usize) -> Vec<Cmd> {
        canvas.clear([0; 4]);
        let status = self.coop_status();
        let codes = self.room_codes();
        let coop_view = self.coop_view(&codes);
        let mut commands = Vec::new();
        let top = self.top();
        // The HUD shows only while playing: nothing competes with a screen.
        if let Some(game) = &self.game
            && top.is_none()
        {
            // The HUD has no focusable widgets.
            let mut no_focus = Focus::default();
            let mut ui = Ui::new(
                canvas,
                &self.atlas,
                &self.font,
                input,
                &mut no_focus,
                &mut self.sounds,
                self.time,
            );
            ui.pad = self.pad;
            ui.theme = Place::Hud.theme();
            ui.art = Some(&mut self.art_cache);
            hud::draw(
                &mut ui,
                game,
                &self.fx,
                &self.camera,
                &self.settings,
                &self.hud,
                &self.hero.atlas,
            );
            if let Some(status) = status {
                coop_ui::hud(&mut ui, game, &self.camera, status);
            }
        }
        // A screen slides and fades into place. Screens with their own fade,
        // and frozen screenshots, appear at once.
        let age = self.time - self.ui_data.opened_at;
        let entering = age < ENTRANCE
            && !self.frozen
            && !matches!(
                top,
                None | Some(Screen::GameOver | Screen::Ending | Screen::Credits)
            );
        if entering {
            self.layer.clear([0; 4]);
        }
        let mut throwaway = Focus::default();
        let focus = self.focus.last_mut().unwrap_or(&mut throwaway);
        let target = if entering {
            &mut self.layer
        } else {
            &mut *canvas
        };
        let mut ui = Ui::new(
            target,
            &self.atlas,
            &self.font,
            input,
            focus,
            &mut self.sounds,
            self.time,
        );
        ui.pad = self.pad;
        if let Some(screen) = top {
            ui.theme = place_for(screen, self.ui_data.bench).theme();
            ui.see_through = sees_through(screen);
        }
        ui.art = Some(&mut self.art_cache);
        let hero = &self.hero.atlas;
        let pet = &self.pet_art.atlas;
        if let Some(screen) = top {
            let data = &mut self.ui_data;
            let mut menu = menus::MenuContext {
                settings: &mut self.settings,
                has_save: self.has_save,
                notice: self.notice.as_deref(),
            };
            match (screen, self.game.as_mut()) {
                (Screen::Title, _) => menus::title(&mut ui, &menu, &mut commands),
                (Screen::Look, None) => {
                    let sprites = look::Sprites { hero, pet };
                    let mut chosen = look::Choices {
                        look: data.look,
                        pet: data.pet,
                    };
                    look::draw(&mut ui, &sprites, &mut chosen, data, true, &mut commands);
                    (data.look, data.pet) = (chosen.look, chosen.pet);
                }
                (Screen::Look, Some(g)) => {
                    let sprites = look::Sprites { hero, pet };
                    let mut chosen = look::Choices {
                        look: g.profile.player.look,
                        pet: g.profile.player.pet.look,
                    };
                    look::draw(&mut ui, &sprites, &mut chosen, data, false, &mut commands);
                    if chosen.look != g.profile.player.look {
                        g.set_look(chosen.look);
                        self.autosave = true;
                    }
                    let name = clean_name(&data.hero_name);
                    if name != g.profile.player.name {
                        g.set_name(&name);
                        self.autosave = true;
                    }
                    let pet_name = clean_name(&data.pet_name);
                    let theirs = &g.profile.player.pet;
                    if chosen.pet != theirs.look || pet_name != theirs.name {
                        g.set_pet(&pet_name, chosen.pet);
                        self.autosave = true;
                    }
                }
                (Screen::Coop, _) => coop_ui::draw(&mut ui, &coop_view, data, &mut commands),
                (Screen::NewGame, _) => {
                    menus::new_game(&mut ui, &menu, hero, data, &mut commands);
                }
                (Screen::Options, _) => menus::options(&mut ui, &mut menu, &mut commands),
                (Screen::Controls, _) => {
                    menus::controls(&mut ui, &mut menu, data, &mut commands);
                }
                (Screen::Ending, _) => menus::ending(&mut ui, data, &mut commands),
                (Screen::Credits, _) => menus::credits(&mut ui, data, &mut commands),
                (Screen::Pause, Some(g)) => {
                    menus::pause(&mut ui, g, hero, status, &mut commands);
                }
                (Screen::GameOver, Some(g)) => menus::game_over(&mut ui, g, data, &mut commands),
                (Screen::Inventory, Some(g)) => {
                    inventory::draw(&mut ui, g, hero, data, &mut commands);
                }
                (Screen::Skills, Some(g)) => skills::draw(&mut ui, g, data, &mut commands),
                (Screen::Glossary, Some(g)) => book::glossary(&mut ui, g, data, &mut commands),
                (Screen::Map, Some(g)) => book::map(&mut ui, g, &mut commands),
                (Screen::Catalog, Some(g)) => {
                    book::catalog_screen(&mut ui, g, data, &mut commands);
                }
                (Screen::Reader, Some(g)) => book::reader(&mut ui, g, data, &mut commands),
                (Screen::Crafting, Some(g)) => {
                    craft::crafting_screen(&mut ui, g, data, &mut commands);
                }
                (Screen::Build(plot), Some(g)) => {
                    craft::build_screen(&mut ui, g, plot, data, &mut commands);
                }
                (Screen::Chest, Some(g)) => craft::chest_screen(&mut ui, g, &mut commands),
                (Screen::Lamps, Some(g)) => craft::lamps_screen(&mut ui, g, &mut commands),
                (Screen::Stats, Some(g)) => stats::draw(&mut ui, g, &mut commands),
                (Screen::Scriptorium, Some(g)) => {
                    craft::scriptorium(&mut ui, g, data, &mut commands);
                }
                (_, None) => commands.push(Cmd::ToTitle),
            }
        }
        // Widgets are laid out; the focus can move before anything else draws.
        ui.finish();
        if entering {
            let t = (age / ENTRANCE).clamp(0.0, 1.0);
            let drop = ((1.0 - t) * (1.0 - t) * 10.0).round() as i32;
            canvas.composite(&self.layer, drop, (80.0 + 175.0 * t) as u8);
        }
        let mut no_focus = Focus::default();
        let mut ui = Ui::new(
            canvas,
            &self.atlas,
            &self.font,
            input,
            &mut no_focus,
            &mut self.sounds,
            self.time,
        );
        ui.pad = self.pad;
        ui.theme = Place::Hud.theme();
        ui.art = Some(&mut self.art_cache);
        if !matches!(top, Some(Screen::Ending | Screen::Credits)) {
            hud::notices(&mut ui, &self.fx);
        }
        if self.show_debug {
            let game = self.game.as_ref().unwrap_or(&self.backdrop);
            let actors = game.enemies.len()
                + game.nodes.len()
                + game.drops.len()
                + game.projectiles.len()
                + game.zones.len();
            let lines = [
                format!("FPS {:.0} ({:.1} ms)", self.stats.fps, self.stats.frame_ms),
                format!(
                    "Actores {actors} · enemigos {} · partículas {}",
                    game.enemies.len(),
                    self.fx.particles.len()
                ),
                format!("Entidades del motor {}", self.stats.engine_entities),
                format!("Luces {lights}/64"),
                format!(
                    "Sitio {} · {:?} · {}",
                    match &game.site.key {
                        SiteKey::Walk { floor, q, r } => format!("{floor} {q} {r}"),
                        SiteKey::Remote { .. } => "remoto".into(),
                    },
                    game.site.layout,
                    self.data.biomes[game.site.recipe.biome].id
                ),
            ];
            ui.canvas.fill(150, 2, 190, 54, [0, 0, 0, 190]);
            for (i, line) in lines.iter().enumerate() {
                ui.text(line, 154, 5 + i as i32 * 10, crate::ui::GOOD);
            }
        }
        commands
    }

    /// Pixels of the hero as currently drawn.
    #[cfg(test)]
    pub fn hero_pixels(&self) -> &[u8] {
        &self.hero.atlas.image.px
    }

    #[cfg(test)]
    pub fn widgets(&self) -> Vec<crate::ui::Rect> {
        self.focus
            .last()
            .map_or_else(Vec::new, |f| f.widgets().to_vec())
    }

    #[cfg(test)]
    pub fn focus_index(&self) -> usize {
        self.focus.last().map_or(0, |f| f.index)
    }

    #[cfg(test)]
    pub fn set_focus(&mut self, index: usize) {
        if let Some(f) = self.focus.last_mut() {
            f.index = index;
        }
    }

    #[cfg(test)]
    pub fn leave_room(&mut self) {
        self.apply(Cmd::LeaveRoom);
    }

    /// Loads a specific site for tool modes.
    pub fn visit(&mut self, profile: Profile, key: SiteKey) {
        let mut game = Game::new(self.data.clone(), profile);
        game.enter(key, Arrival::Hub);
        self.start(game);
    }
}
