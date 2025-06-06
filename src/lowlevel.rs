pub mod client;
pub mod interpolation;
pub mod track;

use track::Track;

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
}

impl From<Vec<Track>> for Tracks {
    fn from(value: Vec<Track>) -> Self {
        Self { inner: value }
    }
}

/// Provides public read only access
impl AsRef<[Track]> for Tracks {
    fn as_ref(&self) -> &[Track] {
        self.inner.as_slice()
    }
}
