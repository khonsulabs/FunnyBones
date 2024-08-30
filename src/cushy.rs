//! Widgets for editing and rendering skeletons.

use cushy::figures::Ranged;

use crate::{Angle, Coordinate};

pub mod skeleton_canvas;

impl cushy::animation::PercentBetween for Angle {
    fn percent_between(&self, min: &Self, max: &Self) -> cushy::animation::ZeroToOne {
        self.radians.percent_between(&min.radians, &max.radians)
    }
}

impl cushy::animation::LinearInterpolate for Angle {
    fn lerp(&self, target: &Self, percent: f32) -> Self {
        Self {
            radians: self.radians.lerp(&target.radians, percent),
        }
    }
}

impl cushy::figures::IntoComponents<f32> for Coordinate {
    fn into_components(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

impl cushy::figures::FromComponents<f32> for Coordinate {
    fn from_components(components: (f32, f32)) -> Self {
        Self::new(components.0, components.1)
    }
}

impl Ranged for Angle {
    const MIN: Self = Self::MIN;
    const MAX: Self = Self::MAX;
}
