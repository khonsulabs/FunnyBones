#![allow(missing_docs)]
use core::f32;

use crate::{BoneId, JointId, Skeleton, Vector};
use cushy::{
    context::{GraphicsContext, LayoutContext},
    figures::{
        units::{Px, UPx},
        FloatConversion, IntoComponents, Point, Round, Size,
    },
    kludgine::shapes::{PathBuilder, StrokeOptions},
    styles::Color,
    value::Dynamic,
    widget::Widget,
    ConstraintLimit,
};

#[derive(Debug)]
pub struct SkeletonCanvas {
    skeleton: Dynamic<Skeleton>,
    hovering: Option<Target>,
}

impl SkeletonCanvas {
    #[must_use]
    pub fn new(skeleton: Dynamic<Skeleton>) -> Self {
        Self {
            skeleton,
            hovering: None,
        }
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

        let scale = if zero_height || width_ratio < height_ratio {
            width_ratio
        } else {
            height_ratio
        };
        let root = root_start * scale;

        let offset = (middle - root).to_vec::<Point<f32>>().map(Px::from).floor();

        let vector_position = |v: Vector| v.to_vec::<Point<f32>>().map(Px::from) * scale + offset;

        for bone in skeleton.bones() {
            let path = if let Some(joint) = bone.solved_joint() {
                PathBuilder::new(vector_position(bone.start()))
                    .line_to(vector_position(joint))
                    .line_to(vector_position(bone.end()))
                    .build()
            } else {
                PathBuilder::new(vector_position(bone.start()))
                    .line_to(vector_position(bone.end()))
                    .build()
            };
            context
                .gfx
                .draw_shape(&path.stroke(StrokeOptions::px_wide(1).colored(Color::WHITE)));
        }
    }

    fn layout(
        &mut self,
        available_space: Size<ConstraintLimit>,
        _context: &mut LayoutContext<'_, '_, '_, '_>,
    ) -> Size<UPx> {
        available_space.map(ConstraintLimit::max)
    }
}

#[derive(Debug)]
pub enum Target {
    Bone(BoneId),
    Joint(JointId),
    DesiredEnd(BoneId),
}
