use crate::track::Track;

pub trait Rocket {
    /// Get Track by name.
    fn get_track(&self, name: &str) -> Option<&Track>;
}
