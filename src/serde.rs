use std::borrow::Cow;

use serde::{
    de::{self, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};

use crate::{Bone, BoneAxis, BoneKind, Joint, Angle, Skeleton, Coordinate};

impl Serialize for Skeleton {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("Skeleton", 2)?;
        s.serialize_field("bones", &self.bones)?;
        s.serialize_field("joints", &self.joints)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Skeleton {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "Skeleton",
            &["bones", "joints"],
            SkeletonVisitor::default(),
        )
    }
}

#[derive(Default)]
struct SkeletonVisitor {
    bones: Vec<DeserializedBone>,
    joints: Vec<DeserializedJoint>,
}

impl<'de> Visitor<'de> for SkeletonVisitor {
    type Value = Skeleton;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a Skeleton")
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            match &*key {
                "bones" => {
                    self.bones = map.next_value()?;
                }
                "joints" => {
                    self.joints = map.next_value()?;
                }
                _ => {
                    return Err(<A::Error as de::Error>::custom(format!(
                        "unexpected field {key}"
                    )))
                }
            }
        }

        let mut skeleton = Skeleton::default();
        for bone in self.bones.drain(..) {
            let id = skeleton.push_bone(bone.kind.with_label(bone.label));
            if let Some(target) = bone.target {
                skeleton[id].set_desired_end(Some(target));
            }
        }
        for joint in self.joints.drain(..) {
            skeleton
                .push_joint(Joint::new(joint.angle, joint.from, joint.to).with_label(joint.label));
        }
        Ok(skeleton)
    }
}

impl Serialize for Bone {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let field_count =
            1 + usize::from(self.label.is_some()) + usize::from(self.desired_end.is_some());
        let mut b = serializer.serialize_struct("Bone", field_count)?;
        b.serialize_field("kind", &self.kind)?;
        if let Some(label) = &self.label {
            b.serialize_field("label", &**label)?;
        }
        if let Some(desired_end) = self.desired_end {
            b.serialize_field("target", &desired_end)?;
        }
        b.end()
    }
}

#[derive(Deserialize)]
struct DeserializedBone {
    #[serde(default)]
    label: String,
    kind: BoneKind,
    #[serde(default)]
    target: Option<Coordinate>,
}

impl Serialize for Joint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let field_count = 3 + usize::from(self.label.is_some());
        let mut b = serializer.serialize_struct("Joint", field_count)?;
        b.serialize_field("from", &self.bone_a)?;
        b.serialize_field("to", &self.bone_b)?;
        b.serialize_field("angle", &self.angle)?;
        if let Some(label) = &self.label {
            b.serialize_field("label", &**label)?;
        }
        b.end()
    }
}

#[derive(Deserialize)]
struct DeserializedJoint {
    from: BoneAxis,
    to: BoneAxis,
    angle: Angle,
    #[serde(default)]
    label: String,
}

#[test]
fn roundtrip() {
    let mut s = Skeleton::default();
    let spine = s.push_bone(BoneKind::Rigid { length: 1.0 }.with_label("spine"));
    let other = s.push_bone(BoneKind::Jointed {
        start_length: 2.0,
        end_length: 3.0,
        inverse: true,
    });
    let joint = s.push_joint(Joint::new(
        Angle::radians(0.),
        spine.axis_a(),
        other.axis_b(),
    ));
    let serialized = pot::to_vec(&s).unwrap();
    let deserialized: Skeleton = dbg!(pot::from_slice(&serialized).unwrap());
    assert_eq!(deserialized[spine].label(), "spine");
    assert_eq!(deserialized[other].label(), "");
    assert_eq!(deserialized[joint].angle(), Angle::radians(0.));
}
