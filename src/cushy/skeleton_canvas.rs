#![allow(missing_docs)]
use core::f32;

use crate::{BoneEnd, BoneId, JointId, Rotation, Skeleton, Vector};
use cushy::{
    context::{EventContext, GraphicsContext, LayoutContext},
    figures::{
        units::{Px, UPx},
        FloatConversion, IntoComponents, Point, Round, Size,
    },
    kludgine::{
        app::winit::{event::MouseButton, window::CursorIcon},
        shapes::{PathBuilder, Shape, StrokeOptions},
        DrawableExt, Origin,
    },
    styles::Color,
    value::{Dynamic, DynamicRead},
    widget::{Callback, EventHandling, Widget, HANDLED, IGNORED},
    window::DeviceId,
    ConstraintLimit,
};

#[derive(Debug)]
pub struct SkeletonCanvas {
    skeleton: Dynamic<Skeleton>,
    hovering: Option<Target>,
    scale: f32,
    maximum_scale: f32,
    offset: Point<Px>,
    drag: Option<DragInfo>,
    on_mutate: Option<Callback<SkeletonMutation>>,
}

impl SkeletonCanvas {
    #[must_use]
    pub fn new(skeleton: Dynamic<Skeleton>) -> Self {
        Self {
            skeleton,
            hovering: None,
            scale: f32::MAX,
            maximum_scale: 0.,
            offset: Point::default(),
            drag: None,
            on_mutate: None,
        }
    }

    #[must_use]
    pub fn on_mutate<F>(mut self, on_mutate: F) -> Self
    where
        F: FnMut(SkeletonMutation) + Send + 'static,
    {
        self.on_mutate = Some(Callback::new(on_mutate));
        self
    }

    fn vector_position(&self, vector: Vector) -> Point<Px> {
        (vector * self.scale).to_vec::<Point<f32>>().map(Px::from) + self.offset
    }

    fn position_to_vector(&self, position: Point<Px>) -> Vector {
        (position - self.offset)
            .map(FloatConversion::into_float)
            .to_vec::<Vector>()
            / self.scale
    }
}

impl Widget for SkeletonCanvas {
    fn redraw(&mut self, context: &mut GraphicsContext<'_, '_, '_, '_>) {
        context.redraw_when_changed(&self.skeleton);
        let mut skeleton = self.skeleton.lock();
        skeleton.prevent_notifications();
        if skeleton.bones().is_empty() {
            return;
        }
        skeleton.solve();
        let root_start = skeleton.bones()[0].start();
        let (min, max) = skeleton.bones().iter().fold(
            (Vector::new(f32::MAX, f32::MAX), Vector::default()),
            |(min, max), bone| {
                let start = bone.start() - root_start;
                let end = bone.end() - root_start;
                (
                    Vector::new(min.x.min(start.x).min(end.x), min.y.min(start.y).min(end.y)),
                    Vector::new(max.x.max(start.x).max(end.x), max.y.max(start.y).max(end.y)),
                )
            },
        );

        let skeleton_extent =
            Vector::new(min.x.abs().max(max.x.abs()), min.y.abs().max(max.y.abs()));

        let middle = context.gfx.size().into_float().to_vec::<Vector>() / 2.;
        let height_ratio = middle.y / skeleton_extent.y;
        let width_ratio = middle.x / skeleton_extent.x;
        let zero_width = width_ratio.is_nan();
        let zero_height = height_ratio.is_nan();
        if zero_height && zero_width {
            return;
        }

        self.maximum_scale = if zero_height || width_ratio < height_ratio {
            width_ratio
        } else {
            height_ratio
        };
        if self.scale > self.maximum_scale {
            self.scale = self.maximum_scale;
        }
        let root = root_start * self.scale;

        self.offset = (middle - root).to_vec::<Point<f32>>().map(Px::from).floor();

        let selected = self.drag.as_ref().map_or(self.hovering, |d| Some(d.target));

        for bone in skeleton.bones() {
            let path = if let Some(joint) = bone.solved_joint() {
                let joint = self.vector_position(joint);
                context.gfx.draw_shape(
                    Shape::filled_circle(Px::new(4), Color::WHITE, Origin::Center)
                        .translate_by(joint),
                );
                PathBuilder::new(self.vector_position(bone.start()))
                    .line_to(joint)
                    .line_to(self.vector_position(bone.end()))
                    .build()
            } else {
                PathBuilder::new(self.vector_position(bone.start()))
                    .line_to(self.vector_position(bone.end()))
                    .build()
            };
            let (selected, stroke) = match selected {
                Some(Target::DesiredEnd(id)) if id == bone.id() => {
                    (true, StrokeOptions::px_wide(2).colored(Color::RED))
                }
                Some(Target::Joint(joint)) if skeleton[joint].bone_b == bone.id().axis_a() => {
                    (true, StrokeOptions::px_wide(2).colored(Color::RED))
                }
                Some(Target::Joint(joint)) if skeleton[joint].bone_a.bone == bone.id() => {
                    (false, StrokeOptions::px_wide(2).colored(Color::BLUE))
                }
                _ => (false, StrokeOptions::px_wide(1).colored(Color::WHITE)),
            };
            context.gfx.draw_shape(&path.stroke(stroke));

            if selected {
                let end = bone.desired_end().unwrap_or_else(|| bone.end());

                let end = self.vector_position(end);
                context.gfx.draw_shape(
                    Shape::filled_circle(Px::new(10), stroke.color, Origin::Center)
                        .translate_by(end),
                );
            }
        }
    }

    fn layout(
        &mut self,
        available_space: Size<ConstraintLimit>,
        _context: &mut LayoutContext<'_, '_, '_, '_>,
    ) -> Size<UPx> {
        available_space.map(ConstraintLimit::max)
    }

    fn hover(&mut self, location: Point<Px>, context: &mut EventContext<'_>) -> Option<CursorIcon> {
        let location = self.position_to_vector(location);
        let skeleton = self.skeleton.read();
        let mut closest_match = 0.1;
        let current_hover = self.hovering.take();
        for bone in skeleton.bones() {
            if let Some(mid) = bone.solved_joint() {
                // This can have its desired_end set
                let mut distance = distance_to_line(location, bone.start(), mid)
                    .min(distance_to_line(location, mid, bone.end()));
                if let Some(desired_end) = bone.desired_end() {
                    distance = distance.min((location - desired_end).magnitude());
                }

                if distance < closest_match {
                    closest_match = distance;
                    self.hovering = Some(Target::DesiredEnd(bone.id()));
                }
            } else if !bone.is_root() {
                // Single line segment
                let distance = distance_to_line(location, bone.start(), bone.end());
                if distance < closest_match {
                    closest_match = distance;
                    // For a non-jointed bone, interacting with it adjusts the
                    // joint angle.
                    if let Some(joint) = skeleton
                        .connections_to(bone.id().axis_a())
                        .and_then(|joints| joints.first())
                    {
                        self.hovering = Some(Target::Joint(*joint));
                    }
                }
            }
        }

        if self.hovering != current_hover {
            context.set_needs_redraw();
        }

        None
    }

    fn unhover(&mut self, context: &mut EventContext<'_>) {
        if self.hovering.take().is_some() {
            context.set_needs_redraw();
        }
    }

    fn mouse_down(
        &mut self,
        location: Point<Px>,
        _device_id: DeviceId,
        _button: MouseButton,
        _context: &mut EventContext<'_>,
    ) -> EventHandling {
        if let Some(target) = self.hovering {
            self.drag = Some(DragInfo {
                target,
                last: self.position_to_vector(location),
            });
            HANDLED
        } else {
            IGNORED
        }
    }

    fn mouse_drag(
        &mut self,
        location: Point<Px>,
        _device_id: DeviceId,
        _button: MouseButton,
        _context: &mut EventContext<'_>,
    ) {
        let location = self.position_to_vector(location);
        if let Some(drag) = &mut self.drag {
            let delta = location - drag.last;
            if delta.magnitude() > f32::EPSILON {
                drag.last = location;
                match drag.target {
                    Target::Joint(joint_id) => {
                        // Calculate the angle needed to make the second bone's
                        // line pass through location.
                        let mut skeleton = self.skeleton.lock();
                        if skeleton.generation == 0 {
                            skeleton.solve();
                        }
                        let joint = &skeleton[joint_id];
                        let bone_a = &skeleton[joint.bone_a.bone];
                        let start = bone_a.solved_joint().unwrap_or_else(|| bone_a.start());
                        let (start, end, inverse) = match joint.bone_a.end {
                            BoneEnd::A => (bone_a.end(), start, false),
                            BoneEnd::B => (start, bone_a.end(), true),
                        };
                        let new_bone_rotation = (location - end).as_rotation();
                        let bone_a_rotation = (start - end).as_rotation();
                        let mut rotation = new_bone_rotation - bone_a_rotation;
                        if inverse {
                            rotation = -rotation;
                        };
                        drop(skeleton);

                        if let Some(on_mutate) = &mut self.on_mutate {
                            on_mutate.invoke(SkeletonMutation::SetJointRotation {
                                joint: joint_id,
                                rotation,
                            });
                        }
                    }
                    Target::DesiredEnd(bone) => {
                        if let Some(on_mutate) = &mut self.on_mutate {
                            on_mutate.invoke(SkeletonMutation::SetDesiredEnd {
                                bone,
                                end: location,
                            });
                        }
                    }
                }
            }
        }
    }

    fn mouse_up(
        &mut self,
        _location: Option<Point<Px>>,
        _device_id: DeviceId,
        _button: MouseButton,
        _context: &mut EventContext<'_>,
    ) {
        self.drag = None;
    }

    fn hit_test(&mut self, _location: Point<Px>, _context: &mut EventContext<'_>) -> bool {
        true
    }
}

fn distance_to_line(test: Vector, p1: Vector, p2: Vector) -> f32 {
    let delta = p2 - p1;
    let segment_length = delta.magnitude();

    let p1_distance = (test - p1).magnitude();
    let p2_distance = (test - p2).magnitude();

    match (p1_distance >= segment_length, p2_distance >= segment_length) {
        (true, true) => p1_distance.min(p2_distance),
        (true, false) => p2_distance,
        (false, true) => p1_distance,
        _ => {
            let numerator = delta.y * test.x - delta.x * test.y + p2.x * p1.y - p2.y * p1.x;

            numerator.abs() / segment_length
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Target {
    Joint(JointId),
    DesiredEnd(BoneId),
}

#[derive(Debug)]
struct DragInfo {
    target: Target,
    last: Vector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkeletonMutation {
    SetDesiredEnd { bone: BoneId, end: Vector },
    SetJointRotation { joint: JointId, rotation: Rotation },
}
