#![cfg(feature = "simple")]

//! An opinionated abstraction for the lower level [`client`](crate::client) and [`player`](crate::player) API.
//!
//! Requires the `simple`-feature.
//! All errors are printed to stderr, and the connection to the tracker will be automatically re-established
//! where applicable.
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
//!     let mut rocket = Rocket::new("tracks.bin", music.get_bpm()).unwrap();
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
//!         while let Some(event) = rocket.poll_events().ok().flatten() {
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
//! - Non-`player` builds: the `poll_events`(Rocket::poll_events) function may block if the rocket tracker disconnects.
//! - **Caution**: reconnection will wipe track state. Make sure to save in the editor before closing and reopening it.
//!
//! # Benefits
//!
//! - Get started quickly!
//! - Avoid writing `#[cfg(...)]`-attributes in your code.
//! - Sensible error handling that you may want to write anyway if you're not size-restricted.

use bincode::error::{DecodeError, EncodeError};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const SECS_PER_MINUTE: f32 = 60.;
const ROWS_PER_BEAT: f32 = 8.;
const PREFIX: &str = "rocket";

/// Print a message to stderr. Prefixed with `prefix: `.
///
/// # Example
///
/// ```rust
/// use rust_rocket::simple::print_msg;
/// print_msg(env!("CARGO_CRATE_NAME"), "Using software renderer");
/// ```
pub fn print_msg(prefix: &str, msg: &str) {
    eprintln!("{prefix}: {msg}");
}

/// Print an error and its sources to stderr. Prefixed with `prefix: `.
pub fn print_errors(prefix: &str, error: &dyn std::error::Error) {
    eprintln!("{prefix}: {error}");
    let mut error = error.source();
    while let Some(e) = error {
        eprintln!("    Caused by: {e}");
        error = e.source();
    }
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
    sent_row: u32,
    #[cfg(not(feature = "player"))]
    connected: bool,
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
    /// during which the function doesn't return and the caller is **blocked**.
    ///
    /// # With `player` feature
    ///
    /// Loads tracks from file specified by `path` using [`bincode`].
    ///
    /// # Errors
    ///
    /// Any errors that occur are first printed to stderr, then returned to the caller.
    ///
    /// An error is returned If the file specified by `path` cannot be read or its contents cannot be decoded.
    ///
    /// The return value can be handled by calling [`unwrap`](Result::unwrap) if you want to panic,
    /// or [`ok`](Result::ok) if you want to ignore the error and continue without using rocket.
    pub fn new<P: AsRef<Path>>(path: P, bpm: f32) -> Result<Self, DecodeError> {
        let path = PathBuf::from(path.as_ref());

        #[cfg(not(feature = "player"))]
        let rocket = Self::connect();

        #[cfg(feature = "player")]
        let rocket = {
            let mut file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(e) => {
                    print_msg(PREFIX, &format!("Failed to open {}", path.display()));
                    print_errors(PREFIX, &e);
                    return Err(DecodeError::Io {
                        inner: e,
                        additional: 0,
                    });
                }
            };
            let tracks = match bincode::decode_from_std_read(&mut file, bincode::config::standard())
            {
                Ok(tracks) => tracks,
                Err(e) => {
                    print_msg(PREFIX, &format!("Failed to read {}", path.display()));
                    print_errors(PREFIX, &e);
                    return Err(e);
                }
            };
            crate::RocketPlayer::new(tracks)
        };

        Ok(Self {
            path,
            bps: bpm / SECS_PER_MINUTE,
            row: 0.,
            #[cfg(not(feature = "player"))]
            sent_row: 0,
            #[cfg(not(feature = "player"))]
            connected: true,
            rocket,
        })
    }

    /// Get value based on previous call to [`set_time`](Self::set_time), by track name.
    ///
    /// # Panics
    ///
    /// With `player` feature: if the file specified in call to [`new`](Self::new) doesn't contain track with `name`,
    /// the function handles the error by printing to stderr and panicking.
    pub fn get_value(&mut self, track: &str) -> f32 {
        #[cfg(not(feature = "player"))]
        let track = {
            if !self.connected {
                return 0.;
            }
            loop {
                match self.rocket.get_track_mut(track) {
                    Ok(track) => break track,
                    Err(ref e) => {
                        print_errors(PREFIX, e);
                        self.connected = false;
                        return 0.;
                    }
                }
            }
        };

        #[cfg(feature = "player")]
        let track = self.rocket.get_track(track).unwrap_or_else(|| {
            print_msg(
                PREFIX,
                &format!("Track {} doesn't exist in {}", track, self.path.display()),
            );
            panic!("{}: Can't recover", PREFIX);
        });

        track.get_value(self.row)
    }

    /// Update rocket with the current time from your time source, e.g. music player.
    pub fn set_time(&mut self, time: Duration) {
        let beat = time.as_secs_f32() * self.bps;
        self.row = beat * ROWS_PER_BEAT;

        #[cfg(not(feature = "player"))]
        {
            let row = self.row as u32;
            if self.connected && row != self.sent_row {
                while let Err(ref e) = self.rocket.set_row(row) {
                    print_errors(PREFIX, e);
                    self.connected = false;
                }
                self.sent_row = row;
            }
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
    /// # Errors
    ///
    /// Any errors that occur are first printed to stderr, then returned to the caller.
    ///
    /// An error is returned if the file specified in call to [`new`](Self::new) cannot be written to.
    ///
    /// The return value can be handled by calling [`unwrap`](Result::unwrap) if you want to panic,
    /// or [`ok`](Result::ok) if you want to ignore the error and continue.
    ///
    /// # With `player` feature
    ///
    /// The function is a no-op.
    pub fn poll_events(&mut self) -> Result<Option<Event>, EncodeError> {
        #[cfg(not(feature = "player"))]
        loop {
            if !self.connected {
                self.rocket = Self::connect();
                self.connected = true;
            }
            match self.rocket.poll_events() {
                Ok(Some(event)) => {
                    let handled = match event {
                        crate::client::Event::SetRow(row) => {
                            let beat = row as f32 / ROWS_PER_BEAT;
                            Event::Seek(Duration::from_secs_f32(beat / self.bps))
                        }
                        crate::client::Event::Pause(flag) => Event::Pause(flag),
                        crate::client::Event::SaveTracks => {
                            self.save_tracks()?;
                            continue;
                        }
                    };
                    return Ok(Some(handled));
                }
                Ok(None) => return Ok(None),
                Err(ref e) => {
                    print_errors(PREFIX, e);
                    self.connected = false;
                }
            }
        }

        #[cfg(feature = "player")]
        Ok(None)
    }

    /// Save a snapshot of the tracks in the session, overwriting the file specified in call to [`new`](Self::new).
    ///
    /// # Errors
    ///
    /// Any errors that occur are first printed to stderr, then returned to the caller.
    ///
    /// An error is returned if the file specified in call to [`new`](Self::new) cannot be written to.
    ///
    /// The return value can be handled by calling [`unwrap`](Result::unwrap) if you want to panic,
    /// or [`ok`](Result::ok) if you want to ignore the error and continue.
    ///
    /// # With `player` feature
    ///
    /// The function is a no-op.
    pub fn save_tracks(&self) -> Result<(), EncodeError> {
        #[cfg(not(feature = "player"))]
        {
            let open_result = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&self.path);

            let mut file = match open_result {
                Ok(file) => file,
                Err(e) => {
                    print_msg(PREFIX, &format!("Failed to open {}", self.path.display()));
                    print_errors(PREFIX, &e);
                    return Err(EncodeError::Io { inner: e, index: 0 });
                }
            };

            let tracks = self.rocket.save_tracks();
            match bincode::encode_into_std_write(tracks, &mut file, bincode::config::standard()) {
                Ok(_) => {
                    print_msg(PREFIX, &format!("Tracks saved to {}", self.path.display()));
                    Ok(())
                }
                Err(e) => {
                    print_msg(
                        PREFIX,
                        &format!("Failed to write to {}", self.path.display()),
                    );
                    print_errors(PREFIX, &e);
                    Err(e)
                }
            }
        }

        #[cfg(feature = "player")]
        Ok((/* No-op */))
    }

    #[cfg(not(feature = "player"))]
    fn connect() -> crate::RocketClient {
        loop {
            print_msg(PREFIX, "Connecting...");
            match crate::RocketClient::new() {
                Ok(rocket) => return rocket,
                Err(_) => std::thread::sleep(Duration::from_secs(1)),
            }
        }
    }
}
