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
    Hit,
    Boom,
    BigBoom,
    Pickup,
    Denied,
    LevelUp,
    Build,
    Siren,
    BossSiren,
    Hurt,
    Heal,
    GameOver,
    Victory,
    Click,
}

const COUNT: usize = 18;
const MUSIC_VOL: f32 = 0.35;

impl Sfx {
    /// (minimum seconds between plays, volume)
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.3),
            Sfx::Laser => (0.08, 0.22),
            Sfx::HeavyLaser => (0.15, 0.35),
            Sfx::Rocket => (0.15, 0.35),
            Sfx::Hit => (0.05, 0.35),
            Sfx::Boom => (0.07, 0.45),
            Sfx::BigBoom => (0.15, 0.8),
            Sfx::Pickup => (0.06, 0.4),
            Sfx::Denied => (0.15, 0.5),
            Sfx::Hurt => (0.35, 0.45),
            Sfx::Click => (0.05, 0.4),
            Sfx::LevelUp | Sfx::Build | Sfx::Siren | Sfx::Heal => (0.25, 0.8),
            Sfx::BossSiren | Sfx::GameOver | Sfx::Victory => (1.0, 0.9),
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
            synth::hit(),
            synth::explode(0.3, 1),
            synth::explode(0.9, 2),
            synth::pickup(),
            synth::denied(),
            synth::level_up(),
            synth::build(),
            synth::siren(false),
            synth::siren(true),
            synth::denied(),
            synth::cleared(),
            synth::game_over(),
            synth::victory(),
            synth::click(),
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
            Event::Hit(_) => Sfx::Hit,
            Event::Kill { big: true, .. } | Event::BuildingLost(_) => Sfx::BigBoom,
            Event::Kill { .. } | Event::Explosion { .. } => Sfx::Boom,
            Event::Pickup => Sfx::Pickup,
            Event::Heal => Sfx::Heal,
            Event::Denied => Sfx::Denied,
            Event::Built => Sfx::Build,
            Event::LevelUp => Sfx::LevelUp,
            Event::WaveStart(_) => Sfx::Siren,
            Event::BossSpawn => Sfx::BossSiren,
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
