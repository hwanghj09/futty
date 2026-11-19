use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

use glam::{Mat4, Quat, Vec3, Vec4};
use gltf::animation::{Interpolation, Property, util::ReadOutputs};

use crate::{AssetResult, Vertex};

#[derive(Clone, Copy)]
pub struct AnimationSample {
    pub clip: u32,
    pub seconds: f64,
    pub looping: bool,
}

pub struct Clip {
    pub index: u32,
    pub name: String,
    pub duration: f32,
    tracks: Vec<Track>,
}

#[derive(Clone, Copy)]
struct Pose {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

pub(crate) struct Node {
    parent: Option<usize>,
    pose: Pose,
    local: Mat4,
}

impl Node {
    pub(crate) fn new(node: &gltf::Node<'_>, parent: Option<usize>) -> AssetResult<Self> {
        let transform = node.transform();
        let local = Mat4::from_cols_array_2d(&transform.clone().matrix());
        let (translation, rotation, scale) = transform.decomposed();
        let pose = Pose {
            translation: translation.into(),
            rotation: Quat::from_array(rotation),
            scale: scale.into(),
        };
        if !local.is_finite() || !pose.rotation.is_normalized() || !pose.scale.is_finite() {
            return Err("Invalid GLB animation node transform".into());
        }
        Ok(Self {
            parent,
            pose,
            local,
        })
    }
}

struct Track {
    node: usize,
    property: Property,
    interpolation: Interpolation,
    times: Vec<f32>,
    values: Vec<Vec4>,
}

impl Track {
    fn value(&self, time: f32) -> Vec4 {
        let upper = self.times.partition_point(|&key| key <= time);
        let left = upper.saturating_sub(1);
        let cubic = self.interpolation == Interpolation::CubicSpline;
        let stride = if cubic { 3 } else { 1 };
        let value = |key| self.values[key * stride + usize::from(cubic)];
        let a = value(left);
        if upper == 0 || upper == self.times.len() || self.interpolation == Interpolation::Step {
            return a;
        }
        let duration = self.times[upper] - self.times[left];
        let t = (time - self.times[left]) / duration;
        let b = value(upper);
        if cubic {
            let t2 = t * t;
            let t3 = t2 * t;
            let out_tangent = self.values[left * 3 + 2] * duration;
            let in_tangent = self.values[upper * 3] * duration;
            a * (2.0 * t3 - 3.0 * t2 + 1.0)
                + out_tangent * (t3 - 2.0 * t2 + t)
                + b * (-2.0 * t3 + 3.0 * t2)
                + in_tangent * (t3 - t2)
        } else if self.property == Property::Rotation {
            let start = Quat::from_array(a.to_array()).normalize();
            let end = Quat::from_array(b.to_array()).normalize();
            Vec4::from_array(start.slerp(end, t).to_array())
        } else {
            a.lerp(b, t)
        }
    }
}

pub(crate) struct Influence {
    joints: [u16; 4],
    weights: [f32; 4],
}

pub(crate) enum Deform {
    Rigid(usize),
    Skin(usize, Vec<Influence>),
}

#[derive(Default)]
pub struct Rig {
    pub(crate) nodes: Vec<Node>,
    pub(crate) groups: Vec<(Range<usize>, Deform)>,
    vertices: Vec<Vertex>,
    skins: HashMap<usize, Vec<(usize, Mat4)>>,
    clips: Vec<Clip>,
}

impl Rig {
    pub(crate) fn finish(
        &mut self,
        glb: &gltf::Gltf,
        slots: &[Option<usize>],
        bin: &[u8],
        vertices: Vec<Vertex>,
    ) -> AssetResult<Vec<Vertex>> {
        self.vertices = vertices;
        let used: HashSet<_> = self
            .groups
            .iter()
            .filter_map(|(_, deform)| match deform {
                Deform::Skin(skin, _) => Some(*skin),
                Deform::Rigid(_) => None,
            })
            .collect();
        for skin in glb.skins().filter(|skin| used.contains(&skin.index())) {
            let joints = skin
                .joints()
                .map(|joint| {
                    slots[joint.index()].ok_or("GLB skin joint is outside the default scene")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let reader = skin.reader(|_| Some(bin));
            let inverse = if skin.inverse_bind_matrices().is_some() {
                let matrices = reader
                    .read_inverse_bind_matrices()
                    .ok_or("Invalid GLB inverse bind matrices")?;
                if matrices.len() < joints.len() {
                    return Err("GLB inverse bind matrix count does not match its joints".into());
                }
                matrices
                    .take(joints.len())
                    .map(|matrix| Mat4::from_cols_array_2d(&matrix))
                    .collect()
            } else {
                vec![Mat4::IDENTITY; joints.len()]
            };
            if inverse
                .iter()
                .any(|matrix| !matrix.is_finite() || matrix.row(3) != Vec4::W)
            {
                return Err("Invalid GLB inverse bind matrix".into());
            }
            self.skins
                .insert(skin.index(), joints.into_iter().zip(inverse).collect());
        }
        self.clips = read_clips(glb, slots, bin)?;
        let mut bind = Vec::new();
        if !self.write_pose(None, 0.0, &mut bind) {
            return Err("GLB bind pose produces non-finite geometry".into());
        }
        Ok(bind)
    }

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn sample(&self, sample: AnimationSample, output: &mut Vec<Vertex>) -> bool {
        let Some(clip) = self.clips.iter().find(|clip| clip.index == sample.clip) else {
            return false;
        };
        let time = if sample.looping && clip.duration > 0.0 {
            sample.seconds.rem_euclid(f64::from(clip.duration))
        } else {
            sample.seconds.clamp(0.0, f64::from(clip.duration))
        };
        time.is_finite() && self.write_pose(Some(clip), time as f32, output)
    }

    fn write_pose(&self, clip: Option<&Clip>, time: f32, output: &mut Vec<Vertex>) -> bool {
        let mut overrides = vec![None; self.nodes.len()];
        if let Some(clip) = clip {
            for track in &clip.tracks {
                let pose = overrides[track.node].get_or_insert(self.nodes[track.node].pose);
                let value = track.value(time);
                match track.property {
                    Property::Translation => pose.translation = value.truncate(),
                    Property::Rotation => {
                        let rotation = Quat::from_array(value.to_array()).normalize();
                        if !rotation.is_finite() {
                            return false;
                        }
                        pose.rotation = rotation;
                    }
                    Property::Scale => pose.scale = value.truncate(),
                    Property::MorphTargetWeights => unreachable!(),
                }
            }
        }
        let mut world = Vec::with_capacity(self.nodes.len());
        for (node, pose) in self.nodes.iter().zip(overrides) {
            let local = pose.map_or(node.local, |pose| {
                Mat4::from_scale_rotation_translation(pose.scale, pose.rotation, pose.translation)
            });
            world.push(node.parent.map_or(local, |parent| world[parent] * local));
        }
        let palettes: HashMap<_, Vec<_>> = self
            .skins
            .iter()
            .map(|(&skin, joints)| {
                let matrices = joints
                    .iter()
                    .map(|&(node, inverse)| {
                        let position = world[node] * inverse;
                        (position, normal_matrix(position))
                    })
                    .collect();
                (skin, matrices)
            })
            .collect();
        let start = output.len();
        for (range, deform) in &self.groups {
            let vertices = &self.vertices[range.clone()];
            match deform {
                Deform::Rigid(node) => {
                    let position = world[*node];
                    let normal = normal_matrix(position);
                    output.extend(vertices.iter().map(|vertex| {
                        posed(
                            vertex,
                            position.transform_point3(vertex.position.into()),
                            normal.transform_vector3(vertex.normal.into()),
                        )
                    }));
                }
                Deform::Skin(skin, influences) => {
                    let joints = &palettes[skin];
                    output.extend(vertices.iter().zip(influences).map(|(vertex, influence)| {
                        let mut point = Vec3::ZERO;
                        let mut normal = Vec3::ZERO;
                        for (joint, weight) in influence.joints.into_iter().zip(influence.weights) {
                            if weight > 0.0 {
                                let (position, rotation) = joints[joint as usize];
                                point += position.transform_point3(vertex.position.into()) * weight;
                                normal += rotation.transform_vector3(vertex.normal.into()) * weight;
                            }
                        }
                        posed(vertex, point, normal)
                    }));
                }
            }
        }
        if output[start..]
            .iter()
            .any(|vertex| !Vec3::from(vertex.position).is_finite())
        {
            output.truncate(start);
            return false;
        }
        true
    }
}

pub(crate) fn normal_matrix(matrix: Mat4) -> Mat4 {
    if matrix.determinant().abs() > 1e-12 {
        matrix.inverse().transpose()
    } else {
        matrix
    }
}

fn posed(vertex: &Vertex, position: Vec3, normal: Vec3) -> Vertex {
    Vertex {
        position: position.to_array(),
        normal: normal.normalize_or_zero().to_array(),
        ..*vertex
    }
}

pub(crate) fn influences(
    primitive: &gltf::Primitive<'_>,
    bin: &[u8],
    count: usize,
    joint_count: usize,
) -> AssetResult<Vec<Influence>> {
    use gltf::Semantic::{Joints, Weights};
    if !(1..=256).contains(&joint_count)
        || primitive
            .attributes()
            .any(|(semantic, _)| matches!(semantic, Joints(set) | Weights(set) if set > 0))
    {
        return Err("GLB skin supports up to 256 joints and four influences per vertex".into());
    }
    let reader = primitive.reader(|_| Some(bin));
    let joints = reader
        .read_joints(0)
        .ok_or("GLB skin is missing valid JOINTS_0")?
        .into_u16();
    let weights = reader
        .read_weights(0)
        .ok_or("GLB skin is missing valid WEIGHTS_0")?
        .into_f32();
    if joints.len() != count || weights.len() != count {
        return Err("GLB skin attribute counts do not match positions".into());
    }
    joints
        .zip(weights)
        .map(|(joints, mut weights)| {
            let sum = weights.iter().sum::<f32>();
            if !sum.is_finite()
                || sum <= 0.0
                || weights.iter().any(|&weight| !(0.0..=1.0).contains(&weight))
                || joints.iter().any(|&joint| joint as usize >= joint_count)
            {
                return Err("Invalid GLB skin weights or joint index".into());
            }
            for weight in &mut weights {
                *weight /= sum;
            }
            Ok(Influence { joints, weights })
        })
        .collect()
}

fn read_clips(glb: &gltf::Gltf, slots: &[Option<usize>], bin: &[u8]) -> AssetResult<Vec<Clip>> {
    let mut clips = Vec::new();
    let mut remaining_keys = 1_000_000usize;
    for animation in glb.animations() {
        let mut clip = Clip {
            index: animation.index() as u32,
            name: animation
                .name()
                .filter(|name| !name.is_empty())
                .map_or_else(|| format!("Clip {}", animation.index()), str::to_owned),
            duration: 0.0,
            tracks: Vec::new(),
        };
        let mut targets = HashSet::new();
        for channel in animation.channels() {
            let target = channel.target();
            let property = target.property();
            let Some(node) = slots[target.node().index()] else {
                continue;
            };
            if property == Property::MorphTargetWeights {
                continue;
            }
            if matches!(
                target.node().transform(),
                gltf::scene::Transform::Matrix { .. }
            ) || !targets.insert((node, property as u8))
            {
                return Err("GLB animation requires unique TRS targets".into());
            }
            let sampler = channel.sampler();
            let interpolation = sampler.interpolation();
            let count = sampler.input().count();
            let cubic = interpolation == Interpolation::CubicSpline;
            let stride = if cubic { 3 } else { 1 };
            if count == 0 || count > remaining_keys || sampler.output().count() != count * stride {
                return Err("Invalid or oversized GLB animation keys".into());
            }
            remaining_keys -= count;
            let reader = channel.reader(|_| Some(bin));
            let times: Vec<_> = reader
                .read_inputs()
                .ok_or("Invalid GLB animation times")?
                .collect();
            if times.len() != count
                || times.iter().any(|&time| !time.is_finite() || time < 0.0)
                || times.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err("GLB animation times must be finite and strictly increasing".into());
            }
            let values: Vec<_> = match reader
                .read_outputs()
                .ok_or("Invalid GLB animation values")?
            {
                ReadOutputs::Translations(values) | ReadOutputs::Scales(values) => {
                    values.map(|value| Vec3::from(value).extend(0.0)).collect()
                }
                ReadOutputs::Rotations(values) => values.into_f32().map(Vec4::from_array).collect(),
                ReadOutputs::MorphTargetWeights(_) => unreachable!(),
            };
            if values.len() != count * stride
                || values.iter().any(|value| !value.is_finite())
                || property == Property::Rotation
                    && values
                        .iter()
                        .skip(usize::from(cubic))
                        .step_by(stride)
                        .any(|value| !(0.99..=1.01).contains(&value.length_squared()))
            {
                return Err("Invalid GLB animation transforms".into());
            }
            clip.duration = clip.duration.max(times[count - 1]);
            clip.tracks.push(Track {
                node,
                property,
                interpolation,
                times,
                values,
            });
        }
        if !clip.tracks.is_empty() {
            clips.push(clip);
        }
    }
    Ok(clips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_clamps_and_scales_cubic_tangents_by_key_spacing() {
        let mut track = Track {
            node: 0,
            property: Property::Translation,
            interpolation: Interpolation::Step,
            times: vec![1.0, 3.0],
            values: vec![Vec4::X, Vec4::X * 3.0],
        };
        assert_eq!(track.value(0.0), Vec4::X);
        assert_eq!(track.value(2.0), Vec4::X);
        assert_eq!(track.value(9.0), Vec4::X * 3.0);
        track.interpolation = Interpolation::CubicSpline;
        track.values = vec![
            Vec4::ZERO,
            Vec4::X,
            Vec4::X * 2.0,
            Vec4::ZERO,
            Vec4::X * 3.0,
            Vec4::ZERO,
        ];
        assert_eq!(track.value(2.0), Vec4::X * 2.5);
    }
}
