extern crate rust_rocket;

use rust_rocket::{Event, Rocket};

use std::time::Duration;

fn main() {
    let mut rocket = Rocket::new().unwrap();
    rocket.get_track_mut("test");
    rocket.get_track_mut("test2");
    rocket.get_track_mut("a:test2");

    let mut current_row = 0;
    loop {
        if let Some(event) = rocket.poll_events() {
            match event {
                Event::SetRow(row) => {
                    println!("SetRow (row: {:?})", row);
                    current_row = row;
                }
                Event::Pause(_) => {
                    let track1 = rocket.get_track("test").unwrap();
                    println!(
                        "Pause (value: {:?}) (row: {:?})",
                        track1.get_value(current_row as f32),
                        current_row
                    );
                }
                _ => (),
            }
            println!("{:?}", event);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
