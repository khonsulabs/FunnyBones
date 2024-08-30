#![doc = include_str!(".crate-docs.md")]

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    f32::consts::PI,
    fmt::{Debug, Display},
    ops::{Add, Deref, Div, Index, IndexMut, Mul, Neg, Sub},
    sync::Arc,
    vec::Vec,
};

pub mod animation;
#[cfg(feature = "cushy")]
pub mod cushy;
#[cfg(feature = "serde")]
mod serde;

/// A two dimensionsional offset/measurement.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct Coordinate {
    /// The x-axis component of this vector.
    pub x: f32,
    /// The y-axis component of this vector.
    pub y: f32,
}

impl Coordinate {
    /// Returns a new vector from the x and y values.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns the magnitude of this vector.
    #[must_use]
    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Returns the result of mapping `x` and `y` to `f`.
    #[must_use]
    pub fn map(self, mut f: impl FnMut(f32) -> f32) -> Self {
        Self {
            x: f(self.x),
            y: f(self.y),
        }
    }

    /// Returns the angle formed a line passing through 0,0 towards this vector.
    #[must_use]
    pub fn as_rotation(self) -> Angle {
        Angle::radians(self.y.atan2(self.x))
    }
}

impl Add for Coordinate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Coordinate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Coordinate {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Div<f32> for Coordinate {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

/// A value representing a direction.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct Angle {
    radians: f32,
}

impl Angle {
    /// The minimum rotation represented by this type.
    pub const MIN: Self = Self { radians: 0. };
    /// The maximum rotation represented by this type.
    pub const MAX: Self = Self {
        radians: PI * 2. - f32::EPSILON,
    };

    /// Returns a rotation representing the given radians.
    #[must_use]
    pub fn radians(radians: f32) -> Self {
        Self { radians }.normalized()
    }

    /// Returns a rotation representing the given degrees.
    #[must_use]
    pub fn degrees(degrees: f32) -> Self {
        Self::radians(degrees * PI / 180.0)
    }

    /// Returns this rotation represented in degrees.
    ///
    /// This value will always be greater than or equal to 0 and will always be
    /// less than 360.0.
    #[must_use]
    pub fn to_degrees(self) -> f32 {
        self.radians * 180.0 / PI
    }

    /// Returns this rotation represented in radians.
    ///
    /// This value will always be greater than or equal to 0 and will always be
    /// less than `2π`.
    #[must_use]
    pub const fn to_radians(self) -> f32 {
        self.radians
    }

    fn normalized(mut self) -> Self {
        const TWO_PI: f32 = PI * 2.0;
        while self.radians >= TWO_PI {
            self.radians -= TWO_PI;
        }
        while self.radians < 0. {
            self.radians += TWO_PI;
        }
        self
    }

    /// Returns the cosine of this angle.
    #[must_use]
    pub fn cos(self) -> f32 {
        self.radians.cos()
    }

    /// Returns the sine of this angle.
    #[must_use]
    pub fn sin(self) -> f32 {
        self.radians.sin()
    }
}

impl Debug for Angle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Angle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}°", self.to_degrees())
    }
}

impl Default for Angle {
    fn default() -> Self {
        Self { radians: 0. }
    }
}

impl Add for Angle {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::radians(self.radians + rhs.radians)
    }
}

impl Sub for Angle {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::radians(self.radians - rhs.radians)
    }
}

impl Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::radians(-self.radians)
    }
}

/// A 2D Euclidean vector.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vector {
    /// The length of the vector.
    pub magnitude: f32,
    /// The direction the vector is heading.
    pub direction: Angle,
}

impl Vector {
    /// Returns a new vector for the given magnitude and direction.
    #[must_use]
    pub const fn new(magnitude: f32, direction: Angle) -> Self {
        Self {
            magnitude,
            direction,
        }
    }
}

impl From<Vector> for Coordinate {
    fn from(vec: Vector) -> Self {
        Self {
            x: vec.magnitude * vec.direction.sin(),
            y: -vec.magnitude * vec.direction.cos(),
        }
    }
}

impl From<Coordinate> for Vector {
    fn from(pt: Coordinate) -> Self {
        Self {
            direction: pt.as_rotation(),
            magnitude: pt.magnitude(),
        }
    }
}

impl Add<Vector> for Coordinate {
    type Output = Self;

    fn add(self, rhs: Vector) -> Self::Output {
        self + Coordinate::from(rhs)
    }
}

impl Sub<Vector> for Coordinate {
    type Output = Self;

    fn sub(self, rhs: Vector) -> Self::Output {
        self - Coordinate::from(rhs)
    }
}

/// A representation of a bone structure inside of a [`Skeleton`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum BoneKind {
    /// A single bone of a fixed length.
    Rigid {
        /// The length of the bone.
        length: f32,
    },
    /// Two bones connected with a joint that automatically adjusts its angle as
    /// needed.
    Jointed {
        /// The length of the bone connected closest to the root of the
        /// skeleton.
        start_length: f32,
        /// The length of the bone connected furthes from the root of the
        /// skeleton.
        end_length: f32,
        /// The bend of the simulated joint always goes in one of two
        /// directions. This boolean toggles which direction the bend goes in.
        inverse: bool,
    },
}

impl BoneKind {
    /// Attaches a label to this bone when pushed into a skeleton.
    #[must_use]
    pub fn with_label(self, label: impl Into<String>) -> LabeledBoneKind {
        LabeledBoneKind {
            kind: self,
            label: label.into(),
        }
    }
}

/// A [`BoneKind`] with an associated label.
pub struct LabeledBoneKind {
    /// The bone to create.
    pub kind: BoneKind,
    /// The label of the bone.
    pub label: String,
}

impl From<BoneKind> for LabeledBoneKind {
    fn from(kind: BoneKind) -> Self {
        kind.with_label(String::new())
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct ArcString(Arc<String>);

impl PartialEq<str> for ArcString {
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

impl Deref for ArcString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for ArcString {
    fn borrow(&self) -> &str {
        self
    }
}

/// A collection of [`Bone`]s. connected by [`Joint`]s.
#[derive(Default, Debug, PartialEq)]
pub struct Skeleton {
    initial_joint: Option<JointId>,
    bones: Vec<Bone>,
    joints: Vec<Joint>,
    connections: HashMap<BoneAxis, Vec<JointId>>,
    generation: usize,
    bones_by_label: HashMap<ArcString, BoneId>,
    joints_by_label: HashMap<ArcString, JointId>,
}

impl Skeleton {
    /// Creates a new [`Bone`] into the skeleton. Returns the unique id of the
    /// created bone.
    ///
    /// The first bone pushed is considered the root of the skeleton. All other
    /// bones must be connected to the root directly or indirectly through
    /// [`Joint`]s.
    pub fn push_bone(&mut self, bone: impl Into<LabeledBoneKind>) -> BoneId {
        let bone = bone.into();
        let id = BoneId(u16::try_from(self.bones.len()).expect("too many bones"));
        if id == BoneId(0) {
            let joint = self.push_joint(Joint::new(Angle::default(), id.axis_a(), id.axis_a()));
            self.initial_joint = Some(joint);
            self.connections.insert(id.axis_a(), vec![joint]);
        }
        let label = if bone.label.is_empty() {
            None
        } else {
            let label = ArcString(Arc::new(bone.label));
            self.bones_by_label.insert(label.clone(), id);
            Some(label)
        };
        self.bones.push(Bone {
            id,
            generation: self.generation,
            label,
            kind: bone.kind,
            start: Coordinate::default(),
            joint_pos: None,
            end: Coordinate::default(),
            desired_end: None,
        });
        id
    }

    /// Returns the list of bones in this skeleton.
    #[must_use]
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    /// Returns the list of joints in this skeleton.
    #[must_use]
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    /// Returns a list of joints connected to a specific bone axis.
    #[must_use]
    pub fn connections_to(&self, axis: BoneAxis) -> Option<&[JointId]> {
        self.connections.get(&axis).map(Vec::as_slice)
    }

    /// Creates a new [`Joint`] in the skeleton, connecting two bones together
    /// by their [axis](BoneAxis). Returns the unique id of the created joint.
    pub fn push_joint(&mut self, mut joint: Joint) -> JointId {
        let id = JointId(u16::try_from(self.joints.len()).expect("too many joints"));
        joint.id = id;
        let bone_a = joint.bone_a;
        let bone_b = joint.bone_b;
        if let Some(label) = joint.label.clone() {
            self.joints_by_label.insert(label, id);
        }
        self.joints.push(joint);
        self.connections.entry(bone_a).or_default().push(id);
        if bone_a != bone_b {
            self.connections.entry(bone_b).or_default().push(id);
        }
        id
    }

    /// Finds an existing [`Joint`] by its label.
    #[must_use]
    pub fn find_joint_by_label(&self, label: &str) -> Option<JointId> {
        self.joints_by_label.get(label).copied()
    }

    /// Finds an existing [`Bone`] by its label.
    #[must_use]
    pub fn find_bone_by_label(&self, label: &str) -> Option<BoneId> {
        self.bones_by_label.get(label).copied()
    }

    /// Sets a translation to be applied to the entire skeleton.
    pub fn set_translation(&mut self, translation: Coordinate) {
        let bone = self.bones.first_mut().expect("root bone must be defined");
        bone.start = translation;
    }

    /// Returns the translation applied to the entire skeleton.
    #[must_use]
    pub fn translation(&self) -> Coordinate {
        self.bones.first().expect("root bone must be defined").start
    }

    /// Sets a base rotation to apply to the entire skeleton.
    pub fn set_rotation(&mut self, rotation: Angle) {
        let joint = self.initial_joint.expect("root bone must be defined");
        let joint = &mut self[joint];
        joint.angle = rotation;
    }

    /// Returns the base rotation being applied to the entire skeleton.
    #[must_use]
    pub fn rotation(&self) -> Angle {
        let joint = self.initial_joint.expect("root bone must be defined");
        self[joint].angle
    }

    /// Updates the solved positions of all bones in this skeleton that are
    /// connected either directly or indirectly to the root bone via [`Joint`]s.
    pub fn solve(&mut self) {
        if !self.bones.is_empty() {
            self.generation = self.generation.wrapping_add(1);
            self.solve_axis(BoneId(0).axis_a());
        }
    }

    fn solve_axis(&mut self, axis: BoneAxis) {
        let mut axis_solved = HashSet::new();
        let mut to_solve = vec![(axis, None, Angle::default(), false)];
        while let Some((axis, current_position, current_rotation, inverse_root)) = to_solve.pop() {
            if !axis_solved.insert(axis) {
                continue;
            }

            let Some(connections) = self.connections.get(&axis) else {
                continue;
            };

            for joint_id in connections {
                let joint = &mut self.joints[usize::from(joint_id.0)];
                let other_axis = joint.other_axis(axis);
                let bone = &mut self.bones[other_axis.bone.index()];
                if bone.generation == self.generation {
                    // We store connections in both directions, which means we
                    // can visit bones twice. We want to ensure we only follow
                    // each bone a single time.
                    continue;
                }
                bone.generation = self.generation;
                joint.calculated_position = if let Some(current_position) = current_position {
                    bone.start = current_position;
                    current_position
                } else {
                    debug_assert_eq!(axis.bone.0, 0);
                    bone.start
                };

                let angle = if inverse_root {
                    Angle::radians(PI) - joint.angle
                } else {
                    joint.angle
                };

                let mut next_rotation = current_rotation + angle;
                let (end, mid) = determine_end_position(
                    joint.calculated_position,
                    bone.desired_end,
                    next_rotation,
                    &bone.kind,
                );
                bone.end = end;
                bone.joint_pos = mid;
                if let Some(mid) = mid {
                    let final_delta = end - mid;
                    let rotation = final_delta.as_rotation();
                    // TODO I don't know why rotating by 90 degrees fixes
                    // everything here. It feels like atan2 should be giving us
                    // the correct rotation, or the correction amount should be
                    // driven by an input angle, but a fixed correction amount
                    // seems to be the correct answer. Without this, a joint
                    // angle of 0 sticks out at a perpendicular angle.
                    next_rotation = rotation + Angle::radians(PI / 2.);
                }

                if axis == BoneId(0).axis_a() && other_axis == axis {
                    // The first joint doesn't have any real connection, so we
                    // must manually traverse the other side of the root bone.
                    to_solve.push((
                        axis.bone.axis_b(),
                        Some(self.bones[0].end),
                        current_rotation,
                        true,
                    ));
                } else {
                    to_solve.push((other_axis.inverse(), Some(bone.end), next_rotation, true));
                }
            }
        }
    }
}

fn determine_end_position(
    start: Coordinate,
    desired_end: Option<Coordinate>,
    angle: Angle,
    bone: &BoneKind,
) -> (Coordinate, Option<Coordinate>) {
    match bone {
        BoneKind::Rigid { length } => (start + Vector::new(*length, angle), None),
        BoneKind::Jointed {
            start_length,
            end_length,
            inverse,
        } => {
            if let Some(desired_end) = desired_end {
                let delta = desired_end - start;
                let desired_angle = delta.as_rotation() + Angle::radians(PI / 2.);
                let distance = delta.magnitude();
                let full_length = start_length + end_length;
                let minimum_size = (start_length - end_length).abs();
                let (capped, desired_length) = if distance < minimum_size {
                    (true, minimum_size)
                } else if distance > full_length {
                    (true, full_length)
                } else {
                    (false, distance)
                };

                let end = if capped {
                    // We need to cap the end point along this sloped line
                    start + Vector::new(desired_length, desired_angle)
                } else {
                    // The end position is valid
                    desired_end
                };

                let joint = get_third_point(
                    *inverse,
                    start,
                    desired_length,
                    desired_angle,
                    *start_length,
                    *end_length,
                );

                (end, Some(joint))
            } else {
                let joint = start + Vector::new(*start_length, angle);
                let end = joint + Vector::new(*end_length, angle);
                (end, Some(joint))
            }
        }
    }
}

fn get_third_point(
    inverse: bool,
    start: Coordinate,
    distance: f32,
    hyp_angle: Angle,
    first: f32,
    second: f32,
) -> Coordinate {
    let hyp = distance;
    let first_angle = ((first * first + hyp * hyp - second * second) / (2. * first * hyp)).acos();
    if first_angle.is_nan() {
        start + Vector::new(first, hyp_angle)
    } else {
        let first_angle = hyp_angle
            - Angle {
                radians: if inverse { -first_angle } else { first_angle },
            };
        start + Vector::new(first, first_angle)
    }
}

impl Index<BoneId> for Skeleton {
    type Output = Bone;

    fn index(&self, index: BoneId) -> &Self::Output {
        &self.bones[index.index()]
    }
}

impl IndexMut<BoneId> for Skeleton {
    fn index_mut(&mut self, index: BoneId) -> &mut Self::Output {
        &mut self.bones[index.index()]
    }
}

impl Index<JointId> for Skeleton {
    type Output = Joint;

    fn index(&self, index: JointId) -> &Self::Output {
        &self.joints[index.index()]
    }
}

impl IndexMut<JointId> for Skeleton {
    fn index_mut(&mut self, index: JointId) -> &mut Self::Output {
        &mut self.joints[index.index()]
    }
}

/// A specific end of a specific bone.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct BoneAxis {
    /// The unique id of the bone of this axis.
    pub bone: BoneId,
    /// The end of the bone being referenced.
    pub end: BoneEnd,
}

impl BoneAxis {
    /// Returns the opposite axis on the same bone.
    #[must_use]
    pub const fn inverse(self) -> Self {
        Self {
            bone: self.bone,
            end: self.end.inverse(),
        }
    }
}

/// A bone in a [`Skeleton`].
#[derive(Debug, PartialEq)]
pub struct Bone {
    id: BoneId,
    generation: usize,
    label: Option<ArcString>,
    kind: BoneKind,
    start: Coordinate,
    joint_pos: Option<Coordinate>,
    end: Coordinate,
    desired_end: Option<Coordinate>,
}

impl Bone {
    /// Returns the unique id of this bone.
    #[must_use]
    pub const fn id(&self) -> BoneId {
        self.id
    }

    /// Returns true if this bone is the root of the skeleton.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.id.0 == 0
    }

    /// Sets the location to aim the end of this bone towards.
    ///
    /// The end of the bone that is aimed is the end that is furthest from the
    /// root of the skeleton.
    ///
    /// This setting only impacts [`BoneKind::Jointed`] bones.
    pub fn set_desired_end(&mut self, end: Option<Coordinate>) {
        self.desired_end = end;
    }

    /// Returns the location this bone is being aimed towards.
    #[must_use]
    pub const fn desired_end(&self) -> Option<Coordinate> {
        self.desired_end
    }

    /// Returns the solved start position of this bone.
    #[must_use]
    pub const fn start(&self) -> Coordinate {
        self.start
    }

    /// Returns the solved end position of this bone.
    #[must_use]
    pub const fn end(&self) -> Coordinate {
        self.end
    }

    /// If this is a [`BoneKind::Jointed`] bone, returns the solved position of
    /// the joint.
    #[must_use]
    pub const fn solved_joint(&self) -> Option<Coordinate> {
        self.joint_pos
    }

    /// Returns the label this bone was created with.
    #[must_use]
    pub fn label(&self) -> &str {
        self.label.as_ref().map_or("", |s| s)
    }
}

/// A connection between two bones.
#[derive(Debug, PartialEq)]
pub struct Joint {
    id: JointId,
    label: Option<ArcString>,
    bone_a: BoneAxis,
    bone_b: BoneAxis,
    calculated_position: Coordinate,
    angle: Angle,
}

impl Joint {
    /// Returns the unique id of this joint.
    #[must_use]
    pub const fn id(&self) -> JointId {
        self.id
    }

    /// Returns a new joint formed by joining `bone_a` and `bone_b` at `angle`.
    #[must_use]
    pub const fn new(angle: Angle, bone_a: BoneAxis, bone_b: BoneAxis) -> Self {
        Self {
            id: JointId(0),
            label: None,
            bone_a,
            bone_b,
            calculated_position: Coordinate::new(0., 0.),
            angle,
        }
    }

    /// Labels this joint and returns self.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        if !label.is_empty() {
            self.label = Some(ArcString(Arc::new(label)));
        }
        self
    }

    /// Returns the label of this joint.
    #[must_use]
    pub fn label(&self) -> &str {
        self.label.as_ref().map_or("", |s| s)
    }

    /// Given `axis` is one of the two connections in this joint, return the
    /// other axis.
    ///
    /// # Panics
    ///
    /// This function has a debug assertion that ensures that `axis` is one of
    /// the bones in this joint.
    #[must_use]
    pub fn other_axis(&self, axis: BoneAxis) -> BoneAxis {
        if self.bone_a == axis {
            self.bone_b
        } else {
            debug_assert_eq!(self.bone_b, axis);
            self.bone_a
        }
    }

    /// Sets the angle to form between these joints.
    ///
    /// This setting is ignored if the bone furthest from the root of the joint
    /// is a [`BoneKind::Jointed`] bone.
    pub fn set_angle(&mut self, angle: Angle) {
        self.angle = angle;
    }

    /// Returns the rotation of this joint.
    #[must_use]
    pub const fn angle(&self) -> Angle {
        self.angle
    }
}

/// The unique ID of a [`Bone`] in a [`Skeleton`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct BoneId(u16);

impl BoneId {
    /// Returns the first axis of this bone.
    #[must_use]
    pub const fn axis_a(self) -> BoneAxis {
        BoneAxis {
            bone: self,
            end: BoneEnd::A,
        }
    }

    /// Returns the second axis of this bone.
    #[must_use]
    pub const fn axis_b(self) -> BoneAxis {
        BoneAxis {
            bone: self,
            end: BoneEnd::B,
        }
    }

    /// Returns the index of this bone within the skeleton.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// The unique ID of a [`Joint`] in a [`Skeleton`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct JointId(u16);

impl JointId {
    /// Returns the index of this joint within the skeleton.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// A specific end of a [`Bone`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum BoneEnd {
    /// The first end of a bone.
    A,
    /// The second end of a bone.
    B,
}

impl BoneEnd {
    /// Returns the opposite end of `self`.
    #[must_use]
    pub const fn inverse(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn rotation() {
    assert_eq!(
        (Angle::degrees(90.) + Angle::degrees(180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        270,
    );
    assert_eq!(
        (Angle::degrees(90.) + Angle::degrees(-180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        270,
    );
}
