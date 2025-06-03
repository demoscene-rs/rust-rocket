//! This crate implements a client library and a player for the [rocket sync tracker](https://github.com/rocket/rocket).
//! You can connect to a rocket tracker, get values from tracks, and live-edit your production.
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
//!
//! All features are mutually compatible, but if you choose to use `bincode` as your serialization library,
//! you don't need to use `serde`.
//!
//! # Usage
//!
//! The usual workflow with this library can be described in a few steps:
//!
//! 0. Install a rocket tracker ([original Qt editor](https://github.com/rocket/rocket)
//!    or [emoon's OpenGL-based editor](https://github.com/emoon/rocket))
//! 1. Connect the [`RocketClient`] to the running tracker by calling [`RocketClient::new`]
//! 2. Create tracks with [`get_track_mut`](RocketClient::get_track_mut)
//! 3. In your main loop, poll for updates from the Rocket tracker by calling [`poll_events`](RocketClient::poll_events).
//! 4. Keep the tracker in sync by calling [`set_row`](RocketClient::set_row) (see tips below)
//! 5. Get values from the tracks with [`Track::get_value`]
//!
//! See the linked documentation items and the examples-directory for more examples.
//!
//! # Tips
//!
//! The library is agnostic to your source of time. In a typical production, some kind of music player library or module
//! determines the time for everything else, including the rocket tracks.
//! It's recommended that you treat every 8th row as a beat of music instead of real time in seconds.
//!
//! ```rust,no_run
//! # use std::time::Duration;
//! # use rust_rocket::client::{RocketClient, Event, Error};
//! struct MusicPlayer(); // Your music player, not included in this crate
//! # impl MusicPlayer {
//! #     fn new() -> Self {
//! #         Self()
//! #     }
//! #     fn get_time(&self) -> Duration {
//! #         Duration::ZERO
//! #     }
//! #     fn seek(&self, _to: Duration) {}
//! #     fn pause(&self, _state: bool) {}
//! # }
//!
//! const ROWS_PER_BEAT: f32 = 8.;
//! const BEATS_PER_MIN: f32 = 123.; // This depends on your choice of music track
//! const SECS_PER_MIN: f32  = 60.;
//!
//! fn time_to_row(time: Duration) -> f32 {
//!     let secs = time.as_secs_f32();
//!     let beats = secs * BEATS_PER_MIN / SECS_PER_MIN;
//!     beats * ROWS_PER_BEAT
//! }
//!
//! fn row_to_time(row: u32) -> Duration {
//!     let beats = row as f32 / ROWS_PER_BEAT;
//!     let secs = beats / (BEATS_PER_MIN / SECS_PER_MIN);
//!     Duration::from_secs_f32(secs)
//! }
//!
//! fn get(rocket: &mut RocketClient, track: &str, row: f32) -> f32 {
//!     let track = rocket.get_track_mut(track).unwrap();
//!     track.get_value(row)
//! }
//!
//! fn demo_main() -> Result<(), Error> {
//!     let mut music = MusicPlayer::new(/* ... */);
//!     let mut rocket = RocketClient::new()?;
//!
//!     // Create window, render resources etc...
//!
//!     loop {
//!         while let Some(event) = rocket.poll_events()? {
//!             match event {
//!                 Event::SetRow(row) => music.seek(row_to_time(row)),
//!                 Event::Pause(state) => music.pause(state),
//!                 Event::SaveTracks => {/* Call save_tracks and serialize to a file */}
//!             }
//!         }
//!
//!         // Get current frame's time
//!         let time = music.get_time();
//!         let row = time_to_row(time);
//!
//!         // Keep the rocket tracker in sync
//!         rocket.set_row(row as u32);
//!
//!         // Render frame and read values with Track's get_value function
//!         let _ = get(&mut rocket, "track0", row);
//!     }
//! }
//! ```
pub mod client;
pub mod interpolation;
pub mod player;
pub mod track;

pub use client::RocketClient;
pub use player::RocketPlayer;
pub use track::Track;

/// Produced by [`RocketClient::save_tracks`] and consumed by [`RocketPlayer::new`]
pub type Tracks = Vec<Track>;
