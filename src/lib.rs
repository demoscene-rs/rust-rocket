//! This crate implements a client library and a player for the [rocket sync tracker](https://github.com/rocket/rocket).
//! You can connect to a rocket tracker, get values from tracks, and live-edit your production.
//!
//! **There are two styles for using this crate:**
//!
//! ## Simple API
//!
//! See the [`simple`] module. Requires enabling the `simple`-feature.
//! Handles both editing and release playback use cases using conditional compilation.
//!
//! ## Low-level API
//!
//! The [`client`] module contains the types which you need to connect to a Rocket tracker and edit your production.
//!
//! The [`player`] module contains a player which you can use when building your production in release mode.
//!
//! # Features
//!
//! | Feature   | Purpose                                                                           |
//! | ---       | ---                                                                               |
//! | `serde`   | Derive [serde](https://crates.io/crates/serde)'s traits on the [`Track`]-type     |
//! | `bincode` | Derive [bincode](https://crates.io/crates/bincode)'s traits on the [`Track`]-type |
//! | `simple`  | Enables the [`simple`] API                                                        |
//! | `player`  | Builds the [`simple`] API in file player mode instead of client mode              |
//!
//! All features are mutually compatible, but if you choose to use `bincode` as your serialization library,
//! you don't need to use `serde`.
//!
//! The `simple` API enables `bincode`.

pub mod client;
pub mod interpolation;
pub mod player;
pub mod simple;
pub mod track;

pub use client::RocketClient;
pub use player::RocketPlayer;
pub use track::Track;

/// Produced by [`RocketClient::save_tracks`] and consumed by [`RocketPlayer::new`]
pub type Tracks = Vec<Track>;
