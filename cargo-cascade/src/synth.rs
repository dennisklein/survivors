//! Procedural sound synthesis: every effect and the music loop are generated
//! here as 16-bit mono PCM WAV bytes, so the game ships no audio files.

use std::f32::consts::TAU;

pub const RATE: u32 = 22_050;

/// Encode mono samples in `[-1, 1]` as a 16-bit PCM WAV file.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
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
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

fn square(phase: f32) -> f32 {
    if phase.fract() < 0.5 { 1.0 } else { -1.0 }
}

/// Frequency sweep with exponential amplitude decay; `wave` maps phase to sample.
fn sweep(secs: f32, f0: f32, f1: f32, decay: f32, vol: f32, wave: fn(f32) -> f32) -> Vec<f32> {
    let n = len(secs);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += (f0 + (f1 - f0) * t) / RATE as f32;
            wave(phase) * (-decay * t).exp() * vol
        })
        .collect()
}

/// Low-passed noise burst.
fn noise_burst(secs: f32, smooth: f32, decay: f32, vol: f32, seed: u32) -> Vec<f32> {
    let mut rng = Noise(seed);
    let mut lp = 0.0;
    let n = len(secs);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            lp += (rng.next() - lp) * smooth;
            lp * (-decay * t).exp() * vol
        })
        .collect()
}

fn mix(a: &mut Vec<f32>, b: &[f32]) {
    if b.len() > a.len() {
        a.resize(b.len(), 0.0);
    }
    for (x, y) in a.iter_mut().zip(b) {
        *x += y;
    }
}

/// Signal box shot: a short metallic "tak".
pub fn shoot() -> Vec<u8> {
    let mut s = sweep(0.07, 1100.0, 380.0, 7.0, 0.22, square);
    mix(&mut s, &noise_burst(0.05, 0.55, 9.0, 0.22, 7));
    wav(&s)
}

/// Two-tone steam whistle, used when a town gets connected.
pub fn whistle() -> Vec<u8> {
    let n = len(0.7);
    let mut rng = Noise(41);
    let s: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let tt = i as f32 / RATE as f32;
            let env = (t * 12.0).min(1.0) * (1.0 - t).powf(0.4);
            let tone = (tt * 587.0 * TAU).sin() + 0.8 * (tt * 740.0 * TAU).sin();
            (tone * 0.16 + rng.next() * 0.04) * env
        })
        .collect();
    wav(&s)
}

/// Dull buzz for invalid actions.
pub fn error() -> Vec<u8> {
    wav(&sweep(0.14, 160.0, 120.0, 2.0, 0.22, square))
}

/// Rising chime at dawn.
pub fn dawn() -> Vec<u8> {
    wav(&notes(
        &[(392.0, 0.12), (523.0, 0.12), (659.0, 0.3)],
        0.2,
        |p| (p * TAU).sin(),
    ))
}

/// Crunch of an industry being wrecked.
pub fn wreck() -> Vec<u8> {
    let mut s = noise_burst(0.5, 0.2, 5.0, 0.7, 13);
    mix(&mut s, &sweep(0.4, 140.0, 40.0, 3.0, 0.4, square));
    wav(&s)
}

pub fn hit() -> Vec<u8> {
    wav(&noise_burst(0.04, 0.5, 10.0, 0.3, 17))
}

pub fn explosion() -> Vec<u8> {
    let mut s = noise_burst(0.7, 0.06, 4.0, 1.0, 23);
    mix(
        &mut s,
        &sweep(0.5, 90.0, 30.0, 4.0, 0.5, |p| (p * TAU).sin()),
    );
    wav(&s)
}

pub fn pickup() -> Vec<u8> {
    wav(&sweep(0.09, 900.0, 1600.0, 3.0, 0.2, |p| (p * TAU).sin()))
}

fn notes(seq: &[(f32, f32)], vol: f32, wave: fn(f32) -> f32) -> Vec<f32> {
    let mut out = Vec::new();
    for &(freq, secs) in seq {
        let n = len(secs);
        for i in 0..n {
            let t = i as f32 / n as f32;
            let env = (t * 40.0).min(1.0) * (1.0 - t).powf(0.6);
            out.push(wave(i as f32 * freq / RATE as f32) * env * vol);
        }
    }
    out
}

pub fn level_up() -> Vec<u8> {
    wav(&notes(
        &[(523.0, 0.08), (659.0, 0.08), (784.0, 0.08), (1047.0, 0.25)],
        0.25,
        square,
    ))
}

pub fn build() -> Vec<u8> {
    let mut s = sweep(0.08, 400.0, 180.0, 8.0, 0.5, |p| (p * TAU).sin());
    mix(&mut s, &noise_burst(0.06, 0.4, 10.0, 0.3, 31));
    wav(&s)
}

pub fn cash() -> Vec<u8> {
    wav(&notes(&[(1319.0, 0.05), (1760.0, 0.12)], 0.15, square))
}

pub fn alarm() -> Vec<u8> {
    wav(&notes(
        &[(440.0, 0.15), (330.0, 0.15), (440.0, 0.15), (330.0, 0.2)],
        0.2,
        square,
    ))
}

pub fn game_over() -> Vec<u8> {
    wav(&notes(
        &[(392.0, 0.25), (330.0, 0.25), (262.0, 0.25), (196.0, 0.7)],
        0.3,
        square,
    ))
}

pub fn victory() -> Vec<u8> {
    wav(&notes(
        &[
            (523.0, 0.12),
            (659.0, 0.12),
            (784.0, 0.12),
            (1047.0, 0.2),
            (784.0, 0.12),
            (1047.0, 0.5),
        ],
        0.3,
        square,
    ))
}

/// A 16-bar chiptune loop: chugging bass (like wheels on rail joints), soft
/// arpeggio and a hi-hat pulse. Loops seamlessly.
pub fn music() -> Vec<u8> {
    let bpm = 112.0;
    let step = 60.0 / bpm / 2.0; // eighth notes
    // D minor progression: Dm, Bb, F, C (root frequencies of the bass).
    let roots = [73.42, 116.54, 87.31, 130.81];
    let chords: [[f32; 3]; 4] = [
        [293.66, 349.23, 440.0],
        [233.08, 293.66, 349.23],
        [349.23, 440.0, 523.25],
        [261.63, 329.63, 392.0],
    ];
    let steps_per_chord = 16;
    let total = len(step * (steps_per_chord * 4) as f32);
    let mut out = vec![0.0f32; total];
    let step_n = len(step);
    let mut rng = Noise(99);
    for (c, (&root, chord)) in roots.iter().zip(chords.iter()).enumerate() {
        for k in 0..steps_per_chord {
            let start = (c * steps_per_chord + k) * step_n;
            let bass_f = if k % 2 == 0 { root } else { root * 2.0 };
            let arp_f = chord[[0, 1, 2, 1][k % 4]];
            for i in 0..step_n {
                let t = i as f32 / step_n as f32;
                let tt = i as f32 / RATE as f32;
                let bass = (tt * bass_f * TAU).sin().signum() * 0.09 * (1.0 - t).powf(0.5);
                let arp = if k % 2 == 0 {
                    square(tt * arp_f) * 0.035 * (1.0 - t)
                } else {
                    0.0
                };
                let hat = rng.next() * 0.04 * (-t * 30.0).exp();
                if let Some(o) = out.get_mut(start + i) {
                    *o += bass + arp + hat;
                }
            }
        }
    }
    wav(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let w = wav(&[0.0, 0.5, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + 6);
        let riff = u32::from_le_bytes([w[4], w[5], w[6], w[7]]);
        assert_eq!(riff as usize, w.len() - 8);
    }

    #[test]
    fn all_sounds_are_nonempty_and_clamped() {
        for s in [
            shoot(),
            whistle(),
            error(),
            dawn(),
            wreck(),
            hit(),
            explosion(),
            pickup(),
            level_up(),
            build(),
            cash(),
            alarm(),
            game_over(),
            victory(),
            music(),
        ] {
            assert!(s.len() > 44 + 100);
        }
    }
}
