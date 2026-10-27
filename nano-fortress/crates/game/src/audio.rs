//! Sound playback: synthesized effects, looping music and nano hum,
//! per-effect throttling and a global mute.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
use rules::game::Event;
use rules::synth;

const MUSIC_VOLUME: f32 = 0.28;
const HUM_VOLUME: f32 = 0.35;

#[derive(Clone, Copy)]
pub enum Sfx {
    Laser,
    Heavy,
    Boom,
    BigBoom,
    Crunch,
    Chew,
    Pickup,
    Place,
    Deny,
    Complete,
    LevelUp,
    Alarm,
    Hurt,
    GameOver,
    Victory,
}

const COUNT: usize = 15;

impl Sfx {
    /// Minimum seconds between two plays of the same effect, and its volume.
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Laser => (0.06, 0.35),
            Sfx::Heavy => (0.12, 0.5),
            Sfx::Boom => (0.06, 0.45),
            Sfx::BigBoom => (0.2, 0.8),
            Sfx::Crunch => (0.15, 0.6),
            Sfx::Chew => (0.18, 0.25),
            Sfx::Pickup => (0.06, 0.4),
            Sfx::Hurt => (0.3, 0.6),
            _ => (0.0, 0.7),
        }
    }
}

pub struct Audio {
    sounds: Vec<Sound>,
    music: Sound,
    hum: Sound,
    last: [f64; COUNT],
    music_on: bool,
    hum_on: bool,
    pub muted: bool,
}

async fn load(samples: Vec<f32>) -> Sound {
    load_sound_from_bytes(&synth::wav_bytes(&samples))
        .await
        .expect("synthesized WAV must decode")
}

impl Audio {
    pub async fn new() -> Self {
        let gens: [fn() -> Vec<f32>; COUNT] = [
            synth::laser,
            synth::heavy_laser,
            synth::explosion_small,
            synth::explosion_big,
            synth::crunch,
            synth::chew,
            synth::pickup,
            synth::place,
            synth::deny,
            synth::complete,
            synth::level_up,
            synth::wave_alarm,
            synth::hurt,
            synth::game_over,
            synth::victory,
        ];
        let mut sounds = Vec::with_capacity(COUNT);
        for g in gens {
            sounds.push(load(g()).await);
        }
        Audio {
            sounds,
            music: load(synth::music()).await,
            hum: load(synth::nano_hum()).await,
            last: [-1.0; COUNT],
            music_on: false,
            hum_on: false,
            muted: false,
        }
    }

    pub fn play(&mut self, s: Sfx, now: f64) {
        let (gap, vol) = s.params();
        let i = s as usize;
        if self.muted || now - self.last[i] < gap {
            return;
        }
        self.last[i] = now;
        play_sound(
            &self.sounds[i],
            PlaySoundParams {
                looped: false,
                volume: vol,
            },
        );
    }

    /// Start the music loop; call after the first user input.
    pub fn start_music(&mut self) {
        if self.music_on {
            return;
        }
        self.music_on = true;
        play_sound(
            &self.music,
            PlaySoundParams {
                looped: true,
                volume: if self.muted { 0.0 } else { MUSIC_VOLUME },
            },
        );
    }

    pub fn set_hum(&mut self, on: bool) {
        if on == self.hum_on {
            return;
        }
        self.hum_on = on;
        if on {
            play_sound(
                &self.hum,
                PlaySoundParams {
                    looped: true,
                    volume: if self.muted { 0.0 } else { HUM_VOLUME },
                },
            );
        } else {
            stop_sound(&self.hum);
        }
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        let (m, h) = if self.muted {
            (0.0, 0.0)
        } else {
            (MUSIC_VOLUME, HUM_VOLUME)
        };
        set_sound_volume(&self.music, m);
        set_sound_volume(&self.hum, h);
    }

    pub fn on_event(&mut self, e: &Event, now: f64) {
        let s = match e {
            Event::Shot { heavy: true, .. } => Sfx::Heavy,
            Event::Shot { .. } => Sfx::Laser,
            Event::EnemyKilled { big: true, .. } => Sfx::BigBoom,
            Event::EnemyKilled { .. } => Sfx::Boom,
            Event::BuildingLost { .. } => Sfx::Crunch,
            Event::Placed => Sfx::Place,
            Event::Denied => Sfx::Deny,
            Event::Completed { .. } => Sfx::Complete,
            Event::Pickup => Sfx::Pickup,
            Event::LevelUp => Sfx::LevelUp,
            Event::WaveStart(_) => Sfx::Alarm,
            Event::CommanderHurt => Sfx::Hurt,
            Event::GameOver => Sfx::GameOver,
            Event::Victory => Sfx::Victory,
        };
        self.play(s, now);
    }

    pub fn chew(&mut self, now: f64) {
        self.play(Sfx::Chew, now);
    }
}
