#[derive(Debug, Copy, Clone)]
pub enum Interpolation {
    Step = 0,
    Linear = 1,
    Smooth = 2,
    Ramp = 3,
}

impl From<u8> for Interpolation {
    fn from(raw: u8) -> Interpolation {
        match raw {
            0 => Interpolation::Step,
            1 => Interpolation::Linear,
            2 => Interpolation::Smooth,
            3 => Interpolation::Ramp,
            _ => Interpolation::Step,
        }
    }
}

impl Interpolation {
    pub fn interpolate(&self, t: f32) -> f32 {
        match self {
            &Interpolation::Step => 0.0,
            &Interpolation::Linear => t,
            &Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
            &Interpolation::Ramp => t.powi(2),
        }
    }
}
