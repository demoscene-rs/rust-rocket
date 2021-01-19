//! This module contains a barebones player.
use crate::track::Track;
use std::collections::HashMap;

/// A player for tracks dumped by
/// [`RocketClient::save_tracks`](crate::RocketClient::save_tracks).
///
/// # Examples
///
/// ```rust,no_run
/// # use rust_rocket::RocketClient;
/// # use rust_rocket::RocketPlayer;
/// let client = RocketClient::new().unwrap();
/// // ...
/// // Run the demo and edit your sync tracks, then call save_tracks
/// // ...
/// let tracks = client.save_tracks();
/// // ...
/// // Serialize tracks to a file (see examples/edit.rs)
/// // And deserialize from a file in your release build (examples/play.rs)
/// // ...
/// let player = RocketPlayer::new(tracks);
/// println!("Value at row 123: {}", player.get_track("test").unwrap().get_value(123.));
/// ```
pub struct RocketPlayer {
    tracks: HashMap<String, Track>,
}

impl RocketPlayer {
    /// Constructs a `RocketPlayer` from `Track`s.
    pub fn new(tracks: Vec<Track>) -> Self {
        // Convert to a HashMap for perf (not benchmarked)
        let mut tracks_map = HashMap::with_capacity(tracks.len());
        for track in tracks {
            tracks_map.insert(track.get_name().to_owned(), track);
        }

        Self { tracks: tracks_map }
    }

    /// Get track by name.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.get(name)
    }
}
