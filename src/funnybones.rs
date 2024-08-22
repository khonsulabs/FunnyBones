//! The FunnyBones 2D Animation Editor.

use cushy::{
    value::{Destination, Dynamic, DynamicRead, ForEach, Source, Switchable, Validation},
    widget::{MakeWidget, WidgetList},
    widgets::{checkbox::Checkable, input::InputValue, slider::Slidable},
    Run,
};
use funnybones::{
    cushy::skeleton_canvas::SkeletonCanvas, BoneAxis, BoneEnd, BoneKind, Joint, Rotation, Skeleton,
};

#[derive(Default, Eq, PartialEq, Debug, Clone, Copy)]
enum Mode {
    #[default]
    Bones,
    Joints,
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

fn main() -> anyhow::Result<()> {
    let bones = Dynamic::<Vec<EditingBone>>::default();
    let joints = Dynamic::<Vec<EditingJoint>>::default();
    let (watcher, skeleton) = ChangeAggregator::new({
        let bones = bones.clone();
        let joints = joints.clone();
        move || {
            let mut skeleton = Skeleton::default();
            let bones = bones.read();
            let mut bone_ids = Vec::with_capacity(bones.len());
            for bone in &*bones {
                let length = bone.length.get();
                let bone_kind = if let Some(second) = bone.jointed.get() {
                    BoneKind::Jointed {
                        start_length: length,
                        end_length: second,
                        inverse: bone.inverse.get(),
                    }
                } else {
                    BoneKind::Rigid { length }
                };
                bone_ids.push(skeleton.push_bone(bone_kind.with_label(bone.label.get())));
            }

            let joints = joints.read();
            for joint in &*joints {
                let first_bone = joint.connections[0]
                    .bone
                    .map_ref(|label| skeleton.find_bone_by_label(label));
                let second_bone = joint.connections[1]
                    .bone
                    .map_ref(|label| skeleton.find_bone_by_label(label));
                let (Some(first), Some(second)) = (first_bone, second_bone) else {
                    println!("Couldn't find one or more bones for joint");
                    continue;
                };

                let first = BoneAxis {
                    bone: first,
                    end: joint.connect_via.get(),
                };
                let second = BoneAxis {
                    bone: second,
                    end: BoneEnd::A,
                };
                skeleton.push_joint(
                    Joint::new(joint.angle.get(), first, second).with_label(joint.label.get()),
                );
            }
            skeleton
        }
    });
    watcher.watch(&bones);
    watcher.watch(&joints);
    let bones_editor = bones_editor(&bones, &watcher).make_widget();
    let joints_editor = joints_editor(&joints, &bones, &watcher).make_widget();

    let mode = Dynamic::<Mode>::default();

    [
        (Mode::Bones, "Bones"),
        (Mode::Joints, "Joints"),
        (Mode::Animation, "Animation"),
    ]
    .into_iter()
    .map(|(selected, label)| mode.clone().new_select(selected, label))
    .collect::<WidgetList>()
    .into_columns()
    .centered()
    .and(
        mode.switcher(move |mode, _mode_dynamic| match mode {
            Mode::Animation => SkeletonCanvas::new(skeleton.clone()).make_widget(),
            Mode::Joints => joints_editor.clone(),
            Mode::Bones => bones_editor.clone(),
        })
        .expand(),
    )
    .into_rows()
    .expand()
    .run()?;
    Ok(())
}

#[derive(PartialEq, Debug, Default, Clone)]
struct EditingBone {
    label: Dynamic<String>,
    length: Dynamic<f32>,
    jointed: Dynamic<Option<f32>>,
    inverse: Dynamic<bool>,
}

#[derive(PartialEq, Debug, Clone)]
struct EditingJoint {
    label: Dynamic<String>,
    angle: Dynamic<Rotation>,
    connections: [JointConnection; 2],
    connect_via: Dynamic<BoneEnd>,
}

impl Default for EditingJoint {
    fn default() -> Self {
        Self {
            label: Dynamic::default(),
            angle: Dynamic::default(),
            connections: [
                JointConnection {
                    bone: Dynamic::default(),
                },
                JointConnection {
                    bone: Dynamic::default(),
                },
            ],
            connect_via: Dynamic::new(BoneEnd::B),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
struct JointConnection {
    bone: Dynamic<String>,
}

fn bones_editor(bones: &Dynamic<Vec<EditingBone>>, watcher: &ChangeAggregator) -> impl MakeWidget {
    // TODO it should be easier to map a Vec<_> to a WidgetList.
    let bone_editors = Dynamic::new(
        bones
            .read()
            .iter()
            .map(|bone| bone_editor(bone.clone(), watcher).make_widget())
            .collect::<WidgetList>(),
    );
    let add = "New Bone".into_button().on_click({
        let bones = bones.clone();
        let bone_editors = bone_editors.clone();
        let watcher = watcher.clone();
        move |_| {
            let new_bone = EditingBone::default();
            bones.lock().push(new_bone.clone());
            bone_editors.lock().push(bone_editor(new_bone, &watcher));
        }
    });
    add.and(bone_editors.into_rows())
        .into_rows()
        .vertical_scroll()
}

fn bone_editor(bone: EditingBone, watcher: &ChangeAggregator) -> impl MakeWidget {
    watcher.watch(&bone.inverse);
    watcher.watch(&bone.jointed);
    watcher.watch(&bone.label);
    watcher.watch(&bone.length);
    let (second, jointed) = if let Some(length) = bone.jointed.get() {
        (length, true)
    } else {
        (1., false)
    };
    let jointed = Dynamic::new(jointed);

    let first = Dynamic::new(bone.length.get().to_string());
    let first_parsed = first.map_each(|s| s.parse::<f32>());
    first_parsed
        .for_each(move |result| {
            let Ok(new_value) = result else { return };
            bone.length.set(*new_value);
        })
        .persist();
    let second = Dynamic::new(second.to_string());
    let second_parsed = second.map_each(|s| s.parse::<f32>());
    (&jointed, &second_parsed)
        .for_each(move |(jointed, result)| {
            if *jointed {
                let Ok(new_value) = result else { return };
                bone.jointed.set(Some(*new_value));
            } else {
                bone.jointed.set(None);
            }
        })
        .persist();

    let label_editor = bone.label.into_input().placeholder("Name");
    let length_editor = "Length"
        .and(first.into_input().validation(first_parsed))
        .into_rows();

    let jointed_editor = jointed
        .clone()
        .into_checkbox("Jointed")
        .and(second.into_input().with_enabled(jointed.clone()))
        .and(bone.inverse.into_checkbox("Inverse").with_enabled(jointed))
        .into_rows()
        .validation(second_parsed);

    label_editor
        .expand()
        .and(length_editor.expand())
        .and(jointed_editor.expand())
        .into_columns()
}

fn joints_editor(
    joints: &Dynamic<Vec<EditingJoint>>,
    bones: &Dynamic<Vec<EditingBone>>,
    watcher: &ChangeAggregator,
) -> impl MakeWidget {
    let joint_editors = Dynamic::new(
        joints
            .read()
            .iter()
            .map(|bone| joint_editor(bone.clone(), bones, watcher).make_widget())
            .collect::<WidgetList>(),
    );
    let add = "New Joint".into_button().on_click({
        let joints = joints.clone();
        let bones = bones.clone();
        let joint_editors = joint_editors.clone();
        let watcher = watcher.clone();
        move |_| {
            let new_joint = EditingJoint::default();
            joints.lock().push(new_joint.clone());
            joint_editors
                .lock()
                .push(joint_editor(new_joint, &bones, &watcher));
        }
    });
    add.and(joint_editors.into_rows())
        .into_rows()
        .vertical_scroll()
}

fn joint_editor(
    joint: EditingJoint,
    bones: &Dynamic<Vec<EditingBone>>,
    watcher: &ChangeAggregator,
) -> impl MakeWidget {
    watcher.watch(&joint.angle);
    watcher.watch(&joint.connect_via);
    watcher.watch(&joint.label);
    watcher.watch(&joint.connections[0].bone);
    watcher.watch(&joint.connections[1].bone);
    let label_editor = joint.label.into_input().placeholder("Name");

    let angle_editor = "Rotation"
        .and(joint.angle.slider_between(Rotation::MIN, Rotation::MAX))
        .into_rows();

    let joint_bones = joint_bone_input("From Bone", joint.connections[0].bone.clone(), bones)
        .and(
            "Connect Via:"
                .and(joint.connect_via.new_radio(BoneEnd::A, "End A"))
                .and(joint.connect_via.new_radio(BoneEnd::B, "End B"))
                .into_columns(),
        )
        .and(joint_bone_input(
            "To Bone",
            joint.connections[1].bone.clone(),
            bones,
        ))
        .into_rows();

    label_editor
        .expand()
        .and(angle_editor.expand())
        .and(joint_bones.expand())
        .into_columns()
}

fn joint_bone_input(
    label: &str,
    bone: Dynamic<String>,
    bones: &Dynamic<Vec<EditingBone>>,
) -> impl MakeWidget {
    let bone_name_is_valid = bone.map_each({
        let bones = bones.clone();
        move |name| {
            let bones = bones.read();

            if bones.iter().any(|b| b.label.map_ref(|l| l == name)) {
                Validation::Valid
            } else {
                Validation::Invalid(String::from("bone not found"))
            }
        }
    });

    bone.into_input()
        .placeholder(label)
        .validation(bone_name_is_valid)
}
