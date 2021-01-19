use rust_rocket::client::{Event, RocketClient};
use std::error::Error;
use std::fs::OpenOptions;
use std::time::Duration;

static TRACKS_FILE: &str = "tracks.bin";

fn main() -> Result<(), Box<dyn Error>> {
    let mut rocket = RocketClient::new()?;
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
                    // Obtain a clone of current track state
                    let tracks = rocket.save_tracks();

                    // Open a file for writing, create if not present,
                    // truncate (overwrite) in case it has previous contents.
                    let file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(TRACKS_FILE)?;

                    // Serialize tracks into the file using bincode
                    bincode::serialize_into(file, &tracks)?;
                    // See examples/play.rs for deserializing and playback
                    println!("Tracks saved to {}", TRACKS_FILE);
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
