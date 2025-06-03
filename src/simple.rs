#![cfg(feature = "simple")]

//! An opinionated abstraction for the lower level [`client`](crate::client) and [`player`](crate::player) API.
//!
//! Requires the `simple`-feature.
//! Errors are handled by printing to stderr, then attempting to reconnect where applicable, and panicking if not.
//!
//! # Usage
//!
//! First, install a rocket tracker ([original Qt editor](https://github.com/rocket/rocket)
//! or [emoon's OpenGL-based editor](https://github.com/emoon/rocket)).
//!
//! The [`Rocket`] type in this module compiles to different code depending on crate feature `player`.
//! When the feature is not enabled, the [`Rocket`] type uses [`RocketClient`](crate::RocketClient) internally.
//! When `player` is enabled, the [`Rocket`] type uses [`RocketPlayer`](crate::RocketPlayer) internally.
//!
//! Enable the feature in your production's Cargo.toml:
//! ```toml
//! [features]
//! player = ["rust-rocket/player"]
//!
//! [dependencies]
//! rust-rocket = { version = "0", features = ["simple"] }
//! ```
//!
//! And build your release accordingly:
//! ```console
//! cargo run                                 # Editing without player-feature
//! cargo build --release --features player   # Release built with player-feature
//! ```
//!
//! A main loop may look like this:
//! ```rust,no_run
//! # use std::time::Duration;
//! # use rust_rocket::simple::{Rocket, Event};
//! struct MusicPlayer; // Your music player, not included in this crate
//! # impl MusicPlayer {
//! #     fn new() -> Self { Self }
//! #     fn get_time(&self) -> Duration { Duration::ZERO }
//! #     fn get_bpm(&self) -> f32 { 0. }
//! #     fn seek(&self, _to: Duration) {}
//! #     fn pause(&self, _state: bool) {}
//! # }
//!
//! fn main() {
//!     let mut music = MusicPlayer::new(/* ... */);
//!     let mut rocket = Rocket::new("tracks.bin", music.get_bpm());
//!
//!     // Create window, render resources etc...
//!
//!     loop {
//!         // Get current frame's time
//!         let time = music.get_time();
//!
//!         // Keep the rocket tracker in sync.
//!         // It's recommended to combine consecutive seek events to a single seek.
//!         // This ensures the smoothest scrolling in editor.
//!         let mut seek = None;
//!         while let Some(event) = rocket.poll_events() {
//!             match dbg!(event) {
//!                 Event::Seek(to) => seek = Some(to),
//!                 Event::Pause(state) => music.pause(state),
//!             }
//!         }
//!         // It's recommended to call set_time only when the not seeking.
//!         match seek {
//!             Some(to) => {
//!                 music.seek(to);
//!                 continue;
//!             }
//!             None => rocket.set_time(time),
//!         }
//!
//!         // Read values with Rocket's get_value function while rendering the frame
//!         let _ = rocket.get_value("track0");
//!     }
//! }
//! ```
//!
//! # Caveats
//!
//! - Can't choose how to handle [`saving the tracks`](crate::RocketClient::save_tracks), this uses [`std::fs::File`]
//!   and [`bincode`].
//! - Sub-optimal performance, the implementation does not support caching tracks
//!   (only [`get_value`](Rocket::get_value), no [`get_track`](crate::RocketClient::get_track)).
//!   It's unlikely that this causes noticeable slowdown unless you have an abnormally large amount of tracks.
//! - Non-`player` builds: most functions may block if the connection to the rocket tracker is lost.
//! - Can't handle or ignore errors manually.
//!
//! # Benefits
//!
//! - Get started quickly!
//! - Avoid writing `#[cfg(...)]`-attributes in your code.
//! - Sensible error handling that you may want if you're not size-restricted.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const SECS_PER_MINUTE: f32 = 60.;
const ROWS_PER_BEAT: f32 = 8.;

fn print_msg(msg: &str) {
    eprintln!("rocket: {msg}");
}

fn print_errors(error: &dyn std::error::Error) {
    let mut error = Some(error);
    while let Some(e) = error {
        eprintln!("rocket: {e}");
        error = e.source();
    }
}

fn die(error: Option<&dyn std::error::Error>, msg: Option<&str>) -> ! {
    if let Some(msg) = msg {
        print_msg(msg);
    }
    if let Some(error) = error {
        print_errors(error);
    }
    panic!("rocket: Can't recover")
}

/// An `Event` type.
#[derive(Debug, Copy, Clone)]
pub enum Event {
    /// The tracker changes row, asking you to update your time source.
    Seek(Duration),
    /// The tracker pauses or unpauses.
    Pause(bool),
}

/// Provides sync values.
///
/// # Usage
///
/// See [module-level documentation](crate::simple#Usage).
pub struct Rocket {
    path: PathBuf,
    bps: f32,
    row: f32,
    #[cfg(not(feature = "player"))]
    rocket: crate::RocketClient,
    #[cfg(feature = "player")]
    rocket: crate::RocketPlayer,
}

impl Rocket {
    /// Initializes rocket.
    ///
    /// # Without `player` feature
    ///
    /// Attemps to connect to a rocket tracker, and retries indefinitely every 1s if connection can't be established,
    /// during which the caller is **blocked**.
    ///
    /// # With `player` feature
    ///
    /// Loads tracks from file specified by `path` using [`bincode`].
    ///
    /// # Panics
    ///
    /// With `player` feature: This function may panic if the file specified by `path` is unreadable or cannot be
    /// decoded by [`bincode`].
    pub fn new<P: AsRef<Path>>(path: P, bpm: f32) -> Self {
        let path = PathBuf::from(path.as_ref());

        #[cfg(not(feature = "player"))]
        let rocket = Self::connect();

        #[cfg(feature = "player")]
        let rocket = {
            let mut file = std::fs::File::open(&path).unwrap_or_else(|ref e| {
                die(Some(e), Some(&format!("Failed to open {}", path.display())))
            });
            let tracks = bincode::decode_from_std_read(&mut file, bincode::config::standard())
                .unwrap_or_else(|ref e| {
                    die(
                        Some(e),
                        Some(&format!("Failed to decode {}", path.display())),
                    )
                });
            crate::RocketPlayer::new(tracks)
        };

        Self {
            path,
            bps: bpm / SECS_PER_MINUTE,
            row: 0.,
            rocket,
        }
    }

    /// Get value based on previous call to [`set_time`](Self::set_time), by track name.
    ///
    /// # Panics
    ///
    /// With `player` feature: if the file specified in call to [`new`](Self::new) doesn't contain track with `name`,
    /// the function handles the error by printing to stderr and panicking.
    pub fn get_value(&mut self, track: &str) -> f32 {
        #[cfg(not(feature = "player"))]
        let track = loop {
            match self.rocket.get_track_mut(track) {
                Ok(track) => break track,
                Err(ref e) => {
                    print_errors(e);
                    self.rocket = Self::connect();
                }
            }
        };

        #[cfg(feature = "player")]
        let track = self.rocket.get_track(track).unwrap_or_else(|| {
            die(
                None,
                Some(&format!(
                    "Track {} doesn't exist in {}",
                    track,
                    self.path.display()
                )),
            )
        });

        track.get_value(self.row)
    }

    /// Update rocket with the current time from your time source, e.g. music player.
    pub fn set_time(&mut self, time: Duration) {
        let beat = time.as_secs_f32() * self.bps;
        self.row = beat * ROWS_PER_BEAT;

        #[cfg(not(feature = "player"))]
        while let Err(ref e) = self.rocket.set_row(self.row as u32) {
            print_errors(e);
            self.rocket = Self::connect();
        }
    }

    /// Poll for new events from rocket.
    ///
    /// # Without `player` feature
    ///
    /// This polls from events from the tracker.
    /// You should call this fairly often your main loop.
    /// It is recommended to keep calling this as long as your receive `Some(Event)`.
    ///
    /// # With `player` feature
    ///
    /// The function is a no-op.
    pub fn poll_events(&mut self) -> Option<Event> {
        #[cfg(not(feature = "player"))]
        loop {
            match self.rocket.poll_events() {
                Ok(Some(event)) => {
                    let handled = match event {
                        crate::client::Event::SetRow(row) => {
                            let beat = row as f32 / ROWS_PER_BEAT;
                            Event::Seek(Duration::from_secs_f32(beat / self.bps))
                        }
                        crate::client::Event::Pause(flag) => Event::Pause(flag),
                        crate::client::Event::SaveTracks => {
                            self.save_tracks();
                            continue;
                        }
                    };
                    return Some(handled);
                }
                Ok(None) => return None,
                Err(ref e) => {
                    print_errors(e);
                    self.rocket = Self::connect();
                }
            }
        }

        #[cfg(feature = "player")]
        None
    }

    /// Save a snapshot of the tracks in the session, overwriting the file specified in call to [`new`](Self::new).
    ///
    /// # With `player` feature
    ///
    /// The function is a no-op.
    pub fn save_tracks(&self) {
        #[cfg(not(feature = "player"))]
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&self.path)
                .unwrap_or_else(|ref e| {
                    die(
                        Some(e),
                        Some(&format!("Failed to open {}", self.path.display())),
                    )
                });

            let tracks = self.rocket.save_tracks();
            bincode::encode_into_std_write(tracks, &mut file, bincode::config::standard())
                .unwrap_or_else(|ref e| {
                    die(
                        Some(e),
                        Some(&format!("Failed to encode {}", self.path.display())),
                    )
                });
        }

        #[cfg(feature = "player")]
        (/* No-op */)
    }

    #[cfg(not(feature = "player"))]
    fn connect() -> crate::RocketClient {
        loop {
            print_msg("Connecting...");
            match crate::RocketClient::new() {
                Ok(rocket) => return rocket,
                Err(_) => std::thread::sleep(Duration::from_secs(1)),
            }
        }
    }
}
