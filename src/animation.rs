#![allow(missing_docs)]
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    num::TryFromIntError,
    ops::{Add, Deref, DerefMut, Sub},
    sync::Arc,
    time::Duration,
};

use easing_function::easings::StandardEasing;

use crate::{Angle, Bone, BoneId, Coordinate, Joint, JointId, Skeleton, Vector};

#[derive(Default, Debug, PartialEq, Clone)]
pub struct Animation(Arc<AnimationData>);

impl Animation {
    fn data_mut(&mut self) -> &mut AnimationData {
        Arc::make_mut(&mut self.0)
    }

    pub fn push(&mut self, timeline: Timeline) {
        self.data_mut().timelines.push(timeline);
    }

    #[must_use]
    pub fn with(mut self, timeline: Timeline) -> Self {
        self.push(timeline);
        self
    }

    pub fn remove(&mut self, timeline_index: usize) -> Timeline {
        self.data_mut().timelines.remove(timeline_index)
    }

    pub fn insert(&mut self, index: usize, timeline: Timeline) {
        self.data_mut().timelines.insert(index, timeline);
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
            repeat: false,
            elapsed: Duration::ZERO,
            timelines: Vec::new(),
        }
    }
}

impl Deref for Animation {
    type Target = [Timeline];

    fn deref(&self) -> &Self::Target {
        &self.0.timelines
    }
}

impl DerefMut for Animation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data_mut().timelines
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename = "Animation"))]
struct AnimationData {
    variables: HashMap<String, f32>,
    timelines: Vec<Timeline>,
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Timeline {
    target: Target,
    frames: Vec<Keyframe>,
}

impl Timeline {
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self {
            target,
            frames: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_frame(mut self, frame: Keyframe) -> Self {
        self.insert_frame(frame);
        self
    }

    pub fn insert_frame(&mut self, frame: Keyframe) {
        match self
            .frames
            .binary_search_by_key(&frame.frame_offset, |f| f.frame_offset)
        {
            Ok(existing_index) => {
                self.frames[existing_index] = frame;
            }
            Err(insert_at) => {
                self.frames.insert(insert_at, frame);
            }
        }
    }

    pub fn set_frame_offset(&mut self, index: usize, new_offset: Frame) {
        let current_offset = self.frames[index].frame_offset;
        let (slice_offset, slice) = match current_offset.cmp(&new_offset) {
            Ordering::Less => (0, &self.frames[0..index]),
            Ordering::Equal => return,
            Ordering::Greater => (index + 1, &self.frames[index + 1..]),
        };

        match slice.binary_search_by_key(&new_offset, |f| f.frame_offset) {
            Ok(relative_index) => {
                let new_index = relative_index + slice_offset;
                self.frames[new_index].easing = self.frames[index].easing;
                self.frames[new_index].update = self.frames[index].update;
                self.frames.remove(index);
            }
            Err(relative_index) => {
                let mut new_index = relative_index + slice_offset;
                let mut removed = self.frames.remove(index);
                removed.frame_offset = new_offset;
                if slice_offset > 0 {
                    new_index -= 1;
                }
                self.frames.insert(new_index, removed);
            }
        }
    }
}

impl Deref for Timeline {
    type Target = [Keyframe];

    fn deref(&self) -> &Self::Target {
        &self.frames
    }
}

impl DerefMut for Timeline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.frames
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Keyframe {
    frame_offset: Frame,
    pub update: PropertyUpdate,
    pub easing: StandardEasing,
}

impl Keyframe {
    #[must_use]
    pub fn new(frame_offset: Frame, update: PropertyUpdate) -> Self {
        Self {
            frame_offset,
            update,
            easing: StandardEasing::default(),
        }
    }

    #[must_use]
    pub fn with_easing(mut self, easing: StandardEasing) -> Self {
        self.easing = easing;
        self
    }
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Target {
    Bone {
        bone: BoneId,
        property: BoneProperty,
    },
    Joint {
        joint: JointId,
        property: JointProperty,
    },
}

impl Target {
    #[must_use]
    pub fn get(&self, skeleton: &Skeleton) -> Value {
        match self {
            Target::Bone { bone, property } => skeleton.bone(*bone).map(|bone| property.get(bone)),
            Target::Joint { joint, property } => {
                skeleton.joint(*joint).map(|joint| property.get(joint))
            }
        }
        .unwrap_or(Value::Invalid)
    }

    pub fn update(&self, value: Value, skeleton: &mut Skeleton) {
        match self {
            Target::Bone { bone, property } => {
                let Some(bone) = skeleton.bone_mut(*bone) else {
                    return;
                };
                property.update(value, bone);
            }
            Target::Joint { joint, property } => {
                let Some(joint) = skeleton.joint_mut(*joint) else {
                    return;
                };
                property.update(value, joint);
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BoneProperty {
    Target,
    // Scale,
    Inverse,
}

impl BoneProperty {
    #[must_use]
    pub fn get(&self, bone: &Bone) -> Value {
        match self {
            BoneProperty::Target => Value::Vector(
                bone.desired_end()
                    .unwrap_or_else(|| Vector::new(bone.kind().full_length(), Angle::default())),
            ),
            // BoneProperty::Scale => ,
            BoneProperty::Inverse => Value::Bool(bone.kind().is_inverse()),
        }
    }

    pub fn update(&self, value: Value, bone: &mut Bone) {
        match self {
            BoneProperty::Target => {
                let Value::Vector(value) = value else {
                    return;
                };
                bone.set_desired_end(Some(value));
            }
            BoneProperty::Inverse => {
                let Value::Bool(value) = value else {
                    return;
                };
                bone.kind_mut().set_inverse(value);
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum JointProperty {
    Angle,
}

impl JointProperty {
    #[must_use]
    pub fn get(&self, joint: &Joint) -> Value {
        match self {
            JointProperty::Angle => Value::Number(joint.angle().to_radians()),
        }
    }

    pub fn update(&self, value: Value, joint: &mut Joint) {
        match self {
            JointProperty::Angle => {
                let Value::Number(value) = value else {
                    return;
                };
                joint.set_angle(Angle::radians(value));
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    Invalid,
    Number(f32),
    Vector(Vector),
    Bool(bool),
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Number(value)
    }
}

impl From<Vector> for Value {
    fn from(value: Vector) -> Self {
        Self::Vector(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl Lerp for Value {
    fn lerp(self, target: Self, percent: f32) -> Self {
        match (self, target) {
            (Value::Number(this), Value::Number(target)) => {
                Self::Number(this.lerp(target, percent))
            }
            (Value::Vector(this), Value::Vector(target)) => {
                Self::Vector(this.lerp(target, percent))
            }
            (Value::Bool(this), Value::Bool(target)) => Self::Bool(this.lerp(target, percent)),
            _ => Value::Invalid,
        }
    }
}

impl Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Number(this), Value::Number(target)) => Self::Number(this + target),
            (Value::Vector(this), Value::Vector(target)) => Self::Vector(this + target),
            (Value::Bool(this), Value::Bool(target)) => Self::Bool(this ^ target),
            _ => Value::Invalid,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropertyUpdate {
    ChangeTo(Value),
    Add(Value),
}

impl PropertyUpdate {
    #[must_use]
    pub fn target(&self, initial: Value) -> Value {
        match self {
            PropertyUpdate::ChangeTo(target) => *target,
            PropertyUpdate::Add(delta) => initial + *delta,
        }
    }
}

pub struct RunningAnimation {
    animation: Animation,
    elapsed: Duration,
    repeat: bool,
    timelines: Vec<RunningTimeline>,
}

impl RunningAnimation {
    #[must_use]
    pub fn looping(mut self) -> Self {
        self.repeat = true;
        self
    }

    pub fn update(&mut self, elapsed: Duration, skeleton: &mut Skeleton) -> bool {
        if self.animation.is_empty() {
            return false;
        }

        if self.timelines.is_empty() {
            self.timelines = self
                .animation
                .iter()
                .map(|timeline| {
                    let frame_start_value = timeline.target.get(skeleton);
                    RunningTimeline {
                        frame_entry: Frame::MIN,
                        frame: 0,
                        frame_start_value,
                        frame_target_value: timeline
                            .frames
                            .first()
                            .map_or(Value::Invalid, |f| f.update.target(frame_start_value)),
                    }
                })
                .collect();
        }

        self.elapsed += elapsed;
        let frame = Frame::try_from(self.elapsed).unwrap_or(Frame::MAX);
        loop {
            let mut still_running = false;
            let mut remaining = frame;
            for (index, timeline) in self.timelines.iter_mut().enumerate() {
                match timeline.update(&self.animation[index], frame, skeleton) {
                    Ok(_) => {
                        still_running = true;
                    }
                    Err(unused) => {
                        remaining = remaining.min(unused);
                    }
                }
            }

            if still_running {
                return false;
            } else if self.repeat {
                self.elapsed = Duration::from(remaining)
                    + Duration::from_nanos(u64::from(self.elapsed.subsec_nanos() % 1_000_000));
            } else {
                return true;
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Ord, PartialOrd, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame(u32);

impl Frame {
    pub const MAX: Self = Self(u32::MAX);
    pub const MIN: Self = Self(0);
    pub const ZERO: Self = Self::MIN;
}

impl Sub for Frame {
    type Output = Frame;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl TryFrom<Duration> for Frame {
    type Error = TryFromIntError;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        value.as_millis().try_into().map(Self)
    }
}

impl From<Frame> for Duration {
    fn from(value: Frame) -> Self {
        Duration::from_millis(u64::from(value.0))
    }
}

impl From<u32> for Frame {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Frame> for u32 {
    fn from(value: Frame) -> Self {
        value.0
    }
}

impl From<Frame> for f32 {
    #[allow(clippy::cast_precision_loss)]
    fn from(value: Frame) -> Self {
        value.0 as f32
    }
}

struct RunningTimeline {
    frame_entry: Frame,
    frame: usize,
    frame_start_value: Value,
    frame_target_value: Value,
}

impl RunningTimeline {
    fn update(
        &mut self,
        timeline: &Timeline,
        absolute_frame: Frame,
        skeleton: &mut Skeleton,
    ) -> Result<Frame, Frame> {
        let Some(mut frame) = timeline.frames.get(self.frame) else {
            return Err(absolute_frame);
        };

        loop {
            let relative_frame = absolute_frame - self.frame_entry;
            let total_frames = frame.frame_offset - self.frame_entry;
            if relative_frame < frame.frame_offset {
                let percent = if total_frames > Frame::ZERO {
                    f32::from(relative_frame) / f32::from(total_frames)
                } else {
                    1.
                };
                let value = self
                    .frame_start_value
                    .lerp(self.frame_target_value, percent);
                timeline.target.update(value, skeleton);
                return Ok(frame.frame_offset - relative_frame);
            }

            self.frame_start_value = self.frame_start_value.lerp(self.frame_target_value, 1.0);
            timeline.target.update(self.frame_start_value, skeleton);
            self.frame += 1;
            self.frame_entry = relative_frame;

            let Some(next_frame) = timeline.frames.get(self.frame) else {
                return Err(self.frame_entry);
            };
            self.frame_target_value = next_frame.update.target(self.frame_start_value);
            frame = next_frame;
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

impl Lerp for Coordinate {
    fn lerp(self, target: Self, percent: f32) -> Self {
        Coordinate::new(
            self.x.lerp(target.x, percent),
            self.y.lerp(target.y, percent),
        )
    }
}

impl Lerp for Angle {
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

impl Lerp for Vector {
    fn lerp(self, target: Self, percent: f32) -> Self {
        Self::new(
            self.magnitude.lerp(target.magnitude, percent),
            self.direction.lerp(target.direction, percent),
        )
    }
}

impl Lerp for bool {
    fn lerp(self, target: Self, percent: f32) -> Self {
        if percent >= 0.5 {
            target
        } else {
            self
        }
    }
}

#[test]
fn basic() {
    use crate::BoneKind;

    #[track_caller]
    fn assert_approx_eq(lhs: f32, rhs: f32) {
        assert!((lhs - rhs).abs() < 0.0001, "{lhs} != {rhs}");
    }

    let mut skeleton = Skeleton::default();
    let root = skeleton.push_bone(BoneKind::Rigid { length: 1. });
    let arm = skeleton.push_bone(BoneKind::Rigid { length: 1. });
    let joint = skeleton.push_joint(Joint::new(Angle::default(), root.axis_b(), arm.axis_a()));

    let animation = Animation::default().with(
        Timeline::new(Target::Joint {
            joint,
            property: JointProperty::Angle,
        })
        .with_frame(Keyframe::new(
            Frame::ZERO,
            PropertyUpdate::ChangeTo(Value::from(0.)),
        ))
        .with_frame(Keyframe::new(
            Frame::from(1_000),
            PropertyUpdate::Add(Value::from(1.)),
        )),
    );

    let mut running = animation.start();
    assert!(!running.update(Duration::from_millis(0), &mut skeleton));
    assert_approx_eq(skeleton[joint].angle().to_radians(), 0.);
    assert!(!running.update(Duration::from_millis(250), &mut skeleton));
    assert_approx_eq(skeleton[joint].angle().to_radians(), 0.25);
    assert!(!running.update(Duration::from_millis(500), &mut skeleton));
    assert_approx_eq(skeleton[joint].angle().to_radians(), 0.75);
    assert!(running.update(Duration::from_millis(500), &mut skeleton));
    assert_approx_eq(skeleton[joint].angle().to_radians(), 1.0);
}
