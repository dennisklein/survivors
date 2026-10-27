//! Sound playback: synthesized effects, a looping music track, throttling and mute.

use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume,
};
use rules::game::Event;
use rules::synth;

const MUSIC_VOLUME: f32 = 0.3;

#[derive(Clone, Copy)]
pub enum Sfx {
    Shoot,
    MilitiaShot,
    Hit,
    Kill,
    Explosion,
    Coin,
    Serve,
    LevelUp,
    Build,
    Whistle,
    Alarm,
    Hurt,
    TownLost,
    Click,
    Deny,
    GameOver,
    Victory,
}

impl Sfx {
    const COUNT: usize = 17;

    /// Minimum seconds between two plays of the same effect, and its volume.
    fn params(self) -> (f64, f32) {
        match self {
            Sfx::Shoot => (0.07, 0.3),
            Sfx::MilitiaShot => (0.09, 0.22),
            Sfx::Hit => (0.06, 0.25),
            Sfx::Kill => (0.05, 0.35),
            Sfx::Explosion => (0.12, 0.6),
            Sfx::Coin => (0.05, 0.35),
            Sfx::Serve => (0.1, 0.5),
            Sfx::Alarm => (1.0, 0.45),
            Sfx::Hurt => (0.25, 0.6),
            Sfx::Click => (0.03, 0.4),
            _ => (0.0, 0.7),
        }
    }
}

pub struct Audio {
    sounds: Vec<Sound>,
    music: Sound,
    last: [f64; Sfx::COUNT],
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
            synth::militia_shot,
            synth::hit,
            synth::kill,
            synth::explosion,
            synth::coin,
            synth::serve,
            synth::level_up,
            synth::build,
            synth::whistle,
            synth::alarm,
            synth::hurt,
            synth::town_lost,
            synth::click,
            synth::deny,
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
            last: [f64::NEG_INFINITY; Sfx::COUNT],
            music_started: false,
            muted: false,
        }
    }

    fn music_volume(&self) -> f32 {
        if self.muted { 0.0 } else { MUSIC_VOLUME }
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
                volume: self.music_volume(),
            },
        );
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        set_sound_volume(&self.music, self.music_volume());
    }

    pub fn play(&mut self, sfx: Sfx) {
        if self.muted {
            return;
        }
        let i = sfx as usize;
        let (gap, volume) = sfx.params();
        let now = macroquad::time::get_time();
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
            Event::MilitiaShot => Sfx::MilitiaShot,
            Event::Hit => Sfx::Hit,
            Event::Killed { big: true, .. } => Sfx::Explosion,
            Event::Killed { .. } => Sfx::Kill,
            Event::Coin => Sfx::Coin,
            Event::Served { .. } => Sfx::Serve,
            Event::LevelUp => Sfx::LevelUp,
            Event::Built | Event::Upgraded => Sfx::Build,
            Event::Denied => Sfx::Deny,
            Event::WaveStart { .. } | Event::BossSpawned | Event::TownHurt(_) => Sfx::Alarm,
            Event::RescueDispatched | Event::RescueArrived(_) => Sfx::Whistle,
            Event::PlayerHurt => Sfx::Hurt,
            Event::TownLost(_) => Sfx::TownLost,
            Event::GameOver => Sfx::GameOver,
            Event::Victory => Sfx::Victory,
            Event::Junction => return,
        };
        self.play(sfx);
    }
}
