//! A basic 2d humanoid skeleton with sliders powered by Cushy.
use core::f32;
use std::ops::RangeInclusive;

use cushy::{
    figures::{
        units::{Lp, Px},
        IntoComponents, IntoSigned, Point,
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
use funnybones::{BoneId, BoneKind, JointId, Rotation, Skeleton, Vector};

fn main() {
    // begin rustme snippet: readme
    let mut skeleton = Skeleton::default();
    let spine = skeleton.push_bone(BoneKind::Rigid { length: 3. }, "spine");
    let r_hip = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "r_hip");
    let r_leg = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: true,
        },
        "r_leg",
    );
    let r_foot = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "r_foot");
    let l_hip = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "l_hip");
    let l_leg = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: false,
        },
        "l_leg",
    );
    let l_foot = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "l_foot");
    let r_shoulder = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "r_shoulder");
    let r_arm = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: true,
        },
        "r_arm",
    );
    let r_hand = skeleton.push_bone(BoneKind::Rigid { length: 0.3 }, "r_hand");
    let l_shoulder = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "l_shoulder");
    let l_arm = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: false,
        },
        "l_arm",
    );
    let l_hand = skeleton.push_bone(BoneKind::Rigid { length: 0.3 }, "l_hand");
    let head = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }, "head");

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
    // end rustme snippet

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
                let start = s[bone].start().to_vec::<Point<f32>>().map(|d| scale * d);
                let end = s[bone].end().to_vec::<Point<f32>>().map(|d| scale * d);
                if let Some(joint) = s[bone].solved_joint() {
                    let joint = joint.to_vec::<Point<f32>>().map(|d| scale * d);
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

                if let Some(handle) = s[bone].desired_end() {
                    let handle = handle.to_vec::<Point<f32>>().map(|d| scale * d);
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
    let angle = Dynamic::new(skeleton.read()[joint].angle());
    angle
        .for_each({
            let skeleton = skeleton.clone();
            move |degrees| {
                skeleton.lock()[joint].set_angle(*degrees);
            }
        })
        .persist();
    let angle_slider = angle.slider_between(Rotation::degrees(0.), Rotation::degrees(359.9));

    label.and(angle_slider).into_rows().contain()
}

fn bone_widget(
    label: &str,
    skeleton: &Dynamic<Skeleton>,
    bone: BoneId,
    x: RangeInclusive<f32>,
    y: RangeInclusive<f32>,
) -> impl MakeWidget {
    let bone_y = Dynamic::new(skeleton.lock()[bone].desired_end().unwrap_or_default().y);
    let bone_x = Dynamic::new(skeleton.lock()[bone].desired_end().unwrap_or_default().x);

    bone_y
        .for_each({
            let skeleton = skeleton.clone();
            move |y| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end().unwrap_or_default();
                skeleton[bone].set_desired_end(Some(Vector::new(current_end.x, *y)));
            }
        })
        .persist();
    bone_x
        .for_each({
            let skeleton = skeleton.clone();
            move |x| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end().unwrap_or_default();
                skeleton[bone].set_desired_end(Some(Vector::new(*x, current_end.y)));
            }
        })
        .persist();
    label
        .and(bone_x.slider_between(*x.start(), *x.end()))
        .and(bone_y.slider_between(*y.start(), *y.end()))
        .into_rows()
        .contain()
}
