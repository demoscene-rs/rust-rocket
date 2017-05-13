//! This module contains the main client code, including the `Rocket` type.
use interpolation::*;
use track::*;

use std;
use std::io::Cursor;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use std::io::prelude::*;
use std::net::TcpStream;

#[derive(Copy, Clone, Debug)]
/// The `RocketErr` Type. This is the main error type.
pub struct RocketErr {}

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
    /// This will raise a `RocketErr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rocket::Rocket;
    ///
    /// let mut rocket = Rocket::new();
    /// ```
    pub fn new() -> Result<Rocket, RocketErr> {
        Rocket::connect("localhost", 1338)
    }

    /// Construct a new Rocket.
    ///
    /// This constructs a new rocket and connects to a specified host and port.
    ///
    /// # Errors
    ///
    /// If a connection cannot be established, or if the handshake fails.
    /// This will raise a `RocketErr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rocket::Rocket;
    ///
    /// let mut rocket = Rocket::connect("localhost", 1338);
    /// ```
    pub fn connect(host: &str, port: u16) -> Result<Rocket, RocketErr> {
        let stream = TcpStream::connect((host, port)).expect("Failed to connect");

        let mut rocket = Rocket {
            stream: stream,
            state: RocketState::New,
            cmd: Vec::new(),
            tracks: Vec::new(),
        };

        rocket.handshake().expect("Failed to handshake");

        rocket.stream.set_nonblocking(true).expect("Failed to set nonblocking mode");

        Ok(rocket)
    }

    /// Get a track by name.
    ///
    /// If the track does not yet exist it will be created.
    ///
    /// # Examples
    ///
    /// ```
    /// let track = rocket.get_track_mut("namespace:track");
    /// track.get_value(3.5);
    /// ```
    pub fn get_track_mut(&mut self, name: &str) -> &mut Track {
        if !self.tracks.iter().any(|t| t.get_name() == name) {

            // Send GET_TRACK message
            let mut buf = vec![2];
            buf.write_u32::<BigEndian>(name.len() as u32).unwrap();
            buf.extend_from_slice(&name.as_bytes());
            self.stream.write_all(&buf).unwrap();

            self.tracks.push(Track::new(name));
        }
        self.tracks.iter_mut().find(|t| t.get_name() == name).unwrap()
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
    pub fn set_row(&mut self, row: u32) {
        // Send SET_ROW message
        let mut buf = vec![3];
        buf.write_u32::<BigEndian>(row).unwrap();
        self.stream.write_all(&buf).unwrap();
    }

    /// Poll for new events from the tracker.
    ///
    /// This polls from events from the tracker.
    /// You should call this fairly often your main loop.
    /// It is recommended to keep calling this as long as your receive
    /// Some(Event).
    ///
    /// # Examples
    ///
    /// ```
    ///  while let Some(Event) = rocket.poll_events() {
    ///      match event {
    ///          // Do something with the various events.
    ///          _ => (),
    ///      }
    ///  }
    /// ```
    pub fn poll_events(&mut self) -> Option<Event> {
        loop {
            let result = self.poll_event();
            match result {
                ReceiveResult::None => return None,
                ReceiveResult::Incomplete => (),
                ReceiveResult::Some(event) => return Some(event),
            }
        }
    }

    fn poll_event(&mut self) -> ReceiveResult {
        match self.state {
            RocketState::New => {
                let mut buf = [0; 1];
                if self.stream.read_exact(&mut buf).is_ok() {
                    self.cmd.extend_from_slice(&buf);
                    match self.cmd[0] {
                        0 => self.state = RocketState::Incomplete(4 + 4 + 4 + 1), //SET_KEY
                        1 => self.state = RocketState::Incomplete(4 + 4), //DELETE_KEY
                        3 => self.state = RocketState::Incomplete(4), //SET_ROW
                        4 => self.state = RocketState::Incomplete(1), //PAUSE
                        5 => self.state = RocketState::Complete, //SAVE_TRACKS
                        _ => self.state = RocketState::Complete, // Error / Unknown
                    }
                    ReceiveResult::Incomplete
                } else {
                    ReceiveResult::None
                }
            }
            RocketState::Incomplete(bytes) => {
                let mut buf = vec![0;bytes];
                if let Ok(bytes_read) = self.stream.read(&mut buf) {
                    self.cmd.extend_from_slice(&buf);
                    if bytes - bytes_read > 0 {
                        self.state = RocketState::Incomplete(bytes - bytes_read);
                    } else {
                        self.state = RocketState::Complete;
                    }
                    ReceiveResult::Incomplete
                } else {
                    ReceiveResult::None
                }
            }
            RocketState::Complete => {
                let mut result = ReceiveResult::None;
                {
                    let mut cursor = Cursor::new(&self.cmd);
                    let cmd = cursor.read_u8().unwrap();
                    match cmd {
                        0 => {
                            let mut track = &mut self.tracks[cursor.read_u32::<BigEndian>().unwrap() as
                                                 usize];
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            let value = cursor.read_f32::<BigEndian>().unwrap();
                            let interpolation = Interpolation::from(cursor.read_u8().unwrap());
                            let key = Key::new(row, value, interpolation);

                            track.set_key(key);

                        }
                        1 => {
                            let mut track = &mut self.tracks[cursor.read_u32::<BigEndian>().unwrap() as
                                                 usize];
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
                self.state = RocketState::New;

                result
            }
        }
    }

    fn handshake(&mut self) -> Result<(), RocketErr> {
        let client_greeting = "hello, synctracker!";
        let server_greeting = "hello, demo!";

        self.stream.write_all(client_greeting.as_bytes()).expect("Failed to write client greeting");
        let mut buf = [0; 12];
        self.stream.read_exact(&mut buf).expect("Failed to read server greeting");
        let read_greeting = std::str::from_utf8(&buf).expect("Failed to convert buf to utf8");
        if read_greeting == server_greeting {
            Ok(())
        } else {
            Err(RocketErr {})
        }
    }
}
