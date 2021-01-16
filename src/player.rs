//! This module contains a barebones loader for track files.
use crate::track::Track;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to open file for reading track data")]
    OpenTrackFile(#[source] std::io::Error),
    #[error("Failed to deserialize track data")]
    DeserializeTracks(#[source] bincode::Error),
}

/// A loader for track binary files dumped by [`Client::save_tracks`](crate::Client::save_tracks).
///
/// # Usage
///
/// After constructing, call [`Player::get_track`] to get tracks.
/// Then call [`Track::get_value`] to get saved values at any given point in time.
pub struct Player {
    tracks: HashMap<String, Track>,
}

impl Player {
    /// Load track data from file for playback.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Error> {
        // Load from file
        let file = File::open(path).map_err(Error::OpenTrackFile)?;
        let tracks_vec: Vec<Track> =
            bincode::deserialize_from(file).map_err(Error::DeserializeTracks)?;

        // Convert to a HashMap for perf (not benchmarked)
        let mut tracks_map = HashMap::with_capacity(tracks_vec.len());
        for track in tracks_vec {
            tracks_map.insert(track.get_name().to_owned(), track);
        }

        Ok(Self { tracks: tracks_map })
    }

    /// Get track by name.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.tracks.get(name)
    }
}
