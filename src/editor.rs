#![allow(missing_docs)]

use std::{
    fmt::Display,
    fs,
    io::{self, BufWriter},
    ops::ControlFlow,
    path::Path,
};

use cushy::{
    animation::ZeroToOne,
    value::{Destination, Dynamic, DynamicRead, Source, Switchable, Watcher},
    widget::{MakeWidget, WidgetInstance, WidgetList},
    widgets::{checkbox::Checkable, input::InputValue, slider::Slidable, Space},
};
use serde::{Deserialize, Serialize};
use tempfile::{NamedTempFile, PersistError};

use crate::{
    cushy::skeleton_canvas::{SkeletonCanvas, SkeletonMutation},
    Angle, BoneAxis, BoneId, BoneKind, Joint, JointId, LabeledBoneKind, Rotation, Skeleton, Vector,
};

pub struct SkeletonEditor {
    pub editor: WidgetInstance,
    pub skeleton: Dynamic<Skeleton>,
}

impl MakeWidget for SkeletonEditor {
    fn make_widget(self) -> WidgetInstance {
        self.editor
    }
}

#[must_use]
pub fn skeleton_editor(editing_skeleton: EditingSkeleton) -> SkeletonEditor {
    let watcher = Watcher::default();
    let skeleton = watcher.map_changed({
        let editing_skeleton = editing_skeleton.clone();
        move || {
            let mut skeleton = Skeleton::default();
            skeleton.set_rotation(editing_skeleton.root.joint_angle.get().into());
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
    let bones_editor = editing_skeleton.editor(&watcher);

    let canvas = SkeletonCanvas::new(skeleton.clone()).on_mutate({
        move |mutation| match mutation {
            SkeletonMutation::SetDesiredEnd { bone, end } => {
                let bone = editing_skeleton.find_bone(bone).expect("missing bone");
                bone.desired_length.set(end.magnitude);
                bone.joint_angle.set(end.direction.into());
            }
            SkeletonMutation::SetJointRotation { joint, rotation } => editing_skeleton
                .find_joint(joint)
                .expect("missing joint")
                .joint_angle
                .set(rotation.into()),
        }
    });
    let zoom = canvas
        .scale()
        .clone()
        .slider_between(canvas.minimum_scale(), canvas.maximum_scale());

    SkeletonEditor {
        editor: bones_editor
            .vertical_scroll()
            .expand()
            .and(canvas.expand().and(zoom).into_rows().expand())
            .into_columns()
            .make_widget(),
        skeleton,
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
        let angle = if let BoneKind::Jointed { .. } = &kind.kind {
            Rotation::default()
        } else {
            bone.joint_angle.get().into()
        };
        let new_bone = skeleton.push_bone(kind);
        skeleton[new_bone].set_desired_end(Some(vector));
        skeleton.push_joint(
            Joint::new(angle, connected_to, new_bone.axis_a()).with_label(bone.joint_label.get()),
        );
        add_bones_to_skeleton(new_bone.axis_b(), &bone.connected_bones, skeleton);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SerializedSkeleton {
    root: SerializedBone,
    a_bones: Vec<SerializedBone>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SerializedBone {
    label: String,
    joint_label: String,
    joint_angle: Angle,
    length: f32,
    jointed: bool,
    joint_ratio: ZeroToOne,
    inverse: bool,
    desired_length: f32,
    connected_bones: Vec<SerializedBone>,
}

#[derive(Debug)]
pub enum ReadError {
    Io(io::Error),
    Rsn(rsn::de::Error),
}

impl From<io::Error> for ReadError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rsn::de::Error> for ReadError {
    fn from(value: rsn::de::Error) -> Self {
        Self::Rsn(value)
    }
}

impl Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(err) => Display::fmt(err, f),
            ReadError::Rsn(err) => Display::fmt(err, f),
        }
    }
}

#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    InvalidPath,
}

impl From<io::Error> for SaveError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<PersistError> for SaveError {
    fn from(err: PersistError) -> Self {
        Self::Io(err.error)
    }
}

impl Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(err) => Display::fmt(err, f),
            SaveError::InvalidPath => f.write_str("invalid file path"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct EditingSkeleton {
    root: SkeletalBone,
    a_bones: Dynamic<Vec<SkeletalBone>>,
}

impl EditingSkeleton {
    pub fn read_from(path: &Path) -> Result<Self, ReadError> {
        let contents = fs::read(path)?;
        let skeleton = rsn::from_slice::<SerializedSkeleton>(&contents)?;
        Ok(Self::from(skeleton))
    }

    pub fn write_to(&self, path: &Path) -> Result<(), SaveError> {
        let skeleton = SerializedSkeleton::from(self);
        let parent = path.parent().ok_or(SaveError::InvalidPath)?;
        let mut temp_file = NamedTempFile::new_in(parent)?;
        let mut writer = BufWriter::new(temp_file.as_file_mut());
        rsn::ser::Config::pretty().serialize_to_writer(&skeleton, &mut writer)?;
        writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        temp_file.persist(path)?;

        Ok(())
    }

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

    fn editor(&self, watcher: &Watcher) -> impl MakeWidget {
        bone_property_editor(self.root.clone(), watcher, true)
            .and(bones_editor(
                "Upper Root Bones",
                &self.root.connected_bones,
                watcher,
            ))
            .and(bones_editor("Lower Root Bones", &self.a_bones, watcher))
            .into_rows()
            .pad()
            .vertical_scroll()
    }
}

impl From<&'_ EditingSkeleton> for SerializedSkeleton {
    fn from(skeleton: &'_ EditingSkeleton) -> Self {
        Self {
            root: SerializedBone::from(&skeleton.root),
            a_bones: skeleton
                .a_bones
                .map_ref(|bones| bones.iter().map(SerializedBone::from).collect()),
        }
    }
}

impl From<SerializedSkeleton> for EditingSkeleton {
    fn from(skeleton: SerializedSkeleton) -> Self {
        Self {
            root: SkeletalBone::from(skeleton.root),
            a_bones: Dynamic::new(
                skeleton
                    .a_bones
                    .into_iter()
                    .map(SkeletalBone::from)
                    .collect(),
            ),
        }
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
            Vector::new(vector_length, self.joint_angle.get().into()),
        )
    }
}

impl Default for SkeletalBone {
    fn default() -> Self {
        Self {
            joint_label: Dynamic::default(),
            joint_angle: Dynamic::default(),
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

impl From<&'_ SkeletalBone> for SerializedBone {
    fn from(bone: &'_ SkeletalBone) -> Self {
        Self {
            label: bone.label.get(),
            joint_label: bone.joint_label.get(),
            joint_angle: bone.joint_angle.get(),
            length: bone.length.get(),
            jointed: bone.jointed.get(),
            joint_ratio: bone.joint_ratio.get(),
            inverse: bone.inverse.get(),
            desired_length: bone.desired_length.get(),
            connected_bones: bone
                .connected_bones
                .map_ref(|bones| bones.iter().map(Self::from).collect()),
        }
    }
}

impl From<SerializedBone> for SkeletalBone {
    fn from(bone: SerializedBone) -> Self {
        Self {
            label: Dynamic::new(bone.label),
            joint_label: Dynamic::new(bone.joint_label),
            joint_angle: Dynamic::new(bone.joint_angle),
            length: Dynamic::new(bone.length),
            jointed: Dynamic::new(bone.jointed),
            joint_ratio: Dynamic::new(bone.joint_ratio),
            inverse: Dynamic::new(bone.inverse),
            desired_length: Dynamic::new(bone.desired_length),
            connected_bones: Dynamic::new(
                bone.connected_bones.into_iter().map(Self::from).collect(),
            ),
        }
    }
}

fn bones_editor(
    label: &str,
    bones: &Dynamic<Vec<SkeletalBone>>,
    watcher: &Watcher,
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
fn bone_editor(bone: SkeletalBone, watcher: &Watcher) -> impl MakeWidget {
    let bones = bones_editor("Connected Bones", &bone.connected_bones, watcher);
    bone_property_editor(bone, watcher, false)
        .and(bones)
        .into_rows()
}

#[allow(clippy::too_many_lines)]
fn bone_property_editor(bone: SkeletalBone, watcher: &Watcher, is_root: bool) -> impl MakeWidget {
    watcher.watch(&bone.joint_label);
    watcher.watch(&bone.inverse);
    watcher.watch(&bone.jointed);
    watcher.watch(&bone.label);
    watcher.watch(&bone.length);
    watcher.watch(&bone.joint_angle);
    watcher.watch(&bone.joint_ratio);
    watcher.watch(&bone.desired_length);

    let columns_wide = 3 + u8::from(!is_root);

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

    let joint_angle = if is_root { "Rotation" } else { "Joint Angle" }
        .align_left()
        .and(rotation.clone())
        .into_rows()
        .expand()
        .make_widget();

    let joint_row = joint_angle
        .clone()
        .and(
            "Midpoint"
                .align_left()
                .and(bone.joint_ratio.slider().with_enabled(bone.jointed.clone()))
                .into_rows()
                .fit_horizontally()
                .align_top()
                .expand(),
        )
        .and(
            bone.inverse
                .into_checkbox("Inverse")
                .with_enabled(bone.jointed.clone())
                .fit_horizontally(),
        )
        .and(Space::clear().expand_weighted(columns_wide - 3))
        .into_columns()
        .make_widget();

    let non_joint_row = joint_angle
        .and(Space::clear().expand_weighted(columns_wide - 1))
        .into_columns()
        .make_widget();

    let second_row = bone
        .jointed
        .clone()
        .switcher(move |jointed, _| {
            if *jointed {
                joint_row.clone()
            } else {
                non_joint_row.clone()
            }
        })
        .make_widget();

    let first_row = if is_root {
        WidgetList::new()
    } else {
        WidgetList::new().and(
            "Joint Name"
                .align_left()
                .and(joint_label_editor)
                .into_rows()
                .fit_horizontally()
                .align_top()
                .expand(),
        )
    };

    first_row
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
