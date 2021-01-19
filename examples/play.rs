use rust_rocket::RocketPlayer;
use std::time::Duration;

fn main() -> Result<(), rust_rocket::player::Error> {
    let rocket = RocketPlayer::new("tracks.bin")?;
    let mut current_row = 0;

    loop {
        println!(
            "value: {:?} (row: {:?})",
            rocket
                .get_track("test")
                .unwrap()
                .get_value(current_row as f32),
            current_row
        );

        current_row += 1;
        std::thread::sleep(Duration::from_millis(32));
    }
}
