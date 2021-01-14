//! This is the rust-rocket crate.
//! It is designed to work as a client library for GNU Rocket.

pub mod client;
pub mod interpolation;
pub mod player;
pub mod track;

pub use client::Client;
pub use player::Player;
