use rust_rocket::simple::{Event, Rocket};
use std::{
    thread,
    time::{Duration, Instant},
};

/// Time source.
///
/// In a full demo, this represents your music player, but this type only
/// implements the necessary controls and timing functionality without audio.
struct TimeSource {
    start: Instant,
    offset: Duration,
    paused: bool,
}

impl TimeSource {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            offset: Duration::from_secs(0),
            paused: false,
        }
    }

    pub fn get_time(&self) -> Duration {
        if self.paused {
            self.offset
        } else {
            self.start.elapsed() + self.offset
        }
    }

    pub fn pause(&mut self, state: bool) {
        self.offset = self.get_time();
        self.start = Instant::now();
        self.paused = state;
    }

    pub fn seek(&mut self, to: Duration) {
        self.offset = to;
        self.start = Instant::now();
    }
}

fn main() {
    let mut rocket = Rocket::new("tracks.bin", 60.).unwrap();
    let mut time_source = TimeSource::new();
    let mut previous_time = Duration::ZERO;

    'main: loop {
        // <Handle other event sources such as SDL here>

        // Get current frame's time
        let time = time_source.get_time();

        // Keep the rocket tracker in sync.
        // It's recommended to combine consecutive seek events to a single seek.
        let mut seek = None;
        while let Some(event) = rocket.poll_events().ok().flatten() {
            match event {
                Event::Seek(to) => seek = Some(to),
                Event::Pause(state) => time_source.pause(state),
                Event::NotConnected => /* Alternatively: break the loop here and keep rendering frames */ {
                    std::thread::sleep(Duration::from_millis(10));
                    continue 'main;
                }
            }
        }
        // It's recommended to call set_time only when necessary.
        // This ensures the smoothest scrolling in the editor.
        match seek {
            Some(to) => {
                time_source.seek(to);
                continue;
            }
            None => rocket.set_time(&time),
        }

        // <In a full demo you would render a frame here>

        // Filter redundant output
        if time != previous_time {
            println!("{:?}: test = {}", time, rocket.get_value("test"));
        }
        previous_time = time;
        thread::sleep(Duration::from_millis(10));
    }
}
