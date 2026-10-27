//! Sound playback: synthesizes every effect at startup and throttles
//! repeated plays so a horde of hits does not turn into noise.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::time::get_time;
use rules::game::Event;
use rules::synth;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    Flak,
    Hit,
    Boom,
    BigBoom,
    Coin,
    Board,
    Fare,
    LevelUp,
    Build,
    Whistle,
    Alarm,
    GameOver,
    Victory,
}

const COUNT: usize = 14;
const MUSIC_VOL: f32 = 0.32;

impl Sfx {
    /// (minimum seconds between plays, volume)
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.45),
            Sfx::Flak => (0.12, 0.7),
            Sfx::Hit => (0.05, 0.5),
            Sfx::Boom => (0.1, 0.6),
            Sfx::BigBoom => (0.15, 0.9),
            Sfx::Coin => (0.06, 0.6),
            Sfx::Board => (0.05, 0.6),
            Sfx::Fare => (0.08, 0.6),
            Sfx::LevelUp | Sfx::Build | Sfx::Whistle | Sfx::Alarm => (0.2, 0.8),
            Sfx::GameOver | Sfx::Victory => (1.0, 0.9),
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
        for s in [
            synth::shoot(),
            synth::flak(),
            synth::hit(),
            synth::explode(0.35, 1),
            synth::explode(0.9, 2),
            synth::coin(),
            synth::board(),
            synth::fare(),
            synth::level_up(),
            synth::build(),
            synth::whistle(),
            synth::alarm(),
            synth::game_over(),
            synth::victory(),
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
            Event::Shot { heavy: true, .. } => Sfx::Flak,
            Event::Shot { .. } => Sfx::Shoot,
            Event::Hit { .. } | Event::CarHit { .. } => Sfx::Hit,
            Event::Boom { .. } => Sfx::Boom,
            Event::Kill { big: true, .. } | Event::CarLost { .. } => Sfx::BigBoom,
            Event::Kill { .. } => Sfx::Hit,
            Event::Coin { .. } => Sfx::Coin,
            Event::Board { .. } => Sfx::Board,
            Event::Fare { .. } => Sfx::Fare,
            Event::Arrive { .. } => Sfx::Whistle,
            Event::LevelUp => Sfx::LevelUp,
            Event::Built => Sfx::Build,
            Event::Surge | Event::BossSpawn => Sfx::Alarm,
            Event::Won => {
                self.play(Sfx::Whistle);
                Sfx::Victory
            }
            Event::Lost => {
                self.play(Sfx::BigBoom);
                Sfx::GameOver
            }
        };
        self.play(sfx);
    }
}
