//! This module contains a player for previously saved tracks, [`RocketPlayer`].
//!
//! # Usage
//!
//! This crate makes no distinction between client builds (debug builds) and player builds (release builds),
//! nor does it offer any generics or traits for polymorphism between the [`RocketClient`](crate::RocketClient)
//! and [`RocketPlayer`].
//!
//! Currently, it's expected that you use conditional compilation to provide a consistent interface to the rest of your
//! production, so that in debug builds you use the [`RocketClient`](crate::RocketClient)
//! and in release builds you use the [`RocketPlayer`].
//!
//! In future releases, this may become a built-in feature, but for the current version,
//! you may use the following pattern:
//!
//! ```rust,no_run
//! # use rust_rocket::{RocketClient, RocketPlayer, Tracks};
//! # use std::fs::File;
//! struct MySync {
//!     row: f32,
//!     #[cfg(feature = "client")]
//!     rocket: RocketClient,
//!     #[cfg(not(feature = "client"))]
//!     rocket: RocketPlayer,
//! }
//!
//! impl MySync {
//!     fn new() -> Self {
//!         #[cfg(feature = "client")]
//!         let rocket = RocketClient::new().unwrap();
//!         #[cfg(not(feature = "client"))]
//!         let rocket = {
//!             let mut file = File::open("tracks.bin").unwrap();
//! #            #[cfg(feature = "bincode")]
//!             let tracks = bincode::decode_from_std_read(
//!                 &mut file,
//!                 bincode::config::standard()
//!             ).unwrap();
//! #            #[cfg(not(feature = "bincode"))]
//! #            let tracks = Tracks::default();
//!             RocketPlayer::new(tracks)
//!         };
//!
//!         Self {
//!             row: 0.,
//!             rocket,
//!         }
//!     }
//!
//!     fn set_row(&mut self, row: f32) {
//!         #[cfg(feature = "client")]
//!         self.rocket.set_row(row as u32);
//!         self.row = row;
//!     }
//!
//!     fn get_value(&mut self, track: &str) -> f32 {
//!         #[cfg(feature = "client")]
//!         let track = self.rocket.get_track_mut(track).unwrap();
//!         #[cfg(not(feature = "client"))]
//!         let track = self.rocket.get_track(track).unwrap();
//!         track.get_value(self.row)
//!     }
//! }
//! ```

use crate::{track::Track, Tracks};
use std::collections::HashMap;

/// A player for tracks from
/// [`RocketClient::save_tracks`](crate::RocketClient::save_tracks).
///
/// # Usage
///
/// See [module-level documentation](crate::player#Usage).
///
/// # Examples
///
/// ```rust,no_run
/// # use rust_rocket::{RocketPlayer, Tracks};
/// # use std::fs::File;
/// // Run the demo and edit your sync tracks (see top level documentation),
/// // then call save_tracks and serialize tracks to a file (see save_tracks documentation)
///
/// // And deserialize from a file in your release build
/// let mut file = File::open("tracks.bin").expect("Failed to open tracks.bin");
/// # #[cfg(feature = "bincode")]
/// # {
/// let tracks = bincode::decode_from_std_read(&mut file, bincode::config::standard())
///     .expect("Failed to decode tracks.bin");
/// let player = RocketPlayer::new(tracks);
/// println!("Value at row 123: {}", player.get_track("test").unwrap().get_value(123.));
/// # }
/// # Ok::<(), rust_rocket::client::Error>(())
/// ```
pub struct RocketPlayer {
    tracks: HashMap<Box<str>, Track>,
}

impl RocketPlayer {
    /// Constructs a `RocketPlayer` from `Track`s.
    pub fn new(tracks: Tracks) -> Self {
        // Convert to a HashMap for perf (not benchmarked)
        let tracks: HashMap<Box<str>, Track> = tracks
            .into_iter()
            .map(|track| (String::from(track.get_name()).into_boxed_str(), track))
            .collect();

        Self { tracks }
    }

    /// Get track by name.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpolation::Interpolation;
    use crate::track::Key;

    fn get_test_tracks() -> Tracks {
        vec![
            {
                let mut track = Track::new("test1");
                track.set_key(Key::new(0, 1.0, Interpolation::Step));
                track.set_key(Key::new(5, 0.0, Interpolation::Step));
                track.set_key(Key::new(10, 1.0, Interpolation::Step));
                track
            },
            {
                let mut track = Track::new("test2");
                track.set_key(Key::new(0, 2.0, Interpolation::Step));
                track.set_key(Key::new(5, 0.0, Interpolation::Step));
                track.set_key(Key::new(10, 2.0, Interpolation::Step));
                track
            },
        ]
    }

    #[test]
    fn finds_all_tracks() {
        let tracks = get_test_tracks();
        let player = RocketPlayer::new(tracks);

        // Ugly repeated calls to get_track to reflect average use case :)

        assert_eq!(player.get_track("test1").unwrap().get_value(0.), 1.0);
        assert_eq!(player.get_track("test2").unwrap().get_value(0.), 2.0);
    }

    #[test]
    fn no_surprise_tracks() {
        let tracks = get_test_tracks();
        let player = RocketPlayer::new(tracks);
        assert!(player
            .get_track("hello this track should not exist")
            .is_none());
    }
}
