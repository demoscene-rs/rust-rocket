use interpolation::*;
use track::*;

use std;
use std::io::Cursor;
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use std::io::prelude::*;
use std::net::TcpStream;

#[derive(Copy, Clone, Debug)]
pub struct RocketErr {}

enum RocketState {
    NewCommand,
    IncompleteCommand(usize),
    CompleteCommand,
}

#[derive(Debug, Copy, Clone)]
pub enum Event {
    SetRow(u32),
    Pause(bool),
}

enum ReceiveResult {
    Some(Event),
    None,
    Incomplete,
}

pub struct Rocket {
    stream: TcpStream,
    state: RocketState,
    cmd: Vec<u8>,
    tracks: Vec<Track>,
    row: u32,
    paused: bool,
}

impl Rocket {
    pub fn new() -> Result<Rocket, RocketErr> {
        Rocket::connect("localhost", 1338)
    }

    pub fn connect(host: &str, port: u16) -> Result<Rocket, RocketErr> {
        let stream = TcpStream::connect((host, port)).expect("Failed to connect");

        let mut rocket = Rocket {
            stream: stream,
            state: RocketState::NewCommand,
            cmd: Vec::new(),
            tracks: Vec::new(),
            row: 0,
            paused: true,
        };

        rocket.handshake().expect("Failed to handshake");

        rocket.stream.set_nonblocking(true).expect("Failed to set nonblocking mode");

        Ok(rocket)
    }

    pub fn get_row(&self) -> u32 {
        self.row
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn get_track(&mut self, name: &str) -> &Track {
        if !self.tracks.iter().any(|t| t.get_name() == name) {

            // Send GET_TRACK message
            let mut buf = vec![2];
            buf.write_u32::<BigEndian>(name.len() as u32).unwrap();
            buf.extend_from_slice(&name.as_bytes());
            self.stream.write(&buf).unwrap();

            self.tracks.push(Track::new(name));
        }
        self.tracks.iter().find(|t| t.get_name() == name).unwrap()
    }

    pub fn set_row(&mut self, row: u32) {
        self.row = row;

        // Send SET_ROW message
        let mut buf = vec![3];
        buf.write_u32::<BigEndian>(row).unwrap();
        self.stream.write(&buf).unwrap();
    }

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
            RocketState::NewCommand => {
                let mut buf = [0; 1];
                if let Ok(_) = self.stream.read_exact(&mut buf) {
                    self.cmd.extend_from_slice(&buf);
                    match self.cmd[0] {
                        0 => self.state = RocketState::IncompleteCommand(4 + 4 + 4 + 1), //SET_KEY
                        1 => self.state = RocketState::IncompleteCommand(4 + 4), //DELETE_KEY
                        3 => self.state = RocketState::IncompleteCommand(4), //SET_ROW
                        4 => self.state = RocketState::IncompleteCommand(1), //PAUSE
                        5 => self.state = RocketState::CompleteCommand, //SAVE_TRACKS
                        _ => self.state = RocketState::CompleteCommand,
                    }
                    ReceiveResult::Incomplete
                } else {
                    ReceiveResult::None
                }
            }
            RocketState::IncompleteCommand(bytes) => {
                let mut buf = vec![0;bytes];
                if let Ok(bytes_read) = self.stream.read(&mut buf) {
                    self.cmd.extend_from_slice(&buf);
                    if bytes - bytes_read > 0 {
                        self.state = RocketState::IncompleteCommand(bytes - bytes_read);
                    } else {
                        self.state = RocketState::CompleteCommand;
                    }
                    ReceiveResult::Incomplete
                } else {
                    ReceiveResult::None
                }
            }
            RocketState::CompleteCommand => {
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
                            println!("SET_KEY (track: {:?}) (key: {:?})", track, key);

                            track.set_key(key);

                        }
                        1 => {
                            let mut track = &mut self.tracks[cursor.read_u32::<BigEndian>().unwrap() as
                                                 usize];
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            println!("DELETE_KEY (track: {:?}) (row: {:?})", track, row);

                            track.delete_key(row);
                        }
                        3 => {
                            let row = cursor.read_u32::<BigEndian>().unwrap();
                            println!("SET_ROW (row: {:?})", row);

                            self.row = row;

                            result = ReceiveResult::Some(Event::SetRow(self.row));
                        }
                        4 => {
                            let flag = cursor.read_u8().unwrap();
                            // 0 or 1
                            println!("PAUSE {:?}", flag);

                            self.paused = flag == 1;
                            result = ReceiveResult::Some(Event::Pause(self.paused));
                        }
                        5 => {
                            println!("SAVE_TRACKS");
                        }
                        _ => println!("Unknown {:?}", cmd),
                    }
                }

                self.cmd.clear();
                self.state = RocketState::NewCommand;

                result
            }
        }
    }

    fn handshake(&mut self) -> Result<(), RocketErr> {
        let client_greeting = "hello, synctracker!";
        let server_greeting = "hello, demo!";

        self.stream.write(client_greeting.as_bytes()).expect("Failed to write client greeting");
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
