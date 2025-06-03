//! Main client code, including the [`RocketClient`] type.
//!
//! # Usage
//!
//! The usual workflow with the low level client API can be described in a few steps:
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
//! The library is agnostic to your source of time. In a typical production, some kind of music player library
//! determines the time for everything else, including the rocket tracks.
//! It's recommended that you treat every 8th row as a beat of music instead of real time in seconds.
//!
//! ```rust,no_run
//! # use std::time::Duration;
//! # use rust_rocket::client::{RocketClient, Event, Error};
//! struct MusicPlayer; // Your music player, not included in this crate
//! # impl MusicPlayer {
//! #     fn new() -> Self { Self }
//! #     fn get_time(&self) -> Duration { Duration::ZERO }
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
//! fn main() -> Result<(), Error> {
//!     let mut music = MusicPlayer::new(/* ... */);
//!     let mut rocket = RocketClient::new()?;
//!
//!     // Create window, render resources etc...
//!
//!     loop {
//!         // Get current frame's time
//!         let time = music.get_time();
//!         let row = time_to_row(time);
//!
//!         // Keep the rocket tracker in sync
//!         // It's recommended to combine consecutive seek events to a single seek.
//!         // This ensures the smoothest scrolling in editor.
//!         let mut seek = None;
//!         while let Some(event) = rocket.poll_events()? {
//!             match event {
//!                 Event::SetRow(to) => seek = Some(to),
//!                 Event::Pause(state) => music.pause(state),
//!                 Event::SaveTracks => {/* Call save_tracks and serialize to a file */}
//!             }
//!         }
//!         // It's recommended to call set_time only when the not seeking.
//!         match seek {
//!             Some(to_row) => {
//!                 music.seek(row_to_time(to_row));
//!                 continue;
//!             }
//!             None => rocket.set_row(row as u32)?,
//!         }
//!
//!         // Render frame and read values with Track's get_value function
//!         let _ = get(&mut rocket, "track0", row);
//!     }
//! }
//! ```
use crate::interpolation::*;
use crate::track::*;
use crate::Tracks;

use byteorder::ByteOrder;
use byteorder::{BigEndian, ReadBytesExt};
use std::hint::unreachable_unchecked;
use std::{
    convert::TryFrom,
    io::{self, Cursor, Read, Write},
    net::{TcpStream, ToSocketAddrs},
};
use thiserror::Error;

// Rocket protocol commands
const CLIENT_GREETING: &[u8] = b"hello, synctracker!";
const SERVER_GREETING: &[u8] = b"hello, demo!";

const SET_KEY: u8 = 0;
const DELETE_KEY: u8 = 1;
const GET_TRACK: u8 = 2;
const SET_ROW: u8 = 3;
const PAUSE: u8 = 4;
const SAVE_TRACKS: u8 = 5;

const SET_KEY_LEN: usize = 4 + 4 + 4 + 1;
const DELETE_KEY_LEN: usize = 4 + 4;
const GET_TRACK_LEN: usize = 4; // Does not account for name length
const SET_ROW_LEN: usize = 4;
const PAUSE_LEN: usize = 1;

const MAX_COMMAND_LEN: usize = SET_KEY_LEN;

/// The `Error` Type. This is the main error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Failure to connect to a rocket tracker. This can happen if the tracker is not running, the
    /// address isn't correct or other network-related reasons.
    #[error("Failed to establish a TCP connection with the Rocket tracker")]
    Connect(#[source] std::io::Error),
    /// Failure to transmit or receive greetings with the tracker
    #[error("Handshake with the Rocket tracker failed")]
    Handshake(#[source] std::io::Error),
    /// Handshake was performed but the the received greeting wasn't correct
    #[error("The Rocket tracker greeting {0:?} wasn't correct")]
    HandshakeGreetingMismatch([u8; SERVER_GREETING.len()]),
    /// Error from [`TcpStream::set_nonblocking`]
    #[error("Cannot set Rocket's TCP connection to nonblocking mode")]
    SetNonblocking(#[source] std::io::Error),
    /// Network IO error during operation
    #[error("Rocket tracker disconnected")]
    IOError(#[source] std::io::Error),
}

#[derive(Debug)]
enum ClientState {
    New,
    Incomplete(usize),
    Complete,
}

/// The `Event` Type. These are the various events from the tracker.
#[derive(Debug, Copy, Clone)]
pub enum Event {
    /// The tracker changes row.
    SetRow(u32),
    /// The tracker pauses or unpauses.
    Pause(bool),
    /// The tracker asks us to save our track data.
    /// You may want to call [`RocketClient::save_tracks`] after receiving this event.
    SaveTracks,
}

#[derive(Debug)]
enum ReceiveResult {
    Some(Event),
    None,
    Incomplete,
}

/// The `RocketClient` type. This contains the connected socket and other fields.
#[derive(Debug)]
pub struct RocketClient {
    stream: TcpStream,
    state: ClientState,
    cmd: Vec<u8>,
    tracks: Vec<Track>,
}

impl RocketClient {
    /// Construct a new RocketClient.
    ///
    /// This constructs a new Rocket client and connects to localhost on port 1338.
    ///
    /// # Errors
    ///
    /// [`Error::Connect`] if connection cannot be established, or [`Error::Handshake`]
    /// if the handshake fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::RocketClient;
    /// let mut rocket = RocketClient::new()?;
    /// # Ok::<(), rust_rocket::client::Error>(())
    /// ```
    pub fn new() -> Result<Self, Error> {
        Self::connect(("localhost", 1338))
    }

    /// Construct a new RocketClient.
    ///
    /// This constructs a new Rocket client and connects to a specified host and port.
    ///
    /// # Errors
    ///
    /// [`Error::Connect`] if connection cannot be established, or [`Error::Handshake`]
    /// if the handshake fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::RocketClient;
    /// let mut rocket = RocketClient::connect(("localhost", 1338))?;
    /// # Ok::<(), rust_rocket::client::Error>(())
    /// ```
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr).map_err(Error::Connect)?;

        let mut rocket = Self {
            stream,
            state: ClientState::New,
            cmd: Vec::new(),
            tracks: Vec::new(),
        };

        rocket.handshake()?;

        rocket
            .stream
            .set_nonblocking(true)
            .map_err(Error::SetNonblocking)?;

        Ok(rocket)
    }

    /// Get track by name.
    ///
    /// If the track does not yet exist it will be created.
    ///
    /// # Errors
    ///
    /// This method can return an [`Error::IOError`] if Rocket tracker disconnects.
    ///
    /// # Panics
    ///
    /// Will panic if `name`'s length exceeds [`u32::MAX`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::RocketClient;
    /// # let mut rocket = RocketClient::new()?;
    /// let track = rocket.get_track_mut("namespace:track")?;
    /// track.get_value(3.5);
    /// # Ok::<(), rust_rocket::client::Error>(())
    /// ```
    pub fn get_track_mut(&mut self, name: &str) -> Result<&mut Track, Error> {
        if let Some((i, _)) = self
            .tracks
            .iter()
            .enumerate()
            .find(|(_, t)| t.get_name() == name)
        {
            Ok(&mut self.tracks[i])
        } else {
            // Send GET_TRACK message
            let mut buf = [GET_TRACK; 1 + GET_TRACK_LEN];
            let name_len = u32::try_from(name.len()).expect("Track name too long");
            BigEndian::write_u32(&mut buf[1..][..GET_TRACK_LEN], name_len);
            self.stream.write_all(&buf).map_err(Error::IOError)?;
            self.stream
                .write_all(name.as_bytes())
                .map_err(Error::IOError)?;

            self.tracks.push(Track::new(name));
            let track = self.tracks.last_mut().unwrap_or_else(||
                // SAFETY: tracks cannot be empty, because it was pushed to on the previous line
                unsafe{ unreachable_unchecked() });
            Ok(track)
        }
    }

    /// Get track by name.
    ///
    /// You should use [`get_track_mut`](RocketClient::get_track_mut) to create a track.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.get_name() == name)
    }

    /// Get a snapshot of the tracks in the session.
    ///
    /// The returned [`Tracks`] can be dumped to a file in any [supported format](crate#features).
    /// The counterpart to this function is [`RocketPlayer::new`](crate::RocketPlayer::new),
    /// which loads tracks for playback.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use rust_rocket::RocketClient;
    /// # use std::fs::OpenOptions;
    /// let mut rocket = RocketClient::new()?;
    ///
    /// // Create tracks, call poll_events, etc...
    ///
    /// // Open a file for writing
    /// let mut file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///     .truncate(true)
    ///     .open("tracks.bin")
    ///     .expect("Failed to open tracks.bin for writing");
    ///
    /// // Save a snapshot of the client to a file for playback in release builds
    /// let tracks = rocket.save_tracks();
    /// # #[cfg(feature = "bincode")]
    /// bincode::encode_into_std_write(tracks, &mut file, bincode::config::standard())
    ///     .expect("Failed to encode tracks.bin");
    /// # Ok::<(), rust_rocket::client::Error>(())
    /// ```
    pub fn save_tracks(&self) -> &Tracks {
        &self.tracks
    }

    /// Send a SetRow message.
    ///
    /// This changes the current row on the tracker side.
    ///
    /// # Errors
    ///
    /// This method can return an [`Error::IOError`] if Rocket tracker disconnects.
    pub fn set_row(&mut self, row: u32) -> Result<(), Error> {
        // Send SET_ROW message
        let mut buf = [SET_ROW; 1 + SET_ROW_LEN];
        BigEndian::write_u32(&mut buf[1..][..SET_ROW_LEN], row);
        self.stream.write_all(&buf).map_err(Error::IOError)
    }

    /// Poll for new events from the tracker.
    ///
    /// This polls from events from the tracker.
    /// You should call this fairly often your main loop.
    /// It is recommended to keep calling this as long as your receive `Some(Event)`.
    ///
    /// # Errors
    ///
    /// This method can return an [`Error::IOError`] if the rocket tracker disconnects.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::RocketClient;
    /// # let mut rocket = RocketClient::new()?;
    /// while let Some(event) = rocket.poll_events()? {
    ///     match event {
    ///         // Do something with the various events.
    ///         _ => (),
    ///     }
    /// }
    /// # Ok::<(), rust_rocket::client::Error>(())
    /// ```
    pub fn poll_events(&mut self) -> Result<Option<Event>, Error> {
        loop {
            match self.poll_event()? {
                ReceiveResult::None => return Ok(None),
                ReceiveResult::Incomplete => { /* Keep reading */ }
                ReceiveResult::Some(event) => return Ok(Some(event)),
            }
        }
    }

    fn poll_event(&mut self) -> Result<ReceiveResult, Error> {
        match self.state {
            ClientState::New => self.poll_event_new(),
            ClientState::Incomplete(bytes) => self.poll_event_incomplete(bytes),
            ClientState::Complete => Ok(self.process_event().unwrap_or_else(|_| unreachable!())),
        }
    }

    fn poll_event_new(&mut self) -> Result<ReceiveResult, Error> {
        let mut buf = [0; 1];
        match self.stream.read_exact(&mut buf) {
            Ok(()) => {
                self.cmd.extend_from_slice(&buf);
                match self.cmd[0] {
                    SET_KEY => self.state = ClientState::Incomplete(SET_KEY_LEN),
                    DELETE_KEY => self.state = ClientState::Incomplete(DELETE_KEY_LEN),
                    SET_ROW => self.state = ClientState::Incomplete(SET_ROW_LEN),
                    PAUSE => self.state = ClientState::Incomplete(PAUSE_LEN),
                    SAVE_TRACKS => self.state = ClientState::Complete,
                    _ => self.state = ClientState::Complete, // Error / Unknown
                }
                Ok(ReceiveResult::Incomplete)
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock => Ok(ReceiveResult::None),
                _ => Err(Error::IOError(e)),
            },
        }
    }

    fn poll_event_incomplete(&mut self, bytes: usize) -> Result<ReceiveResult, Error> {
        let mut buf = [0; MAX_COMMAND_LEN];
        match self.stream.read(&mut buf[..bytes]) {
            Ok(bytes_read) => {
                self.cmd.extend_from_slice(&buf[..bytes_read]);
                if bytes - bytes_read > 0 {
                    self.state = ClientState::Incomplete(bytes - bytes_read);
                } else {
                    self.state = ClientState::Complete;
                }
                Ok(ReceiveResult::Incomplete)
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock => Ok(ReceiveResult::None),
                _ => Err(Error::IOError(e)),
            },
        }
    }

    // This function should never fail if [`poll_event_new`] and [`poll_event_incomplete`] are correct
    fn process_event(&mut self) -> Result<ReceiveResult, io::Error> {
        let mut result = ReceiveResult::None;

        let mut cursor = Cursor::new(&self.cmd);
        let cmd = cursor.read_u8()?;
        match cmd {
            SET_KEY => {
                // usize::try_from(u32) will only be None if usize is smaller, and
                // more than usize::MAX tracks are in use. That isn't possible because
                // I'd imagine Vec::push and everything else will panic first.
                // If you're running this on a microcontroller, I'd love to see it!
                let index = usize::try_from(cursor.read_u32::<BigEndian>()?).unwrap();
                let track = &mut self.tracks[index];
                let row = cursor.read_u32::<BigEndian>()?;
                let value = cursor.read_f32::<BigEndian>()?;
                let interpolation = Interpolation::from(cursor.read_u8()?);
                let key = Key::new(row, value, interpolation);

                track.set_key(key);
            }
            DELETE_KEY => {
                let index = usize::try_from(cursor.read_u32::<BigEndian>()?).unwrap();
                let track = &mut self.tracks[index];
                let row = cursor.read_u32::<BigEndian>()?;

                track.delete_key(row);
            }
            SET_ROW => {
                let row = cursor.read_u32::<BigEndian>()?;
                result = ReceiveResult::Some(Event::SetRow(row));
            }
            PAUSE => {
                let flag = cursor.read_u8()? == 1;
                result = ReceiveResult::Some(Event::Pause(flag));
            }
            SAVE_TRACKS => {
                result = ReceiveResult::Some(Event::SaveTracks);
            }
            _ => eprintln!("rocket: Unknown command: {:?}", cmd),
        }

        self.cmd.clear();
        self.state = ClientState::New;

        Ok(result)
    }

    fn handshake(&mut self) -> Result<(), Error> {
        self.stream
            .write_all(CLIENT_GREETING)
            .map_err(Error::Handshake)?;

        let mut buf = [0; SERVER_GREETING.len()];
        self.stream.read_exact(&mut buf).map_err(Error::Handshake)?;

        if buf == SERVER_GREETING {
            Ok(())
        } else {
            Err(Error::HandshakeGreetingMismatch(buf))
        }
    }
}
