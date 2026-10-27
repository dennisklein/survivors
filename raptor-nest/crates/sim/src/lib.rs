//! Pure, deterministic rules for Raptor Nest.
//!
//! Nothing in this crate touches the window, input devices or audio output,
//! so every rule can be unit tested natively.

pub mod game;
pub mod math;
pub mod rng;
pub mod synth;
