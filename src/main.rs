use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    fmt::{Debug, Display},
    ops::{Add, Index, IndexMut, Neg, RangeInclusive, Sub},
};

use cushy::{
    animation::{LinearInterpolate, PercentBetween},
    figures::{
        units::{Lp, Px},
        IntoSigned, Point,
    },
    kludgine::{
        shapes::{PathBuilder, Shape, StrokeOptions},
        DrawableExt, Origin,
    },
    styles::Color,
    value::{Dynamic, DynamicRead, Source},
    widget::MakeWidget,
    widgets::{slider::Slidable, Canvas},
    Run,
};

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Rotation {
    radians: f32,
}

impl Rotation {
    pub fn radians(radians: f32) -> Self {
        Self { radians }.normalized()
    }

    pub fn degrees(degrees: f32) -> Self {
        Self::radians(degrees * PI / 180.0)
    }

    pub fn to_degrees(&self) -> f32 {
        self.radians * 180.0 / PI
    }

    fn normalized(mut self) -> Self {
        while self.radians > PI {
            self.radians -= PI * 2.0;
        }
        while self.radians < -PI {
            self.radians += PI * 2.0;
        }
        self
    }
}

impl PercentBetween for Rotation {
    fn percent_between(&self, min: &Self, max: &Self) -> cushy::animation::ZeroToOne {
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
        Self {
            radians: self.radians + rhs.radians,
        }
    }
}

impl Sub for Rotation {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            radians: self.radians - rhs.radians,
        }
    }
}

impl Neg for Rotation {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            radians: self.radians,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bone {
    Rigid {
        length: f32,
    },
    Jointed {
        start_length: f32,
        end_length: f32,
        inverse: bool,
    },
}

#[derive(Default, Debug)]
pub struct Skeleton {
    initial_joint: Option<JointId>,
    bones: Vec<SkeletonBone>,
    joints: Vec<SkeletonJoint>,
    connections: HashMap<BoneAxis, Vec<JointId>>,
    generation: usize,
}

impl Skeleton {
    pub fn push_bone(&mut self, bone: Bone, label: &'static str) -> BoneId {
        let id = BoneId(u8::try_from(self.bones.len()).expect("too many bones"));
        if id == BoneId(0) {
            let joint = self.push_joint(Rotation::default(), id.axis_a(), id.axis_a());
            self.initial_joint = Some(joint);
            self.connections.insert(id.axis_a(), vec![joint]);
        }
        self.bones.push(SkeletonBone {
            generation: self.generation,
            label,
            kind: bone,
            start: Point::default(),
            joint_pos: None,
            end: Point::default(),
            desired_end: None,
        });
        id
    }

    pub fn push_joint(&mut self, angle: Rotation, bone_a: BoneAxis, bone_b: BoneAxis) -> JointId {
        let id = JointId(u8::try_from(self.joints.len()).expect("too many joints"));
        self.joints.push(SkeletonJoint {
            bone_a,
            bone_b,
            angle,
            calculated_position: Point::default(),
        });
        self.connections.entry(bone_a).or_default().push(id);
        if bone_a != bone_b {
            self.connections.entry(bone_b).or_default().push(id);
        }
        id
    }

    pub fn set_translation(&mut self, translation: Point<f32>) {
        let bone = self.bones.first_mut().expect("root bone must be defined");
        bone.start = translation;
    }

    pub fn translation(&self) -> Point<f32> {
        self.bones.first().expect("root bone must be defined").start
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        let joint = self.initial_joint.expect("root bone must be defined");
        let joint = &mut self[joint];
        joint.angle = rotation;
    }

    pub fn rotation(&self) -> Rotation {
        let joint = self.initial_joint.expect("root bone must be defined");
        self[joint].angle
    }

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

                let mut next_rotation = (current_rotation + angle).normalized();
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
                    next_rotation = (rotation + Rotation::radians(PI / 2.)).normalized();
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

fn next_point(mut point: Point<f32>, angle: Rotation, length: f32) -> Point<f32> {
    point.x += length * angle.radians.sin();
    point.y -= length * angle.radians.cos();
    point
}

fn determine_end_position(
    start: Point<f32>,
    desired_end: Option<Point<f32>>,
    angle: Rotation,
    bone: &Bone,
) -> (Point<f32>, Option<Point<f32>>) {
    match bone {
        Bone::Rigid { length } => (next_point(start, angle, *length), None),
        Bone::Jointed {
            start_length,
            end_length,
            inverse,
        } => {
            if let Some(desired_end) = desired_end {
                let delta = desired_end - start;
                let full_length = start_length + end_length;
                let distance = delta.magnitude();
                let minimum_size = (start_length - end_length).abs();
                let desired_length = if distance < minimum_size {
                    minimum_size
                } else if distance > full_length {
                    full_length
                } else {
                    distance
                };

                let desired_angle = Rotation::radians(delta.y.atan2(delta.x) + PI / 2.);
                let end = if desired_length != distance {
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
    start: Point<f32>,
    distance: f32,
    hyp_angle: Rotation,
    first: f32,
    second: f32,
) -> Point<f32> {
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
    type Output = SkeletonBone;

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
    type Output = SkeletonJoint;

    fn index(&self, index: JointId) -> &Self::Output {
        &self.joints[usize::from(index.0)]
    }
}

impl IndexMut<JointId> for Skeleton {
    fn index_mut(&mut self, index: JointId) -> &mut Self::Output {
        &mut self.joints[usize::from(index.0)]
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct BoneAxis {
    pub bone: BoneId,
    pub end: BoneEnd,
}

impl BoneAxis {
    pub fn inverse(self) -> Self {
        Self {
            bone: self.bone,
            end: self.end.inverse(),
        }
    }
}

#[derive(Debug)]
pub struct SkeletonBone {
    generation: usize,
    label: &'static str,
    kind: Bone,
    start: Point<f32>,
    joint_pos: Option<Point<f32>>,
    end: Point<f32>,
    desired_end: Option<Point<f32>>,
}

impl SkeletonBone {
    pub fn set_desired_end(&mut self, end: Option<Point<f32>>) {
        self.desired_end = end;
    }
}

#[derive(Debug)]
pub struct SkeletonJoint {
    bone_a: BoneAxis,
    bone_b: BoneAxis,
    calculated_position: Point<f32>,
    angle: Rotation,
}

impl SkeletonJoint {
    pub fn other_axis(&self, axis: BoneAxis) -> BoneAxis {
        if self.bone_a == axis {
            self.bone_b
        } else {
            debug_assert_eq!(self.bone_b, axis);
            self.bone_a
        }
    }

    pub fn set_angle(&mut self, angle: Rotation) {
        self.angle = angle;
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoneId(u8);

impl BoneId {
    pub const fn axis_a(self) -> BoneAxis {
        BoneAxis {
            bone: self,
            end: BoneEnd::A,
        }
    }

    pub const fn axis_b(self) -> BoneAxis {
        BoneAxis {
            bone: self,
            end: BoneEnd::B,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct JointId(u8);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BoneEnd {
    A,
    B,
}

impl BoneEnd {
    pub fn inverse(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

fn main() {
    let mut skeleton = Skeleton::default();
    let spine = skeleton.push_bone(Bone::Rigid { length: 3. }, "spine");
    let r_hip = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "r_hip");
    let r_leg = skeleton.push_bone(
        Bone::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: true,
        },
        "r_leg",
    );
    let r_foot = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "r_foot");
    let l_hip = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "l_hip");
    let l_leg = skeleton.push_bone(
        Bone::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: false,
        },
        "l_leg",
    );
    let l_foot = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "l_foot");
    let r_shoulder = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "r_shoulder");
    let r_arm = skeleton.push_bone(
        Bone::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: true,
        },
        "r_arm",
    );
    let r_hand = skeleton.push_bone(Bone::Rigid { length: 0.3 }, "r_hand");
    let l_shoulder = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "l_shoulder");
    let l_arm = skeleton.push_bone(
        Bone::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: false,
        },
        "l_arm",
    );
    let l_hand = skeleton.push_bone(Bone::Rigid { length: 0.3 }, "l_hand");
    let head = skeleton.push_bone(Bone::Rigid { length: 0.5 }, "head");

    let neck = skeleton.push_joint(Rotation::degrees(180.), spine.axis_b(), head.axis_a());
    // Create some width for the legs to be spaced.
    skeleton.push_joint(Rotation::degrees(90.), spine.axis_a(), l_hip.axis_a());
    skeleton.push_joint(Rotation::degrees(-90.), spine.axis_a(), r_hip.axis_a());
    skeleton.push_joint(Rotation::degrees(90.), l_hip.axis_b(), l_leg.axis_a());
    let l_ankle_id = skeleton.push_joint(Rotation::degrees(-90.), l_leg.axis_b(), l_foot.axis_a());
    skeleton.push_joint(Rotation::degrees(-90.), r_hip.axis_b(), r_leg.axis_a());
    let r_ankle_id = skeleton.push_joint(Rotation::degrees(90.), r_leg.axis_b(), r_foot.axis_a());

    skeleton.push_joint(Rotation::degrees(90.), spine.axis_b(), l_shoulder.axis_a());
    skeleton.push_joint(Rotation::degrees(-90.), spine.axis_b(), r_shoulder.axis_a());
    let l_arm_socket =
        skeleton.push_joint(Rotation::degrees(90.), l_shoulder.axis_b(), l_arm.axis_a());
    let l_wrist_id = skeleton.push_joint(Rotation::degrees(-175.), l_arm.axis_b(), l_hand.axis_a());
    let r_arm_socket =
        skeleton.push_joint(Rotation::degrees(-90.), r_shoulder.axis_b(), r_arm.axis_a());
    let r_wrist_id = skeleton.push_joint(Rotation::degrees(175.), r_arm.axis_b(), r_hand.axis_a());

    let skeleton = Dynamic::new(skeleton);

    Canvas::new({
        let skeleton = skeleton.clone();
        move |context| {
            let mut s = skeleton.lock();
            s.prevent_notifications();
            s.solve();

            let center = Point::from(context.gfx.size().into_signed()) / 2;

            let scale = Px::new(50);
            for (bone, color) in [
                (spine, Color::RED),
                (r_hip, Color::DARKBLUE),
                (r_leg, Color::BLUE),
                (r_foot, Color::LIGHTBLUE),
                (l_hip, Color::DARKGREEN),
                (l_leg, Color::GREEN),
                (l_foot, Color::LIGHTGREEN),
                (r_shoulder, Color::DARKGOLDENROD),
                (r_arm, Color::GOLDENROD),
                (r_hand, Color::LIGHTGOLDENRODYELLOW),
                (l_shoulder, Color::DARKMAGENTA),
                (l_arm, Color::MAGENTA),
                (l_hand, Color::LIGHTPINK),
                (head, Color::YELLOW),
            ] {
                let start = s[bone].start.map(|d| scale * d);
                let end = s[bone].end.map(|d| scale * d);
                if let Some(joint) = s[bone].joint_pos {
                    let joint = joint.map(|d| scale * d);
                    context.gfx.draw_shape(
                        PathBuilder::new(start)
                            .line_to(joint)
                            .build()
                            .stroke(StrokeOptions::px_wide(1).colored(color))
                            .translate_by(center),
                    );
                    context.gfx.draw_shape(
                        PathBuilder::new(joint)
                            .line_to(end)
                            .build()
                            .stroke(StrokeOptions::px_wide(1).colored(color))
                            .translate_by(center),
                    );
                } else {
                    context.gfx.draw_shape(
                        PathBuilder::new(start)
                            .line_to(end)
                            .build()
                            .stroke(StrokeOptions::px_wide(1).colored(color))
                            .translate_by(center),
                    );
                }

                if let Some(handle) = s[bone].desired_end {
                    let handle = handle.map(|d| scale * d);
                    context.gfx.draw_shape(
                        Shape::filled_circle(Px::new(3), Color::WHITE, Origin::Center)
                            .translate_by(handle + center),
                    );
                }
            }

            drop(s);

            context.redraw_when_changed(&skeleton);
        }
    })
    .expand()
    .and(
        bone_widget("Lower Left Leg", &skeleton, l_leg, 0.5..=3.0, 0.5..=3.0)
            .and(joint_widget("Left Ankle", &skeleton, l_ankle_id))
            .and(bone_widget(
                "Lower Right Leg",
                &skeleton,
                r_leg,
                -3.0..=-0.5,
                0.5..=3.0,
            ))
            .and(joint_widget("Right Ankle", &skeleton, r_ankle_id))
            .and(joint_widget("Left Shoulder", &skeleton, l_arm_socket))
            .and(joint_widget("Left Wrist", &skeleton, l_wrist_id))
            .and(joint_widget("Right Shoulder", &skeleton, r_arm_socket))
            .and(joint_widget("Right Wrist", &skeleton, r_wrist_id))
            .and(joint_widget("Neck", &skeleton, neck))
            .into_rows()
            .pad()
            .width(Lp::inches(2)),
    )
    .into_columns()
    .expand()
    .run()
    .unwrap();
}

fn joint_widget(label: &str, skeleton: &Dynamic<Skeleton>, joint: JointId) -> impl MakeWidget {
    let angle = Dynamic::new(skeleton.read()[joint].angle);
    angle
        .for_each({
            let skeleton = skeleton.clone();
            move |degrees| {
                skeleton.lock()[joint].set_angle(*degrees);
            }
        })
        .persist();
    let angle_slider = angle.slider_between(Rotation::degrees(-180.), Rotation::degrees(180.));

    label.and(angle_slider).into_rows().contain()
}

fn bone_widget(
    label: &str,
    skeleton: &Dynamic<Skeleton>,
    bone: BoneId,
    x: RangeInclusive<f32>,
    y: RangeInclusive<f32>,
) -> impl MakeWidget {
    let bone_y = Dynamic::new(skeleton.lock()[bone].desired_end.unwrap_or_default().y);
    let bone_x = Dynamic::new(skeleton.lock()[bone].desired_end.unwrap_or_default().x);

    bone_y
        .for_each({
            let skeleton = skeleton.clone();
            move |y| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end.unwrap_or_default();
                skeleton[bone].set_desired_end(Some(Point::new(current_end.x, *y)));
            }
        })
        .persist();
    bone_x
        .for_each({
            let skeleton = skeleton.clone();
            move |x| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end.unwrap_or_default();
                skeleton[bone].set_desired_end(Some(Point::new(*x, current_end.y)));
            }
        })
        .persist();
    label
        .and(bone_x.slider_between(*x.start(), *x.end()))
        .and(bone_y.slider_between(*y.start(), *y.end()))
        .into_rows()
        .contain()
}

#[test]
fn rotation() {
    assert_eq!(
        (Rotation::degrees(90.) + Rotation::degrees(180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        -90,
    );
    assert_eq!(
        (Rotation::degrees(90.) + Rotation::degrees(-180.))
            .normalized()
            .to_degrees()
            .round() as i32,
        -90,
    );
    // assert_eq!(
    //     (Rotation::degrees(90.) + Rotation::degrees(-180.))
    //         .normalized()
    //         .to_degrees(),
    //     -90.
    // );
}
