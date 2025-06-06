//! An abstraction for the [lower level API](crate::lowlevel).
//!
//! Errors are printed to stderr, and the connection to the tracker will be automatically re-established
//! as long as [`poll_events`](Rocket::poll_events) is called frequently enough.
//!
//! # Usage
//!
//! First, install a rocket tracker ([original Qt editor](https://github.com/rocket/rocket)
//! or [emoon's OpenGL-based editor](https://github.com/emoon/rocket)).
//!
//! The [`Rocket`] type in this module compiles to different code depending on crate feature `client`.
//! When the feature is enabled, the [`Rocket`] type connects to a rocket tracker when possible.
//! Otherwise [`Rocket`] only plays back [`Tracks`] that you construct it with.
//!
//! Configure the feature in your production's Cargo.toml:
//! ```toml
//! [features]
//! default = ["rocket-client"]
//! rocket-client = ["rust-rocket/client"]
//!
//! [dependencies]
//! rust-rocket = "0"
//! ```
//!
//! And build your release accordingly:
//! ```console
//! cargo run                                      # Editing with the client feature
//! cargo build --release --no-default-features    # Build a release without client feature
//! ```
//!
//! A main loop may look like this:
//! ```rust,no_run
//! # use std::time::Duration;
//! # use rust_rocket::{Event, Rocket, Tracks};
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
//!     let mut rocket = Rocket::new(Tracks::default(), music.get_bpm());
//!
//!     // Create window, render resources etc...
//!
//!     loop {
//!         // Handle events from the rocket tracker
//!         while let Some(event) = rocket.poll_events() {
//!             match event {
//!                 Event::Seek(to) => music.seek(to),
//!                 Event::Pause(state) => music.pause(state),
//!                 Event::SaveTracks => {/* Call rocket.get_tracks() and serialize to a file */},
//!                 Event::NotConnected => break,
//!             }
//!         }
//!
//!         // Get current frame's time and keep the tracker updated
//!         let time = music.get_time();
//!         rocket.set_time(&time);
//!
//!         // Read values with Rocket's get_value function while rendering the frame
//!         let _ = rocket.get_value("track0");
//!     }
//! }
//! ```
//!
//! For a more thorough example,
//! see [`examples/simple.rs`](https://github.com/demoscene-rs/rust-rocket/blob/master/examples/simple.rs).
//!
//! **Caution**: reconnection will wipe track state. Make sure to use the save feature in the editor!

#[cfg(feature = "client")]
use crate::lowlevel::client::{self, Client};
use crate::lowlevel::Tracks;
use std::time::Duration;

const SECS_PER_MINUTE: f32 = 60.;
const ROWS_PER_BEAT: f32 = 8.;
const PREFIX: &str = "rocket";

/// Print a message to stderr. Prefixed with `prefix: `.
fn print_msg(prefix: &str, msg: &str) {
    eprintln!("{prefix}: {msg}");
}

/// Print an error and its sources to stderr. Prefixed with `prefix: `.
#[cfg(feature = "client")]
fn print_errors(prefix: &str, error: &dyn std::error::Error) {
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
    /// The tracker asks you to export tracks.
    SaveTracks,
    /// The client is not connected. Next calls to [`poll_events`](Rocket::poll_events) will eventually attempt to
    /// reconnect.
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
    /// See [`simple.rs`](https://github.com/demoscene-rs/rust-rocket/blob/master/examples/simple.rs) in the
    /// `examples`-directory.
    NotConnected,
}

/// Provides sync values.
///
/// # Usage
///
/// See [module documentation](crate::rocket#Usage).
pub struct Rocket {
    bps: f32,
    row: f32,
    tracks: Tracks,
    #[cfg(feature = "client")]
    tracker_row: u32,
    #[cfg(feature = "client")]
    connection_attempted: std::time::Instant,
    #[cfg(feature = "client")]
    client: Option<Client>,
}

impl Rocket {
    /// Initializes rocket.
    ///
    /// # When the `client` feature is enabled
    ///
    /// Attempts to connect to a rocket tracker.
    pub fn new(tracks: Tracks, bpm: f32) -> Self {
        #[cfg(feature = "client")]
        let client = Self::connect().ok();

        Self {
            bps: bpm / SECS_PER_MINUTE,
            row: 0.,
            tracks,
            #[cfg(feature = "client")]
            tracker_row: 0,
            #[cfg(feature = "client")]
            connection_attempted: std::time::Instant::now(),
            #[cfg(feature = "client")]
            client,
        }
    }

    /// Get track value based on previous call to [`set_time`](Self::set_time).
    ///
    /// # Panics
    ///
    /// If the `client` feature is not enabled and the `tracks` passed to [`Rocket::new`] don't contain a track
    /// with `name`, the function handles the error by printing to stderr and panicking.
    pub fn get_value(&mut self, track: &str) -> f32 {
        #[cfg(feature = "client")]
        let track = match &mut self.client {
            Some(client) => match client.get_track_mut(&mut self.tracks, track) {
                Ok(track) => track,
                Err(ref e) => {
                    print_errors(PREFIX, e);
                    self.client = None;
                    return 0.;
                }
            },
            None => match self.tracks.get_track(track) {
                Some(track) => track,
                None => return 0.,
            },
        };

        #[cfg(not(feature = "client"))]
        let track = self.tracks.get_track(track).unwrap_or_else(|| {
            print_msg(PREFIX, &format!("Track {} doesn't exist", track,));
            panic!("{}: Can't recover", PREFIX);
        });

        track.get_value(self.row)
    }

    /// Update rocket with the current time from your time source, e.g. music player.
    pub fn set_time(&mut self, time: &Duration) {
        let beat = time.as_secs_f32() * self.bps;
        self.row = beat * ROWS_PER_BEAT;

        #[cfg(feature = "client")]
        {
            let row = self.row as u32;
            if let Some(client) = &mut self.client {
                if row != self.tracker_row {
                    match client.set_row(row) {
                        Ok(()) => self.tracker_row = row,
                        Err(ref e) => {
                            print_errors(PREFIX, e);
                            self.client = None;
                        }
                    }
                }
            }
        }
    }

    /// Poll for new events from rocket.
    ///
    /// # When the `client` feature is enabled
    ///
    /// This polls events from the tracker.
    /// You should call this at least once per frame.
    /// It is recommended to keep calling this in a `while` loop until you receive `Ok(None)`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use std::time::Duration;
    /// # use rust_rocket::{Event, Rocket, Tracks};
    /// # struct MusicPlayer; // Your music player, not included in this crate
    /// # impl MusicPlayer {
    /// #     fn new() -> Self { Self }
    /// #     fn get_time(&self) -> Duration { Duration::ZERO }
    /// #     fn seek(&self, _to: Duration) {}
    /// #     fn pause(&self, _state: bool) {}
    /// # }
    /// # let music = MusicPlayer::new();
    /// # let mut rocket = Rocket::new(Tracks::default(), 60.);
    /// while let Some(event) = rocket.poll_events() {
    ///     match event {
    ///         Event::Seek(to) => music.seek(to),
    ///         Event::Pause(state) => music.pause(state),
    ///         Event::SaveTracks => {/* Call rocket.get_tracks() and serialize to a file */},
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
    /// # Without `client` feature
    ///
    /// The function is a no-op.
    pub fn poll_events(&mut self) -> Option<Event> {
        #[cfg(feature = "client")]
        loop {
            match &mut self.client {
                None => {
                    // Don't spam connect
                    if self.connection_attempted.elapsed() < Duration::from_secs(1) {
                        return Some(Event::NotConnected);
                    }
                    self.connection_attempted = std::time::Instant::now();
                    match Self::connect() {
                        Ok(rocket) => {
                            self.client = Some(rocket);
                        }
                        Err(_) => return Some(Event::NotConnected),
                    }
                }
                Some(client) => match client.poll_events(&mut self.tracks) {
                    Ok(Some(event)) => {
                        let handled = match event {
                            client::Event::SetRow(row) => {
                                self.tracker_row = row;
                                let beat = row as f32 / ROWS_PER_BEAT;
                                Event::Seek(Duration::from_secs_f32(beat / self.bps))
                            }
                            client::Event::Pause(flag) => Event::Pause(flag),
                            client::Event::SaveTracks => Event::SaveTracks,
                        };
                        return Some(handled);
                    }
                    Ok(None) => return None,
                    Err(ref e) => {
                        print_errors(PREFIX, e);
                        self.client = None;
                    }
                },
            }
        }

        #[cfg(not(feature = "client"))]
        None
    }

    /// Get a reference to current [`Tracks`] state.
    ///
    /// [`Tracks`] can be serialized and written to a file when the `serde` or
    /// `bincode` features are enabled.
    /// See [`examples/simple.rs`](https://github.com/demoscene-rs/rust-rocket/blob/master/examples/simple.rs) for an
    /// example.
    pub fn get_tracks(&self) -> &Tracks {
        &self.tracks
    }

    #[cfg(feature = "client")]
    fn connect() -> Result<Client, client::Error> {
        print_msg(PREFIX, "Connecting...");
        Client::new()
    }
}
