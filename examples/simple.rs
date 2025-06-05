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
    let mut previous_print_time = Duration::ZERO;

    'main: loop {
        // <Handle other event sources such as SDL or winit here>

        // Handle events from the rocket tracker
        while let Some(event) = rocket.poll_events().ok().flatten() {
            match event {
                Event::Seek(to) => time_source.seek(to),
                Event::Pause(state) => time_source.pause(state),
                Event::NotConnected =>
                /* Alternatively: break the loop here and keep rendering frames */
                {
                    std::thread::sleep(Duration::from_millis(10));
                    continue 'main;
                }
            }
        }

        // Get current frame's time and keep the tracker updated
        let time = time_source.get_time();
        rocket.set_time(&time);

        // <In a full demo you would render a frame here>

        // Filter redundant output
        if time != previous_print_time {
            println!("{:?}: test = {}", time, rocket.get_value("test"));
        }
        previous_print_time = time;
        thread::sleep(Duration::from_millis(10));
    }
}
