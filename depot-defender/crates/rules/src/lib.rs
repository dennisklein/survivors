//! Pure, renderer-independent rules for Depot Defender.
//!
//! Everything here is deterministic given a seed, so it can be unit tested
//! without a window or an audio device.

pub mod data;
pub mod game;
pub mod math;
pub mod rng;
pub mod synth;
