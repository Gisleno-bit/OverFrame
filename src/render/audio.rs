//! Procedural sound effects.
//!
//! No audio files: every sound is synthesised at start-up into a small WAV
//! (22 kHz mono 16-bit) and handed to macroquad's mixer. Hits come in three
//! weights chosen by the hit's magnitude, so a jab *taps* and a smash
//! *cracks*; shields *tock*, powershields *ping*, KOs *boom*. Triggering is
//! driven by the simulation's effect list (`GameState::fx`, each with the
//! frame it was born on), which keeps sound a pure function of the sim like
//! everything else in the renderer — rollbacks never double-fire.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};

use crate::sim::{FxKind, GameState};

const RATE: u32 = 22_050;

pub struct AudioBank {
    hit: [Sound; 3],
    shield: Sound,
    powershield: Sound,
    blast: Sound,
    land: Sound,
    jump: Sound,
    swing: [Sound; 2],
    tech: Sound,
    dash: Sound,
    /// (born frame, kind tag) of effects already voiced, newest last.
    played: Vec<(u64, u8)>,
    pub volume: f32,
}

/// Deterministic noise (xorshift) so the bank is identical every run.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Render `secs` of audio with `f(t, noise) -> sample` into WAV bytes.
fn synth(secs: f32, mut f: impl FnMut(f32, &mut Noise) -> f32) -> Vec<u8> {
    let n = (secs * RATE as f32) as usize;
    let mut noise = Noise(0x9E37_79B9);
    let mut samples = Vec::with_capacity(n);
    // One-pole low-pass state for the noise, so "thuds" are not hiss.
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let v = f(t, &mut noise).clamp(-1.0, 1.0);
        samples.push((v * 32767.0) as i16);
    }
    wav(&samples)
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[inline]
fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

/// Low-passed noise generator state helper.
struct Lp(f32);
impl Lp {
    fn feed(&mut self, x: f32, k: f32) -> f32 {
        self.0 += (x - self.0) * k;
        self.0
    }
}

fn sine(t: f32, hz: f32) -> f32 {
    (t * hz * std::f32::consts::TAU).sin()
}

/// Build every sound. `None` if the audio backend is unavailable (the game
/// simply plays silent).
pub async fn load() -> Option<AudioBank> {
    async fn s(bytes: Vec<u8>) -> Option<Sound> {
        load_sound_from_bytes(&bytes).await.ok()
    }
    // --- hits: tap / thud / crack
    let hit0 = synth(0.09, |t, n| {
        let e = env(t, 0.002, 0.03);
        (sine(t, 520.0 - t * 2400.0) * 0.5 + n.next() * 0.5) * e
    });
    let hit1 = synth(0.16, |t, n| {
        let e = env(t, 0.003, 0.05);
        let body = sine(t, 170.0 - t * 300.0) * 0.7;
        let crack = n.next() * env(t, 0.001, 0.02) * 0.8;
        (body + crack) * e
    });
    let mut lp = Lp(0.0);
    let hit2 = synth(0.28, move |t, n| {
        let e = env(t, 0.004, 0.09);
        let boom = sine(t, 95.0 - t * 120.0) * 0.8;
        let crack = lp.feed(n.next(), 0.45) * env(t, 0.001, 0.03) * 1.2;
        let ring = sine(t, 1400.0) * env(t, 0.0, 0.015) * 0.3;
        (boom + crack + ring) * e
    });
    let shield = synth(0.08, |t, n| {
        let e = env(t, 0.001, 0.02);
        (sine(t, 880.0) * 0.6 + sine(t, 1320.0) * 0.3 + n.next() * 0.2) * e
    });
    let powershield = synth(0.18, |t, _| {
        let e = env(t, 0.001, 0.06);
        (sine(t, 1400.0) * 0.5 + sine(t, 2100.0) * 0.4 + sine(t, 2800.0) * 0.2) * e
    });
    let mut lp2 = Lp(0.0);
    let blast = synth(0.5, move |t, n| {
        let e = env(t, 0.005, 0.18);
        let boom = sine(t, 60.0 - t * 40.0) * 0.9;
        let rumble = lp2.feed(n.next(), 0.2) * 0.9;
        (boom + rumble) * e
    });
    let mut lp3 = Lp(0.0);
    let land = synth(0.09, move |t, n| {
        let e = env(t, 0.002, 0.03);
        (lp3.feed(n.next(), 0.25) * 0.9 + sine(t, 120.0) * 0.3) * e
    });
    let mut lp4 = Lp(0.0);
    let jump = synth(0.08, move |t, n| {
        let e = env(t, 0.01, 0.03);
        lp4.feed(n.next(), 0.35 + t * 4.0) * 0.6 * e
    });
    let mut lp5 = Lp(0.0);
    let swing0 = synth(0.1, move |t, n| {
        let e = env(t, 0.02, 0.035);
        lp5.feed(n.next(), 0.5) * 0.45 * e
    });
    let mut lp6 = Lp(0.0);
    let swing1 = synth(0.16, move |t, n| {
        let e = env(t, 0.03, 0.05);
        (lp6.feed(n.next(), 0.3) * 0.6 + sine(t, 140.0) * 0.15) * e
    });
    let tech = synth(0.06, |t, n| {
        let e = env(t, 0.001, 0.015);
        (sine(t, 700.0) * 0.5 + n.next() * 0.5) * e
    });
    let mut lp7 = Lp(0.0);
    let dash = synth(0.07, move |t, n| {
        let e = env(t, 0.015, 0.025);
        lp7.feed(n.next(), 0.4) * 0.35 * e
    });

    Some(AudioBank {
        hit: [s(hit0).await?, s(hit1).await?, s(hit2).await?],
        shield: s(shield).await?,
        powershield: s(powershield).await?,
        blast: s(blast).await?,
        land: s(land).await?,
        jump: s(jump).await?,
        swing: [s(swing0).await?, s(swing1).await?],
        tech: s(tech).await?,
        dash: s(dash).await?,
        played: Vec::new(),
        volume: 0.8,
    })
}

impl AudioBank {
    fn play(&self, sound: &Sound, volume: f32) {
        play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume: (volume * self.volume).clamp(0.0, 1.0),
            },
        );
    }

    /// Voice every effect born since the last call. Call once per drawn
    /// frame with the state being displayed.
    pub fn update(&mut self, gs: &GameState) {
        // Forget entries older than the fx lifetime (and everything if the
        // sim went backwards, e.g. a rollback / new match).
        self.played
            .retain(|(born, _)| *born + 40 >= gs.frame && *born <= gs.frame);
        for fx in &gs.fx {
            let tag = fx.kind as u8;
            if self.played.iter().any(|(b, k)| *b == fx.born && *k == tag) {
                continue;
            }
            // Only voice effects from the last few frames (skip stale ones
            // after a long rollback or a stalled draw).
            if fx.born + 6 < gs.frame {
                continue;
            }
            self.played.push((fx.born, tag));
            if self.played.len() > 96 {
                self.played.remove(0);
            }
            let m = fx.magnitude;
            match fx.kind {
                FxKind::Hit => {
                    let (i, v) = if m < 9.0 {
                        (0, 0.7)
                    } else if m < 18.0 {
                        (1, 0.85)
                    } else {
                        (2, 1.0)
                    };
                    self.play(&self.hit[i], v);
                }
                FxKind::Shield => self.play(&self.shield, 0.7),
                FxKind::Powershield => self.play(&self.powershield, 0.9),
                FxKind::Blast => self.play(&self.blast, 1.0),
                FxKind::Land => self.play(&self.land, (0.3 + m * 0.05).min(0.8)),
                FxKind::Jump => self.play(&self.jump, 0.35),
                FxKind::Swing => {
                    let i = if m >= 12.0 { 1 } else { 0 };
                    self.play(&self.swing[i], 0.5);
                }
                FxKind::Tech => self.play(&self.tech, 0.7),
                FxKind::Dust => {
                    // Dust alone (dash start) gets a soft scuff; landings
                    // already carry their own thump.
                    if fx.dir.y.abs() < 0.5 {
                        self.play(&self.dash, 0.3);
                    }
                }
            }
        }
    }
}
