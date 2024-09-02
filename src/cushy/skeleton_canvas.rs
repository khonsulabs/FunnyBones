#![allow(missing_docs)]
use core::f32;

use crate::{Rotation, BoneEnd, BoneId, Coordinate, JointId, Skeleton, Vector};
use cushy::{
    context::{EventContext, GraphicsContext, LayoutContext, Trackable},
    figures::{
        units::{Lp, Px, UPx},
        FloatConversion, IntoComponents, Point, Rect, Round, ScreenScale, Size,
    },
    kludgine::{
        app::winit::{
            event::{MouseButton, MouseScrollDelta, TouchPhase},
            window::CursorIcon,
        },
        shapes::{PathBuilder, Shape, StrokeOptions},
        DrawableExt, Origin,
    },
    styles::Color,
    value::{Destination, Dynamic, DynamicRead, Source},
    widget::{Callback, EventHandling, Widget, HANDLED, IGNORED},
    window::DeviceId,
    ConstraintLimit,
};

#[derive(Debug)]
pub struct SkeletonCanvas {
    skeleton: Dynamic<Skeleton>,
    hovering: Option<Target>,
    scale: Dynamic<f32>,
    handle_size: f32,
    maximum_scale: Dynamic<f32>,
    minimum_scale: Dynamic<f32>,
    offset: Point<Px>,
    drag: Option<DragInfo>,
    on_mutate: Option<Callback<SkeletonMutation>>,
}

impl SkeletonCanvas {
    #[must_use]
    pub fn new(skeleton: Dynamic<Skeleton>) -> Self {
        let maximum_scale = Dynamic::new(0.);
        let minimum_scale = maximum_scale.map_each_cloned(|s| s / 100.);
        Self {
            skeleton,
            hovering: None,
            handle_size: 0.1,
            scale: Dynamic::new(f32::MAX),
            maximum_scale,
            minimum_scale,
            offset: Point::default(),
            drag: None,
            on_mutate: None,
        }
    }

    #[must_use]
    pub fn maximum_scale(&self) -> &Dynamic<f32> {
        &self.maximum_scale
    }

    #[must_use]
    pub fn minimum_scale(&self) -> &Dynamic<f32> {
        &self.minimum_scale
    }

    #[must_use]
    pub fn scale(&self) -> &Dynamic<f32> {
        &self.scale
    }

    #[must_use]
    pub fn on_mutate<F>(mut self, on_mutate: F) -> Self
    where
        F: FnMut(SkeletonMutation) + Send + 'static,
    {
        self.on_mutate = Some(Callback::new(on_mutate));
        self
    }

    fn coordinate_to_point(&self, vector: Coordinate) -> Point<Px> {
        (vector * self.scale.get())
            .to_vec::<Point<f32>>()
            .map(Px::from)
            + self.offset
    }

    fn point_to_coordinate(&self, position: Point<Px>) -> Coordinate {
        (position - self.offset)
            .map(FloatConversion::into_float)
            .to_vec::<Coordinate>()
            / self.scale.get()
    }
}

impl Widget for SkeletonCanvas {
    #[allow(clippy::too_many_lines)]
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
            (Coordinate::new(f32::MAX, f32::MAX), Coordinate::default()),
            |(min, max), bone| {
                let start = bone.start() - root_start;
                let end = bone.end() - root_start;
                (
                    Coordinate::new(min.x.min(start.x).min(end.x), min.y.min(start.y).min(end.y)),
                    Coordinate::new(max.x.max(start.x).max(end.x), max.y.max(start.y).max(end.y)),
                )
            },
        );

        let skeleton_extent =
            Coordinate::new(min.x.abs().max(max.x.abs()), min.y.abs().max(max.y.abs()));

        let middle = context.gfx.size().into_float().to_vec::<Coordinate>() / 2.;
        let height_ratio = middle.y / skeleton_extent.y;
        let width_ratio = middle.x / skeleton_extent.x;
        let zero_width = width_ratio.is_nan();
        let zero_height = height_ratio.is_nan();
        if zero_height && zero_width {
            return;
        }

        let maximum_scale = if zero_height || width_ratio < height_ratio {
            width_ratio
        } else {
            height_ratio
        };
        self.maximum_scale.set(maximum_scale);
        self.scale.redraw_when_changed(context);
        let scale = {
            let mut scale = self.scale.lock();
            if *scale > maximum_scale {
                *scale = maximum_scale;
                scale.prevent_notifications();
            }
            *scale
        };
        let handle_size = Lp::mm(2).into_px(context.gfx.scale()).ceil();
        self.handle_size = handle_size.into_float() / scale;

        let root = root_start * scale;

        self.offset = (middle - root).to_vec::<Point<f32>>().map(Px::from).floor();

        let selected = self.drag.as_ref().map_or(self.hovering, |d| Some(d.target));

        for bone in skeleton.bones() {
            let (selected, color) = match selected {
                Some(Target::DesiredEnd(id)) if id == bone.id() => (true, Color::RED),
                Some(Target::Joint(joint)) if skeleton[joint].bone_b == bone.id().axis_a() => {
                    (true, Color::RED)
                }
                Some(Target::Joint(joint)) if skeleton[joint].bone_a.bone == bone.id() => {
                    (false, Color::BLUE)
                }
                _ => (false, (Color::WHITE)),
            };
            let path = if let Some(joint) = bone.solved_joint() {
                let joint = self.coordinate_to_point(joint);
                context
                    .gfx
                    .draw_shape(centered_square(handle_size / 2, color).translate_by(joint));
                PathBuilder::new(self.coordinate_to_point(bone.start()))
                    .line_to(joint)
                    .line_to(self.coordinate_to_point(bone.end()))
                    .build()
            } else {
                PathBuilder::new(self.coordinate_to_point(bone.start()))
                    .line_to(self.coordinate_to_point(bone.end()))
                    .build()
            };
            if bone.is_root() {
                context.gfx.draw_shape(
                    centered_square(handle_size / 2, color)
                        .translate_by(self.coordinate_to_point(bone.start())),
                );
            }
            context.gfx.draw_shape(
                centered_square(handle_size / 2, color)
                    .translate_by(self.coordinate_to_point(bone.end())),
            );
            let width = 1 + i32::from(selected);
            let stroke = StrokeOptions::px_wide(width).colored(color);
            context.gfx.draw_shape(&path.stroke(stroke));

            if selected {
                let end = if let Some(desired_end) =
                    bone.solved_joint().and_then(|_| bone.desired_end())
                {
                    bone.start() + (desired_end + bone.entry_angle())
                } else {
                    bone.end()
                };
                let end = self.coordinate_to_point(end);

                context.gfx.draw_shape(
                    Shape::filled_circle(handle_size, stroke.color, Origin::Center)
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
        let location = self.point_to_coordinate(location);
        let skeleton = self.skeleton.read();
        let mut closest_match = self.handle_size;
        let current_hover = self.hovering.take();
        for bone in skeleton.bones() {
            let mut distance = (location - bone.end()).magnitude();
            if let Some(mid) = bone.solved_joint() {
                // This can have its desired_end set
                distance = distance.min(
                    distance_to_line(location, bone.start(), mid)
                        .min(distance_to_line(location, mid, bone.end()))
                        .max(self.handle_size / 10.)
                        * 5.0,
                );
                if let Some(desired_end) = bone.desired_end() {
                    distance = distance.min(
                        (location - bone.start() - (desired_end + bone.entry_angle())).magnitude(),
                    );
                }

                if distance < closest_match {
                    closest_match = distance;
                    self.hovering = Some(Target::DesiredEnd(bone.id()));
                }
            } else if !bone.is_root() {
                // Single line segment
                distance = distance.min(
                    distance_to_line(location, bone.start(), bone.end())
                        .max(self.handle_size / 10.)
                        * 5.0,
                );
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
                last: self.point_to_coordinate(location),
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
        let location = self.point_to_coordinate(location);
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
                        let bone_b = &skeleton[joint.bone_b.bone];
                        let bone_a_rotation = bone_b.entry_angle();
                        let start = bone_a.solved_joint().unwrap_or_else(|| bone_a.start());
                        let end = match joint.bone_a.end {
                            BoneEnd::A => start,
                            BoneEnd::B => bone_a.end(),
                        };
                        let new_bone_rotation = (location - end).as_rotation();
                        let rotation = new_bone_rotation - bone_a_rotation;
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
                            let mut skeleton = self.skeleton.lock();
                            if skeleton.generation == 0 {
                                skeleton.solve();
                            }
                            let end = Vector::from(location - skeleton[bone].start())
                                - skeleton[bone].entry_angle();
                            drop(skeleton);
                            on_mutate.invoke(SkeletonMutation::SetDesiredEnd { bone, end });
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

    #[allow(clippy::cast_possible_truncation)]
    fn mouse_wheel(
        &mut self,
        _device_id: DeviceId,
        delta: MouseScrollDelta,
        _phase: TouchPhase,
        _context: &mut EventContext<'_>,
    ) -> EventHandling {
        let maximum_scale = self.maximum_scale.get();
        let minimum_scale = self.minimum_scale.get();
        let mut scale = self.scale.lock();
        let delta = match delta {
            MouseScrollDelta::LineDelta(_, y_lines) => y_lines,
            MouseScrollDelta::PixelDelta(pt) => pt.y as f32 / 12.,
        };

        *scale = (*scale + *scale * delta / 10.)
            .min(maximum_scale)
            .max(minimum_scale);

        HANDLED
    }

    fn hit_test(&mut self, _location: Point<Px>, _context: &mut EventContext<'_>) -> bool {
        true
    }
}

fn centered_square(size: Px, color: Color) -> Shape<Px, false> {
    Shape::filled_rect(
        Rect::new(Point::squared(-size / 2), Size::squared(size)),
        color,
    )
}

fn distance_to_line(test: Coordinate, p1: Coordinate, p2: Coordinate) -> f32 {
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
    last: Coordinate,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkeletonMutation {
    SetDesiredEnd { bone: BoneId, end: Vector },
    SetJointRotation { joint: JointId, rotation: Rotation },
}
