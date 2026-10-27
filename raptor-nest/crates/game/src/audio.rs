//! Sound playback: synthesizes every effect at startup and throttles
//! repeated plays so a horde of hits does not turn into noise.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::time::get_time;
use sim::game::Event;
use sim::synth;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    Laser,
    HeavyLaser,
    Rocket,
    Spit,
    Hit,
    Boom,
    BigBoom,
    NestPop,
    Pickup,
    Denied,
    LevelUp,
    Build,
    Screech,
    Hurt,
    Cleared,
    GameOver,
    Victory,
    Click,
}

const COUNT: usize = 19;
const MUSIC_VOL: f32 = 0.35;

impl Sfx {
    /// (minimum seconds between plays, volume)
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.3),
            Sfx::Laser => (0.08, 0.22),
            Sfx::HeavyLaser => (0.15, 0.35),
            Sfx::Rocket => (0.15, 0.35),
            Sfx::Spit => (0.12, 0.3),
            Sfx::Hit => (0.05, 0.35),
            Sfx::Boom => (0.07, 0.45),
            Sfx::BigBoom => (0.15, 0.8),
            Sfx::NestPop => (0.3, 1.0),
            Sfx::Pickup => (0.06, 0.4),
            Sfx::Denied => (0.15, 0.5),
            Sfx::Hurt => (0.35, 0.45),
            Sfx::Click => (0.05, 0.4),
            Sfx::LevelUp | Sfx::Build | Sfx::Screech | Sfx::Cleared => (0.25, 0.8),
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
            synth::laser(false),
            synth::laser(true),
            synth::rocket(),
            synth::spit(),
            synth::hit(),
            synth::explode(0.3, 1),
            synth::explode(0.8, 2),
            synth::explode(1.4, 4),
            synth::pickup(),
            synth::denied(),
            synth::level_up(),
            synth::build(),
            synth::screech(0.8, 700.0),
            synth::screech(0.25, 1300.0),
            synth::cleared(),
            synth::game_over(),
            synth::victory(),
            synth::laser(false),
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
            Event::Beam { heavy: true, .. } => Sfx::HeavyLaser,
            Event::Beam { .. } => Sfx::Laser,
            Event::RocketLaunch => Sfx::Rocket,
            Event::Spit => Sfx::Spit,
            Event::Hit(_) => Sfx::Hit,
            Event::Kill { big: true, .. } | Event::StructureLost(_) => Sfx::BigBoom,
            Event::Kill { .. } | Event::Explosion { .. } => Sfx::Boom,
            Event::NestDestroyed(_) => Sfx::NestPop,
            Event::Pickup => Sfx::Pickup,
            Event::Denied => Sfx::Denied,
            Event::Built => Sfx::Build,
            Event::LevelUp => Sfx::LevelUp,
            Event::WaveStart(_) => Sfx::Screech,
            Event::WaveCleared(_) => Sfx::Cleared,
            Event::CommanderHurt => Sfx::Hurt,
            Event::Won => Sfx::Victory,
            Event::Lost => {
                self.play(Sfx::BigBoom);
                Sfx::GameOver
            }
        };
        self.play(sfx);
    }
}
