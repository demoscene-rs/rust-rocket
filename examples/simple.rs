use rust_rocket::simple::{Event, Rocket};
use std::{
    thread,
    time::{Duration, Instant},
};

struct TimeSource {
    start: Instant,
    offset: Duration,
    paused: bool,
}

impl TimeSource {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            offset: Duration::from_secs(0),
            paused: false,
        }
    }

    fn get_time(&self) -> Duration {
        if self.paused {
            self.offset
        } else {
            self.start.elapsed() + self.offset
        }
    }

    fn pause(&mut self, state: bool) {
        self.offset = self.get_time();
        self.start = Instant::now();
        self.paused = state;
    }

    fn seek(&mut self, to: Duration) {
        self.offset = to;
        self.start = Instant::now();
    }
}

fn main() {
    let mut rocket = Rocket::new("tracks.bin", 60.);
    let mut time_source = TimeSource::new();

    loop {
        // Get current frame's time
        let time = time_source.get_time();

        // Keep the rocket tracker in sync.
        // It's recommended to combine consecutive seek events to a single seek.
        // This ensures the smoothest scrolling in editor.
        let mut seek = None;
        while let Some(event) = rocket.poll_events() {
            match dbg!(event) {
                Event::Seek(to) => seek = Some(to),
                Event::Pause(state) => time_source.pause(state),
            }
        }
        // It's recommended to call set_time only when the not seeking.
        match seek {
            Some(to) => {
                time_source.seek(to);
                continue;
            }
            None => rocket.set_time(time),
        }

        // Render your production here
        println!("{:?}: test = {}", time, rocket.get_value("test"));
        thread::sleep(Duration::from_millis(10));
    }
}
