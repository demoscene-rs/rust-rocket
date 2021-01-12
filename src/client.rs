//! This module contains the main client code, including the `Rocket` type.
use crate::interpolation::*;
use crate::track::*;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::prelude::*;
use std::io::Cursor;
use std::net::TcpStream;
use thiserror::Error;

#[derive(Debug, Error)]
/// The `Error` Type. This is the main error type.
pub enum Error {
    #[error("Failed to establish a TCP connection with the Rocket server")]
    Connect(#[source] std::io::Error),
    #[error("Handshake with the Rocket server failed")]
    Handshake(#[source] std::io::Error),
    #[error("The Rocket server greeting {0:?} wasn't correct (check your address and port)")]
    HandshakeGreetingMismatch([u8; 12]),
    #[error("Cannot set Rocket's TCP connection to nonblocking mode")]
    SetNonblocking(#[source] std::io::Error),
    #[error("Rocket server disconnected")]
    IOError(#[from] std::io::Error),
    #[error("Failed to save tracks")]
    SerializeTracks(#[source] bincode::Error),
    #[error("Failed to open tracks for writing")]
    SaveTracks(#[source] std::io::Error),
}

#[derive(Debug)]
enum RocketState {
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
    SaveTracks,
}

enum ReceiveResult {
    Some(Event),
    None,
    Incomplete,
}

#[derive(Debug)]
/// The `Rocket` type. This contains the connected socket and other fields.
pub struct Rocket {
    stream: TcpStream,
    state: RocketState,
    cmd: Vec<u8>,
    tracks: Vec<Track>,
}

impl Rocket {
    /// Construct a new Rocket.
    ///
    /// This constructs a new rocket and connect to localhost on port 1338.
    ///
    /// # Errors
    ///
    /// If a connection cannot be established, or if the handshake fails.
    /// This will raise an `Error`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::Rocket;
    /// let mut rocket = Rocket::new().unwrap();
    /// ```
    pub fn new() -> Result<Rocket, Error> {
        Rocket::connect("localhost", 1338)
    }

    /// Construct a new Rocket.
    ///
    /// This constructs a new rocket and connects to a specified host and port.
    ///
    /// # Errors
    ///
    /// If a connection cannot be established, or if the handshake fails.
    /// This will raise an `Error`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::Rocket;
    /// let mut rocket = Rocket::connect("localhost", 1338).unwrap();
    /// ```
    pub fn connect(host: &str, port: u16) -> Result<Rocket, Error> {
        let stream = TcpStream::connect((host, port)).map_err(Error::Connect)?;

        let mut rocket = Rocket {
            stream,
            state: RocketState::New,
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

    /// Get a track by name.
    ///
    /// If the track does not yet exist it will be created.
    ///
    /// # Errors
    ///
    /// This method can return an [IOError](Error::IOError) if Rocket server disconnects.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::Rocket;
    /// # let mut rocket = Rocket::new().unwrap();
    /// let track = rocket.get_track_mut("namespace:track").unwrap();
    /// track.get_value(3.5);
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
            buf.write_u32::<BigEndian>(name.len() as u32).unwrap();
            buf.extend_from_slice(&name.as_bytes());
            self.stream.write_all(&buf)?;

            self.tracks.push(Track::new(name));
            Ok(self.tracks.last_mut().unwrap())
        }
    }

    /// Get Track by name.
    ///
    /// You should use `get_track_mut` to create a track.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.get_name() == name)
    }

    /// Send a SetRow message.
    ///
    /// This changes the current row on the tracker side.
    ///
    /// # Errors
    ///
    /// This method can return an [IOError](Error::IOError) if Rocket server disconnects.
    pub fn set_row(&mut self, row: u32) -> Result<(), Error> {
        // Send SET_ROW message
        let mut buf = vec![3];
        buf.write_u32::<BigEndian>(row).unwrap();
        self.stream.write_all(&buf).map_err(|e| e.into())
    }

    /// Poll for new events from the tracker.
    ///
    /// This polls from events from the tracker.
    /// You should call this fairly often your main loop.
    /// It is recommended to keep calling this as long as your receive
    /// Some(Event).
    ///
    /// # Errors
    ///
    /// This method can return an [IOError](Error::IOError) if Rocket server disconnects.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use rust_rocket::Rocket;
    /// # let mut rocket = Rocket::new().unwrap();
    /// while let Some(event) = rocket.poll_events().unwrap() {
    ///     match event {
    ///         // Do something with the various events.
    ///         _ => (),
    ///     }
    /// }
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
            RocketState::New => {
                let mut buf = [0; 1];
                match self.stream.read_exact(&mut buf) {
                    Ok(()) => {
                        self.cmd.extend_from_slice(&buf);
                        match self.cmd[0] {
                            0 => self.state = RocketState::Incomplete(4 + 4 + 4 + 1), //SET_KEY
                            1 => self.state = RocketState::Incomplete(4 + 4),         //DELETE_KEY
                            3 => self.state = RocketState::Incomplete(4),             //SET_ROW
                            4 => self.state = RocketState::Incomplete(1),             //PAUSE
                            5 => self.state = RocketState::Complete,                  //SAVE_TRACKS
                            _ => self.state = RocketState::Complete, // Error / Unknown
                        }
                        Ok(ReceiveResult::Incomplete)
                    }
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::WouldBlock => Ok(ReceiveResult::None),
                        _ => Err(e.into()),
                    },
                }
            }
            RocketState::Incomplete(bytes) => {
                let mut buf = vec![0; bytes];
                match self.stream.read(&mut buf) {
                    Ok(bytes_read) => {
                        self.cmd.extend_from_slice(&buf);
                        if bytes - bytes_read > 0 {
                            self.state = RocketState::Incomplete(bytes - bytes_read);
                        } else {
                            self.state = RocketState::Complete;
                        }
                        Ok(ReceiveResult::Incomplete)
                    }
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::WouldBlock => Ok(ReceiveResult::None),
                        _ => Err(e.into()),
                    },
                }
            }
            RocketState::Complete => {
                let mut result = ReceiveResult::None;
                {
                    let mut cursor = Cursor::new(&self.cmd);
                    let cmd = cursor.read_u8().unwrap();
                    match cmd {
                        0 => {
                            let track =
                                &mut self.tracks[cursor.read_u32::<BigEndian>().unwrap() as usize];
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            let value = cursor.read_f32::<BigEndian>().unwrap();
                            let interpolation = Interpolation::from(cursor.read_u8().unwrap());
                            let key = Key::new(row, value, interpolation);

                            track.set_key(key);
                        }
                        1 => {
                            let track =
                                &mut self.tracks[cursor.read_u32::<BigEndian>().unwrap() as usize];
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
                            self.save_tracks()?;
                        }
                        _ => println!("Unknown {:?}", cmd),
                    }
                }

                self.cmd.clear();
                self.state = RocketState::New;

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

    fn save_tracks(&self) -> Result<(), Error> {
        use std::fs::OpenOptions;
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("tracks.bin")
            .map_err(Error::SaveTracks)?;
        bincode::serialize_into(file, &self.tracks).map_err(Error::SerializeTracks)
    }
}
