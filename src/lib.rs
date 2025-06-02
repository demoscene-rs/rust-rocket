//! This crate implements a client library and player for the Rocket sync tracker.
//! You can connect to a rocket editor, get values from tracks, and save them to a file
//! (with the optional `serde` and `bincode` features).
//!
//! The [`client`] module contains types which you need to connect to a Rocket server and edit your production.
//!
//! The [`player`] module contains a [player implementation](RocketPlayer) which you can use in release mode.
//! It supports loading previously saved tracks and getting values from them.
//!
//! # Features
//!
//! - `serde`: Derive [serde](https://crates.io/crates/serde)'s traits on the [`Track`]-type
//! - `bincode`: Derive [bincode](https://crates.io/crates/bincode)'s traits on the [`Track`]-type
//!
//! Both features are mutually compatible, but if you choose to use bincode as your serialization library,
//! you don't need to use `serde`.
//!
//! # Usage
//!
//! Start by connecting the [`RocketClient`]. Then create tracks with [`get_track_mut`](RocketClient::get_track_mut)
//! and poll for updates from the Rocket server by calling [`poll_events`](RocketClient::poll_events) in your main loop.
//!
//! See linked documentation items for more examples.

pub mod client;
pub mod interpolation;
pub mod player;
pub mod track;

pub use client::RocketClient;
pub use player::RocketPlayer;
pub use track::Track;

/// Produced by [`RocketClient::save_tracks`] and consumed by [`RocketPlayer::new`]
pub type Tracks = Vec<Track>;
