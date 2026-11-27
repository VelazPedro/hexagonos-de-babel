//! Developer tool modes: atlas dump, site screenshots, site search and a
//! performance benchmark.

use crate::render::atlas::Atlas;
use crate::render::canvas::Canvas;
use crate::render::icon;
#[cfg(windows)]
use crate::render::software::Picture;
use crate::shell::Shell;
use crate::ui::state::Screen;
use babel_core::babel::rarity::Rarity;
use babel_core::data::{GameData, ToolKind};
use babel_core::game::events::Input;
use babel_core::game::profile::Profile;
use babel_core::game::vec::{V2, v2};
use babel_core::progression::items;
use babel_core::world::grid::Pos;
use babel_core::world::layout::LayoutKind;
use babel_core::world::site::{self, SiteKey};
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::{Path, PathBuf};

/// Writes the compiled atlas as a PNG so it can be inspected.
pub fn dump_atlas(atlas: &Atlas, path: &Path) -> Result<(), String> {
    save_png(&atlas.image, path)
}

/// Writes a picture of the compatibility mode as a PNG.
#[cfg(windows)]
pub fn save_picture(picture: &Picture, path: &Path) -> Result<(), String> {
    let mut canvas = Canvas::new(picture.w as i32, picture.h as i32);
    for (bytes, pixel) in canvas
        .px
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(&picture.pixels)
    {
        let [_, r, g, b] = pixel.to_be_bytes();
        *bytes = [r, g, b, 255];
    }
    save_png(&canvas, path)
}

pub(crate) fn save_png(canvas: &Canvas, path: &Path) -> Result<(), String> {
    let image = Image::new(
        Extent3d {
            width: canvas.w as u32,
            height: canvas.h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        canvas.px.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let dynamic = image.try_into_dynamic().map_err(|e| e.to_string())?;
    dynamic.save(path).map_err(|e| e.to_string())
}

/// Writes the application icon into `dir`: the `.ico` linked into the
/// executable and a large PNG to look at.
pub fn dump_icon(atlas: &Atlas, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("hexagonos.ico"), icon::ico(atlas)).map_err(|e| e.to_string())?;
    save_png(&icon::art(atlas, 256), &dir.join("hexagonos.png"))
}

/// Finds a site matching a description, for screenshots and testing.
pub fn find_site(data: &GameData, seed: u64, wanted: &str) -> Option<(u32, i32, i32)> {
    let guardian = wanted.strip_prefix("guardian");
    if let Some(n) = guardian {
        let floor: u32 = n.trim_start_matches(':').parse().unwrap_or(5);
        let (q, r) = babel_core::world::floors::stair_offset(seed, floor);
        return Some((floor, q, r));
    }
    for floor in 1..=29u32 {
        for ring in 1..=14i32 {
            for q in -ring..=ring {
                for r in -ring..=ring {
                    if q.abs().max(r.abs()) != ring {
                        continue;
                    }
                    let s = site::generate(data, seed, &SiteKey::walk(floor, q, r));
                    let biome = &data.biomes[s.recipe.biome].id;
                    let layout = format!("{:?}", s.layout).to_lowercase();
                    let hit = if let Some(b) = wanted.strip_prefix("biome:") {
                        biome == b && !s.enemies.is_empty()
                    } else if let Some(l) = wanted.strip_prefix("layout:") {
                        layout == l && s.layout != LayoutKind::Home
                    } else {
                        match wanted {
                            "elite" => s.enemies.iter().any(|e| e.elite.is_some()),
                            "canon" => !s.chests.is_empty() && !s.analysis.canons.is_empty(),
                            "dark" => s.recipe.ambient <= 0.2 && s.enemies.len() >= 3,
                            "bright" => s.recipe.ambient >= 0.85 && s.enemies.len() >= 3,
                            _ => false,
                        }
                    };
                    if hit {
                        return Some((floor, q, r));
                    }
                }
            }
        }
    }
    None
}

#[derive(Resource)]
pub struct ScreenshotJob {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub waited: u32,
}

impl ScreenshotJob {
    pub fn new(path: PathBuf) -> ScreenshotJob {
        ScreenshotJob {
            path,
            frames: 0,
            requested: false,
            waited: 0,
        }
    }
}

/// A well-equipped profile so screenshots show a mid-game state.
pub fn showcase_profile(data: &GameData, seed: u64, floor: u32) -> Profile {
    let mut p = Profile::new(seed);
    let tier = (floor / 6).min(5) as u8;
    for t in ToolKind::ALL {
        p.player.tools[t.index()] = tier.max(1);
    }
    for (i, (word, rarity)) in [
        ("daga", Rarity::Rare),
        ("capa", Rarity::Epic),
        ("bota", Rarity::Uncommon),
        ("linterna", Rarity::Legendary),
        ("grial", Rarity::Rare),
        ("espada", Rarity::Common),
        ("yelmo", Rarity::Epic),
    ]
    .into_iter()
    .enumerate()
    {
        let item = items::from_words(data, i as u64 + 10, word, None, rarity);
        p.player.receive_item(item);
    }
    p.player.gain_xp(1400);
    for skill in ["e_paso", "e_esquiva", "c_pulso", "c_carga", "c_paginas"] {
        p.player.learn(data, skill);
    }
    for (i, r) in babel_core::data::Resource::ALL.into_iter().enumerate() {
        p.player.add_resource(r, 6 + (i as u32 * 7) % 23);
    }
    for (i, e) in data
        .lexicon
        .entries()
        .iter()
        .step_by(31)
        .take(30)
        .enumerate()
    {
        p.player.glossary.insert(
            e.word.clone(),
            babel_core::progression::player::GlossEntry {
                rarity: Rarity::ALL[i % 5],
                times: 1 + i as u32 % 4,
            },
        );
    }
    p.world.tutorial = u16::MAX;
    p
}

/// Prepares the shell for one screenshot.
pub fn stage(
    shell: &mut Shell,
    seed: u64,
    key: SiteKey,
    seconds: f32,
    screen: Option<Screen>,
    reveal: bool,
) {
    let profile = showcase_profile(&shell.data, seed, key.depth());
    shell.ephemeral = true;
    shell.visit(profile, key);
    if let Some(game) = shell.game.as_mut() {
        // Walk a little so the scene has motion and attacks underway.
        let steps = (seconds * 60.0) as u32;
        for i in 0..steps {
            game.profile.player.hp = game.derived.max_hp;
            let target = game
                .enemies
                .first()
                .map_or(V2::ZERO, |e| (e.pos - game.body.pos).norm());
            let near = game
                .enemies
                .iter()
                .any(|e| e.pos.dist(game.body.pos) < 34.0);
            let input = Input {
                movement: if near || i > steps * 2 / 3 {
                    V2::ZERO
                } else {
                    target * 0.55
                },
                attack_pressed: near && i % 20 == 0,
                ..Default::default()
            };
            game.update(1.0 / 60.0, &input);
            game.drain_events();
        }
        if reveal {
            game.body.invulnerable = 0.0;
        }
        game.update(1.0 / 60.0, &Input::default());
        game.drain_events();
    }
    if let Some(screen) = screen {
        if screen == Screen::Title {
            shell.game = None;
        }
        shell.screens.clear();
        shell.push(screen);
    }
    shell.fx.banners.clear();
    shell.fx.toasts.clear();
    shell.frozen = true;
}

pub fn drive_screenshot(
    mut commands: Commands,
    job: Option<ResMut<ScreenshotJob>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut job) = job else {
        return;
    };
    job.frames += 1;
    // Give shaders and render targets time to be ready.
    if !job.requested && job.frames > 90 {
        job.requested = true;
        let _ = std::fs::remove_file(&job.path);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(job.path.clone()));
    } else if job.requested {
        job.waited += 1;
        let written = std::fs::metadata(&job.path).is_ok_and(|m| m.len() > 0);
        if (written && job.waited > 20) || job.waited > 900 {
            exit.write(if written {
                AppExit::Success
            } else {
                AppExit::error()
            });
        }
    }
}

#[derive(Resource)]
pub struct BenchJob {
    pub seconds: f32,
    elapsed: f32,
    samples: Vec<f32>,
    lights_seen: usize,
}

impl BenchJob {
    pub fn new(seconds: f32) -> BenchJob {
        BenchJob {
            seconds,
            elapsed: 0.0,
            samples: Vec::new(),
            lights_seen: 0,
        }
    }
}

/// Stress scene: 40 enemies, 64 lights and a constant particle storm.
pub fn stage_bench(shell: &mut Shell) {
    let seed = 4242;
    let (q, r) = babel_core::world::floors::stair_offset(seed, 12);
    let profile = showcase_profile(&shell.data, seed, 12);
    shell.ephemeral = true;
    shell.show_debug = true;
    shell.visit(profile, SiteKey::walk(12, q + 1, r + 1));
    let data = shell.data.clone();
    let Some(game) = shell.game.as_mut() else {
        return;
    };
    let hub = game.site.hub;
    let open: Vec<Pos> = game
        .site
        .grid
        .positions()
        .filter(|&p| game.site.grid.walkable(p.x, p.y))
        .collect();
    let mut near = open.clone();
    near.sort_by_key(|p| p.dist2(hub));
    game.enemies.clear();
    for i in 0..40usize {
        let pos = near[(20 + i * 5) % near.len()];
        let spawn = site::EnemySpawn {
            id: 1000 + i as u32,
            archetype: i % data.enemies.archetypes.len(),
            word: None,
            modifiers: vec![i % data.enemies.modifiers.len()],
            elite: None,
            pos,
        };
        let mut enemy = babel_core::game::actors::make_enemy(&data, 5000 + i as u32, 12, &spawn);
        enemy.hp = 1.0e9;
        enemy.max_hp = enemy.hp;
        game.enemies.push(enemy);
    }
    let id = game.site.key.id();
    let delta = game.profile.world.deltas.entry(id).or_default();
    for i in 0..64usize {
        delta.torches.push(near[(i * 7) % near.len()]);
    }
}

pub fn drive_bench(
    job: Option<ResMut<BenchJob>>,
    mut shell: ResMut<Shell>,
    time: Res<Time>,
    frame: Res<crate::render::pipeline::Frame>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut job) = job else {
        return;
    };
    let dt = time.delta_secs();
    job.elapsed += dt;
    job.lights_seen = job.lights_seen.max(frame.lights.count);
    let t = job.elapsed;
    if let Some(game) = shell.game.as_mut() {
        game.profile.player.hp = game.derived.max_hp;
        let pos = game.body.pos;
        shell.fx.burst(
            pos + v2((t * 3.0).sin() * 90.0, (t * 2.3).cos() * 60.0),
            10.0,
            [255, 200, 90],
            12,
            90.0,
        );
    }
    // The first seconds include shader compilation.
    if job.elapsed > 4.0 {
        job.samples.push(dt);
    }
    if job.elapsed > 4.0 + job.seconds {
        let mut sorted = job.samples.clone();
        sorted.sort_by(f32::total_cmp);
        let total: f32 = sorted.iter().sum();
        let mean = total / sorted.len().max(1) as f32;
        let p99 = sorted
            .get(sorted.len().saturating_sub(1) * 99 / 100)
            .copied()
            .unwrap_or(0.0);
        let enemies = shell.game.as_ref().map_or(0, |g| g.enemies.len());
        println!(
            "bench: {} frames, enemies {enemies}, lights {}, particles {}",
            sorted.len(),
            job.lights_seen,
            shell.fx.particles.len()
        );
        println!(
            "bench: mean {:.2} ms ({:.0} FPS), p99 {:.2} ms ({:.0} FPS)",
            mean * 1000.0,
            1.0 / mean.max(1e-6),
            p99 * 1000.0,
            1.0 / p99.max(1e-6)
        );
        exit.write(AppExit::Success);
    }
}

/// `--coop-direct`, host: waits for the reply code a guest leaves in `file`.
pub fn answer_from(shell: &mut Shell, file: PathBuf) {
    for _ in 0..600 {
        if let Ok(text) = std::fs::read_to_string(&file)
            && !text.trim().is_empty()
        {
            shell.answer_with(text.trim());
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("No llegó ninguna respuesta");
}

/// `--coop-direct`, guest: leaves its reply code in `file` for the host.
pub fn leave_reply(shell: &Shell, file: PathBuf) {
    for _ in 0..300 {
        if let Some(code) = shell.reply_code() {
            // Renamed into place, so that half a code is never read.
            let draft = file.with_extension("draft");
            if std::fs::write(&draft, code)
                .and_then(|()| std::fs::rename(&draft, &file))
                .is_err()
            {
                eprintln!("No se pudo escribir la respuesta");
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("No hubo código de respuesta");
}
