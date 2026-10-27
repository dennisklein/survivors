//! Sound playback: synthesizes every effect at startup and throttles
//! repeated plays so a horde of hits does not turn into noise.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::time::get_time;
use sim::synth;
use sim::world::Event;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    Hit,
    Boom,
    BigBoom,
    Coin,
    Build,
    Thunk,
    LevelUp,
    Click,
    Alarm,
    GameOver,
    Victory,
    Whistle,
    Steam,
    Zap,
    Buzz,
}

const COUNT: usize = 16;
const MUSIC_VOL: f32 = 0.3;

impl Sfx {
    /// (minimum seconds between plays, volume)
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.3),
            Sfx::Hit => (0.08, 0.45),
            Sfx::Boom => (0.08, 0.45),
            Sfx::BigBoom => (0.2, 0.9),
            Sfx::Coin => (0.08, 0.55),
            Sfx::Build | Sfx::LevelUp => (0.2, 0.8),
            Sfx::Thunk => (0.15, 0.6),
            Sfx::Click => (0.05, 0.6),
            Sfx::Alarm => (1.5, 0.6),
            Sfx::GameOver | Sfx::Victory => (1.0, 0.9),
            Sfx::Whistle => (1.2, 0.5),
            Sfx::Steam => (0.15, 0.5),
            Sfx::Zap => (0.2, 0.5),
            Sfx::Buzz => (0.2, 0.6),
        }
    }
}

async fn load(samples: Vec<f32>) -> Sound {
    load_sound_from_bytes(&synth::wav_bytes(&samples))
        .await
        .expect("synthesized WAV decodes")
}

pub struct Audio {
    sounds: Vec<Sound>,
    last: [f64; COUNT],
    music: Sound,
    music_on: bool,
    pub muted: bool,
}

impl Audio {
    pub async fn new() -> Self {
        let mut sounds = Vec::with_capacity(COUNT);
        // Same order as `Sfx`.
        for s in [
            synth::shoot(),
            synth::hit(),
            synth::explode(0.3, 1),
            synth::explode(0.9, 2),
            synth::coin(),
            synth::build(),
            synth::thunk(),
            synth::level_up(),
            synth::click(),
            synth::alarm(),
            synth::game_over(),
            synth::victory(),
            synth::whistle(),
            synth::steam(),
            synth::zap(),
            synth::buzz(),
        ] {
            sounds.push(load(s).await);
        }
        Audio {
            sounds,
            last: [-10.0; COUNT],
            music: load(synth::music()).await,
            music_on: false,
            muted: false,
        }
    }

    /// Starts the background loop; call after the first user input so the
    /// browser lets the audio context run.
    pub fn start_music(&mut self) {
        if self.music_on {
            return;
        }
        self.music_on = true;
        play_sound(
            &self.music,
            PlaySoundParams {
                looped: true,
                volume: if self.muted { 0.0 } else { MUSIC_VOL },
            },
        );
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        set_sound_volume(&self.music, if self.muted { 0.0 } else { MUSIC_VOL });
    }

    pub fn play(&mut self, sfx: Sfx) {
        if self.muted {
            return;
        }
        let (gap, volume) = sfx.params();
        let now = get_time();
        let i = sfx as usize;
        if now - self.last[i] < gap {
            return;
        }
        self.last[i] = now;
        play_sound(
            &self.sounds[i],
            PlaySoundParams {
                looped: false,
                volume,
            },
        );
    }

    pub fn on_event(&mut self, e: &Event) {
        let sfx = match e {
            Event::Shot => Sfx::Shoot,
            Event::Blast => Sfx::Steam,
            Event::TrainHit | Event::TownHit => Sfx::Hit,
            Event::Killed { big: true, .. } | Event::Wrecked => Sfx::BigBoom,
            Event::Killed { .. } => Sfx::Boom,
            Event::Delivered { .. } => Sfx::Coin,
            Event::Built | Event::Repaired => Sfx::Build,
            Event::Removed | Event::TrackBroken(_) => Sfx::Thunk,
            Event::LevelUp => Sfx::LevelUp,
            Event::Picked => Sfx::Click,
            Event::Plasma => Sfx::Zap,
            Event::Departed => Sfx::Whistle,
            Event::UfoArrived | Event::BossArrived => Sfx::Alarm,
            Event::Won => Sfx::Victory,
            Event::Lost => {
                self.play(Sfx::BigBoom);
                Sfx::GameOver
            }
        };
        self.play(sfx);
    }
}
