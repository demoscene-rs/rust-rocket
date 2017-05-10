extern crate rust_rocket;

use rust_rocket::{Rocket, Event};

fn main() {
    let mut rocket = Rocket::new().unwrap();
    rocket.get_track("test");
    rocket.get_track("test2");
    rocket.get_track("a:test2");

    loop {
        if let Some(event) = rocket.poll_events() {
            match event {
                Event::Pause(_) => {
                    let row = rocket.get_row() as f32;
                    {
                        let track1 = rocket.get_track("test");
                        println!("{:?}", track1.get_value(row));
                    }
                }
                _ => (),
            }
            println!("{:?}", event);
        }
        std::thread::sleep_ms(1);
    }
}
