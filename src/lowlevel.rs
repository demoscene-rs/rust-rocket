//! Low level API.
//!
//! This module contains the implementation for a rocket [`client`], [`interpolation`] and [`track`]s.
//! See each module for their respective documentation.

pub mod client;
pub mod interpolation;
pub mod track;

use track::Track;

/// A collection of [`Track`]s. To construct this type manually, use [`Tracks::default`] or [`Tracks::from`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[derive(Default, Clone)]
pub struct Tracks {
    inner: Vec<Track>,
}

impl Tracks {
    /// Get track by name.
    ///
    /// You should use [`Client::get_track_mut`](client::Client::get_track_mut) to create a track.
    pub fn get_track(&self, name: &str) -> Option<&Track> {
        self.inner.iter().find(|t| t.get_name() == name)
    }

    /// Provides read only access to the [`Track`]s in the collection.
    pub fn as_slice(&self) -> &[Track] {
        self.inner.as_slice()
    }

    /// Drops all tracks in the collection.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl From<Vec<Track>> for Tracks {
    fn from(value: Vec<Track>) -> Self {
        Self { inner: value }
    }
}
