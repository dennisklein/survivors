//! Procedural sound synthesis: every effect and the music loop are generated
//! here as mono PCM and wrapped in a WAV container, so the game ships no
//! sound files at all.

use crate::rng::Pcg32;
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

pub fn hit() -> Vec<f32> {
    let mut r = Pcg32::new(5);
    let mut s = render(0.05, |t, _| r.range(-1.0, 1.0) * (1.0 - t / 0.05) * 0.4);
    lowpass(&mut s, 0.5);
    s
}

pub fn explode(len: f32, seed: u64) -> Vec<f32> {
    let mut r = Pcg32::new(seed);
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

pub fn level_up() -> Vec<f32> {
    let notes = [60.0, 64.0, 67.0, 72.0, 76.0, 79.0, 84.0];
    render(0.7, |t, _| {
        let k = ((t / 0.07) as usize).min(notes.len() - 1);
        let f = midi(notes[k]);
        square(t * f, 0.25) * 0.22 * (1.0 - t / 0.7)
    })
}

pub fn build() -> Vec<f32> {
    // Two metallic clanks: track bolted down.
    let mut r = Pcg32::new(3);
    render(0.4, |t, _| {
        let lt = if t < 0.18 { t } else { t - 0.18 };
        let env = (-lt * 30.0).exp();
        let ring = (lt * 523.0 * TAU).sin() * 0.5 + (lt * 1307.0 * TAU).sin() * 0.3;
        (ring + r.range(-1.0, 1.0) * 0.4) * env * 0.5
    })
}

/// Short low knock.
pub fn thunk() -> Vec<f32> {
    let mut r = Pcg32::new(23);
    let mut s = render(0.12, |t, _| {
        let env = (-t * 35.0).exp();
        ((t * 140.0 * TAU).sin() * 0.7 + r.range(-1.0, 1.0) * 0.4) * env * 0.6
    });
    lowpass(&mut s, 0.3);
    s
}

/// Low two-tone buzz for a refused action.
pub fn buzz() -> Vec<f32> {
    render(0.16, |t, _| {
        square(t * 110.0, 0.5) * 0.15 * (1.0 - t / 0.16)
    })
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

/// Steam whistle: two detuned square tones with a soft attack.
pub fn whistle() -> Vec<f32> {
    let len = 0.55;
    render(len, |t, _| {
        let env = (t / 0.05).min(1.0) * (1.0 - t / len).powf(0.5);
        let f = 740.0 + (t * 30.0).sin() * 6.0;
        (square(t * f, 0.5) * 0.5 + square(t * f * 1.26, 0.5) * 0.5) * env * 0.12
    })
}

/// Steam blast: filtered noise that swells and fades.
pub fn steam() -> Vec<f32> {
    let mut r = Pcg32::new(31);
    let len = 0.5;
    let mut s = render(len, |t, _| {
        let env = (t / 0.04).min(1.0) * (1.0 - t / len).powi(2);
        r.range(-1.0, 1.0) * env * 0.5
    });
    lowpass(&mut s, 0.25);
    s
}

/// Rising zap for a plasma ball.
pub fn zap() -> Vec<f32> {
    let mut ph = 0.0;
    render(0.3, |t, _| {
        let f = 200.0 + t * 2400.0;
        ph += f / RATE as f32;
        (ph * TAU).sin() * 0.3 * (1.0 - t / 0.3)
    })
}

/// A short click for picking a card.
pub fn click() -> Vec<f32> {
    render(0.05, |t, _| {
        square(t * 1200.0, 0.5) * 0.2 * (1.0 - t / 0.05)
    })
}

/// A looping folk-like tune over a steam train beat: "chuff" noise on
/// every eighth, a walking bass and a fiddle-like lead.
pub fn music() -> Vec<f32> {
    let bpm = 132.0;
    let step = 60.0 / bpm / 4.0;
    let steps = 16 * 8;
    // Chord roots (MIDI) per bar in A minor: Am Am F G Am Am Dm E.
    let roots = [45.0, 45.0, 41.0, 43.0, 45.0, 45.0, 38.0, 40.0];
    let minor = [0.0, 3.0, 7.0, 12.0];
    let major = [0.0, 4.0, 7.0, 12.0];
    let melody: [i32; 32] = [
        0, -1, 3, 5, 7, -1, 5, 3, 5, -1, 7, -1, 10, 8, 7, -1, 7, -1, 8, 10, 12, -1, 10, 8, 7, -1,
        5, 3, 2, -1, -1, -1,
    ];
    let mut r = Pcg32::new(99);
    let total = step * steps as f32;
    let mut out = render(total, |t, _| {
        let si = (t / step) as usize % steps;
        let lt = t - si as f32 * step;
        let bar = si / 16;
        let root = roots[bar];
        let chord = if matches!(bar, 2 | 3 | 7) {
            major
        } else {
            minor
        };
        let mut v = 0.0;
        // Walking bass on quarters: root, fifth, octave, fifth.
        let walk = [0.0, 7.0, 12.0, 7.0][(si / 4) % 4];
        v += tri(t * midi(root + walk)) * 0.3 * (1.0 - lt / (step * 4.0)).max(0.2);
        // Arpeggio.
        let arp = root + 24.0 + chord[si % 4];
        v += square(t * midi(arp), 0.25) * 0.04 * (-lt * 14.0).exp();
        // Lead in bars 4-7, one note per eighth.
        if bar >= 4 {
            let m = melody[(si - 64) / 2 % 32];
            if m >= 0 {
                let note = 69.0 + m as f32;
                let vib = 1.0 + (t * 33.0).sin() * 0.004;
                v += square(t * midi(note) * vib, 0.35) * 0.06 * (-lt * 3.0).exp();
            }
        }
        // Chuff: noise on every eighth, stronger on the beat.
        if si.is_multiple_of(2) {
            let acc = if si.is_multiple_of(4) { 0.2 } else { 0.1 };
            v += r.range(-1.0, 1.0) * acc * (-lt * 22.0).exp();
        }
        // Kick on beats 1 and 3.
        if si.is_multiple_of(8) {
            v += (lt * (110.0 - lt * 900.0).max(40.0) * TAU).sin() * 0.4 * (-lt * 18.0).exp();
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
            hit(),
            explode(0.5, 1),
            coin(),
            level_up(),
            build(),
            thunk(),
            buzz(),
            alarm(),
            game_over(),
            victory(),
            whistle(),
            steam(),
            zap(),
            click(),
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
            (secs - 8.0 * 4.0 * 60.0 / 132.0).abs() < 0.1,
            "8 bars at 132 bpm, got {secs}"
        );
        assert!(m.iter().all(|v| v.abs() <= 1.0));
    }
}
