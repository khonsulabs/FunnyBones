//! The FunnyBones 2D Animation Editor.

use std::ops::ControlFlow;

use cushy::{
    animation::ZeroToOne,
    value::{Destination, Dynamic, DynamicRead, Source, Switchable},
    widget::{MakeWidget, WidgetList},
    widgets::{checkbox::Checkable, input::InputValue, slider::Slidable, Space},
    Run,
};
use funnybones::{
    cushy::skeleton_canvas::{SkeletonCanvas, SkeletonMutation},
    Angle, BoneAxis, BoneId, BoneKind, Joint, JointId, LabeledBoneKind, Skeleton, Vector,
};

#[derive(Default, Eq, PartialEq, Debug, Clone, Copy)]
enum Mode {
    #[default]
    Bones,
    Animation,
}

#[derive(Debug, Clone)]
struct ChangeAggregator(Dynamic<usize>);

impl ChangeAggregator {
    pub fn new<F, T>(mut when_changed: F) -> (Self, Dynamic<T>)
    where
        F: FnMut() -> T + Send + 'static,
        T: PartialEq + Send + 'static,
    {
        let counter = Dynamic::new(0);
        let result = counter.map_each(move |_| when_changed());

        (Self(counter), result)
    }

    pub fn watch<T>(&self, other: &Dynamic<T>)
    where
        T: Send + 'static,
    {
        let counter = self.0.clone();
        other
            .for_each_subsequent_generational(move |guard| {
                drop(guard);
                *counter.lock() += 1;
            })
            .persist();
    }
}

fn add_bones_to_skeleton(
    connected_to: BoneAxis,
    bones: &Dynamic<Vec<SkeletalBone>>,
    skeleton: &mut Skeleton,
) {
    let bones = bones.read();
    for bone in &*bones {
        let (kind, vector) = bone.as_bone_kind();
        let new_bone = skeleton.push_bone(kind);
        skeleton[new_bone].set_desired_end(Some(vector));
        skeleton.push_joint(
            Joint::new(bone.joint_angle.get(), connected_to, new_bone.axis_a())
                .with_label(bone.joint_label.get()),
        );
        add_bones_to_skeleton(new_bone.axis_b(), &bone.connected_bones, skeleton);
    }
}

fn main() -> cushy::Result {
    let editing_skeleton = EditingSkeleton::default();
    let (watcher, skeleton) = ChangeAggregator::new({
        let editing_skeleton = editing_skeleton.clone();
        move || {
            let mut skeleton = Skeleton::default();
            let (kind, _vector) = editing_skeleton.root.as_bone_kind();
            let root = skeleton.push_bone(kind);
            add_bones_to_skeleton(
                root.axis_b(),
                &editing_skeleton.root.connected_bones,
                &mut skeleton,
            );
            add_bones_to_skeleton(root.axis_a(), &editing_skeleton.a_bones, &mut skeleton);
            skeleton
        }
    });
    let bones_editor = skeleton_editor(&editing_skeleton, &watcher).make_widget();

    let mode = Dynamic::<Mode>::default();
    let canvas = SkeletonCanvas::new(skeleton).on_mutate({
        move |mutation| match mutation {
            SkeletonMutation::SetDesiredEnd { bone, end } => {
                let bone = editing_skeleton.find_bone(bone).expect("missing bone");
                bone.desired_length.set(end.magnitude);
                bone.joint_angle.set(end.direction);
            }
            SkeletonMutation::SetJointRotation { joint, rotation } => editing_skeleton
                .find_joint(joint)
                .expect("missing joint")
                .joint_angle
                .set(rotation),
        }
    });
    let zoom = canvas
        .scale()
        .clone()
        .slider_between(canvas.minimum_scale(), canvas.maximum_scale());

    [(Mode::Bones, "Bones"), (Mode::Animation, "Animation")]
        .into_iter()
        .map(|(selected, label)| mode.new_select(selected, label))
        .collect::<WidgetList>()
        .into_columns()
        .centered()
        .and(
            mode.switcher(move |mode, _mode_dynamic| match mode {
                Mode::Animation => "Animation Editor".make_widget(),
                Mode::Bones => bones_editor.clone(),
            })
            .expand(),
        )
        .into_rows()
        .expand()
        .and(canvas.expand().and(zoom).into_rows().expand())
        .into_columns()
        .run()
}

#[derive(Clone, Debug, PartialEq, Default)]
struct EditingSkeleton {
    root: SkeletalBone,
    a_bones: Dynamic<Vec<SkeletalBone>>,
}

impl EditingSkeleton {
    fn find_bone(&self, id: BoneId) -> Option<SkeletalBone> {
        let mut index = id.index();
        if index == 0 {
            Some(self.root.clone())
        } else {
            index -= 1;
            match Self::find_bone_in(&self.root.connected_bones, index) {
                ControlFlow::Continue(index) => match Self::find_bone_in(&self.a_bones, index) {
                    ControlFlow::Break(bone) => Some(bone),
                    ControlFlow::Continue(_) => None,
                },
                ControlFlow::Break(bone) => Some(bone),
            }
        }
    }

    fn find_bone_in(
        bones: &Dynamic<Vec<SkeletalBone>>,
        mut index: usize,
    ) -> ControlFlow<SkeletalBone, usize> {
        let bones = bones.read();
        for bone in &*bones {
            if index == 0 {
                return ControlFlow::Break(bone.clone());
            }

            index -= 1;
            index = Self::find_bone_in(&bone.connected_bones, index)?;
        }

        ControlFlow::Continue(index)
    }

    fn find_joint(&self, id: JointId) -> Option<SkeletalBone> {
        let index = id.index();
        match Self::find_joint_in(&self.root.connected_bones, index) {
            ControlFlow::Continue(index) => match Self::find_joint_in(&self.a_bones, index) {
                ControlFlow::Break(bone) => Some(bone),
                ControlFlow::Continue(_) => None,
            },
            ControlFlow::Break(bone) => Some(bone),
        }
    }

    fn find_joint_in(
        bones: &Dynamic<Vec<SkeletalBone>>,
        mut index: usize,
    ) -> ControlFlow<SkeletalBone, usize> {
        let bones = bones.read();
        for bone in &*bones {
            if index == 0 {
                return ControlFlow::Break(bone.clone());
            }

            index -= 1;
            index = Self::find_bone_in(&bone.connected_bones, index)?;
        }

        ControlFlow::Continue(index)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SkeletalBone {
    label: Dynamic<String>,
    joint_label: Dynamic<String>,
    joint_angle: Dynamic<Angle>,
    length: Dynamic<f32>,
    jointed: Dynamic<bool>,
    joint_ratio: Dynamic<ZeroToOne>,
    inverse: Dynamic<bool>,
    desired_length: Dynamic<f32>,
    connected_bones: Dynamic<Vec<SkeletalBone>>,
}

impl SkeletalBone {
    pub fn as_bone_kind(&self) -> (LabeledBoneKind, Vector) {
        let length = self.length.get();
        let (vector_length, kind) = if self.jointed.get() {
            let joint_ratio = self.joint_ratio.get();
            let start_length = length * *joint_ratio;
            let end_length = length - start_length;
            (
                self.desired_length.get(),
                BoneKind::Jointed {
                    start_length,
                    end_length,
                    inverse: self.inverse.get(),
                },
            )
        } else {
            (
                length,
                BoneKind::Rigid {
                    length: self.length.get(),
                },
            )
        };

        (
            kind.with_label(self.label.get()),
            Vector::new(vector_length, self.joint_angle.get()),
        )
    }
}

impl Default for SkeletalBone {
    fn default() -> Self {
        Self {
            joint_label: Dynamic::default(),
            joint_angle: Dynamic::new(Angle::degrees(90.)),
            label: Dynamic::default(),
            length: Dynamic::new(1.),
            jointed: Dynamic::default(),
            joint_ratio: Dynamic::new(ZeroToOne::new(0.5)),
            inverse: Dynamic::default(),
            desired_length: Dynamic::default(),
            connected_bones: Dynamic::default(),
        }
    }
}

fn skeleton_editor(skeleton: &EditingSkeleton, watcher: &ChangeAggregator) -> impl MakeWidget {
    bone_property_editor(skeleton.root.clone(), watcher, true)
        .and(bones_editor(
            "Upper Root Bones",
            &skeleton.root.connected_bones,
            watcher,
        ))
        .and(bones_editor("Lower Root Bones", &skeleton.a_bones, watcher))
        .into_rows()
        .pad()
        .vertical_scroll()
}

fn bones_editor(
    label: &str,
    bones: &Dynamic<Vec<SkeletalBone>>,
    watcher: &ChangeAggregator,
) -> impl MakeWidget {
    watcher.watch(bones);
    let bone_editors = Dynamic::new(
        bones
            .read()
            .iter()
            .map(|bone| bone_editor(bone.clone(), watcher).make_widget())
            .collect::<WidgetList>(),
    );
    let collapsed = Dynamic::new(true);
    let add = "+".into_button().on_click({
        let bones = bones.clone();
        let bone_editors = bone_editors.clone();
        let watcher = watcher.clone();
        let collapsed = collapsed.clone();
        move |_| {
            let new_bone = SkeletalBone::default();
            bones.lock().push(new_bone.clone());
            bone_editors.lock().push(bone_editor(new_bone, &watcher));
            collapsed.set(false);
        }
    });
    bone_editors
        .into_rows()
        .disclose()
        .labelled_by(label.align_left().expand().and(add).into_columns())
        .collapsed(collapsed)
        .contain()
}
fn bone_editor(bone: SkeletalBone, watcher: &ChangeAggregator) -> impl MakeWidget {
    let bones = bones_editor("Connected Bones", &bone.connected_bones, watcher);
    bone_property_editor(bone, watcher, false)
        .and(bones)
        .into_rows()
}

#[allow(clippy::too_many_lines)]
fn bone_property_editor(
    bone: SkeletalBone,
    watcher: &ChangeAggregator,
    is_root: bool,
) -> impl MakeWidget {
    watcher.watch(&bone.joint_label);
    watcher.watch(&bone.inverse);
    watcher.watch(&bone.jointed);
    watcher.watch(&bone.label);
    watcher.watch(&bone.length);
    watcher.watch(&bone.joint_angle);
    watcher.watch(&bone.joint_ratio);
    watcher.watch(&bone.desired_length);

    bone.jointed
        .for_each_cloned({
            let mut was_jointed = bone.jointed.get();
            let desired_length = bone.desired_length.clone();
            let length = bone.length.clone();
            move |jointed| {
                if jointed && !was_jointed {
                    // When enabling jointed, we want to initialize the desired
                    // length to the current length.
                    desired_length.set(length.get());
                }
                was_jointed = jointed;
            }
        })
        .persist();

    let first = Dynamic::new(bone.length.get().to_string());
    let first_parsed = first.map_each(|s| s.parse::<f32>());
    first_parsed
        .for_each(move |result| {
            let Ok(new_value) = result else { return };
            bone.length.set(*new_value);
        })
        .persist();

    let joint_label_editor = bone.joint_label.into_input().placeholder("Joint Name");
    let label_editor = bone.label.into_input().placeholder("Bone Name");
    let length_editor = first
        .into_input()
        .placeholder("Length")
        .validation(first_parsed);

    let jointed_editor = bone.jointed.clone().into_checkbox("Jointed");

    let rotation = bone.joint_angle.slider();

    let joint_row = "Second Bone Segment Length"
        .align_left()
        .and(bone.joint_ratio.slider().with_enabled(bone.jointed.clone()))
        .into_rows()
        .fit_horizontally()
        .align_top()
        .expand()
        .and(
            bone.inverse
                .into_checkbox("Inverse")
                .with_enabled(bone.jointed.clone())
                .fit_horizontally(),
        )
        .and(Space::clear().expand_weighted(2))
        .into_columns()
        .make_widget();

    let non_joint_row = "Joint Angle"
        .align_left()
        .and(rotation)
        .into_rows()
        .expand()
        .and(Space::clear().expand_weighted(4))
        .into_columns()
        .make_widget();

    let second_row = if is_root {
        joint_row
            .collapse_vertically(bone.jointed.map_each_cloned(|j| !j))
            .make_widget()
    } else {
        bone.jointed
            .clone()
            .switcher(move |jointed, _| {
                if *jointed {
                    joint_row.clone()
                } else {
                    non_joint_row.clone()
                }
            })
            .make_widget()
    };

    "Joint Name"
        .align_left()
        .and(joint_label_editor)
        .into_rows()
        .fit_horizontally()
        .align_top()
        .expand()
        .and(
            "Bone Name"
                .align_left()
                .and(label_editor)
                .into_rows()
                .fit_horizontally()
                .align_top()
                .expand(),
        )
        .and(
            "Bone Length"
                .align_left()
                .and(length_editor)
                .into_rows()
                .fit_horizontally()
                .align_top()
                .expand(),
        )
        .and(jointed_editor.fit_horizontally().expand())
        .into_columns()
        .and(second_row)
        .into_rows()
}
