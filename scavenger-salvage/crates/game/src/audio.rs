//! Sound playback: synthesizes every effect at startup and throttles
//! repeated plays so a horde of hits does not turn into noise.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use macroquad::time::get_time;
use sim::game::{Event, StructKind};
use sim::synth;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    Tower,
    Hit,
    Boom,
    BigBoom,
    Deposit,
    Grab,
    Release,
    LevelUp,
    Build,
    BuildStart,
    Alarm,
    GameOver,
    Victory,
    Heavy,
}

const COUNT: usize = 15;
const MUSIC_VOL: f32 = 0.3;

impl Sfx {
    /// (minimum seconds between plays, volume)
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.35),
            Sfx::Tower => (0.1, 0.45),
            Sfx::Hit => (0.05, 0.4),
            Sfx::Boom => (0.08, 0.55),
            Sfx::BigBoom => (0.15, 0.9),
            Sfx::Deposit => (0.06, 0.6),
            Sfx::Grab => (0.1, 0.6),
            Sfx::Release => (0.1, 0.6),
            Sfx::LevelUp | Sfx::Build => (0.25, 0.8),
            Sfx::BuildStart => (0.2, 0.5),
            Sfx::Alarm => (1.5, 0.6),
            Sfx::GameOver | Sfx::Victory => (1.0, 0.9),
            Sfx::Heavy => (0.3, 0.7),
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
            synth::metal(),
            synth::thunk(),
            synth::level_up(),
            synth::build(),
            synth::reclaim(),
            synth::alarm(),
            synth::game_over(),
            synth::victory(),
            synth::dgun(),
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
            Event::TurretShot(StructKind::Annihilator) => Sfx::Heavy,
            Event::TurretShot(_) => Sfx::Tower,
            Event::Hit | Event::PlayerHurt => Sfx::Hit,
            Event::Kill { big: true, .. } => Sfx::BigBoom,
            Event::Kill { .. } => Sfx::Boom,
            Event::Grab => Sfx::Grab,
            Event::Release => Sfx::Release,
            Event::Deposit(_) => Sfx::Deposit,
            Event::BuildStart(_) => Sfx::BuildStart,
            Event::Built(_) => Sfx::Build,
            Event::LevelUp => Sfx::LevelUp,
            Event::WaveStart(_) | Event::RecyclerHit => Sfx::Alarm,
            Event::Won => Sfx::Victory,
            Event::Lost => {
                self.play(Sfx::BigBoom);
                Sfx::GameOver
            }
        };
        self.play(sfx);
    }
}
