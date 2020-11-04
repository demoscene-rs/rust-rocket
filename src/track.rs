//! This module contains `Key` and `Track` types.

use crate::interpolation::*;

#[derive(Debug, Clone, Copy)]
/// The `Key` Type.
pub struct Key {
    row: u32,
    value: f32,
    interpolation: Interpolation,
}

impl Key {
    /// Construct a new `Key`.
    pub fn new(row: u32, value: f32, interp: Interpolation) -> Key {
        Key {
            row,
            value,
            interpolation: interp,
        }
    }
}

#[derive(Debug)]
/// The `Track` Type. This is a collection of `Key`s with a name.
pub struct Track {
    name: String,
    keys: Vec<Key>,
}

impl Track {
    /// Construct a new Track with a name.
    pub fn new<S: Into<String>>(name: S) -> Track {
        Track {
            name: name.into(),
            keys: Vec::new(),
        }
    }

    /// Get the name of the track.
    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    fn get_exact_position(&self, row: u32) -> Option<usize> {
        self.keys.iter().position(|k| k.row == row)
    }

    fn get_insert_position(&self, row: u32) -> Option<usize> {
        self.keys.iter().position(|k| k.row >= row)
    }

    fn get_lower_bound_position(&self, row: u32) -> usize {
        self.keys
            .iter()
            .position(|k| k.row > row)
            .unwrap_or(self.keys.len())
            - 1
    }

    /// Insert or update a key on a track.
    pub fn set_key(&mut self, key: Key) {
        if let Some(pos) = self.get_exact_position(key.row) {
            self.keys[pos] = key;
        } else if let Some(pos) = self.get_insert_position(key.row) {
            self.keys.insert(pos, key);
        } else {
            self.keys.push(key);
        }
    }

    /// Delete a key from a track.
    ///
    /// If a key does not exist this will do nothing.
    pub fn delete_key(&mut self, row: u32) {
        if let Some(pos) = self.get_exact_position(row) {
            self.keys.remove(pos);
        }
    }

    /// Get a value based on a row.
    ///
    /// The row can be between two integers.
    /// This will perform the required interpolation.
    pub fn get_value(&self, row: f32) -> f32 {
        if self.keys.is_empty() {
            return 0.0;
        }

        let lower_row = row.floor() as u32;

        if lower_row <= self.keys[0].row {
            return self.keys[0].value;
        }

        if lower_row >= self.keys[self.keys.len() - 1].row {
            return self.keys[self.keys.len() - 1].value;
        }

        let pos = self.get_lower_bound_position(lower_row);

        let lower = &self.keys[pos];
        let higher = &self.keys[pos + 1];

        let t = (row - (lower.row as f32)) / ((higher.row as f32) - (lower.row as f32));
        let it = lower.interpolation.interpolate(t);

        (lower.value as f32) + ((higher.value as f32) - (lower.value as f32)) * it
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_keys() {
        let mut track = Track::new("test");
        track.set_key(Key::new(0, 1.0, Interpolation::Step));
        track.set_key(Key::new(5, 0.0, Interpolation::Step));
        track.set_key(Key::new(10, 1.0, Interpolation::Step));

        assert_eq!(track.get_value(-1.), 1.0);
        assert_eq!(track.get_value(0.), 1.0);
        assert_eq!(track.get_value(1.), 1.0);

        assert_eq!(track.get_value(4.), 1.0);
        assert_eq!(track.get_value(5.), 0.0);
        assert_eq!(track.get_value(6.), 0.0);

        assert_eq!(track.get_value(9.), 0.0);
        assert_eq!(track.get_value(10.), 1.0);
        assert_eq!(track.get_value(11.), 1.0);
    }
}
