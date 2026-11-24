//! A tiny sfxr-style synthesiser plus procedural ambient music. No audio files.

use babel_core::babel::seeds::Dice;
use babel_core::game::events::Sfx;

pub const RATE: u32 = 22_050;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    Square,
    Saw,
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
    pub duty: f32,
    /// One-pole low-pass coefficient in `(0, 1]`.
    pub lowpass: f32,
    pub volume: f32,
    pub delay: f32,
}

const BASE: Voice = Voice {
    wave: Wave::Square,
    freq: 440.0,
    slide: 0.0,
    attack: 0.004,
    sustain: 0.05,
    decay: 0.1,
    vibrato: 0.0,
    vibrato_rate: 0.0,
    arp: 1.0,
    arp_at: 0.0,
    duty: 0.5,
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
            duty: 0.3,
            ..tone(wave, f, gap * 0.6, decay, volume)
        })
        .collect()
}

/// The recipe of every sound effect.
pub fn recipe(sfx: Sfx) -> Vec<Voice> {
    use Wave::{Saw, Sine, Square, Triangle};
    let v = |wave, freq, slide: f32, sustain, decay, volume| Voice {
        slide,
        ..tone(wave, freq, sustain, decay, volume)
    };
    match sfx {
        Sfx::Step => vec![noise(900.0, 0.008, 0.03, 0.10, 0.16)],
        Sfx::StepWater => vec![
            noise(2600.0, 0.012, 0.07, 0.12, 0.35),
            v(Sine, 520.0, 3.0, 0.01, 0.04, 0.06),
        ],
        Sfx::Swing => vec![noise(5200.0, 0.02, 0.08, 0.16, 0.45)],
        Sfx::SwingHeavy => vec![
            noise(3400.0, 0.04, 0.12, 0.24, 0.35),
            v(Triangle, 180.0, -2.0, 0.03, 0.08, 0.14),
        ],
        Sfx::Charge => vec![v(Square, 330.0, 2.2, 0.12, 0.08, 0.16)],
        Sfx::Hit => vec![
            noise(1800.0, 0.02, 0.07, 0.34, 0.5),
            v(Square, 190.0, -4.0, 0.02, 0.06, 0.2),
        ],
        Sfx::HitArmor => vec![
            v(Square, 760.0, -0.6, 0.015, 0.09, 0.2),
            v(Triangle, 1140.0, -0.5, 0.01, 0.12, 0.16),
        ],
        Sfx::EnemyDie => vec![
            v(Square, 420.0, -3.6, 0.05, 0.18, 0.24),
            noise(900.0, 0.04, 0.14, 0.2, 0.3),
        ],
        Sfx::EliteDie => vec![
            v(Saw, 300.0, -1.8, 0.16, 0.4, 0.3),
            noise(700.0, 0.12, 0.4, 0.28, 0.25),
            Voice {
                delay: 0.2,
                ..v(Triangle, 880.0, 0.0, 0.08, 0.3, 0.2)
            },
        ],
        Sfx::Dash => vec![
            noise(6000.0, 0.03, 0.14, 0.16, 0.5),
            v(Sine, 360.0, 3.4, 0.03, 0.09, 0.14),
        ],
        Sfx::Hurt => vec![
            v(Saw, 220.0, -2.8, 0.05, 0.14, 0.34),
            noise(500.0, 0.03, 0.08, 0.22, 0.2),
        ],
        Sfx::Death => vec![
            v(Triangle, 330.0, -1.2, 0.3, 0.6, 0.34),
            Voice {
                delay: 0.18,
                ..v(Saw, 110.0, -0.8, 0.25, 0.55, 0.22)
            },
        ],
        Sfx::HarvestWood => vec![
            v(Triangle, 170.0, -2.0, 0.012, 0.07, 0.4),
            noise(1100.0, 0.008, 0.04, 0.18, 0.25),
        ],
        Sfx::HarvestStone => vec![
            noise(2400.0, 0.01, 0.06, 0.3, 0.3),
            v(Square, 130.0, -1.0, 0.008, 0.05, 0.16),
        ],
        Sfx::HarvestMetal => vec![
            v(Square, 640.0, 0.0, 0.01, 0.13, 0.16),
            v(Sine, 1520.0, 0.0, 0.01, 0.18, 0.18),
        ],
        Sfx::HarvestGlass => vec![
            v(Sine, 1760.0, 0.4, 0.008, 0.14, 0.2),
            v(Triangle, 2360.0, 0.0, 0.008, 0.1, 0.14),
            noise(8000.0, 0.005, 0.05, 0.1, 0.8),
        ],
        Sfx::HarvestPlant => vec![noise(3200.0, 0.015, 0.09, 0.2, 0.2)],
        Sfx::NodeBreak => vec![
            noise(1200.0, 0.05, 0.18, 0.32, 0.3),
            v(Triangle, 240.0, -2.6, 0.03, 0.12, 0.2),
        ],
        Sfx::Pickup => vec![Voice {
            arp: 1.5,
            arp_at: 0.04,
            duty: 0.25,
            ..v(Square, 780.0, 0.0, 0.06, 0.06, 0.16)
        }],
        Sfx::DropCommon => vec![v(Triangle, 520.0, 1.0, 0.03, 0.08, 0.2)],
        Sfx::DropRare => arpeggio(Square, &[660.0, 880.0, 1175.0], 0.05, 0.14, 0.14),
        Sfx::DropEpic => arpeggio(Square, &[523.0, 659.0, 880.0, 1319.0], 0.06, 0.18, 0.15),
        Sfx::DropLegendary => arpeggio(
            Triangle,
            &[523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0],
            0.07,
            0.32,
            0.22,
        ),
        Sfx::DropCanonical => arpeggio(
            Sine,
            &[659.0, 988.0, 1319.0, 1175.0, 1760.0, 1976.0, 2637.0],
            0.08,
            0.5,
            0.2,
        ),
        Sfx::Craft => vec![
            v(Square, 300.0, 0.0, 0.02, 0.05, 0.2),
            Voice {
                delay: 0.1,
                ..v(Square, 400.0, 0.0, 0.02, 0.05, 0.2)
            },
            Voice {
                delay: 0.2,
                ..v(Sine, 900.0, 0.0, 0.05, 0.2, 0.25)
            },
        ],
        Sfx::Build => vec![
            noise(600.0, 0.08, 0.25, 0.3, 0.18),
            Voice {
                delay: 0.14,
                ..v(Triangle, 392.0, 0.0, 0.08, 0.25, 0.28)
            },
            Voice {
                delay: 0.26,
                ..v(Triangle, 587.0, 0.0, 0.1, 0.3, 0.28)
            },
        ],
        Sfx::LevelUp => arpeggio(
            Square,
            &[392.0, 523.0, 659.0, 784.0, 1047.0],
            0.08,
            0.26,
            0.18,
        ),
        Sfx::SkillLearn => arpeggio(Triangle, &[440.0, 660.0, 990.0], 0.07, 0.22, 0.26),
        Sfx::ReadPage => vec![noise(4200.0, 0.03, 0.18, 0.14, 0.12)],
        Sfx::WordFound => arpeggio(Sine, &[988.0, 1319.0], 0.09, 0.2, 0.2),
        Sfx::LegendaryFanfare => {
            let mut out = arpeggio(
                Square,
                &[392.0, 392.0, 523.0, 659.0, 784.0, 1047.0],
                0.1,
                0.3,
                0.16,
            );
            out.push(Voice {
                delay: 0.5,
                vibrato: 0.012,
                vibrato_rate: 6.0,
                ..v(Triangle, 523.0, 0.0, 0.35, 0.6, 0.26)
            });
            out
        }
        Sfx::Inscribe => vec![
            noise(5000.0, 0.1, 0.1, 0.1, 0.1),
            Voice {
                delay: 0.14,
                vibrato: 0.03,
                vibrato_rate: 11.0,
                ..v(Sine, 700.0, 0.7, 0.14, 0.3, 0.26)
            },
        ],
        Sfx::LampLight => vec![
            v(Sine, 294.0, 1.0, 0.15, 0.35, 0.26),
            noise(1500.0, 0.05, 0.25, 0.14, 0.18),
        ],
        Sfx::TorchPlace => vec![noise(1400.0, 0.04, 0.2, 0.22, 0.22)],
        Sfx::Teleport => vec![
            Voice {
                vibrato: 0.08,
                vibrato_rate: 18.0,
                ..v(Sine, 220.0, 2.4, 0.25, 0.25, 0.26)
            },
            noise(3000.0, 0.15, 0.25, 0.08, 0.3),
        ],
        Sfx::Stairs => arpeggio(Triangle, &[392.0, 330.0, 262.0, 196.0], 0.09, 0.14, 0.26),
        Sfx::Door => vec![
            noise(500.0, 0.05, 0.16, 0.18, 0.14),
            v(Sine, 160.0, 0.8, 0.04, 0.12, 0.16),
        ],
        Sfx::ChestOpen => {
            let mut out = vec![noise(800.0, 0.04, 0.1, 0.2, 0.2)];
            out.extend(
                arpeggio(Square, &[587.0, 784.0, 1175.0], 0.08, 0.2, 0.14)
                    .into_iter()
                    .map(|mut v| {
                        v.delay += 0.08;
                        v
                    }),
            );
            out
        }
        Sfx::UiMove => vec![v(Square, 520.0, 0.0, 0.012, 0.025, 0.07)],
        Sfx::UiConfirm => vec![Voice {
            arp: 1.5,
            arp_at: 0.035,
            ..v(Square, 540.0, 0.0, 0.05, 0.06, 0.12)
        }],
        Sfx::UiBack => vec![Voice {
            arp: 0.72,
            arp_at: 0.035,
            ..v(Square, 470.0, 0.0, 0.05, 0.05, 0.11)
        }],
        Sfx::UiError => vec![v(Saw, 150.0, 0.0, 0.1, 0.07, 0.18)],
        Sfx::Throw => vec![
            noise(4800.0, 0.02, 0.1, 0.14, 0.4),
            v(Triangle, 700.0, -2.2, 0.02, 0.07, 0.12),
        ],
        Sfx::Explosion => vec![
            noise(420.0, 0.08, 0.42, 0.45, 0.22),
            v(Sine, 90.0, -1.0, 0.06, 0.3, 0.36),
        ],
        Sfx::BossRoar => vec![
            Voice {
                vibrato: 0.14,
                vibrato_rate: 22.0,
                ..v(Saw, 96.0, -0.35, 0.4, 0.45, 0.34)
            },
            noise(300.0, 0.3, 0.4, 0.24, 0.12),
        ],
        Sfx::Freeze => vec![v(Sine, 1900.0, -1.4, 0.03, 0.22, 0.14)],
        Sfx::Poison => vec![Voice {
            vibrato: 0.2,
            vibrato_rate: 14.0,
            ..v(Square, 210.0, -0.6, 0.08, 0.14, 0.14)
        }],
        Sfx::Burn => vec![noise(2200.0, 0.06, 0.24, 0.18, 0.28)],
        Sfx::Locked => vec![
            v(Square, 180.0, 0.0, 0.03, 0.05, 0.18),
            Voice {
                delay: 0.09,
                ..v(Square, 150.0, 0.0, 0.03, 0.07, 0.18)
            },
        ],
        Sfx::Heal => arpeggio(Sine, &[523.0, 659.0, 784.0], 0.07, 0.24, 0.24),
        Sfx::Victory => arpeggio(Triangle, &[523.0, 784.0, 659.0, 1047.0], 0.11, 0.3, 0.24),
    }
}

fn oscillator(wave: Wave, phase: f32, duty: f32) -> f32 {
    let p = phase.fract();
    match wave {
        Wave::Square => {
            if p < duty {
                0.7
            } else {
                -0.7
            }
        }
        Wave::Saw => 1.0 - 2.0 * p,
        Wave::Sine => (p * std::f32::consts::TAU).sin(),
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
        Wave::Noise => 0.0,
    }
}

/// Mixes the voices of one effect into mono samples.
pub fn render(voices: &[Voice]) -> Vec<f32> {
    let length = voices
        .iter()
        .map(|v| v.delay + v.attack + v.sustain + v.decay)
        .fold(0.0f32, f32::max);
    let mut out = vec![0.0f32; (length * RATE as f32) as usize + 1];
    let dt = 1.0 / RATE as f32;
    for (index, v) in voices.iter().enumerate() {
        let mut dice = Dice::from_u64(v.freq.to_bits() as u64 + index as u64);
        let start = (v.delay * RATE as f32) as usize;
        let total = v.attack + v.sustain + v.decay;
        let samples = (total * RATE as f32) as usize;
        let (mut phase, mut held, mut filtered) = (0.0f32, 0.0f32, 0.0f32);
        let mut cycle = -1.0f32;
        for i in 0..samples {
            let t = i as f32 * dt;
            let envelope = if t < v.attack {
                t / v.attack.max(dt)
            } else if t < v.attack + v.sustain {
                1.0
            } else {
                let k = 1.0 - (t - v.attack - v.sustain) / v.decay.max(dt);
                k.max(0.0) * k.max(0.0)
            };
            let mut freq = v.freq * (v.slide * t).exp2();
            if v.arp_at > 0.0 && t >= v.arp_at {
                freq *= v.arp;
            }
            if v.vibrato > 0.0 {
                freq *= 1.0 + v.vibrato * (t * v.vibrato_rate * std::f32::consts::TAU).sin();
            }
            phase += freq.clamp(20.0, 11_000.0) * dt;
            let raw = if v.wave == Wave::Noise {
                // Sample-and-hold noise pitched by the frequency.
                if phase.floor() != cycle {
                    cycle = phase.floor();
                    held = dice.between(-1.0, 1.0);
                }
                held
            } else {
                oscillator(v.wave, phase, v.duty)
            };
            filtered += (raw - filtered) * v.lowpass.clamp(0.01, 1.0);
            if let Some(slot) = out.get_mut(start + i) {
                *slot += filtered * envelope * v.volume;
            }
        }
    }
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
        out[(start + i) % len] += oscillator(wave, phase, 0.5) * rise * fall * volume;
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
            if dice.chance(0.36) {
                let i = dice.index(5);
                let octave = 24 + 12 * dice.below(2) as i32;
                let freq = semitone(root, scale[i] + octave);
                let when = at + beat as f32 * CHORD / 8.0;
                add_note(&mut out, when, freq, 1.3, 0.008, 0.055, Wave::Triangle);
                // A quiet echo gives the galleries depth.
                add_note(&mut out, when + 0.42, freq, 1.0, 0.01, 0.022, Wave::Sine);
            }
        }
    }
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
            0.004,
            0.11,
            Wave::Saw,
        );
        if step % 4 == 0 {
            add_note(&mut out, at, 62.0, 0.2, 0.002, 0.30, Wave::Sine);
        }
        if step % 8 == 4 {
            for k in 0..5 {
                add_note(
                    &mut out,
                    at,
                    1700.0 + k as f32 * 437.0,
                    0.09,
                    0.001,
                    0.022,
                    Wave::Square,
                );
            }
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
            let lead = [24, 27, 31, 34][step % 4];
            add_note(
                &mut out,
                at,
                semitone(root, lead),
                BEAT * 1.2,
                0.004,
                0.05,
                Wave::Square,
            );
        }
    }
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
