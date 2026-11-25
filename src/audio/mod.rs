//! Plays the procedural effects and the ambient music.

pub mod synth;

use babel_core::game::events::Sfx;
use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

/// What the music should be playing right now.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Track {
    #[default]
    Silence,
    Biome {
        id: String,
        root: i32,
        minor: bool,
    },
    Guardian,
}

/// Requests queued by the game; the audio systems drain it every frame.
#[derive(Resource, Default)]
pub struct Mixer {
    queue: Vec<Sfx>,
    pub track: Track,
    pub master: f32,
    pub music: f32,
    pub effects: f32,
    /// Disabled in headless tool modes.
    pub muted: bool,
}

impl Mixer {
    pub fn play(&mut self, sfx: Sfx) {
        self.queue.push(sfx);
    }

    fn music_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master * self.music * 0.8
        }
    }
}

#[derive(Resource, Default)]
struct Bank {
    effects: HashMap<Sfx, Handle<AudioSource>>,
    tracks: HashMap<String, Handle<AudioSource>>,
    /// Tracks being composed in the background.
    composing: HashMap<String, Mutex<Receiver<Vec<u8>>>>,
    playing: Track,
}

impl Track {
    /// Name under which a composed track is kept.
    fn key(&self) -> Option<String> {
        match self {
            Track::Silence => None,
            Track::Biome { id, .. } => Some(id.clone()),
            Track::Guardian => Some("#guardian".to_string()),
        }
    }
}

/// Composes a track on its own thread: it takes long enough to drop frames.
fn compose(track: Track) -> Receiver<Vec<u8>> {
    let (done, result) = channel();
    let job = move || {
        let samples = match &track {
            Track::Biome { id, root, minor } => synth::ambient(id, *root, *minor),
            Track::Guardian => synth::guardian_theme(),
            Track::Silence => return,
        };
        let _ = done.send(synth::wav(&samples));
    };
    // If no thread can be started the track is simply skipped.
    let _ = std::thread::Builder::new()
        .name("composer".into())
        .spawn(job);
    result
}

#[derive(Component)]
struct Music;

fn source(samples: &[f32]) -> AudioSource {
    AudioSource {
        bytes: Arc::from(synth::wav(samples).into_boxed_slice()),
    }
}

fn setup(mut bank: ResMut<Bank>, mut assets: ResMut<Assets<AudioSource>>) {
    for sfx in Sfx::ALL {
        let samples = synth::render(&synth::recipe(sfx));
        bank.effects.insert(sfx, assets.add(source(&samples)));
    }
}

fn play_effects(mut commands: Commands, mut mixer: ResMut<Mixer>, bank: Res<Bank>) {
    let queued = std::mem::take(&mut mixer.queue);
    let volume = mixer.master * mixer.effects;
    if mixer.muted || volume <= 0.001 {
        return;
    }
    // One instance per sound per frame keeps busy fights from clipping.
    let mut seen = HashSet::new();
    for sfx in queued {
        if !seen.insert(sfx) {
            continue;
        }
        if let Some(handle) = bank.effects.get(&sfx) {
            commands.spawn((
                AudioPlayer(handle.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
            ));
        }
    }
}

fn play_music(
    mut commands: Commands,
    mixer: Res<Mixer>,
    mut bank: ResMut<Bank>,
    mut assets: ResMut<Assets<AudioSource>>,
    music: Query<Entity, With<Music>>,
    mut sinks: Query<&mut AudioSink, With<Music>>,
) {
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(mixer.music_volume()));
    }
    // Collect what the background threads have finished.
    let mut finished = Vec::new();
    bank.composing.retain(|key, result| {
        let received = result.get_mut().ok().map(|r| r.try_recv());
        match received {
            Some(Ok(bytes)) => {
                finished.push((key.clone(), bytes));
                false
            }
            Some(Err(TryRecvError::Empty)) => true,
            _ => false,
        }
    });
    for (key, bytes) in finished {
        let handle = assets.add(AudioSource {
            bytes: Arc::from(bytes.into_boxed_slice()),
        });
        bank.tracks.insert(key, handle);
    }
    if mixer.muted || bank.playing == mixer.track {
        return;
    }
    let handle = match mixer.track.key() {
        None => None,
        Some(key) => match bank.tracks.get(&key) {
            Some(handle) => Some(handle.clone()),
            None => {
                // The current music plays on until the new one is ready.
                bank.composing
                    .entry(key)
                    .or_insert_with(|| Mutex::new(compose(mixer.track.clone())));
                return;
            }
        },
    };
    bank.playing = mixer.track.clone();
    for entity in &music {
        commands.entity(entity).despawn();
    }
    if let Some(handle) = handle {
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(mixer.music_volume())),
            Music,
        ));
    }
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Mixer>()
            .init_resource::<Bank>()
            .add_systems(Startup, setup)
            .add_systems(PostUpdate, (play_effects, play_music));
    }
}
