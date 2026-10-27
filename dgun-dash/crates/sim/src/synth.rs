//! Procedural sound synthesis: every effect and the music loop are generated
//! here as mono PCM and wrapped in a WAV container, so the game ships no
//! sound files at all.

use crate::rng::Rng;
use std::f32::consts::TAU;

pub const RATE: u32 = 22_050;

/// Wraps mono `[-1, 1]` samples into a 16-bit PCM WAV file.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
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
        let v = (s.clamp(-1.0, 1.0) * 32_000.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn n(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

fn square(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty { 1.0 } else { -1.0 }
}

fn tri(phase: f32) -> f32 {
    let p = phase.fract();
    4.0 * (p - 0.5).abs() - 1.0
}

fn midi(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0)
}

/// Renders `secs` of audio where `f(t, i)` yields each sample.
fn render(secs: f32, mut f: impl FnMut(f32, usize) -> f32) -> Vec<f32> {
    (0..n(secs)).map(|i| f(i as f32 / RATE as f32, i)).collect()
}

/// One-pole low-pass filter in place.
fn lowpass(s: &mut [f32], k: f32) {
    let mut y = 0.0;
    for v in s {
        y += (*v - y) * k;
        *v = y;
    }
}

pub fn shoot() -> Vec<f32> {
    let mut ph = 0.0;
    render(0.09, |t, _| {
        let f = 900.0 - t * 7000.0;
        ph += f / RATE as f32;
        square(ph, 0.3) * (1.0 - t / 0.09).powi(2) * 0.35
    })
}

pub fn flak() -> Vec<f32> {
    let mut r = Rng::new(11);
    let mut ph = 0.0;
    let mut s = render(0.22, |t, _| {
        let f = 220.0 - t * 600.0;
        ph += f / RATE as f32;
        let env = (1.0 - t / 0.22).powi(3);
        (tri(ph) * 0.6 + r.range(-1.0, 1.0) * 0.5) * env * 0.6
    });
    lowpass(&mut s, 0.35);
    s
}

pub fn hit() -> Vec<f32> {
    let mut r = Rng::new(5);
    let mut s = render(0.05, |t, _| r.range(-1.0, 1.0) * (1.0 - t / 0.05) * 0.4);
    lowpass(&mut s, 0.5);
    s
}

pub fn explode(len: f32, seed: u64) -> Vec<f32> {
    let mut r = Rng::new(seed);
    let mut ph = 0.0;
    let mut s = render(len, |t, _| {
        let env = (1.0 - t / len).powi(2);
        ph += (90.0 - t * 60.0).max(30.0) / RATE as f32;
        (r.range(-1.0, 1.0) * 0.8 + (ph * TAU).sin() * 0.6) * env * 0.7
    });
    lowpass(&mut s, 0.18);
    s
}

pub fn coin() -> Vec<f32> {
    render(0.16, |t, _| {
        let f = if t < 0.05 { midi(88.0) } else { midi(95.0) };
        (t * f * TAU).sin() * (1.0 - t / 0.16) * 0.35
    })
}

pub fn reclaim() -> Vec<f32> {
    render(0.12, |t, _| {
        let f = midi(72.0) + t * 1500.0;
        square(t * f, 0.5) * (1.0 - t / 0.12) * 0.18
    })
}

pub fn metal() -> Vec<f32> {
    render(0.35, |t, _| {
        let env = (-t * 9.0).exp();
        ((t * midi(84.0) * TAU).sin() + 0.5 * (t * midi(91.0) * TAU).sin()) * env * 0.28
    })
}

pub fn level_up() -> Vec<f32> {
    let notes = [60.0, 64.0, 67.0, 72.0, 76.0, 79.0, 84.0];
    render(0.7, |t, _| {
        let k = ((t / 0.07) as usize).min(notes.len() - 1);
        let f = midi(notes[k]);
        square(t * f, 0.25) * 0.22 * (1.0 - t / 0.7)
    })
}

pub fn build() -> Vec<f32> {
    // Two metallic clanks: a nano frame snapping into place.
    let mut r = Rng::new(3);
    render(0.4, |t, _| {
        let lt = if t < 0.18 { t } else { t - 0.18 };
        let env = (-lt * 30.0).exp();
        let ring = (lt * 523.0 * TAU).sin() * 0.5 + (lt * 1307.0 * TAU).sin() * 0.3;
        (ring + r.range(-1.0, 1.0) * 0.4) * env * 0.5
    })
}

/// D-Gun: a charged, falling energy sweep over a heavy noise burst.
pub fn dgun() -> Vec<f32> {
    let mut r = Rng::new(17);
    let mut ph = 0.0;
    let mut s = render(0.9, |t, _| {
        let f = 1400.0 * (-t * 4.0).exp() + 60.0;
        ph += f / RATE as f32;
        let env = (1.0 - t / 0.9).powi(2);
        (square(ph, 0.5) * 0.45 + r.range(-1.0, 1.0) * 0.5 * (-t * 3.0).exp()) * env * 0.6
    });
    lowpass(&mut s, 0.4);
    s
}

pub fn alarm() -> Vec<f32> {
    render(0.9, |t, _| {
        let f = if ((t * 5.0) as u32).is_multiple_of(2) {
            440.0
        } else {
            330.0
        };
        square(t * f, 0.5) * 0.18 * (1.0 - t / 0.9)
    })
}

pub fn game_over() -> Vec<f32> {
    let notes = [67.0, 63.0, 60.0, 55.0];
    render(1.6, |t, _| {
        let k = ((t / 0.35) as usize).min(notes.len() - 1);
        let f = midi(notes[k]) * (1.0 - (t - k as f32 * 0.35) * 0.05);
        (tri(t * f) * 0.5 + square(t * f * 0.5, 0.5) * 0.12) * (1.0 - t / 1.6)
    })
}

pub fn victory() -> Vec<f32> {
    let notes = [
        (60.0, 0.0),
        (64.0, 0.15),
        (67.0, 0.3),
        (72.0, 0.45),
        (67.0, 0.75),
        (72.0, 0.9),
    ];
    render(1.8, |t, _| {
        let mut v = 0.0;
        for (note, start) in notes {
            if t >= start {
                let lt = t - start;
                v += square(lt * midi(note), 0.5) * (-lt * 3.0).exp() * 0.12;
            }
        }
        v
    })
}

/// A looping, driving minor-key synth track with a mechanical pulse.
pub fn music() -> Vec<f32> {
    let bpm = 140.0;
    let step = 60.0 / bpm / 4.0;
    let steps = 16 * 8;
    // Chord roots (MIDI) per bar in A minor: Am Am F G Am Am F E.
    let roots = [45.0, 45.0, 41.0, 43.0, 45.0, 45.0, 41.0, 40.0];
    let minor = [0.0, 3.0, 7.0, 12.0];
    let major = [0.0, 4.0, 7.0, 12.0];
    let melody: [i32; 32] = [
        7, -1, 10, 12, -1, 10, 7, -1, 5, -1, 3, -1, 5, 7, -1, -1, 7, -1, 10, 12, 15, -1, 12, 10,
        12, -1, 11, -1, 8, -1, -1, -1,
    ];
    let mut r = Rng::new(99);
    let total = step * steps as f32;
    let mut out = render(total, |t, _| {
        let si = (t / step) as usize % steps;
        let lt = t - si as f32 * step;
        let bar = si / 16;
        let root = roots[bar];
        let chord = if matches!(bar, 2 | 3 | 6 | 7) {
            major
        } else {
            minor
        };
        let mut v = 0.0;
        // Bass on eighths with an octave bounce.
        let bass_note = root + if (si / 2) % 2 == 1 { 12.0 } else { 0.0 };
        v += tri(t * midi(bass_note)) * 0.32 * (1.0 - lt / (step * 2.0)).max(0.2);
        // Arpeggio.
        let arp = root + 24.0 + chord[si % 4];
        v += square(t * midi(arp), 0.25) * 0.05 * (-lt * 14.0).exp();
        // Lead melody, one note per eighth, played in bars 4-7.
        if bar >= 4 {
            let m = melody[(si - 64) / 2 % 32];
            if m >= 0 {
                let note = 69.0 + m as f32;
                v += square(t * midi(note), 0.5) * 0.07 * (-lt * 4.0).exp();
            }
        }
        // Hi-hat: noise on every eighth, accented on the beat.
        if si.is_multiple_of(2) {
            let acc = if si.is_multiple_of(4) { 0.22 } else { 0.1 };
            v += r.range(-1.0, 1.0) * acc * (-lt * 40.0).exp();
        }
        // Kick on beats 1 and 3.
        if si.is_multiple_of(8) {
            v += (lt * (110.0 - lt * 900.0).max(40.0) * TAU).sin() * 0.45 * (-lt * 18.0).exp();
        }
        v
    });
    // Soft clip.
    for s in &mut out {
        *s = (*s * 1.2).tanh() * 0.8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let w = wav_bytes(&[0.0, 0.5, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 6);
    }

    #[test]
    fn effects_are_non_silent_and_bounded() {
        let all = [
            shoot(),
            flak(),
            hit(),
            explode(0.5, 1),
            coin(),
            reclaim(),
            metal(),
            level_up(),
            build(),
            dgun(),
            alarm(),
            game_over(),
            victory(),
        ];
        for s in all {
            assert!(!s.is_empty());
            assert!(s.iter().any(|v| v.abs() > 0.01));
            assert!(s.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn music_loops_whole_bars() {
        let m = music();
        let secs = m.len() as f32 / RATE as f32;
        assert!(
            (secs - 960.0 / 70.0).abs() < 0.1,
            "8 bars at 140 bpm = 13.7s, got {secs}"
        );
        assert!(m.iter().all(|v| v.abs() <= 1.0));
    }
}
