//! A tiny sfxr-style synthesiser plus procedural ambient music. No audio files.
//!
//! The sound is meant to be warm, like wood and paper: oscillators carry only
//! a few low harmonics (so nothing aliases), pitches stay low and every mix
//! goes through a gentle low-pass.

use babel_core::babel::seeds::Dice;
use babel_core::game::events::Sfx;

pub const RATE: u32 = 22_050;

/// Cut-off of the filter every effect goes through.
const EFFECT_WARMTH: f32 = 2_300.0;
const MUSIC_WARMTH: f32 = 2_000.0;
/// No voice starts faster than this, which avoids clicks.
const MIN_ATTACK: f32 = 0.006;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// A rounded square: hollow, like a wooden pipe.
    Reed,
    /// A rounded saw: fuller, for growls and bass.
    Brass,
    Sine,
    Triangle,
    Noise,
}

/// One voice: oscillator, envelope, pitch sweep, vibrato, arpeggio and filter.
#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub wave: Wave,
    pub freq: f32,
    /// Octaves per second.
    pub slide: f32,
    pub attack: f32,
    pub sustain: f32,
    pub decay: f32,
    pub vibrato: f32,
    pub vibrato_rate: f32,
    /// Frequency multiplier applied after `arp_at` seconds.
    pub arp: f32,
    pub arp_at: f32,
    /// One-pole low-pass coefficient in `(0, 1]`.
    pub lowpass: f32,
    pub volume: f32,
    pub delay: f32,
}

const BASE: Voice = Voice {
    wave: Wave::Sine,
    freq: 440.0,
    slide: 0.0,
    attack: MIN_ATTACK,
    sustain: 0.05,
    decay: 0.1,
    vibrato: 0.0,
    vibrato_rate: 0.0,
    arp: 1.0,
    arp_at: 0.0,
    lowpass: 1.0,
    volume: 0.4,
    delay: 0.0,
};

fn tone(wave: Wave, freq: f32, sustain: f32, decay: f32, volume: f32) -> Voice {
    Voice {
        wave,
        freq,
        sustain,
        decay,
        volume,
        ..BASE
    }
}

/// Filtered noise; `freq` sets its grain and `lowpass` how muffled it is.
fn noise(freq: f32, sustain: f32, decay: f32, volume: f32, lowpass: f32) -> Voice {
    Voice {
        lowpass,
        ..tone(Wave::Noise, freq, sustain, decay, volume)
    }
}

/// Rising notes, one after another.
fn arpeggio(wave: Wave, notes: &[f32], gap: f32, decay: f32, volume: f32) -> Vec<Voice> {
    notes
        .iter()
        .enumerate()
        .map(|(i, &f)| Voice {
            delay: i as f32 * gap,
            attack: 0.012,
            ..tone(wave, f, gap * 0.6, decay, volume)
        })
        .collect()
}

/// The recipe of every sound effect.
pub fn recipe(sfx: Sfx) -> Vec<Voice> {
    use Wave::{Brass, Reed, Sine, Triangle};
    let v = |wave, freq, slide: f32, sustain, decay, volume| Voice {
        slide,
        ..tone(wave, freq, sustain, decay, volume)
    };
    // The same voice, starting later.
    let at = |delay: f32, voice: Voice| Voice { delay, ..voice };
    match sfx {
        Sfx::Step => vec![
            noise(520.0, 0.006, 0.035, 0.13, 0.09),
            v(Sine, 108.0, -3.0, 0.006, 0.035, 0.10),
        ],
        Sfx::StepWater => vec![
            noise(1200.0, 0.012, 0.07, 0.14, 0.16),
            v(Sine, 300.0, 2.2, 0.01, 0.05, 0.07),
        ],
        Sfx::Swing => vec![
            noise(1600.0, 0.02, 0.08, 0.22, 0.14),
            v(Sine, 230.0, -2.4, 0.02, 0.06, 0.08),
        ],
        Sfx::SwingHeavy => vec![
            noise(1100.0, 0.04, 0.13, 0.30, 0.12),
            v(Triangle, 130.0, -2.0, 0.03, 0.10, 0.20),
        ],
        Sfx::Charge => vec![v(Reed, 196.0, 1.6, 0.12, 0.09, 0.15)],
        Sfx::Hit => vec![
            noise(700.0, 0.02, 0.07, 0.40, 0.18),
            v(Triangle, 150.0, -3.6, 0.02, 0.08, 0.30),
        ],
        Sfx::HitArmor => vec![
            v(Triangle, 392.0, -0.6, 0.015, 0.10, 0.22),
            v(Sine, 587.0, -0.4, 0.01, 0.13, 0.14),
            noise(600.0, 0.01, 0.04, 0.16, 0.14),
        ],
        Sfx::EnemyDie => vec![
            v(Reed, 247.0, -2.8, 0.05, 0.18, 0.22),
            noise(480.0, 0.04, 0.15, 0.24, 0.13),
        ],
        Sfx::EliteDie => vec![
            v(Brass, 175.0, -1.5, 0.16, 0.4, 0.26),
            noise(360.0, 0.12, 0.4, 0.30, 0.11),
            at(0.2, v(Triangle, 440.0, 0.0, 0.08, 0.32, 0.20)),
        ],
        Sfx::Dash => vec![
            noise(1900.0, 0.03, 0.14, 0.20, 0.13),
            v(Sine, 220.0, 2.4, 0.03, 0.10, 0.14),
        ],
        Sfx::Hurt => vec![
            v(Brass, 147.0, -2.4, 0.05, 0.15, 0.30),
            noise(320.0, 0.03, 0.09, 0.26, 0.11),
        ],
        Sfx::Death => vec![
            v(Triangle, 220.0, -1.1, 0.3, 0.6, 0.34),
            at(0.18, v(Brass, 82.0, -0.7, 0.25, 0.55, 0.20)),
        ],
        Sfx::HarvestWood => vec![
            v(Triangle, 142.0, -2.0, 0.012, 0.08, 0.42),
            noise(500.0, 0.008, 0.04, 0.22, 0.14),
        ],
        Sfx::HarvestStone => vec![
            noise(820.0, 0.01, 0.07, 0.36, 0.16),
            v(Triangle, 98.0, -1.0, 0.01, 0.06, 0.24),
        ],
        Sfx::HarvestMetal => vec![
            v(Triangle, 330.0, 0.0, 0.01, 0.15, 0.20),
            v(Sine, 659.0, 0.0, 0.01, 0.18, 0.13),
        ],
        Sfx::HarvestGlass => vec![
            v(Sine, 784.0, 0.3, 0.008, 0.15, 0.17),
            v(Sine, 1047.0, 0.0, 0.008, 0.11, 0.09),
            noise(900.0, 0.005, 0.04, 0.08, 0.2),
        ],
        Sfx::HarvestPlant => vec![noise(1100.0, 0.015, 0.09, 0.26, 0.12)],
        Sfx::NodeBreak => vec![
            noise(520.0, 0.05, 0.19, 0.36, 0.13),
            v(Triangle, 165.0, -2.4, 0.03, 0.13, 0.24),
        ],
        Sfx::Pickup => vec![Voice {
            arp: 1.5,
            arp_at: 0.045,
            attack: 0.01,
            ..v(Triangle, 392.0, 0.0, 0.06, 0.08, 0.20)
        }],
        Sfx::DropCommon => vec![v(Triangle, 294.0, 0.8, 0.03, 0.09, 0.22)],
        Sfx::DropRare => arpeggio(Triangle, &[330.0, 440.0, 587.0], 0.06, 0.16, 0.18),
        Sfx::DropEpic => arpeggio(Reed, &[262.0, 330.0, 440.0, 659.0], 0.07, 0.2, 0.14),
        Sfx::DropLegendary => arpeggio(
            Triangle,
            &[262.0, 330.0, 392.0, 523.0, 659.0, 784.0],
            0.075,
            0.34,
            0.22,
        ),
        Sfx::DropCanonical => arpeggio(
            Sine,
            &[330.0, 494.0, 659.0, 587.0, 740.0, 880.0, 988.0],
            0.09,
            0.5,
            0.19,
        ),
        Sfx::Craft => vec![
            v(Triangle, 175.0, -0.5, 0.02, 0.06, 0.30),
            at(0.1, v(Triangle, 220.0, -0.5, 0.02, 0.06, 0.30)),
            at(0.2, v(Sine, 440.0, 0.0, 0.05, 0.22, 0.24)),
        ],
        Sfx::Build => vec![
            noise(300.0, 0.08, 0.25, 0.34, 0.10),
            at(0.14, v(Triangle, 196.0, 0.0, 0.08, 0.25, 0.28)),
            at(0.26, v(Triangle, 294.0, 0.0, 0.1, 0.3, 0.28)),
        ],
        Sfx::LevelUp => arpeggio(Reed, &[196.0, 262.0, 330.0, 392.0, 523.0], 0.09, 0.28, 0.17),
        Sfx::SkillLearn => arpeggio(Triangle, &[220.0, 330.0, 494.0], 0.08, 0.24, 0.28),
        Sfx::ReadPage => vec![noise(1500.0, 0.03, 0.2, 0.18, 0.09)],
        Sfx::WordFound => arpeggio(Sine, &[494.0, 659.0], 0.1, 0.22, 0.22),
        Sfx::LegendaryFanfare => {
            let mut out = arpeggio(
                Reed,
                &[196.0, 196.0, 262.0, 330.0, 392.0, 523.0],
                0.11,
                0.32,
                0.15,
            );
            out.push(Voice {
                delay: 0.55,
                vibrato: 0.01,
                vibrato_rate: 5.0,
                attack: 0.03,
                ..v(Triangle, 262.0, 0.0, 0.35, 0.65, 0.28)
            });
            out
        }
        Sfx::Inscribe => vec![
            noise(1700.0, 0.1, 0.1, 0.13, 0.08),
            Voice {
                delay: 0.14,
                vibrato: 0.02,
                vibrato_rate: 9.0,
                ..v(Sine, 392.0, 0.6, 0.14, 0.3, 0.26)
            },
        ],
        Sfx::LampLight => vec![
            v(Sine, 196.0, 0.9, 0.15, 0.35, 0.26),
            noise(620.0, 0.05, 0.25, 0.16, 0.10),
        ],
        Sfx::TorchPlace => vec![noise(580.0, 0.04, 0.22, 0.26, 0.12)],
        Sfx::Teleport => vec![
            Voice {
                vibrato: 0.05,
                vibrato_rate: 12.0,
                attack: 0.03,
                ..v(Sine, 147.0, 1.9, 0.25, 0.28, 0.28)
            },
            noise(1000.0, 0.15, 0.25, 0.10, 0.09),
        ],
        Sfx::Stairs => arpeggio(Triangle, &[262.0, 220.0, 175.0, 131.0], 0.1, 0.16, 0.28),
        Sfx::Door => vec![
            noise(260.0, 0.05, 0.18, 0.22, 0.09),
            v(Sine, 104.0, 0.7, 0.04, 0.13, 0.20),
        ],
        Sfx::ChestOpen => {
            let mut out = vec![noise(380.0, 0.04, 0.1, 0.24, 0.12)];
            out.extend(
                arpeggio(Triangle, &[294.0, 392.0, 587.0], 0.09, 0.22, 0.18)
                    .into_iter()
                    .map(|mut v| {
                        v.delay += 0.08;
                        v
                    }),
            );
            out
        }
        // Menu sounds play constantly, so they are the softest of all.
        Sfx::UiMove => vec![v(Sine, 262.0, 0.0, 0.008, 0.03, 0.09)],
        Sfx::UiConfirm => vec![Voice {
            arp: 1.335,
            arp_at: 0.035,
            ..v(Sine, 294.0, 0.0, 0.045, 0.07, 0.15)
        }],
        Sfx::UiBack => vec![Voice {
            arp: 0.75,
            arp_at: 0.035,
            ..v(Sine, 294.0, 0.0, 0.045, 0.06, 0.14)
        }],
        Sfx::UiError => vec![v(Reed, 117.0, 0.0, 0.09, 0.08, 0.18)],
        Sfx::Throw => vec![
            noise(1500.0, 0.02, 0.1, 0.18, 0.12),
            v(Triangle, 330.0, -2.0, 0.02, 0.08, 0.13),
        ],
        Sfx::Explosion => vec![
            noise(230.0, 0.08, 0.44, 0.50, 0.10),
            v(Sine, 72.0, -0.9, 0.06, 0.32, 0.38),
        ],
        Sfx::BossRoar => vec![
            Voice {
                vibrato: 0.11,
                vibrato_rate: 17.0,
                attack: 0.03,
                ..v(Brass, 73.0, -0.3, 0.4, 0.45, 0.34)
            },
            noise(190.0, 0.3, 0.4, 0.28, 0.07),
        ],
        Sfx::Freeze => vec![v(Sine, 880.0, -1.5, 0.03, 0.24, 0.13)],
        Sfx::Poison => vec![Voice {
            vibrato: 0.16,
            vibrato_rate: 12.0,
            ..v(Reed, 139.0, -0.5, 0.08, 0.15, 0.15)
        }],
        Sfx::Burn => vec![noise(760.0, 0.06, 0.25, 0.22, 0.13)],
        Sfx::Locked => vec![
            v(Reed, 123.0, 0.0, 0.03, 0.06, 0.20),
            at(0.1, v(Reed, 104.0, 0.0, 0.03, 0.08, 0.20)),
        ],
        Sfx::Heal => arpeggio(Sine, &[262.0, 330.0, 392.0], 0.08, 0.26, 0.24),
        Sfx::Victory => arpeggio(Triangle, &[262.0, 392.0, 330.0, 523.0], 0.12, 0.32, 0.25),
    }
}

/// Band-limited waves: only the first few harmonics, so nothing aliases.
fn oscillator(wave: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    let angle = p * std::f32::consts::TAU;
    match wave {
        Wave::Reed => 0.62 * (angle.sin() + 0.2 * (3.0 * angle).sin()),
        Wave::Brass => {
            0.62 * (angle.sin() + 0.34 * (2.0 * angle).sin() + 0.14 * (3.0 * angle).sin())
        }
        Wave::Sine => angle.sin(),
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
        Wave::Noise => 0.0,
    }
}

/// Second-order Butterworth low-pass over the whole buffer. A loop is warmed
/// up on its own tail so that the seam carries the filter state across.
fn warm(samples: &mut [f32], cutoff: f32, looping: bool) {
    let w = std::f32::consts::TAU * cutoff / RATE as f32;
    let (sin, cos) = w.sin_cos();
    let alpha = sin * std::f32::consts::FRAC_1_SQRT_2;
    let a0 = 1.0 + alpha;
    let b = [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0].map(|v| v / a0);
    let a = [-2.0 * cos / a0, (1.0 - alpha) / a0];
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut run = |x: f32| {
        let y = b[0] * x + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
        (x2, x1, y2, y1) = (x1, x, y1, y);
        y
    };
    if looping {
        let tail = samples.len().saturating_sub(RATE as usize / 4);
        for &s in &samples[tail..] {
            run(s);
        }
    }
    for s in samples.iter_mut() {
        *s = run(*s);
    }
}

/// Mixes the voices of one effect into mono samples.
pub fn render(voices: &[Voice]) -> Vec<f32> {
    let length = voices
        .iter()
        .map(|v| v.delay + v.attack.max(MIN_ATTACK) + v.sustain + v.decay)
        .fold(0.0f32, f32::max);
    let mut out = vec![0.0f32; (length * RATE as f32) as usize + 1];
    let dt = 1.0 / RATE as f32;
    for (index, v) in voices.iter().enumerate() {
        let mut dice = Dice::from_u64(v.freq.to_bits() as u64 + index as u64);
        let start = (v.delay * RATE as f32) as usize;
        let attack = v.attack.max(MIN_ATTACK);
        let total = attack + v.sustain + v.decay;
        let samples = (total * RATE as f32) as usize;
        let (mut phase, mut held, mut filtered) = (0.0f32, 0.0f32, 0.0f32);
        let mut cycle = -1.0f32;
        for i in 0..samples {
            let t = i as f32 * dt;
            let envelope = if t < attack {
                t / attack
            } else if t < attack + v.sustain {
                1.0
            } else {
                let k = 1.0 - (t - attack - v.sustain) / v.decay.max(dt);
                k.max(0.0) * k.max(0.0)
            };
            let mut freq = v.freq * (v.slide * t).exp2();
            if v.arp_at > 0.0 && t >= v.arp_at {
                freq *= v.arp;
            }
            if v.vibrato > 0.0 {
                freq *= 1.0 + v.vibrato * (t * v.vibrato_rate * std::f32::consts::TAU).sin();
            }
            phase += freq.clamp(20.0, 5_000.0) * dt;
            let raw = if v.wave == Wave::Noise {
                // Sample-and-hold noise pitched by the frequency.
                if phase.floor() != cycle {
                    cycle = phase.floor();
                    held = dice.between(-1.0, 1.0);
                }
                held
            } else {
                oscillator(v.wave, phase)
            };
            filtered += (raw - filtered) * v.lowpass.clamp(0.01, 1.0);
            if let Some(slot) = out.get_mut(start + i) {
                *slot += filtered * envelope * v.volume;
            }
        }
    }
    warm(&mut out, EFFECT_WARMTH, false);
    out
}

/// 16-bit mono PCM WAV.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32_000.0) as i16).to_le_bytes());
    }
    out
}

fn semitone(root: i32, steps: i32) -> f32 {
    // A2 = 110 Hz.
    110.0 * ((root + steps) as f32 / 12.0).exp2()
}

/// Adds a note into a looping buffer: tails wrap around so the loop is seamless.
fn add_note(
    out: &mut [f32],
    at: f32,
    freq: f32,
    length: f32,
    attack: f32,
    volume: f32,
    wave: Wave,
) {
    let len = out.len();
    if len == 0 {
        return;
    }
    let start = (at * RATE as f32) as usize;
    let samples = (length * RATE as f32) as usize;
    let mut phase = 0.0f32;
    for i in 0..samples {
        let t = i as f32 / RATE as f32;
        let rise = (t / attack.max(0.001)).min(1.0);
        let fall = (1.0 - t / length).max(0.0);
        phase += freq / RATE as f32;
        out[(start + i) % len] += oscillator(wave, phase) * rise * fall * volume;
    }
}

/// A slow pentatonic pad with sparse plucked notes, tuned per biome.
pub fn ambient(biome_id: &str, root: i32, minor: bool) -> Vec<f32> {
    const CHORD: f32 = 4.8;
    let scale: [i32; 5] = if minor {
        [0, 3, 5, 7, 10]
    } else {
        [0, 2, 4, 7, 9]
    };
    let mut dice = Dice::from_u64(babel_core::babel::seeds::stable_hash(biome_id));
    let mut out = vec![0.0f32; (CHORD * 4.0 * RATE as f32) as usize];
    let progression = if minor { [0, 3, 1, 2] } else { [0, 2, 3, 1] };
    for (bar, &degree) in progression.iter().enumerate() {
        let at = bar as f32 * CHORD;
        for (voice, offset) in [0usize, 2, 4].into_iter().enumerate() {
            let i = degree + offset;
            let note = scale[i % 5] + 12 * (i / 5) as i32;
            let freq = semitone(root, note + 12);
            let volume = 0.075 - voice as f32 * 0.012;
            add_note(&mut out, at, freq, CHORD * 1.35, 1.4, volume, Wave::Sine);
            add_note(
                &mut out,
                at,
                freq * 1.004,
                CHORD * 1.35,
                1.8,
                volume * 0.5,
                Wave::Triangle,
            );
        }
        let bass = semitone(root, scale[degree % 5]);
        add_note(&mut out, at, bass, CHORD * 1.1, 0.6, 0.09, Wave::Sine);
        for beat in 0..8 {
            if dice.chance(0.32) {
                let i = dice.index(5);
                let octave = 12 + 12 * dice.below(2) as i32;
                let freq = semitone(root, scale[i] + octave);
                let when = at + beat as f32 * CHORD / 8.0;
                add_note(&mut out, when, freq, 1.4, 0.03, 0.058, Wave::Triangle);
                // A quiet echo gives the galleries depth.
                add_note(&mut out, when + 0.42, freq, 1.1, 0.04, 0.025, Wave::Sine);
            }
        }
    }
    warm(&mut out, MUSIC_WARMTH, true);
    out
}

/// A driving theme for guardian fights.
pub fn guardian_theme() -> Vec<f32> {
    const BEAT: f32 = 0.2;
    let mut out = vec![0.0f32; (BEAT * 64.0 * RATE as f32) as usize];
    let roots = [0, 0, -4, -2];
    let pattern = [0, 12, 7, 12, 3, 12, 7, 10];
    for step in 0..64 {
        let at = step as f32 * BEAT;
        let root = roots[(step / 16) % 4];
        add_note(
            &mut out,
            at,
            semitone(root, pattern[step % 8]),
            BEAT * 1.4,
            0.014,
            0.12,
            Wave::Brass,
        );
        if step % 4 == 0 {
            add_note(&mut out, at, 62.0, 0.2, 0.008, 0.30, Wave::Sine);
        }
        if step % 8 == 4 {
            // A low wooden knock instead of a metallic hat.
            add_note(&mut out, at, 196.0, 0.11, 0.008, 0.10, Wave::Triangle);
            add_note(&mut out, at, 131.0, 0.14, 0.008, 0.10, Wave::Sine);
        }
        if step % 16 == 0 {
            for n in [0, 3, 7] {
                add_note(
                    &mut out,
                    at,
                    semitone(root, n + 24),
                    BEAT * 15.0,
                    0.4,
                    0.04,
                    Wave::Triangle,
                );
            }
        }
        if step % 16 >= 12 {
            let lead = [12, 15, 19, 22][step % 4];
            add_note(
                &mut out,
                at,
                semitone(root, lead),
                BEAT * 1.2,
                0.016,
                0.065,
                Wave::Reed,
            );
        }
    }
    warm(&mut out, MUSIC_WARMTH, true);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_renders_audible_samples() {
        assert!(Sfx::ALL.len() > 25);
        for sfx in Sfx::ALL {
            let samples = render(&recipe(sfx));
            let peak = samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
            assert!(peak > 0.03 && peak <= 1.5, "{sfx:?} peak {peak}");
            assert!(samples.len() < RATE as usize * 3, "{sfx:?}");
        }
    }

    /// Share of a signal's amplitude (RMS) that lies above `cutoff` hertz,
    /// measured with a steep 4th-order Butterworth high-pass.
    fn treble_share(samples: &[f32], cutoff: f32) -> f32 {
        let w = std::f32::consts::TAU * cutoff / RATE as f32;
        let (sin, cos) = w.sin_cos();
        let mut filtered = samples.to_vec();
        for q in [0.541_f32, 1.307] {
            let alpha = sin / (2.0 * q);
            let a0 = 1.0 + alpha;
            let b = [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0].map(|v| v / a0);
            let a = [-2.0 * cos / a0, (1.0 - alpha) / a0];
            let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
            for s in &mut filtered {
                let y = b[0] * *s + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
                (x2, x1, y2, y1) = (x1, *s, y1, y);
                *s = y;
            }
        }
        let energy = |s: &[f32]| s.iter().map(|v| v * v).sum::<f32>();
        (energy(&filtered) / energy(samples).max(1e-9)).sqrt()
    }

    #[test]
    fn no_effect_is_shrill_or_clicks() {
        let mut harsh = Vec::new();
        for sfx in Sfx::ALL {
            let samples = render(&recipe(sfx));
            let share = treble_share(&samples, 3_000.0);
            if share > 0.16 {
                harsh.push(format!("{sfx:?} {share:.2}"));
            }
            let first = samples.first().copied().unwrap_or(0.0);
            assert!(first.abs() < 0.02, "{sfx:?} starts with a click: {first}");
        }
        assert!(harsh.is_empty(), "too much treble: {harsh:?}");
    }

    #[test]
    fn music_is_soft_on_the_ears() {
        for (name, samples) in [
            ("galería", ambient("agua", 3, false)),
            ("galería menor", ambient("ceniza", 7, true)),
            ("guardián", guardian_theme()),
        ] {
            let share = treble_share(&samples, 3_000.0);
            assert!(share < 0.08, "{name}: {share:.2}");
        }
    }

    #[test]
    fn wav_header_and_music_loops() {
        let music = ambient("agua", 3, false);
        assert!(music.len() > RATE as usize * 10);
        assert!(music.iter().all(|s| s.abs() <= 1.2));
        let bytes = wav(&music);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + music.len() * 2);
        assert!(!guardian_theme().is_empty());
    }
}
