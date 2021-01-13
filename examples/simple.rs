use rust_rocket::{Event, Rocket};
use std::time::Duration;

fn main() -> Result<(), rust_rocket::Error> {
    let mut rocket = Rocket::new()?;
    rocket.get_track_mut("test")?;
    rocket.get_track_mut("test2")?;
    rocket.get_track_mut("a:test2")?;

    let mut current_row = 0;
    let mut paused = true;

    loop {
        if let Some(event) = rocket.poll_events()? {
            match event {
                Event::SetRow(row) => {
                    println!("SetRow (row: {:?})", row);
                    current_row = row;
                }
                Event::Pause(state) => {
                    paused = state;

                    let track1 = rocket.get_track("test").unwrap();
                    println!(
                        "Pause (value: {:?}) (row: {:?})",
                        track1.get_value(current_row as f32),
                        current_row
                    );
                }
                Event::SaveTracks => {
                    println!("Saving tracks");
                    rocket.save_tracks("tracks.bin")?;
                }
            }
            println!("{:?}", event);
        }

        if !paused {
            current_row += 1;
            rocket.set_row(current_row)?;
        }

        std::thread::sleep(Duration::from_millis(32));
    }
}
