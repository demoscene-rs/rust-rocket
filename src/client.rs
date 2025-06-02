//! This module contains the main client code, including the [`RocketClient`] type.
use crate::interpolation::*;
use crate::track::*;
use crate::Tracks;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
    convert::TryFrom,
    io::{Cursor, Read, Write},
    net::{TcpStream, ToSocketAddrs},
};
use thiserror::Error;

#[derive(Debug, Error)]
/// The `Error` Type. This is the main error type.
pub enum Error {
    #[error("Failed to establish a TCP connection with the Rocket server")]
    /// Failure to connect to a rocket tracker. This can happen if the tracker is not running, the
    /// address isn't correct or other network-related reasons.
    Connect(#[source] std::io::Error),
    #[error("Handshake with the Rocket server failed")]
    /// Failure to transmit or receive greetings with the tracker
    Handshake(#[source] std::io::Error),
    #[error("The Rocket server greeting {0:?} wasn't correct")]
    /// Handshake was performed but the the received greeting wasn't correct
    HandshakeGreetingMismatch([u8; 12]),
    #[error("Cannot set Rocket's TCP connection to nonblocking mode")]
    /// Error from [`TcpStream::set_nonblocking`]
    SetNonblocking(#[source] std::io::Error),
    #[error("Rocket server disconnected")]
    /// Network IO error during operation
    IOError(#[source] std::io::Error),
}

#[derive(Debug)]
enum ClientState {
    New,
    Incomplete(usize),
    Complete,
}

#[derive(Debug, Copy, Clone)]
/// The `Event` Type. These are the various events from the tracker.
pub enum Event {
    /// The tracker changes row.
    SetRow(u32),
    /// The tracker pauses or unpauses.
    Pause(bool),
    /// The tracker asks us to save our track data.
    /// You may want to call [`RocketClient::save_tracks`] after receiving this event.
    SaveTracks,
}

enum ReceiveResult {
    Some(Event),
    None,
    Incomplete,
}

#[derive(Debug)]
/// The `RocketClient` type. This contains the connected socket and other fields.
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
            let mut buf = vec![2];
            buf.write_u32::<BigEndian>(u32::try_from(name.len()).expect("Track name too long"))
                .unwrap_or_else(|_|
                // Can writes to a vec fail? Consider changing to unreachable_unchecked in 1.0
                unreachable!());
            buf.extend_from_slice(name.as_bytes());
            self.stream.write_all(&buf).map_err(Error::IOError)?;

            self.tracks.push(Track::new(name));
            Ok(self.tracks.last_mut().unwrap_or_else(||
                // tracks cannot be empty right after pushing into it, consider changing to
                // unreachable_unchecked in 1.0
                unreachable!()))
        }
    }

    /// Get track by name.
    ///
    /// You should use [`get_track_mut`](RocketClient::get_track_mut) to create a track.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.get_name() == name)
    }

    /// Get a snapshot of the tracks in the session.
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
    pub fn save_tracks(&self) -> Tracks {
        Tracks {
            inner: self.tracks.clone(),
        }
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
        let mut buf = vec![3];
        buf.write_u32::<BigEndian>(row).unwrap_or_else(|_|
                // Can writes to a vec fail? Consider changing to unreachable_unchecked in 1.0
                unreachable!());
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
    /// This method can return an [`Error::IOError`] if Rocket tracker disconnects.
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
            let result = self.poll_event()?;
            match result {
                ReceiveResult::None => return Ok(None),
                ReceiveResult::Incomplete => (),
                ReceiveResult::Some(event) => return Ok(Some(event)),
            }
        }
    }

    fn poll_event(&mut self) -> Result<ReceiveResult, Error> {
        match self.state {
            ClientState::New => {
                let mut buf = [0; 1];
                match self.stream.read_exact(&mut buf) {
                    Ok(()) => {
                        self.cmd.extend_from_slice(&buf);
                        match self.cmd[0] {
                            0 => self.state = ClientState::Incomplete(4 + 4 + 4 + 1), //SET_KEY
                            1 => self.state = ClientState::Incomplete(4 + 4),         //DELETE_KEY
                            3 => self.state = ClientState::Incomplete(4),             //SET_ROW
                            4 => self.state = ClientState::Incomplete(1),             //PAUSE
                            5 => self.state = ClientState::Complete,                  //SAVE_TRACKS
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
            ClientState::Incomplete(bytes) => {
                let mut buf = vec![0; bytes];
                match self.stream.read(&mut buf) {
                    Ok(bytes_read) => {
                        self.cmd.extend_from_slice(&buf);
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
            ClientState::Complete => {
                let mut result = ReceiveResult::None;
                {
                    // Following reads from cmd should never fail if above match arms are correct
                    let mut cursor = Cursor::new(&self.cmd);
                    let cmd = cursor.read_u8().unwrap();
                    match cmd {
                        0 => {
                            // usize::try_from(u32) will only be None if usize is smaller, and
                            // more than usize::MAX tracks are in use. That isn't possible because
                            // I'd imagine Vec::push and everything else will panic first.
                            // If you're running this on a microcontroller, I'd love to see it!
                            let track = &mut self.tracks
                                [usize::try_from(cursor.read_u32::<BigEndian>().unwrap()).unwrap()];
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            let value = cursor.read_f32::<BigEndian>().unwrap();
                            let interpolation = Interpolation::from(cursor.read_u8().unwrap());
                            let key = Key::new(row, value, interpolation);

                            track.set_key(key);
                        }
                        1 => {
                            let track = &mut self.tracks
                                [usize::try_from(cursor.read_u32::<BigEndian>().unwrap()).unwrap()];
                            let row = cursor.read_u32::<BigEndian>().unwrap();

                            track.delete_key(row);
                        }
                        3 => {
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            result = ReceiveResult::Some(Event::SetRow(row));
                        }
                        4 => {
                            let flag = cursor.read_u8().unwrap() == 1;
                            result = ReceiveResult::Some(Event::Pause(flag));
                        }
                        5 => {
                            result = ReceiveResult::Some(Event::SaveTracks);
                        }
                        _ => println!("Unknown {:?}", cmd),
                    }
                }

                self.cmd.clear();
                self.state = ClientState::New;

                Ok(result)
            }
        }
    }

    fn handshake(&mut self) -> Result<(), Error> {
        let client_greeting = b"hello, synctracker!";
        let server_greeting = b"hello, demo!";

        self.stream
            .write_all(client_greeting)
            .map_err(Error::Handshake)?;

        let mut buf = [0; 12];
        self.stream.read_exact(&mut buf).map_err(Error::Handshake)?;

        if &buf == server_greeting {
            Ok(())
        } else {
            Err(Error::HandshakeGreetingMismatch(buf))
        }
    }
}
