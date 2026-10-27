//! Procedural sound synthesis: every effect and the music loop are generated
//! as mono PCM samples and wrapped in a 16-bit WAV container.

use std::f32::consts::TAU;

pub const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Saw,
    Sine,
}

fn osc(w: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    match w {
        Wave::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * p - 1.0,
        Wave::Sine => (p * TAU).sin(),
    }
}

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Exponential pitch sweep from `f0` to `f1` with an exponential decay.
fn sweep(secs: f32, f0: f32, f1: f32, w: Wave, vol: f32, decay: f32) -> Vec<f32> {
    let n = len(secs);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            phase += f0 * (f1 / f0).powf(t) / RATE as f32;
            let attack = (i as f32 / 40.0).min(1.0);
            osc(w, phase) * vol * attack * (-decay * t).exp()
        })
        .collect()
}

/// Low-passed noise burst; smaller `smooth` is darker.
fn noise(secs: f32, vol: f32, decay: f32, smooth: f32, seed: u32) -> Vec<f32> {
    let n = len(secs);
    let mut s = seed | 1;
    let mut lp = 0.0f32;
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let white = (s as f32 / u32::MAX as f32) * 2.0 - 1.0;
            lp += (white - lp) * smooth;
            lp * vol * (-decay * i as f32 / n as f32).exp()
        })
        .collect()
}

/// Sum `src` into `dst` starting at sample `at`, growing `dst` as needed.
fn mix(dst: &mut Vec<f32>, src: &[f32], at: usize) {
    if dst.len() < at + src.len() {
        dst.resize(at + src.len(), 0.0);
    }
    for (d, s) in dst[at..].iter_mut().zip(src) {
        *d += s;
    }
}

fn notes(seq: &[(f32, f32)], w: Wave, vol: f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut at = 0;
    for &(f, secs) in seq {
        mix(&mut out, &sweep(secs * 1.6, f, f, w, vol, 4.0), at);
        at += len(secs);
    }
    out
}

pub fn laser() -> Vec<f32> {
    sweep(0.12, 1800.0, 500.0, Wave::Square, 0.25, 5.0)
}

pub fn heavy_laser() -> Vec<f32> {
    let mut v = sweep(0.3, 900.0, 120.0, Wave::Saw, 0.35, 4.0);
    mix(&mut v, &noise(0.2, 0.3, 6.0, 0.3, 11), 0);
    v
}

pub fn explosion_small() -> Vec<f32> {
    let mut v = noise(0.35, 0.7, 6.0, 0.25, 3);
    mix(&mut v, &sweep(0.25, 140.0, 40.0, Wave::Sine, 0.5, 5.0), 0);
    v
}

pub fn explosion_big() -> Vec<f32> {
    let mut v = noise(1.1, 0.9, 4.0, 0.08, 5);
    mix(&mut v, &sweep(0.8, 90.0, 25.0, Wave::Sine, 0.8, 3.0), 0);
    v
}

pub fn crunch() -> Vec<f32> {
    let mut v = noise(0.5, 0.8, 5.0, 0.15, 21);
    mix(&mut v, &sweep(0.4, 220.0, 50.0, Wave::Square, 0.25, 4.0), 0);
    v
}

pub fn chew() -> Vec<f32> {
    noise(0.08, 0.35, 8.0, 0.5, 9)
}

pub fn pickup() -> Vec<f32> {
    notes(&[(880.0, 0.04), (1320.0, 0.06)], Wave::Triangle, 0.3)
}

pub fn place() -> Vec<f32> {
    notes(&[(520.0, 0.05), (780.0, 0.07)], Wave::Square, 0.18)
}

pub fn deny() -> Vec<f32> {
    sweep(0.18, 160.0, 110.0, Wave::Square, 0.25, 2.0)
}

pub fn complete() -> Vec<f32> {
    notes(
        &[(523.0, 0.06), (659.0, 0.06), (784.0, 0.06), (1046.0, 0.14)],
        Wave::Triangle,
        0.3,
    )
}

pub fn level_up() -> Vec<f32> {
    let mut v = notes(
        &[
            (392.0, 0.08),
            (523.0, 0.08),
            (659.0, 0.08),
            (784.0, 0.08),
            (1046.0, 0.25),
        ],
        Wave::Square,
        0.18,
    );
    mix(&mut v, &notes(&[(196.0, 0.5)], Wave::Triangle, 0.3), 0);
    v
}

pub fn wave_alarm() -> Vec<f32> {
    let mut v = Vec::new();
    for k in 0..3 {
        mix(
            &mut v,
            &sweep(0.18, 600.0, 900.0, Wave::Square, 0.18, 1.0),
            k * len(0.24),
        );
    }
    v
}

pub fn hurt() -> Vec<f32> {
    let mut v = sweep(0.2, 300.0, 90.0, Wave::Saw, 0.35, 3.0);
    mix(&mut v, &noise(0.12, 0.3, 6.0, 0.4, 13), 0);
    v
}

/// Looping hum played while any nano beam is active.
pub fn nano_hum() -> Vec<f32> {
    let n = len(0.5);
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let wob = 1.0 + 0.5 * (t * TAU * 6.0).sin();
            ((t * 220.0 * TAU).sin() * 0.5 + (t * 440.0 * TAU + wob).sin() * 0.3) * 0.12
        })
        .collect()
}

pub fn game_over() -> Vec<f32> {
    let mut v = notes(
        &[(392.0, 0.25), (330.0, 0.25), (262.0, 0.25), (196.0, 0.7)],
        Wave::Saw,
        0.2,
    );
    mix(&mut v, &explosion_big(), 0);
    v
}

pub fn victory() -> Vec<f32> {
    notes(
        &[
            (523.0, 0.12),
            (523.0, 0.12),
            (523.0, 0.12),
            (659.0, 0.3),
            (587.0, 0.15),
            (659.0, 0.15),
            (784.0, 0.6),
        ],
        Wave::Square,
        0.2,
    )
}

/// A 16-bar minor-key loop: pulsing bass, arpeggio and hi-hat ticks.
pub fn music() -> Vec<f32> {
    let bpm = 128.0;
    let step = 60.0 / bpm / 4.0; // sixteenth note
    let roots = [110.0, 87.31, 130.81, 98.0]; // A, F, C, G
    let bars = 16;
    let total = len(step * 16.0 * bars as f32);
    let mut out = vec![0.0f32; total];
    for bar in 0..bars {
        let root = roots[(bar / 2) % 4];
        for s in 0..16 {
            let at = len(step * (bar * 16 + s) as f32);
            if s % 2 == 0 {
                mix(
                    &mut out,
                    &sweep(step * 1.8, root, root, Wave::Triangle, 0.32, 3.0),
                    at,
                );
            }
            if bar >= 2 {
                let arp = [1.0, 1.5, 2.0, 2.4, 3.0, 2.4, 2.0, 1.5];
                let f = root * 2.0 * arp[s % 8];
                mix(
                    &mut out,
                    &sweep(step * 0.9, f, f, Wave::Square, 0.06, 5.0),
                    at,
                );
            }
            if bar >= 4 && s % 4 == 2 {
                mix(
                    &mut out,
                    &noise(0.04, 0.12, 6.0, 0.9, (bar * 16 + s) as u32),
                    at,
                );
            }
            if bar >= 8 && s % 8 == 0 {
                mix(
                    &mut out,
                    &sweep(0.12, 120.0, 45.0, Wave::Sine, 0.4, 4.0),
                    at,
                );
            }
        }
    }
    out.truncate(total);
    out
}

/// Wrap mono samples in a 16-bit PCM WAV file.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let b = wav_bytes(&[0.0, 1.0, -1.0]);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert_eq!(b.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 6);
        assert_eq!(i16::from_le_bytes([b[46], b[47]]), i16::MAX);
    }

    #[test]
    fn sounds_are_non_empty_and_finite() {
        let all: [fn() -> Vec<f32>; 16] = [
            laser,
            heavy_laser,
            explosion_small,
            explosion_big,
            crunch,
            chew,
            pickup,
            place,
            deny,
            complete,
            level_up,
            wave_alarm,
            hurt,
            nano_hum,
            game_over,
            victory,
        ];
        for f in all {
            let s = f();
            assert!(!s.is_empty());
            assert!(s.iter().all(|x| x.is_finite()));
            assert!(s.iter().any(|x| x.abs() > 0.01));
        }
    }

    #[test]
    fn music_loop_has_expected_length() {
        let m = music();
        let secs = m.len() as f32 / RATE as f32;
        assert!((secs - 30.0).abs() < 0.1, "{secs}");
    }
}
