#![doc = include_str!("../README.md")]

use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    fmt::{Debug, Display},
    ops::{Add, Index, IndexMut, Neg, Sub},
};

/// A two dimensionsional offset/measurement.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub struct Vector {
    /// The x-axis component of this vector.
    pub x: f32,
    /// The y-axis component of this vector.
    pub y: f32,
}

impl Vector {
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
}

impl Sub for Vector {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[cfg(feature = "cushy")]
impl cushy::figures::IntoComponents<f32> for Vector {
    fn into_components(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

#[cfg(feature = "cushy")]
impl cushy::figures::FromComponents<f32> for Vector {
    fn from_components(components: (f32, f32)) -> Self {
        Self::new(components.0, components.1)
    }
}

/// A value representing a rotation between no rotation and a full rotation.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Rotation {
    radians: f32,
}

impl Rotation {
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
}

#[cfg(feature = "cushy")]
impl cushy::animation::PercentBetween for Rotation {
    fn percent_between(&self, min: &Self, max: &Self) -> cushy::animation::ZeroToOne {
        self.radians.percent_between(&min.radians, &max.radians)
    }
}

#[cfg(feature = "cushy")]
impl cushy::animation::LinearInterpolate for Rotation {
    fn lerp(&self, target: &Self, percent: f32) -> Self {
        Self {
            radians: self.radians.lerp(&target.radians, percent),
        }
    }
}

impl Debug for Rotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Rotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}°", self.to_degrees())
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Self { radians: 0. }
    }
}

impl Add for Rotation {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::radians(self.radians + rhs.radians)
    }
}

impl Sub for Rotation {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::radians(self.radians - rhs.radians)
    }
}

impl Neg for Rotation {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::radians(-self.radians)
    }
}

/// A representation of a bone structure inside of a [`Skeleton`].
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// A collection of [`Bone`]s. connected by [`Joint`]s.
#[derive(Default, Debug)]
pub struct Skeleton {
    initial_joint: Option<JointId>,
    bones: Vec<Bone>,
    joints: Vec<Joint>,
    connections: HashMap<BoneAxis, Vec<JointId>>,
    generation: usize,
}

impl Skeleton {
    /// Creates a new [`Bone`] into the skeleton. Returns the unique id of the
    /// created bone.
    ///
    /// The first bone pushed is considered the root of the skeleton. All other
    /// bones must be connected to the root directly or indirectly through
    /// [`Joint`]s.
    pub fn push_bone(&mut self, bone: BoneKind, label: &'static str) -> BoneId {
        let id = BoneId(u8::try_from(self.bones.len()).expect("too many bones"));
        if id == BoneId(0) {
            let joint = self.push_joint(Rotation::default(), id.axis_a(), id.axis_a());
            self.initial_joint = Some(joint);
            self.connections.insert(id.axis_a(), vec![joint]);
        }
        self.bones.push(Bone {
            generation: self.generation,
            label,
            kind: bone,
            start: Vector::default(),
            joint_pos: None,
            end: Vector::default(),
            desired_end: None,
        });
        id
    }

    /// Creates a new [`Joint`] in the skeleton, connecting two bones together
    /// by their [axis](BoneAxis). Returns the unique id of the created joint.
    pub fn push_joint(&mut self, angle: Rotation, bone_a: BoneAxis, bone_b: BoneAxis) -> JointId {
        let id = JointId(u8::try_from(self.joints.len()).expect("too many joints"));
        self.joints.push(Joint {
            bone_a,
            bone_b,
            angle,
            calculated_position: Vector::default(),
        });
        self.connections.entry(bone_a).or_default().push(id);
        if bone_a != bone_b {
            self.connections.entry(bone_b).or_default().push(id);
        }
        id
    }

    /// Sets a translation to be applied to the entire skeleton.
    pub fn set_translation(&mut self, translation: Vector) {
        let bone = self.bones.first_mut().expect("root bone must be defined");
        bone.start = translation;
    }

    /// Returns the translation applied to the entire skeleton.
    #[must_use]
    pub fn translation(&self) -> Vector {
        self.bones.first().expect("root bone must be defined").start
    }

    /// Sets a base rotation to apply to the entire skeleton.
    pub fn set_rotation(&mut self, rotation: Rotation) {
        let joint = self.initial_joint.expect("root bone must be defined");
        let joint = &mut self[joint];
        joint.angle = rotation;
    }

    /// Returns the base rotation being applied to the entire skeleton.
    #[must_use]
    pub fn rotation(&self) -> Rotation {
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
        let mut to_solve = vec![(axis, None, Rotation::default(), false)];
        while let Some((axis, current_position, current_rotation, inverse_root)) = to_solve.pop() {
            if !axis_solved.insert(axis) {
                continue;
            }

            let Some(connections) = self.connections.get(&axis) else {
                continue;
            };

            println!(
                "Solving {}:{:?} at {current_position:?} - {current_rotation} - {inverse_root}",
                self.bones[usize::from(axis.bone.0)].label,
                axis.end
            );

            for joint_id in connections {
                let joint = &mut self.joints[usize::from(joint_id.0)];
                let other_axis = joint.other_axis(axis);
                let bone = &mut self.bones[usize::from(other_axis.bone.0)];
                if bone.generation == self.generation {
                    // We store connections in both directions, which means we
                    // can visit bones twice. We want to ensure we only follow
                    // each bone a single time.
                    continue;
                }
                bone.generation = self.generation;
                println!(
                    "  -> {joint_id:?} -> {}:{:?} ({})",
                    bone.label, other_axis.end, joint.angle
                );
                joint.calculated_position = if let Some(current_position) = current_position {
                    bone.start = current_position;
                    current_position
                } else {
                    debug_assert_eq!(axis.bone.0, 0);
                    bone.start
                };

                let angle = if inverse_root {
                    Rotation::radians(PI) - joint.angle
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
                    let rotation = Rotation::radians(final_delta.y.atan2(final_delta.x));
                    // TODO I don't know why rotating by 90 degrees fixes
                    // everything here. It feels like atan2 should be giving us
                    // the correct rotation, or the correction amount should be
                    // driven by an input angle, but a fixed correction amount
                    // seems to be the correct answer. Without this, a joint
                    // angle of 0 sticks out at a perpendicular angle.
                    next_rotation = rotation + Rotation::radians(PI / 2.);
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

fn next_point(mut point: Vector, angle: Rotation, length: f32) -> Vector {
    point.x += length * angle.radians.sin();
    point.y -= length * angle.radians.cos();
    point
}

fn determine_end_position(
    start: Vector,
    desired_end: Option<Vector>,
    angle: Rotation,
    bone: &BoneKind,
) -> (Vector, Option<Vector>) {
    match bone {
        BoneKind::Rigid { length } => (next_point(start, angle, *length), None),
        BoneKind::Jointed {
            start_length,
            end_length,
            inverse,
        } => {
            if let Some(desired_end) = desired_end {
                let delta = desired_end - start;
                let desired_angle = Rotation::radians(delta.y.atan2(delta.x) + PI / 2.);
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
                    next_point(start, desired_angle, desired_length)
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
                let joint = next_point(start, angle, *start_length);
                let end = next_point(joint, angle, *end_length);
                (end, Some(joint))
            }
        }
    }
}

fn get_third_point(
    inverse: bool,
    start: Vector,
    distance: f32,
    hyp_angle: Rotation,
    first: f32,
    second: f32,
) -> Vector {
    let hyp = distance;
    let first_angle = ((first * first + hyp * hyp - second * second) / (2. * first * hyp)).acos();
    if first_angle.is_nan() {
        next_point(start, hyp_angle, first)
    } else {
        let first_angle = hyp_angle
            - Rotation {
                radians: if inverse { -first_angle } else { first_angle },
            };
        next_point(start, first_angle, first)
    }
}

impl Index<BoneId> for Skeleton {
    type Output = Bone;

    fn index(&self, index: BoneId) -> &Self::Output {
        &self.bones[usize::from(index.0)]
    }
}

impl IndexMut<BoneId> for Skeleton {
    fn index_mut(&mut self, index: BoneId) -> &mut Self::Output {
        &mut self.bones[usize::from(index.0)]
    }
}

impl Index<JointId> for Skeleton {
    type Output = Joint;

    fn index(&self, index: JointId) -> &Self::Output {
        &self.joints[usize::from(index.0)]
    }
}

impl IndexMut<JointId> for Skeleton {
    fn index_mut(&mut self, index: JointId) -> &mut Self::Output {
        &mut self.joints[usize::from(index.0)]
    }
}

/// A specific end of a specific bone.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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
#[derive(Debug)]
pub struct Bone {
    generation: usize,
    label: &'static str,
    kind: BoneKind,
    start: Vector,
    joint_pos: Option<Vector>,
    end: Vector,
    desired_end: Option<Vector>,
}

impl Bone {
    /// Sets the location to aim the end of this bone towards.
    ///
    /// The end of the bone that is aimed is the end that is furthest from the
    /// root of the skeleton.
    ///
    /// This setting only impacts [`BoneKind::Jointed`] bones.
    pub fn set_desired_end(&mut self, end: Option<Vector>) {
        self.desired_end = end;
    }

    /// Returns the location this bone is being aimed towards.
    #[must_use]
    pub const fn desired_end(&self) -> Option<Vector> {
        self.desired_end
    }

    /// Returns the solved start position of this bone.
    #[must_use]
    pub const fn start(&self) -> Vector {
        self.start
    }

    /// Returns the solved end position of this bone.
    #[must_use]
    pub const fn end(&self) -> Vector {
        self.end
    }

    /// If this is a [`BoneKind::Jointed`] bone, returns the solved position of
    /// the joint.
    #[must_use]
    pub const fn solved_joint(&self) -> Option<Vector> {
        self.joint_pos
    }

    /// Returns the label this bone was created with.
    #[must_use]
    pub fn label(&self) -> &str {
        self.label
    }
}

/// A connection between two bones.
#[derive(Debug)]
pub struct Joint {
    bone_a: BoneAxis,
    bone_b: BoneAxis,
    calculated_position: Vector,
    angle: Rotation,
}

impl Joint {
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
    pub fn set_angle(&mut self, angle: Rotation) {
        self.angle = angle;
    }

    /// Returns the rotation of this joint.
    #[must_use]
    pub const fn angle(&self) -> Rotation {
        self.angle
    }
}

/// The unique ID of a [`Bone`] in a [`Skeleton`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoneId(u8);

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
}

/// The unique ID of a [`Joint`] in a [`Skeleton`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct JointId(u8);

/// A specific end of a [`Bone`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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
        (Rotation::degrees(90.) + Rotation::degrees(180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        270,
    );
    assert_eq!(
        (Rotation::degrees(90.) + Rotation::degrees(-180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        270,
    );
}