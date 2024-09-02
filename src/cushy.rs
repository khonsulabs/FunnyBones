//! Widgets for editing and rendering skeletons.

use cushy::{
    animation::{LinearInterpolate, PercentBetween, ZeroToOne},
    figures::{IntoComponents, Ranged},
};

use crate::{Angle, Coordinate, Rotation, Vector};

pub mod skeleton_canvas;

impl PercentBetween for Rotation {
    fn percent_between(&self, min: &Self, max: &Self) -> ZeroToOne {
        self.radians.percent_between(&min.radians, &max.radians)
    }
}

impl LinearInterpolate for Rotation {
    fn lerp(&self, target: &Self, percent: f32) -> Self {
        Self {
            radians: self.radians.lerp(&target.radians, percent),
        }
    }
}

impl LinearInterpolate for Vector {
    fn lerp(&self, target: &Self, percent: f32) -> Self {
        Self {
            magnitude: self.magnitude.lerp(&target.magnitude, percent),
            direction: self.direction.lerp(&target.direction, percent),
        }
    }
}

impl IntoComponents<f32> for Coordinate {
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

impl PercentBetween for Angle {
    fn percent_between(&self, min: &Self, max: &Self) -> ZeroToOne {
        self.0.percent_between(&min.0, &max.0)
    }
}

impl LinearInterpolate for Angle {
    fn lerp(&self, target: &Self, percent: f32) -> Self {
        Self(self.0.lerp(&target.0, percent).clamped())
    }
}
