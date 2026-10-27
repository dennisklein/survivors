//! Procedural sound synthesis: every effect and the music loop are generated
//! here as mono 16-bit PCM WAV bytes, so the game ships no audio files.

use crate::rng::Rng;
use std::f32::consts::TAU;

pub const SAMPLE_RATE: u32 = 22_050;
const SR: f32 = SAMPLE_RATE as f32;

/// Encode samples in `[-1, 1]` as a mono 16-bit PCM WAV file.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
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

fn len(secs: f32) -> usize {
    (secs * SR) as usize
}

fn square(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty { 1.0 } else { -1.0 }
}

fn triangle(phase: f32) -> f32 {
    let p = phase.fract();
    4.0 * (p - 0.5).abs() - 1.0
}

fn midi(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// Pitch sweep from `f0` to `f1` with an exponential decay envelope.
fn sweep(secs: f32, f0: f32, f1: f32, decay: f32, wave: fn(f32) -> f32, gain: f32) -> Vec<f32> {
    let n = len(secs);
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += (f0 + (f1 - f0) * t) / SR;
            wave(phase) * (-decay * t).exp() * (1.0 - t) * gain
        })
        .collect()
}

fn noise(secs: f32, decay: f32, lowpass: f32, gain: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let n = len(secs);
    let mut y = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = rng.range(-1.0, 1.0);
            y += (x - y) * lowpass;
            y * (-decay * t).exp() * gain
        })
        .collect()
}

fn mix_into(dst: &mut Vec<f32>, src: &[f32], offset: usize) {
    if dst.len() < offset + src.len() {
        dst.resize(offset + src.len(), 0.0);
    }
    for (d, s) in dst[offset..].iter_mut().zip(src) {
        *d += *s;
    }
}

fn sq50(p: f32) -> f32 {
    square(p, 0.5)
}

fn sq25(p: f32) -> f32 {
    square(p, 0.25)
}

/// Rivet gun: short bright downward blip.
pub fn shoot() -> Vec<f32> {
    sweep(0.07, 1400.0, 500.0, 4.0, sq25, 0.25)
}

/// Mortar launch: low thump.
pub fn mortar() -> Vec<f32> {
    let mut v = sweep(0.15, 220.0, 70.0, 3.0, triangle, 0.6);
    mix_into(&mut v, &noise(0.08, 6.0, 0.3, 0.3, 11), 0);
    v
}

/// Enemy hit: tiny noise tick.
pub fn hit() -> Vec<f32> {
    noise(0.04, 5.0, 0.6, 0.35, 3)
}

/// Explosion: long low noise burst with a sub drop.
pub fn explosion() -> Vec<f32> {
    let mut v = noise(0.55, 5.0, 0.12, 0.9, 7);
    mix_into(&mut v, &sweep(0.4, 120.0, 35.0, 3.0, triangle, 0.6), 0);
    v
}

/// Enemy death pop.
pub fn kill() -> Vec<f32> {
    let mut v = sweep(0.1, 600.0, 150.0, 3.0, sq50, 0.18);
    mix_into(&mut v, &noise(0.08, 6.0, 0.4, 0.25, 5), 0);
    v
}

/// XP gem pickup: quick rising chirp.
pub fn gem() -> Vec<f32> {
    sweep(0.06, 900.0, 1800.0, 2.0, triangle, 0.25)
}

/// Crate pickup: two-note arpeggio.
pub fn crate_pickup() -> Vec<f32> {
    let mut v = sweep(0.07, midi(76.0), midi(76.0), 1.0, sq25, 0.2);
    mix_into(
        &mut v,
        &sweep(0.1, midi(83.0), midi(83.0), 2.0, sq25, 0.2),
        len(0.06),
    );
    v
}

/// Level up: bright major arpeggio.
pub fn level_up() -> Vec<f32> {
    let mut v = Vec::new();
    for (k, n) in [72.0, 76.0, 79.0, 84.0, 88.0].iter().enumerate() {
        mix_into(
            &mut v,
            &sweep(0.18, midi(*n), midi(*n), 2.0, sq25, 0.22),
            len(0.07) * k,
        );
    }
    v
}

/// Building / leasing a line: metallic clanks.
pub fn build() -> Vec<f32> {
    let mut v = Vec::new();
    for k in 0..3 {
        let mut clank = sweep(0.12, 900.0, 860.0, 6.0, sq50, 0.15);
        mix_into(
            &mut clank,
            &sweep(0.12, 1330.0, 1300.0, 7.0, triangle, 0.2),
            0,
        );
        mix_into(&mut clank, &noise(0.05, 8.0, 0.7, 0.3, 20 + k), 0);
        mix_into(&mut v, &clank, len(0.11) * k as usize);
    }
    v
}

/// Train whistle: two-tone chord with vibrato.
pub fn whistle() -> Vec<f32> {
    let n = len(0.9);
    let (mut a, mut b) = (0.0f32, 0.0f32);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let vib = 1.0 + 0.01 * (t * 40.0).sin();
            a += midi(81.0) * vib / SR;
            b += midi(85.0) * vib / SR;
            let env = (t * 12.0).min(1.0) * (1.0 - t).powf(0.6);
            ((a * TAU).sin() + (b * TAU).sin() + 0.3 * triangle(a * 2.0)) * env * 0.18
        })
        .collect()
}

/// Unloading a train: chuff of steam plus a cheerful blip.
pub fn unload() -> Vec<f32> {
    let mut v = noise(0.35, 4.0, 0.25, 0.5, 31);
    mix_into(&mut v, &crate_pickup(), len(0.05));
    v
}

/// Player takes damage.
pub fn hurt() -> Vec<f32> {
    let mut v = sweep(0.2, 300.0, 90.0, 2.0, sq50, 0.3);
    mix_into(&mut v, &noise(0.12, 4.0, 0.5, 0.3, 41), 0);
    v
}

/// Depot under attack: low warning buzz.
pub fn alarm() -> Vec<f32> {
    let mut v = sweep(0.15, 180.0, 180.0, 1.0, sq50, 0.18);
    mix_into(
        &mut v,
        &sweep(0.15, 140.0, 140.0, 1.0, sq50, 0.18),
        len(0.16),
    );
    v
}

/// Wave start: ascending horn.
pub fn wave_start() -> Vec<f32> {
    let mut v = sweep(0.25, midi(57.0), midi(57.0), 1.0, sq50, 0.18);
    mix_into(
        &mut v,
        &sweep(0.45, midi(64.0), midi(64.0), 1.5, sq50, 0.18),
        len(0.22),
    );
    v
}

/// Game over: slow descending phrase.
pub fn game_over() -> Vec<f32> {
    let mut v = Vec::new();
    for (k, n) in [67.0, 63.0, 60.0, 55.0].iter().enumerate() {
        let d = if k == 3 { 1.0 } else { 0.3 };
        mix_into(
            &mut v,
            &sweep(d, midi(*n), midi(*n) * 0.98, 1.5, triangle, 0.4),
            len(0.3) * k,
        );
        mix_into(
            &mut v,
            &sweep(d, midi(*n - 12.0), midi(*n - 12.0), 1.5, sq50, 0.12),
            len(0.3) * k,
        );
    }
    v
}

/// Victory fanfare.
pub fn victory() -> Vec<f32> {
    let mut v = Vec::new();
    let notes = [
        (72.0, 0.0),
        (72.0, 0.15),
        (72.0, 0.3),
        (79.0, 0.45),
        (76.0, 0.75),
        (84.0, 0.9),
    ];
    for (n, at) in notes {
        mix_into(
            &mut v,
            &sweep(0.4, midi(n), midi(n), 2.0, sq25, 0.2),
            len(at),
        );
        mix_into(
            &mut v,
            &sweep(0.4, midi(n - 24.0), midi(n - 24.0), 2.0, triangle, 0.3),
            len(at),
        );
    }
    v
}

/// A seamless 16-second chiptune loop with a "chugging" train rhythm.
pub fn music() -> Vec<f32> {
    const BPM: f32 = 132.0;
    let beat = 60.0 / BPM;
    let eighth = beat / 2.0;
    // Am, F, C, G, Am, F, Dm, E (one bar each, 4 beats).
    let chords: [[f32; 3]; 8] = [
        [57.0, 60.0, 64.0],
        [53.0, 57.0, 60.0],
        [48.0, 52.0, 55.0],
        [55.0, 59.0, 62.0],
        [57.0, 60.0, 64.0],
        [53.0, 57.0, 60.0],
        [50.0, 53.0, 57.0],
        [52.0, 56.0, 59.0],
    ];
    let melody: [f32; 32] = [
        76.0, 0.0, 72.0, 74.0, 76.0, 0.0, 79.0, 76.0, //
        77.0, 0.0, 76.0, 74.0, 72.0, 0.0, 0.0, 0.0, //
        76.0, 0.0, 72.0, 74.0, 76.0, 0.0, 81.0, 79.0, //
        77.0, 76.0, 74.0, 0.0, 71.0, 0.0, 0.0, 0.0,
    ];
    let total = len(beat * 4.0 * chords.len() as f32);
    let mut out = vec![0.0; total];
    for (bar, chord) in chords.iter().enumerate() {
        let bar_start = beat * 4.0 * bar as f32;
        for e in 0..8 {
            let at = len(bar_start + eighth * e as f32);
            // Bass: root, alternating octave, like wheels on rail joints.
            let root = chord[0] - 12.0 + if e % 2 == 1 { 12.0 } else { 0.0 };
            mix_into(
                &mut out,
                &sweep(eighth * 0.95, midi(root), midi(root), 1.5, triangle, 0.32),
                at,
            );
            // Arpeggio on sixteenths, quiet.
            for s in 0..2 {
                let note = chord[(e * 2 + s) % 3] + 12.0;
                let a = at + len(eighth / 2.0) * s;
                mix_into(
                    &mut out,
                    &sweep(eighth * 0.45, midi(note), midi(note), 3.0, sq25, 0.05),
                    a,
                );
            }
            // Steam chuff on the off-beats.
            if e % 2 == 1 {
                let seed = (bar * 8 + e) as u64;
                mix_into(&mut out, &noise(eighth * 0.8, 6.0, 0.35, 0.12, seed), at);
            }
        }
        // Lead melody, quarter notes.
        for q in 0..4 {
            let n = melody[(bar * 4 + q) % melody.len()];
            if n > 0.0 {
                let at = len(bar_start + beat * q as f32);
                mix_into(
                    &mut out,
                    &sweep(beat * 0.9, midi(n), midi(n), 1.2, sq50, 0.07),
                    at,
                );
            }
        }
    }
    out.truncate(total);
    // Gentle soft clip so stacked voices never wrap.
    for s in &mut out {
        *s = s.tanh() * 0.8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5]);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(bytes.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 36 + 6);
    }

    #[test]
    fn all_sounds_are_non_empty_and_bounded() {
        let all: [fn() -> Vec<f32>; 16] = [
            shoot,
            mortar,
            hit,
            explosion,
            kill,
            gem,
            crate_pickup,
            level_up,
            build,
            whistle,
            unload,
            hurt,
            alarm,
            wave_start,
            game_over,
            victory,
        ];
        for f in all {
            let s = f();
            assert!(!s.is_empty());
            assert!(s.iter().all(|x| x.is_finite()));
            assert!(s.iter().any(|x| x.abs() > 0.01), "sound is silent");
        }
    }

    #[test]
    fn music_loops_for_sixteen_seconds_and_stays_in_range() {
        let m = music();
        let secs = m.len() as f32 / SR;
        assert!((secs - 4.0 * 8.0 * 60.0 / 132.0).abs() < 0.01);
        assert!(m.iter().all(|x| x.abs() <= 0.8));
    }
}
