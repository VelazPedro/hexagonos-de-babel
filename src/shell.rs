//! The game shell: owns the session, routes input to the simulation or to the
//! open screen, and turns the state into pixels, light and sound.

use crate::audio::{Mixer, Track};
use crate::input::{Hotkeys, Snapshot};
use crate::render::atlas::Atlas;
use crate::render::canvas::Canvas;
use crate::render::font::Font;
use crate::render::fx::Fx;
use crate::render::pipeline::{CompositeSettings, Frame};
use crate::render::scene::{self, Camera, SCENE_H, SCENE_W, VIEW_H, VIEW_W};
use crate::render::tiles::{self, SiteArt};
use crate::settings::Settings;
use crate::ui::state::{Cmd, Screen, UiData};
use crate::ui::{Focus, Ui, UiInput, book, craft, hud, inventory, menus, skills};
use babel_core::data::GameData;
use babel_core::game::Game;
use babel_core::game::events::{GameEvent, Input, ScreenRequest, Sfx};
use babel_core::game::profile::{Profile, parse_seed};
use babel_core::game::travel::Arrival;
use babel_core::game::vec::{V2, v2};
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

/// Numbers shown by the F3 overlay.
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugStats {
    pub fps: f32,
    pub frame_ms: f32,
    pub engine_entities: usize,
}

#[derive(Resource)]
pub struct Shell {
    pub data: Arc<GameData>,
    pub atlas: Atlas,
    pub font: Font,
    pub settings: Settings,
    settings_path: PathBuf,
    pub save_path: PathBuf,
    pub game: Option<Game>,
    /// A quiet gallery rendered behind the title menu.
    backdrop: Game,
    art: Option<(String, SiteArt)>,
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
        ScreenRequest::Crafting => Screen::Crafting,
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
        Shell {
            data,
            atlas,
            font: Font::default(),
            settings,
            settings_path,
            save_path,
            game: None,
            backdrop,
            art: None,
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
            Screen::Reader => {
                if let Some(game) = self.game.as_mut() {
                    self.ui_data.reading = game.read_book();
                    self.ui_data.reader_half = 0;
                }
            }
            Screen::Catalog | Screen::NewGame => self.ui_data.text.clear(),
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
            Screen::Inventory | Screen::Skills | Screen::Map => tabs,
            // Past the nine category filters, on the first word.
            Screen::Glossary => tabs + 9,
            // The Catalog is only reachable once sealed open: first letter.
            Screen::Catalog => 5,
            // Past the two craft tabs, on the first recipe.
            Screen::Crafting => 2,
            _ => 0,
        };
        self.focus.push(focus);
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

    pub fn save(&mut self) {
        self.autosave = false;
        if self.ephemeral {
            return;
        }
        if let Some(game) = &self.game {
            match save::store(&self.save_path, &game.profile) {
                Ok(()) => self.has_save = true,
                Err(e) => self.fx.toast(format!("No se pudo guardar: {e}")),
            }
        }
    }

    pub fn start(&mut self, mut game: Game) {
        if game.profile.player.hp <= 0.0 {
            game.respawn();
        }
        self.fx = Fx::new();
        self.art = None;
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
        if self.unusable_save && !self.ephemeral {
            if let Err(e) = save::set_aside(&self.save_path) {
                // Without a safe copy, refuse to overwrite the old files.
                self.notice = Some(format!("No se pudo apartar la partida anterior: {e}"));
                return;
            }
            self.unusable_save = false;
        }
        self.start(Game::new(self.data.clone(), Profile::new(seed)));
        self.save();
    }

    fn continue_game(&mut self) {
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
            Cmd::NewGame(seed) => self.new_game(seed),
            Cmd::Continue => self.continue_game(),
            Cmd::Quit => {
                self.save();
                self.quit = true;
            }
            Cmd::ToTitle => {
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
        let mut open = Vec::new();
        let mut ending = false;
        for event in &events {
            self.fx.handle(event);
            match event {
                GameEvent::Sound(s) => self.sounds.push(*s),
                GameEvent::Open(request) => open.push(requested(*request)),
                GameEvent::Autosave => self.autosave = true,
                GameEvent::SiteEntered => self.fx.clear_world(),
                GameEvent::Ending => ending = true,
                _ => {}
            }
        }
        for screen in open {
            if self.top() != Some(screen) {
                self.push(screen);
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
        match self.top() {
            None if falling => {}
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
        let screens_before = self.screens.len();
        let top_before = self.top();
        self.handle_hotkeys(snapshot.hot, &snapshot.ui);
        let changed = screens_before != self.screens.len() || top_before != self.top();

        // Simulate only while nothing covers the world.
        let playing = self.screens.is_empty() && self.game.is_some() && !changed;
        let mut aim = V2::ZERO;
        if playing && !self.frozen {
            let mut input: Input = snapshot.game;
            input.aim_at = snapshot.aim_view.map(|v| self.camera.to_world(v));
            if let Some(game) = self.game.as_mut() {
                game.update(dt, &input);
                if let Some(target) = input.aim_at {
                    aim = ((target - game.body.pos) * (1.0 / 160.0)).clamp_len(1.0);
                } else if let Some(d) = input.aim_dir {
                    aim = d;
                }
            }
        }
        self.drain_game();
        let dead = self
            .game
            .as_ref()
            .is_some_and(|g| g.body.dead > 0.0 && g.body.dead <= 0.001);
        if dead && !self.death_shown {
            self.death_shown = true;
            self.close_all();
            self.push(Screen::GameOver);
        }
        let fx_running = playing || self.top() == Some(Screen::GameOver);
        if !self.frozen {
            self.fx.update(if fx_running { dt } else { dt * 0.35 });
        }
        self.fx.shake_enabled = self.settings.screen_shake;

        self.render_world(dt, aim, frame, playing);
        // A screen opened by this same input must not react to it.
        let mut input = snapshot.ui.clone();
        if changed || (playing && !self.screens.is_empty()) {
            input.confirm = false;
            input.click = false;
            input.release = false;
            input.tab = 0;
            input.back = false;
            input.typed.clear();
            input.nav = (0, 0);
        }
        let commands = self.draw_ui(&input, &mut frame.ui, frame.lights.count);
        for command in commands {
            self.apply(command);
            self.drain_game();
        }
        if self.autosave {
            self.save();
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

    fn render_world(&mut self, dt: f32, aim: V2, frame: &mut Frame, playing: bool) {
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
            &self.atlas,
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
        let mut commands = Vec::new();
        let top = self.top();
        let covers = matches!(
            top,
            Some(
                Screen::Title
                    | Screen::NewGame
                    | Screen::Reader
                    | Screen::Ending
                    | Screen::Credits
                    | Screen::Inventory
                    | Screen::Skills
                    | Screen::Glossary
                    | Screen::Map
                    | Screen::Catalog
                    | Screen::Scriptorium
                    | Screen::Crafting
                    | Screen::Chest
            )
        );
        if let Some(game) = &self.game
            && !covers
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
            hud::draw(
                &mut ui,
                game,
                &self.fx,
                &self.camera,
                &self.settings,
                self.pad,
            );
        }
        let mut throwaway = Focus::default();
        let focus = self.focus.last_mut().unwrap_or(&mut throwaway);
        let mut ui = Ui::new(
            canvas,
            &self.atlas,
            &self.font,
            input,
            focus,
            &mut self.sounds,
            self.time,
        );
        if let Some(screen) = top {
            let data = &mut self.ui_data;
            let mut menu = menus::MenuContext {
                settings: &mut self.settings,
                has_save: self.has_save,
                notice: self.notice.as_deref(),
            };
            match (screen, self.game.as_mut()) {
                (Screen::Title, _) => menus::title(&mut ui, &menu, &mut commands),
                (Screen::NewGame, _) => menus::new_game(&mut ui, &menu, data, &mut commands),
                (Screen::Options, _) => menus::options(&mut ui, &mut menu, &mut commands),
                (Screen::Controls, _) => {
                    menus::controls(&mut ui, &mut menu, data, &mut commands);
                }
                (Screen::Ending, _) => menus::ending(&mut ui, data, &mut commands),
                (Screen::Credits, _) => menus::credits(&mut ui, data, &mut commands),
                (Screen::Pause, Some(g)) => menus::pause(&mut ui, g, &mut commands),
                (Screen::GameOver, Some(g)) => menus::game_over(&mut ui, g, data, &mut commands),
                (Screen::Inventory, Some(g)) => inventory::draw(&mut ui, g, data, &mut commands),
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
                (Screen::Scriptorium, Some(g)) => {
                    craft::scriptorium(&mut ui, g, data, &mut commands);
                }
                (_, None) => commands.push(Cmd::ToTitle),
            }
        }
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
        ui.finish();
        commands
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

    /// Loads a specific site for tool modes.
    pub fn visit(&mut self, profile: Profile, key: SiteKey) {
        let mut game = Game::new(self.data.clone(), profile);
        game.enter(key, Arrival::Hub);
        self.start(game);
    }
}
