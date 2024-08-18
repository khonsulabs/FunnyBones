#![allow(missing_docs)]
use std::{
    borrow::Cow,
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};

use easing_function::{easings::StandardEasing, Easing};

use crate::{BoneId, JointId, Rotation, Skeleton, Vector};

#[derive(Default, Debug, PartialEq, Clone)]
pub struct Animation(Arc<AnimationData>);

impl Animation {
    fn data_mut(&mut self) -> &mut AnimationData {
        Arc::make_mut(&mut self.0)
    }

    pub fn push(&mut self, frame: Frame) {
        self.data_mut().frames.push(frame);
    }

    #[must_use]
    pub fn with(mut self, frame: Frame) -> Self {
        self.push(frame);
        self
    }

    pub fn remove(&mut self, frame_index: usize) -> Frame {
        self.data_mut().frames.remove(frame_index)
    }

    pub fn insert(&mut self, index: usize, frame: Frame) {
        self.data_mut().frames.insert(index, frame);
    }

    #[must_use]
    pub fn with_variable(mut self, name: impl Into<String>, value: f32) -> Self {
        self.set_variable(name.into(), value);
        self
    }

    pub fn set_variable<'a>(&mut self, name: impl Into<Cow<'a, str>>, value: f32) {
        match name.into() {
            Cow::Owned(name) => {
                self.data_mut().variables.insert(name, value);
            }
            Cow::Borrowed(name) => {
                if let Some(var) = self.data_mut().variables.get_mut(name) {
                    *var = value;
                } else {
                    self.data_mut().variables.insert(name.to_string(), value);
                }
            }
        }
    }

    #[must_use]
    pub fn variable(&self, name: &str) -> Option<f32> {
        self.0.variables.get(name).copied()
    }

    #[must_use]
    pub fn start(&self) -> RunningAnimation {
        RunningAnimation {
            animation: self.clone(),
            frame_elapsed: Duration::ZERO,
            frame: 0,
            repeat: false,
            frame_props: Vec::new(),
        }
    }
}

impl Deref for Animation {
    type Target = [Frame];

    fn deref(&self) -> &Self::Target {
        &self.0.frames
    }
}

impl DerefMut for Animation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data_mut().frames
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "Animation"))]
struct AnimationData {
    variables: HashMap<String, f32>,
    frames: Vec<Frame>,
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
    duration: Duration,
    changes: Vec<Change>,
}

impl Frame {
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            changes: Vec::new(),
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn with_change(mut self, change: impl Into<Change>) -> Self {
        self.push_change(change.into());
        self
    }

    pub fn push_change(&mut self, change: impl Into<Change>) {
        self.changes.push(change.into());
    }
}

impl Deref for Frame {
    type Target = [Change];

    fn deref(&self) -> &Self::Target {
        &self.changes
    }
}

impl DerefMut for Frame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.changes
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Change {
    kind: ChangeKind,
    easing: StandardEasing,
}

impl From<ChangeKind> for Change {
    fn from(kind: ChangeKind) -> Self {
        Self {
            kind,
            easing: StandardEasing::Linear,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChangeKind {
    Bone { bone: BoneId, position: Vector },
    Joint { joint: JointId, rotation: Rotation },
}

impl ChangeKind {
    #[must_use]
    pub const fn with_easing(self, easing: StandardEasing) -> Change {
        Change { kind: self, easing }
    }
}

enum OriginalProperty {
    Rotation(Rotation),
    Vector(Vector),
}

pub struct RunningAnimation {
    animation: Animation,
    frame: usize,
    frame_elapsed: Duration,
    repeat: bool,
    frame_props: Vec<OriginalProperty>,
}

impl RunningAnimation {
    #[must_use]
    pub fn looping(mut self) -> Self {
        self.repeat = true;
        self
    }

    pub fn update(&mut self, elapsed: Duration, skeleton: &mut Skeleton) -> bool {
        loop {
            let Some(frame) = self.animation.get(self.frame) else {
                return false;
            };

            self.frame_elapsed += elapsed;
            if let Some(after_frame) = self.frame_elapsed.checked_sub(frame.duration) {
                self.frame_elapsed = after_frame;
                self.frame += 1;
                self.frame_props.clear();
                if self.frame == self.animation.len() && self.repeat {
                    self.frame = 0;
                }
                // Ensure all of the changes are fully tweened.
                for change in &frame.changes {
                    match change.kind {
                        ChangeKind::Bone {
                            bone,
                            position: target,
                        } => {
                            skeleton[bone].set_desired_end(Some(target));
                        }

                        ChangeKind::Joint {
                            joint,
                            rotation: target,
                        } => {
                            skeleton[joint].set_angle(target);
                        }
                    }
                }
            } else {
                // If this is the start of the frame, grab the currrent values
                // to tween towards the next keyframe.
                if self.frame_props.len() != frame.changes.len() {
                    self.frame_props.clear();
                    self.frame_props.reserve(frame.changes.len());
                    for change in &frame.changes {
                        self.frame_props.push(match change.kind {
                            ChangeKind::Bone { bone, .. } => {
                                OriginalProperty::Vector(skeleton[bone].end())
                            }

                            ChangeKind::Joint { joint, .. } => {
                                OriginalProperty::Rotation(skeleton[joint].angle())
                            }
                        });
                    }
                }

                let percent = self.frame_elapsed.as_secs_f32() / frame.duration.as_secs_f32();
                for (change, original) in frame.changes.iter().zip(&self.frame_props) {
                    let factor = change.easing.ease(percent);
                    match (change.kind, original) {
                        (
                            ChangeKind::Bone {
                                bone,
                                position: target,
                            },
                            OriginalProperty::Vector(original),
                        ) => {
                            skeleton[bone].set_desired_end(Some(original.lerp(target, factor)));
                        }
                        (
                            ChangeKind::Joint {
                                joint,
                                rotation: target,
                            },
                            OriginalProperty::Rotation(original),
                        ) => {
                            skeleton[joint].set_angle(original.lerp(target, factor));
                        }
                        _ => unreachable!(),
                    }
                }
                return true;
            }
        }
    }
}

trait Lerp: Sized {
    fn lerp(self, target: Self, percent: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, target: Self, percent: f32) -> Self {
        let delta = target - self;
        self + delta * percent
    }
}

impl Lerp for Vector {
    fn lerp(self, target: Self, percent: f32) -> Self {
        Vector::new(
            self.x.lerp(target.x, percent),
            self.y.lerp(target.y, percent),
        )
    }
}

impl Lerp for Rotation {
    fn lerp(self, target: Self, percent: f32) -> Self {
        let delta_neg = self.radians - target.radians;
        let delta_pos = target.radians - self.radians;

        Self::radians(
            self.radians
                + if delta_neg.abs() < delta_pos.abs() {
                    delta_neg * percent
                } else {
                    delta_pos * percent
                },
        )
    }
}
