//! Procedural sound synthesis: every effect and the music loop are generated
//! as 16-bit mono PCM WAV bytes, so no audio files need to be shipped.

use std::f32::consts::TAU;

pub const RATE: u32 = 22_050;

/// Encode samples in [-1, 1] as a 16-bit mono PCM WAV file.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
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
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_000.0) as i16;
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

/// Tone sweeping from `f0` to `f1` with an exponential decay envelope.
fn sweep(secs: f32, f0: f32, f1: f32, decay: f32, square: bool, vol: f32) -> Vec<f32> {
    let n = len(secs);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let f = f0 + (f1 - f0) * t;
            phase += f / RATE as f32;
            let s = if square {
                if phase.fract() < 0.5 { 1.0 } else { -1.0 }
            } else {
                (phase * TAU).sin()
            };
            s * (-t * decay).exp() * vol
        })
        .collect()
}

fn noise_burst(secs: f32, decay: f32, smooth: f32, vol: f32, seed: u32) -> Vec<f32> {
    let n = len(secs);
    let mut rng = Noise(seed);
    let mut lp = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            lp += (rng.next() - lp) * smooth;
            lp * (-t * decay).exp() * vol
        })
        .collect()
}

fn mix(a: &mut Vec<f32>, b: &[f32], offset: usize) {
    if a.len() < offset + b.len() {
        a.resize(offset + b.len(), 0.0);
    }
    for (i, s) in b.iter().enumerate() {
        a[offset + i] += s;
    }
}

pub fn laser() -> Vec<u8> {
    wav(&sweep(0.09, 1400.0, 500.0, 5.0, true, 0.25))
}
pub fn cannon() -> Vec<u8> {
    let mut s = sweep(0.25, 220.0, 60.0, 6.0, false, 0.6);
    mix(&mut s, &noise_burst(0.2, 8.0, 0.3, 0.5, 7), 0);
    wav(&s)
}
pub fn rocket() -> Vec<u8> {
    let mut s = noise_burst(0.35, 4.0, 0.15, 0.6, 11);
    mix(&mut s, &sweep(0.3, 300.0, 900.0, 4.0, true, 0.12), 0);
    wav(&s)
}
pub fn hit() -> Vec<u8> {
    wav(&noise_burst(0.07, 9.0, 0.6, 0.45, 3))
}
pub fn explosion() -> Vec<u8> {
    let mut s = noise_burst(0.7, 5.0, 0.08, 1.0, 21);
    mix(&mut s, &sweep(0.5, 120.0, 35.0, 5.0, false, 0.6), 0);
    wav(&s)
}
pub fn big_explosion() -> Vec<u8> {
    let mut s = noise_burst(1.6, 3.5, 0.04, 1.0, 99);
    mix(&mut s, &sweep(1.2, 80.0, 25.0, 3.0, false, 0.8), 0);
    wav(&s)
}
pub fn pickup() -> Vec<u8> {
    wav(&sweep(0.08, 900.0, 1800.0, 3.0, false, 0.3))
}
pub fn level_up() -> Vec<u8> {
    let mut s = Vec::new();
    for (i, f) in [523.0, 659.0, 784.0, 1046.0].iter().enumerate() {
        mix(&mut s, &sweep(0.25, *f, *f, 4.0, true, 0.18), len(0.08) * i);
    }
    wav(&s)
}
pub fn build() -> Vec<u8> {
    let mut s = sweep(0.12, 330.0, 330.0, 3.0, true, 0.2);
    mix(&mut s, &sweep(0.18, 495.0, 495.0, 3.0, true, 0.2), len(0.1));
    wav(&s)
}
pub fn spawn() -> Vec<u8> {
    wav(&sweep(0.2, 200.0, 700.0, 2.0, false, 0.3))
}
pub fn alarm() -> Vec<u8> {
    let mut s = Vec::new();
    for i in 0..3 {
        mix(
            &mut s,
            &sweep(0.15, 880.0, 660.0, 1.0, true, 0.2),
            len(0.2) * i,
        );
    }
    wav(&s)
}
pub fn deny() -> Vec<u8> {
    wav(&sweep(0.15, 160.0, 120.0, 2.0, true, 0.2))
}
pub fn game_over() -> Vec<u8> {
    let mut s = Vec::new();
    for (i, f) in [392.0, 330.0, 262.0, 196.0].iter().enumerate() {
        mix(
            &mut s,
            &sweep(0.45, *f, *f * 0.97, 2.5, true, 0.2),
            len(0.3) * i,
        );
    }
    wav(&s)
}
pub fn victory() -> Vec<u8> {
    let mut s = Vec::new();
    for (i, f) in [523.0, 659.0, 784.0, 659.0, 784.0, 1046.0]
        .iter()
        .enumerate()
    {
        mix(&mut s, &sweep(0.3, *f, *f, 2.5, true, 0.18), len(0.15) * i);
    }
    wav(&s)
}

/// An 8-bar industrial loop: pulsing bass, hi-hat ticks and a sparse arpeggio.
pub fn music() -> Vec<u8> {
    let bpm = 120.0;
    let step = 60.0 / bpm / 2.0; // eighth notes
    let steps = 64;
    let mut s = vec![0.0; len(step * steps as f32)];
    let bass = [55.0, 55.0, 65.4, 49.0, 55.0, 55.0, 73.4, 61.7];
    let arp = [220.0, 261.6, 329.6, 261.6, 196.0, 246.9, 293.7, 246.9];
    for i in 0..steps {
        let at = len(step * i as f32);
        let bar = i / 8;
        let b = bass[bar % bass.len()];
        mix(&mut s, &sweep(step * 0.9, b, b, 3.0, true, 0.12), at);
        if i % 2 == 1 {
            mix(
                &mut s,
                &noise_burst(0.04, 10.0, 0.9, 0.05, 5 + i as u32),
                at,
            );
        }
        if i % 8 == 0 || i % 8 == 6 {
            mix(&mut s, &sweep(0.15, 110.0, 40.0, 6.0, false, 0.35), at);
        }
        if bar >= 2 && i % 3 == 0 {
            let f = arp[(i / 3) % arp.len()];
            mix(&mut s, &sweep(step * 1.5, f, f, 4.0, false, 0.07), at);
        }
    }
    s.truncate(len(step * steps as f32));
    wav(&s)
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
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 6);
    }

    #[test]
    fn all_sounds_are_non_empty() {
        for s in [
            laser(),
            cannon(),
            rocket(),
            hit(),
            explosion(),
            big_explosion(),
            pickup(),
            level_up(),
            build(),
            spawn(),
            alarm(),
            deny(),
            game_over(),
            victory(),
            music(),
        ] {
            assert!(s.len() > 1000);
        }
    }
}
