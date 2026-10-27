//! Sound playback: synthesized effects, a looping music track, throttling and mute.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use rules::data::Cargo;
use rules::game::Event;
use rules::synth;

const MUSIC_VOLUME: f32 = 0.35;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    Mortar,
    Hit,
    Explosion,
    Kill,
    Gem,
    Crate,
    LevelUp,
    Build,
    Whistle,
    Unload,
    Hurt,
    Alarm,
    WaveStart,
    GameOver,
    Victory,
}

impl Sfx {
    const COUNT: usize = 16;

    /// Minimum seconds between two plays, and volume.
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.06, 0.35),
            Sfx::Mortar => (0.1, 0.6),
            Sfx::Hit => (0.05, 0.3),
            Sfx::Explosion => (0.08, 0.55),
            Sfx::Kill => (0.04, 0.4),
            Sfx::Gem => (0.04, 0.35),
            Sfx::Crate => (0.06, 0.6),
            Sfx::Alarm => (1.2, 0.6),
            Sfx::Hurt => (0.2, 0.7),
            _ => (0.0, 0.8),
        }
    }
}

pub struct Audio {
    sounds: Vec<Sound>,
    music: Sound,
    last_played: [f64; Sfx::COUNT],
    music_started: bool,
    pub muted: bool,
}

async fn load(samples: Vec<f32>) -> Sound {
    load_sound_from_bytes(&synth::wav_bytes(&samples))
        .await
        .expect("synthesized WAV must decode")
}

impl Audio {
    pub async fn new() -> Self {
        let gens: [fn() -> Vec<f32>; Sfx::COUNT] = [
            synth::shoot,
            synth::mortar,
            synth::hit,
            synth::explosion,
            synth::kill,
            synth::gem,
            synth::crate_pickup,
            synth::level_up,
            synth::build,
            synth::whistle,
            synth::unload,
            synth::hurt,
            synth::alarm,
            synth::wave_start,
            synth::game_over,
            synth::victory,
        ];
        let mut sounds = Vec::with_capacity(Sfx::COUNT);
        for g in gens {
            sounds.push(load(g()).await);
        }
        Self {
            sounds,
            music: load(synth::music()).await,
            last_played: [f64::NEG_INFINITY; Sfx::COUNT],
            music_started: false,
            muted: false,
        }
    }

    /// Start the music loop. Call after the first user input so browsers allow audio.
    pub fn start_music(&mut self) {
        if self.music_started {
            return;
        }
        self.music_started = true;
        play_sound(
            &self.music,
            PlaySoundParams {
                looped: true,
                volume: if self.muted { 0.0 } else { MUSIC_VOLUME },
            },
        );
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        let v = if self.muted { 0.0 } else { MUSIC_VOLUME };
        set_sound_volume(&self.music, v);
    }

    pub fn play(&mut self, sfx: Sfx) {
        if self.muted {
            return;
        }
        let i = sfx as usize;
        let (gap, volume) = sfx.params();
        let now = macroquad::time::get_time();
        if now - self.last_played[i] < gap {
            return;
        }
        self.last_played[i] = now;
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
            Event::Shot(Cargo::Coal) => Sfx::Mortar,
            Event::Shot(_) => Sfx::Shoot,
            Event::Hit(_) => Sfx::Hit,
            Event::Killed(..) => Sfx::Kill,
            Event::Explosion(..) => Sfx::Explosion,
            Event::Gem => Sfx::Gem,
            Event::Crate(_) => Sfx::Crate,
            Event::WeaponUnlocked(_) | Event::LevelUp => Sfx::LevelUp,
            Event::TrainArriving(_) => Sfx::Whistle,
            Event::Unloaded(_) => Sfx::Unload,
            Event::LineBought(_) | Event::LineExtended(_) => Sfx::Build,
            Event::PlayerHurt => Sfx::Hurt,
            Event::DepotHurt | Event::TrainMissed(_) | Event::BossSpawned => Sfx::Alarm,
            Event::WaveStart(_) | Event::WaveCleared(_) => Sfx::WaveStart,
            Event::Victory => Sfx::Victory,
            Event::GameOver => Sfx::GameOver,
            Event::TrainDocked(_) => return,
        };
        self.play(sfx);
    }
}
