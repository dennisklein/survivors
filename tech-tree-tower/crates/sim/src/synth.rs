//! Procedural sound synthesis for Tech Tree Tower: every effect and the
//! music loop are generated here as mono PCM and wrapped in a WAV container, so the game ships no
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

/// Commander plasma shot: a bright, fast downward zap.
pub fn shoot() -> Vec<f32> {
    let mut ph = 0.0;
    render(0.1, |t, _| {
        ph += (1800.0 * (-t * 25.0).exp() + 350.0) / RATE as f32;
        square(ph, 0.4) * (1.0 - t / 0.1).powi(2) * 0.22
    })
}

/// Tower laser beam; `heavy` gives a lower, longer hum.
pub fn laser(heavy: bool) -> Vec<f32> {
    let (len, f0) = if heavy { (0.3, 260.0) } else { (0.12, 900.0) };
    render(len, |t, _| {
        let f = f0 * (1.0 - t / len * 0.4);
        ((t * f * TAU).sin() * 0.6 + square(t * f * 0.5, 0.5) * 0.25) * (1.0 - t / len) * 0.35
    })
}

/// Rocket launch: hissing noise with a rising whoosh.
pub fn rocket() -> Vec<f32> {
    let mut r = Rng::new(31);
    let mut ph = 0.0;
    let mut s = render(0.35, |t, _| {
        ph += (200.0 + t * 900.0) / RATE as f32;
        (r.range(-1.0, 1.0) * 0.6 + tri(ph) * 0.3) * (1.0 - t / 0.35) * 0.5
    });
    lowpass(&mut s, 0.3);
    s
}

/// UI blip for hovering/choosing a card.
pub fn click() -> Vec<f32> {
    render(0.06, |t, _| {
        square(t * midi(88.0), 0.5) * (1.0 - t / 0.06) * 0.15
    })
}

/// Wave-start alarm: two rising siren sweeps; `boss` is lower and longer.
pub fn siren(boss: bool) -> Vec<f32> {
    let (len, f0) = if boss { (1.2, 180.0) } else { (0.6, 520.0) };
    let mut ph = 0.0;
    render(len, |t, _| {
        let sweep = (t / len * 2.0).fract();
        ph += f0 * (1.0 + sweep * 0.8) / RATE as f32;
        let env = (t / 0.02).min(1.0) * (1.0 - t / len);
        (square(ph, 0.5) * 0.25 + (ph * TAU).sin() * 0.3) * env * 0.5
    })
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

/// Metal pickup chime.
pub fn pickup() -> Vec<f32> {
    render(0.14, |t, _| {
        let f = if t < 0.05 { midi(84.0) } else { midi(91.0) };
        (t * f * TAU).sin() * (1.0 - t / 0.14) * 0.3
    })
}

pub fn denied() -> Vec<f32> {
    render(0.18, |t, _| square(t * 140.0, 0.5) * (1.0 - t / 0.18) * 0.2)
}

pub fn level_up() -> Vec<f32> {
    let notes = [62.0, 66.0, 69.0, 74.0, 78.0, 81.0, 86.0];
    render(0.7, |t, _| {
        let k = ((t / 0.07) as usize).min(notes.len() - 1);
        square(t * midi(notes[k]), 0.25) * 0.22 * (1.0 - t / 0.7)
    })
}

/// Construction: two metallic clanks.
pub fn build() -> Vec<f32> {
    let mut r = Rng::new(3);
    render(0.4, |t, _| {
        let lt = if t < 0.18 { t } else { t - 0.18 };
        let env = (-lt * 30.0).exp();
        let ring = (lt * 523.0 * TAU).sin() * 0.5 + (lt * 1307.0 * TAU).sin() * 0.3;
        (ring + r.range(-1.0, 1.0) * 0.4) * env * 0.5
    })
}

/// Wave cleared: a short rising fanfare.
pub fn cleared() -> Vec<f32> {
    let notes = [(67.0, 0.0), (71.0, 0.12), (74.0, 0.24), (79.0, 0.36)];
    render(1.0, |t, _| {
        let mut v = 0.0;
        for (note, start) in notes {
            if t >= start {
                let lt = t - start;
                v += tri(lt * midi(note)) * (-lt * 4.0).exp() * 0.25;
            }
        }
        v
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

/// Looping industrial build-up track in A minor: kick, hats, a pulsing
/// saw-ish bass, a chord pad and a sparse lead. 8 bars at 128 bpm (15 s).
pub fn music() -> Vec<f32> {
    let bpm = 128.0;
    let step = 60.0 / bpm / 4.0;
    let steps = 16 * 8;
    // Am F C G Am F G E
    let roots = [45.0, 41.0, 48.0, 43.0, 45.0, 41.0, 43.0, 40.0];
    let minor = [0.0, 3.0, 7.0];
    let major = [0.0, 4.0, 7.0];
    let lead: [i32; 16] = [12, -1, 7, -1, 10, 12, -1, 15, -1, 14, 12, -1, 10, -1, 7, -1];
    let mut r = Rng::new(91);
    let total = step * steps as f32;
    let mut out = render(total, |t, _| {
        let si = (t / step) as usize % steps;
        let lt = t - si as f32 * step;
        let bar = si / 16;
        let root = roots[bar];
        let chord = if matches!(bar, 0 | 4) { minor } else { major };
        let mut v = 0.0;
        // Kick on every beat.
        if si.is_multiple_of(4) {
            let f = 55.0 + 120.0 * (-lt * 30.0).exp();
            v += (lt * f * TAU).sin() * 0.55 * (-lt * 9.0).exp();
        }
        // Offbeat hats.
        if si % 4 == 2 {
            v += r.range(-1.0, 1.0) * 0.1 * (-lt * 60.0).exp();
        }
        // Rolling bass: octave bounce on 8ths.
        let bn = root - 12.0 + if (si / 2) % 2 == 1 { 12.0 } else { 0.0 };
        let saw = (t * midi(bn)).fract() * 2.0 - 1.0;
        v += saw * 0.16 * (-lt * 6.0).exp();
        // Pad.
        for n in chord {
            v += tri(t * midi(root + 12.0 + n)) * 0.035;
        }
        // Lead in bars 4-7.
        if bar >= 4 {
            let m = lead[si % 16];
            if m >= 0 {
                v += square(t * midi(root + 12.0 + m as f32), 0.3) * 0.05 * (-lt * 5.0).exp();
            }
        }
        v
    });
    lowpass(&mut out, 0.5);
    for s in &mut out {
        *s = (*s * 1.3).tanh() * 0.8;
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
            laser(false),
            laser(true),
            rocket(),
            click(),
            siren(false),
            siren(true),
            hit(),
            explode(0.5, 1),
            pickup(),
            denied(),
            level_up(),
            build(),
            cleared(),
            game_over(),
            victory(),
        ];
        for s in all {
            assert!(!s.is_empty());
            assert!(s.iter().any(|v| v.abs() > 0.01));
            assert!(s.iter().all(|v| v.is_finite() && v.abs() <= 1.5));
        }
    }

    #[test]
    fn music_loops_whole_bars() {
        let m = music();
        let secs = m.len() as f32 / RATE as f32;
        assert!(
            (secs - 15.0).abs() < 0.1,
            "8 bars at 128 bpm = 15s, got {secs}"
        );
        assert!(m.iter().all(|v| v.abs() <= 1.0));
    }
}
