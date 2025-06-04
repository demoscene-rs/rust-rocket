#![cfg(feature = "simple")]

//! An opinionated abstraction for the lower level [`client`](crate::client) and [`player`](crate::player) API.
//!
//! Requires the `simple` feature.
//! All errors are printed to stderr, and the connection to the tracker will be automatically re-established
//! as long as [`poll_events`](Rocket::poll_events) is called frequently enough.
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
//! cargo run                                 # Editing without player feature
//! cargo build --release --features player   # Release built with player feature
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
//! #     fn get_bpm(&self) -> f32 { 120. }
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
//!         let mut seek = None;
//!         while let Some(event) = rocket.poll_events().ok().flatten() {
//!             match dbg!(event) {
//!                 Event::Seek(to) => seek = Some(to),
//!                 Event::Pause(state) => music.pause(state),
//!                 Event::NotConnected => break,
//!             }
//!         }
//!         // It's recommended to call set_time only when the not seeking.
//!         // This ensures the smoothest scrolling in editor.
//!         match seek {
//!             Some(to) => {
//!                 music.seek(to);
//!                 continue;
//!             }
//!             None => rocket.set_time(&time),
//!         }
//!
//!         // Read values with Rocket's get_value function while rendering the frame
//!         let _ = rocket.get_value("track0");
//!     }
//! }
//! ```
//!
//! For a more thorough example, see `examples/simple.rs`.
//!
//! # Caveats
//!
//! - Can't choose how to handle [`saving the tracks`](crate::RocketClient::save_tracks), this uses [`std::fs::File`]
//!   and [`bincode`].
//! - Sub-optimal performance, the implementation does not support caching tracks
//!   (only [`get_value`](Rocket::get_value), no [`get_track`](crate::RocketClient::get_track)).
//!   It's unlikely that this causes noticeable slowdown unless you have an abnormally large amount of tracks.
//! - **Caution**: reconnection will wipe track state. Make sure to save in the editor before closing and reopening it.
//!
//! # Benefits
//!
//! - Get started quickly!
//! - Avoid writing `#[cfg(...)]`-attributes in your code.
//! - Sensible error handling that you may want to write anyway if you're not size-restricted.

use bincode::error::{DecodeError, EncodeError};
use std::{path::Path, time::Duration};

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
    /// The client is not connected. Next call to `poll_events` will attempt a reconnection.
    ///
    /// There are three equally sensible ways to handle this variant:
    ///
    /// 1. `break`: End your event polling `while let`-loop and proceed to rendering the frame.
    ///    All [`Rocket`] methods keep working, but without control from the tracker.
    /// 2. `continue 'main`: Restart your main loop, don't render the frame.
    ///    This lets you keep calling other event polling functions from other libraries, e.g. SDL or winit.
    /// 3. `{}`: Ignore it and let your event polling loop continue.
    ///
    /// Options 2 and 3 result is a busy wait, e.g. waste a lot of CPU time.
    /// It's better to combine them with `std::thread::sleep` for at least a few milliseconds in order to mitigate that.
    /// 
    /// See [module documentation](crate::simple#Examples) and [`poll_events`](Rocket::poll_events).
    NotConnected,
}

/// Provides sync values.
///
/// # Usage
///
/// See [module-level documentation](crate::simple#Usage).
pub struct Rocket<P: AsRef<Path>> {
    path: P,
    bps: f32,
    row: f32,
    #[cfg(not(feature = "player"))]
    sent_row: u32,
    #[cfg(not(feature = "player"))]
    connected: bool,
    #[cfg(not(feature = "player"))]
    connection_attempted: std::time::Instant,
    #[cfg(not(feature = "player"))]
    rocket: crate::RocketClient,
    #[cfg(feature = "player")]
    rocket: crate::RocketPlayer,
}

impl<P: AsRef<Path>> Rocket<P> {
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
    pub fn new(path: P, bpm: f32) -> Result<Self, DecodeError> {
        #[cfg(not(feature = "player"))]
        let rocket = loop {
            match Self::connect() {
                Ok(rocket) => break rocket,
                Err(_) => std::thread::sleep(Duration::from_secs(1)),
            }
        };

        #[cfg(feature = "player")]
        let rocket = {
            let mut file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(e) => {
                    print_msg(
                        PREFIX,
                        &format!("Failed to open {}", path.as_ref().display()),
                    );
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
                    print_msg(
                        PREFIX,
                        &format!("Failed to read {}", path.as_ref().display()),
                    );
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
            #[cfg(not(feature = "player"))]
            connection_attempted: std::time::Instant::now(),
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
        let track = match self.rocket.get_track_mut(track) {
            Ok(track) => track,
            Err(_) => {
                self.connected = false;
                return 0.;
            }
        };

        #[cfg(feature = "player")]
        let track = self.rocket.get_track(track).unwrap_or_else(|| {
            print_msg(
                PREFIX,
                &format!(
                    "Track {} doesn't exist in {}",
                    track,
                    self.path.as_ref().display()
                ),
            );
            panic!("{}: Can't recover", PREFIX);
        });

        track.get_value(self.row)
    }

    /// Update rocket with the current time from your time source, e.g. music player.
    pub fn set_time(&mut self, time: &Duration) {
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
    /// You should call this at least once per frame.
    /// It is recommended to keep calling this in a `while` loop until you receive `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Any errors that occur are first printed to stderr, then returned to the caller.
    ///
    /// An error is returned if the file specified in call to [`new`](Self::new) cannot be written to.
    ///
    /// The return value can be handled by calling [`unwrap`](Result::unwrap) if you want to panic,
    /// or `.ok().flatten()` if you want to ignore the error and continue.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use std::time::Duration;
    /// # use rust_rocket::simple::{Rocket, Event};
    /// # struct MusicPlayer; // Your music player, not included in this crate
    /// # impl MusicPlayer {
    /// #     fn new() -> Self { Self }
    /// #     fn get_time(&self) -> Duration { Duration::ZERO }
    /// #     fn seek(&self, _to: Duration) {}
    /// #     fn pause(&self, _state: bool) {}
    /// # }
    /// # let music = MusicPlayer::new();
    /// # let mut rocket = Rocket::new("tracks.bin", 60.).unwrap();
    /// while let Some(event) = rocket.poll_events().ok().flatten() {
    ///     match event {
    ///         Event::Seek(to) => music.seek(to),
    ///         Event::Pause(state) => music.pause(state),
    ///         Event::NotConnected => break,
    ///     }
    /// }
    /// ```
    ///
    /// # Tips
    ///
    /// There are three sensible ways to handle the `Event::NotConnected` variant:
    ///
    /// 1. `break`: End your event polling `while let`-loop and proceed to rendering the frame.
    ///    All [`Rocket`] methods keep working, but without control from the tracker.
    /// 2. `continue 'main`: Restart your main loop, don't render the frame.
    ///    This lets you keep calling other event polling functions from other libraries, e.g. SDL or winit.
    /// 3. `{}`: Ignore it and let your event polling loop continue.
    ///
    /// Options 2 and 3 result is a busy wait, e.g. waste a lot of CPU time.
    /// It's better to combine them with `std::thread::sleep` for at least a few milliseconds in order to mitigate that.
    ///
    /// # With `player` feature
    ///
    /// The function is a no-op.
    pub fn poll_events(&mut self) -> Result<Option<Event>, EncodeError> {
        #[cfg(not(feature = "player"))]
        loop {
            if !self.connected {
                // Don't spam connect
                if self.connection_attempted.elapsed() < Duration::from_secs(1) {
                    return Ok(Some(Event::NotConnected));
                }
                self.connection_attempted = std::time::Instant::now();
                match Self::connect() {
                    Ok(rocket) => {
                        self.rocket = rocket;
                        self.connected = true;
                    }
                    Err(_) => return Ok(Some(Event::NotConnected)),
                }
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
                    print_msg(
                        PREFIX,
                        &format!("Failed to open {}", self.path.as_ref().display()),
                    );
                    print_errors(PREFIX, &e);
                    return Err(EncodeError::Io { inner: e, index: 0 });
                }
            };

            let tracks = self.rocket.save_tracks();
            match bincode::encode_into_std_write(tracks, &mut file, bincode::config::standard()) {
                Ok(_) => {
                    print_msg(
                        PREFIX,
                        &format!("Tracks saved to {}", self.path.as_ref().display()),
                    );
                    Ok(())
                }
                Err(e) => {
                    print_msg(
                        PREFIX,
                        &format!("Failed to write to {}", self.path.as_ref().display()),
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
    fn connect() -> Result<crate::RocketClient, crate::client::Error> {
        print_msg(PREFIX, "Connecting...");
        crate::RocketClient::new()
    }
}

#[cfg(feature = "player")]
impl Rocket<&str> {
    /// An escape hatch constructor for advanced users who want to handle track loading via other means than `File`.
    ///
    /// This function is only available when the `player` feature is enabled, so you should not default to using it.
    ///
    /// # Usage
    ///
    /// The function makes it possible to load from e.g. [`std::include_bytes!`] in release builds.
    ///
    /// ```rust,no_run
    /// # use rust_rocket::simple::Rocket;
    /// # const SYNC_DATA: &[u8] = &[];
    /// // const SYNC_DATA: &[u8] = include_bytes!("tracks.bin");
    ///
    /// #[cfg(feature = "player")]
    /// let rocket = Rocket::from_std_read(&mut SYNC_DATA, 120.).unwrap_or_else(|_| unsafe {
    ///     std::hint::unreachable_unchecked()
    /// });
    /// ```
    pub fn from_std_read<R: std::io::Read>(read: &mut R, bpm: f32) -> Result<Self, DecodeError> {
        let tracks = bincode::decode_from_std_read(read, bincode::config::standard())?;
        let rocket = crate::RocketPlayer::new(tracks);
        Ok(Self {
            path: "release",
            bps: bpm / SECS_PER_MINUTE,
            row: 0.,
            rocket,
        })
    }
}
