//! Procedural sound synthesis: every effect and the music loop are generated
//! as mono PCM samples and wrapped in a 16-bit WAV container.

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
        Wave::Sine => (p * std::f32::consts::TAU).sin(),
    }
}

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Pitch sweep from `f0` to `f1` (exponential) with an exponential decay envelope.
fn sweep(secs: f32, f0: f32, f1: f32, w: Wave, vol: f32, decay: f32) -> Vec<f32> {
    let n = len(secs);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let f = f0 * (f1 / f0).powf(t);
            phase += f / RATE as f32;
            let attack = (i as f32 / 40.0).min(1.0);
            osc(w, phase) * vol * attack * (-decay * t).exp()
        })
        .collect()
}

/// Low-passed white noise burst. `smooth` in (0, 1]: smaller is darker.
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
            let t = i as f32 / n as f32;
            lp * vol * (-decay * t).exp()
        })
        .collect()
}

fn mix(mut a: Vec<f32>, b: &[f32], offset: usize) -> Vec<f32> {
    if a.len() < offset + b.len() {
        a.resize(offset + b.len(), 0.0);
    }
    for (i, x) in b.iter().enumerate() {
        a[offset + i] += x;
    }
    a
}

fn notes(seq: &[(f32, f32)], w: Wave, vol: f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut at = 0;
    for &(f, d) in seq {
        out = mix(out, &sweep(d * 1.6, f, f, w, vol, 4.0), at);
        at += len(d);
    }
    out
}

fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

pub fn shoot() -> Vec<f32> {
    sweep(0.09, 1400.0, 380.0, Wave::Square, 0.35, 5.0)
}

pub fn militia_shot() -> Vec<f32> {
    mix(
        sweep(0.06, 900.0, 500.0, Wave::Triangle, 0.3, 6.0),
        &noise(0.04, 0.2, 6.0, 0.8, 11),
        0,
    )
}

pub fn hit() -> Vec<f32> {
    mix(
        noise(0.06, 0.4, 7.0, 0.5, 3),
        &sweep(0.05, 300.0, 150.0, Wave::Square, 0.2, 5.0),
        0,
    )
}

pub fn kill() -> Vec<f32> {
    mix(
        noise(0.18, 0.5, 5.0, 0.35, 5),
        &sweep(0.15, 500.0, 90.0, Wave::Square, 0.25, 4.0),
        0,
    )
}

pub fn explosion() -> Vec<f32> {
    mix(
        noise(0.8, 0.9, 4.5, 0.12, 9),
        &sweep(0.6, 120.0, 35.0, Wave::Sine, 0.7, 4.0),
        0,
    )
}

pub fn coin() -> Vec<f32> {
    notes(&[(midi(88.0), 0.05), (midi(93.0), 0.1)], Wave::Square, 0.25)
}

pub fn serve() -> Vec<f32> {
    // Cash register "ka-ching".
    mix(
        notes(
            &[(midi(84.0), 0.06), (midi(91.0), 0.06), (midi(96.0), 0.18)],
            Wave::Triangle,
            0.45,
        ),
        &noise(0.05, 0.3, 8.0, 0.9, 21),
        0,
    )
}

pub fn level_up() -> Vec<f32> {
    let seq: Vec<(f32, f32)> = [72.0, 76.0, 79.0, 84.0, 88.0, 91.0]
        .iter()
        .map(|&n| (midi(n), 0.07))
        .collect();
    notes(&seq, Wave::Square, 0.3)
}

pub fn build() -> Vec<f32> {
    // Two hammer clanks on a rail.
    let clank = mix(
        sweep(0.14, 1800.0, 1500.0, Wave::Triangle, 0.4, 9.0),
        &noise(0.06, 0.5, 9.0, 0.7, 31),
        0,
    );
    mix(clank.clone(), &clank, len(0.16))
}

pub fn whistle() -> Vec<f32> {
    // Steam whistle: detuned square chord with breathy noise.
    let n = len(0.9);
    let mut phase = [0.0f32; 3];
    let freqs = [midi(74.0), midi(78.0), midi(81.0)];
    let air = noise(0.9, 0.12, 1.0, 0.6, 41);
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let env = (t * 12.0).min(1.0) * (1.0 - t).powf(0.6);
            let bend = 1.0 - 0.04 * (1.0 - (t * 10.0).min(1.0));
            let mut s = 0.0;
            for (p, f) in phase.iter_mut().zip(freqs) {
                *p += f * bend / RATE as f32;
                s += osc(Wave::Triangle, *p) * 0.2;
            }
            (s + air[i]) * env
        })
        .collect()
}

pub fn alarm() -> Vec<f32> {
    let mut out = Vec::new();
    for k in 0..3 {
        out = mix(
            out,
            &sweep(0.18, 660.0, 880.0, Wave::Saw, 0.25, 1.0),
            len(0.22) * k,
        );
    }
    out
}

pub fn hurt() -> Vec<f32> {
    mix(
        sweep(0.25, 220.0, 70.0, Wave::Saw, 0.45, 3.0),
        &noise(0.2, 0.4, 5.0, 0.3, 51),
        0,
    )
}

pub fn town_lost() -> Vec<f32> {
    mix(
        explosion(),
        &notes(
            &[(midi(64.0), 0.2), (midi(60.0), 0.2), (midi(55.0), 0.5)],
            Wave::Saw,
            0.25,
        ),
        len(0.1),
    )
}

pub fn click() -> Vec<f32> {
    sweep(0.04, 1200.0, 900.0, Wave::Square, 0.2, 8.0)
}

pub fn deny() -> Vec<f32> {
    sweep(0.18, 160.0, 120.0, Wave::Square, 0.3, 3.0)
}

pub fn game_over() -> Vec<f32> {
    notes(
        &[
            (midi(67.0), 0.25),
            (midi(63.0), 0.25),
            (midi(60.0), 0.25),
            (midi(55.0), 0.8),
        ],
        Wave::Triangle,
        0.5,
    )
}

pub fn victory() -> Vec<f32> {
    notes(
        &[
            (midi(72.0), 0.12),
            (midi(76.0), 0.12),
            (midi(79.0), 0.12),
            (midi(84.0), 0.3),
            (midi(79.0), 0.12),
            (midi(84.0), 0.6),
        ],
        Wave::Square,
        0.35,
    )
}

/// A looping chiptune track with a "chugging" rhythm section (about 13.7 s).
pub fn music() -> Vec<f32> {
    let step = 60.0 / 140.0 / 4.0;
    let steps_per_bar = 16;
    // (root midi note, minor?)
    let bars: [(f32, bool); 8] = [
        (45.0, true),
        (41.0, false),
        (48.0, false),
        (43.0, false),
        (45.0, true),
        (41.0, false),
        (43.0, false),
        (40.0, false),
    ];
    let total = len(step * (steps_per_bar * bars.len()) as f32);
    let mut out = vec![0.0f32; total];
    let melody = [0usize, 1, 2, 1, 2, 3, 2, 1];
    for (b, &(root, minor)) in bars.iter().enumerate() {
        let third = if minor { 3.0 } else { 4.0 };
        let chord = [0.0, third, 7.0, 12.0];
        for s in 0..steps_per_bar {
            let at = len(step * (b * steps_per_bar + s) as f32);
            // Bass on eighths, octave bounce.
            if s % 2 == 0 {
                let n = root + if s % 4 == 2 { 12.0 } else { 0.0 };
                out = mix(
                    out,
                    &sweep(step * 1.8, midi(n), midi(n), Wave::Square, 0.11, 3.0),
                    at,
                );
            }
            // Chug: kick on beats, steam hiss on off-beats.
            if s % 4 == 0 {
                out = mix(out, &sweep(0.12, 140.0, 45.0, Wave::Sine, 0.45, 5.0), at);
            }
            if s % 4 == 2 {
                out = mix(out, &noise(0.07, 0.12, 6.0, 0.9, 7 + s as u32), at);
            }
            // Arpeggio.
            let arp = chord[s % 4] + 24.0;
            out = mix(
                out,
                &sweep(
                    step * 0.9,
                    midi(arp + root - 12.0),
                    midi(arp + root - 12.0),
                    Wave::Triangle,
                    0.06,
                    4.0,
                ),
                at,
            );
            // Lead melody on the second half of the loop.
            if b >= 4 && s % 4 == 0 {
                let n = root + 24.0 + chord[melody[(s / 4 + b) % melody.len()]];
                out = mix(
                    out,
                    &sweep(step * 3.5, midi(n), midi(n), Wave::Square, 0.06, 2.0),
                    at,
                );
            }
        }
    }
    out.truncate(total);
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-6);
    out.iter().map(|x| x / peak * 0.7).collect()
}

/// Wrap mono samples in [-1, 1] as a 16-bit PCM WAV file.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
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
        let w = wav_bytes(&[0.0, 1.0, -1.0]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 6);
    }

    #[test]
    fn all_sounds_are_audible_and_bounded() {
        let all: [fn() -> Vec<f32>; 17] = [
            shoot,
            militia_shot,
            hit,
            kill,
            explosion,
            coin,
            serve,
            level_up,
            build,
            whistle,
            alarm,
            hurt,
            town_lost,
            click,
            deny,
            game_over,
            victory,
        ];
        for f in all {
            let s = f();
            assert!(!s.is_empty());
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.05 && peak.is_finite());
        }
    }

    #[test]
    fn music_loop_is_normalized() {
        let m = music();
        assert!(m.len() > RATE as usize * 10);
        let peak = m.iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!((peak - 0.7).abs() < 1e-3);
    }
}
