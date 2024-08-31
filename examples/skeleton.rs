//! A basic 2d humanoid skeleton with sliders powered by Cushy.
#![allow(clippy::too_many_lines)]
use core::f32;

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
use funnybones::{Angle, BoneId, BoneKind, Joint, JointId, Skeleton, Vector};

fn main() {
    // begin rustme snippet: readme
    let mut skeleton = Skeleton::default();

    // Create our root bone: the spine
    let spine = skeleton.push_bone(BoneKind::Rigid { length: 3. }.with_label("spine"));
    // Create the right-half of the hips.
    let r_hip = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("r_hip"));
    // Connect the right hip to the spine.
    skeleton.push_joint(Joint::new(
        Angle::degrees(-90.),
        spine.axis_a(),
        r_hip.axis_a(),
    ));
    // Create the right leg as a jointed bone with equal sizes for the upper and
    // lower leg.
    let r_leg = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: true,
        }
        .with_label("r_leg"),
    );

    // Connect the right leg to the right hip.
    skeleton.push_joint(Joint::new(
        Angle::degrees(0.),
        r_hip.axis_b(),
        r_leg.axis_a(),
    ));
    // Create the right foot.
    let r_foot = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("r_foot"));
    // Connect the right foot to the right leg.
    let r_ankle_id = skeleton.push_joint(Joint::new(
        Angle::degrees(90.),
        r_leg.axis_b(),
        r_foot.axis_a(),
    ));
    // end rustme snippet

    // Create the left-half of our lower half.
    let l_hip = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("l_hip"));
    skeleton.push_joint(Joint::new(
        Angle::degrees(90.),
        spine.axis_a(),
        l_hip.axis_a(),
    ));
    let l_leg = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.5,
            end_length: 1.5,
            inverse: false,
        }
        .with_label("l_leg"),
    );
    skeleton.push_joint(Joint::new(
        Angle::degrees(90.),
        l_hip.axis_b(),
        l_leg.axis_a(),
    ));
    let l_foot = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("l_foot"));
    let l_ankle_id = skeleton.push_joint(Joint::new(
        Angle::degrees(-90.),
        l_leg.axis_b(),
        l_foot.axis_a(),
    ));

    // Create our two arms in the same fashion as our leg structure.
    let r_shoulder = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("r_shoulder"));
    skeleton.push_joint(Joint::new(
        Angle::degrees(-90.),
        spine.axis_b(),
        r_shoulder.axis_a(),
    ));
    let r_arm = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: true,
        }
        .with_label("r_arm"),
    );
    skeleton.push_joint(Joint::new(
        Angle::degrees(-90.),
        r_shoulder.axis_b(),
        r_arm.axis_a(),
    ));
    let r_hand = skeleton.push_bone(BoneKind::Rigid { length: 0.3 }.with_label("r_hand"));
    let r_wrist_id = skeleton.push_joint(Joint::new(
        Angle::degrees(175.),
        r_arm.axis_b(),
        r_hand.axis_a(),
    ));

    let l_shoulder = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("l_shoulder"));
    skeleton.push_joint(Joint::new(
        Angle::degrees(90.),
        spine.axis_b(),
        l_shoulder.axis_a(),
    ));
    let l_arm = skeleton.push_bone(
        BoneKind::Jointed {
            start_length: 1.0,
            end_length: 1.0,
            inverse: false,
        }
        .with_label("l_arm"),
    );
    skeleton.push_joint(Joint::new(
        Angle::degrees(90.),
        l_shoulder.axis_b(),
        l_arm.axis_a(),
    ));
    let l_hand = skeleton.push_bone(BoneKind::Rigid { length: 0.3 }.with_label("l_hand"));
    let l_wrist_id = skeleton.push_joint(Joint::new(
        Angle::degrees(-175.),
        l_arm.axis_b(),
        l_hand.axis_a(),
    ));

    // Finally, create a bone to represent our head.
    let head = skeleton.push_bone(BoneKind::Rigid { length: 0.5 }.with_label("head"));
    let neck = skeleton.push_joint(Joint::new(
        Angle::degrees(180.),
        spine.axis_b(),
        head.axis_a(),
    ));

    let skeleton = Dynamic::new(skeleton);

    let rotation = Dynamic::new(Angle::degrees(-90.));
    rotation
        .for_each_cloned({
            let skeleton = skeleton.clone();
            move |rotation| {
                skeleton.lock().set_rotation(rotation);
            }
        })
        .persist();

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
                    let handle = s[bone].start() + (handle + s[bone].entry_angle());
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
        "Overall Rotation"
            .and(rotation.slider())
            .into_rows()
            .contain()
            .and(bone_widget(
                "Left Leg",
                Angle::degrees(90.),
                &skeleton,
                l_leg,
            ))
            .and(joint_widget("Left Ankle", &skeleton, l_ankle_id))
            .and(bone_widget(
                "Right Leg",
                Angle::degrees(-90.),
                &skeleton,
                r_leg,
            ))
            .and(joint_widget("Right Ankle", &skeleton, r_ankle_id))
            .and(bone_widget(
                "Left Arm",
                Angle::degrees(0.),
                &skeleton,
                l_arm,
            ))
            .and(joint_widget("Left Wrist", &skeleton, l_wrist_id))
            .and(bone_widget(
                "Right Arm",
                Angle::degrees(0.),
                &skeleton,
                r_arm,
            ))
            .and(joint_widget("Right Wrist", &skeleton, r_wrist_id))
            .and(joint_widget("Neck", &skeleton, neck))
            .into_rows()
            .pad()
            .width(Lp::inches(2))
            .vertical_scroll(),
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
    let angle_slider = angle.slider_between(Angle::degrees(0.), Angle::degrees(359.9));

    label.and(angle_slider).into_rows().contain()
}

fn bone_widget(
    label: &str,
    initial_angle: Angle,
    skeleton: &Dynamic<Skeleton>,
    bone: BoneId,
) -> impl MakeWidget {
    let bone_magnitude = Dynamic::new(skeleton.lock()[bone].kind().full_length());
    let bone_direction = Dynamic::new(initial_angle);

    let length = skeleton.lock()[bone].kind().full_length();

    bone_direction
        .for_each({
            let skeleton = skeleton.clone();
            move |direction| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end().unwrap_or_default();
                skeleton[bone]
                    .set_desired_end(Some(Vector::new(current_end.magnitude, *direction)));
            }
        })
        .persist();
    bone_magnitude
        .for_each({
            let skeleton = skeleton.clone();
            move |magnitude| {
                let mut skeleton = skeleton.lock();
                let current_end = skeleton[bone].desired_end().unwrap_or_default();
                skeleton[bone]
                    .set_desired_end(Some(Vector::new(*magnitude, current_end.direction)));
            }
        })
        .persist();
    label
        .and(bone_magnitude.slider_between(0., length))
        .and(bone_direction.slider())
        .into_rows()
        .contain()
}
