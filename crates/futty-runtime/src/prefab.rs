use std::collections::{HashMap, HashSet};

use futty_core::Transform;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Hierarchy, Object, Scene, SceneResult};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prefab {
    pub name: String,
    pub entities: Vec<Object>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabLink {
    pub asset: Uuid,
    pub instance: Uuid,
    pub node: Uuid,
    #[serde(default)]
    overrides: u8,
}

const POSITION: u8 = 1 << 0;
const ROTATION: u8 = 1 << 1;
const SCALE: u8 = 1 << 2;
const MATERIAL: u8 = 1 << 3;
const TEXTURE: u8 = 1 << 4;
const COLOR: u8 = 1 << 5;
const OVERRIDE_FLAGS: u8 = POSITION | ROTATION | SCALE | MATERIAL | TEXTURE | COLOR;

impl PrefabLink {
    pub fn has_overrides(self) -> bool {
        self.overrides != 0
    }
}

impl Prefab {
    pub fn hierarchy(&self) -> SceneResult<Hierarchy> {
        Hierarchy::new(&self.entities)
    }

    pub(crate) fn copies(
        &self,
        root: Uuid,
        position: Vec3,
        listener_exists: bool,
        mut next_id: impl FnMut() -> Uuid,
    ) -> Vec<Object> {
        let remap: HashMap<_, _> = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, object)| (object.uuid, if index == 0 { root } else { next_id() }))
            .collect();
        let mut copies: Vec<_> = self
            .entities
            .iter()
            .map(|object| {
                let mut copy = object.clone();
                copy.uuid = remap[&object.uuid];
                copy.parent = object.parent.map(|parent| remap[&parent]);
                if listener_exists {
                    copy.audio_listener = None;
                }
                copy
            })
            .collect();
        copies[0].transform.translation = position;
        copies[0].name = self.name.clone();
        copies
    }
}

impl Scene {
    pub fn referenced_objects(&self) -> impl Iterator<Item = &Object> {
        self.entities
            .iter()
            .chain(self.prefabs.values().flat_map(|prefab| &prefab.entities))
    }

    pub fn model_animation_references(&self, id: Uuid) -> impl Iterator<Item = (&str, u32)> {
        self.referenced_objects()
            .filter(move |object| object.mesh == Some(id))
            .filter_map(|object| Some((object.name.as_str(), object.animation?.clip)))
    }

    fn capture_prefab(
        &self,
        root: Uuid,
        hierarchy: &Hierarchy,
        live: bool,
    ) -> SceneResult<Vec<Object>> {
        let index = self
            .entities
            .iter()
            .position(|object| object.uuid == root)
            .ok_or("Select an object to save as a prefab.")?;
        let start = hierarchy
            .order
            .iter()
            .position(|&(i, _)| i == index)
            .unwrap();
        let depth = hierarchy.order[start].1;
        let mut entities = Vec::new();
        let mut skip_depth = None;
        for &(i, depth) in hierarchy
            .order
            .iter()
            .skip(start)
            .take_while(|&&(i, d)| i == index || d > depth)
        {
            if skip_depth.is_some_and(|skip| depth > skip) {
                continue;
            }
            skip_depth = None;
            let mut object = self.entities[i].clone();
            if live && object.prefab.is_some() {
                skip_depth = Some(depth);
                continue;
            }
            object.prefab = None;
            entities.push(object);
        }
        let mut transform = if entities[0].parent.is_none() {
            entities[0].transform
        } else {
            let world = hierarchy.world_matrices(|i| self.entities[i].transform.matrix());
            Transform::from_matrix(world[index])
                .ok_or("This object's parents distort its shape. Use uniform parent scales before creating a prefab.")?
        };
        transform.translation = Vec3::ZERO;
        entities[0].transform = transform;
        entities[0].parent = None;
        Ok(entities)
    }

    pub fn create_prefab(&mut self, root: Uuid) -> SceneResult<Uuid> {
        self.validate()?;
        let source = self
            .entities
            .iter()
            .find(|object| object.uuid == root)
            .filter(|object| object.prefab.is_none())
            .map(|object| object.uuid);
        let entities = self.capture_prefab(root, &self.hierarchy()?, source.is_some())?;
        let base = entities[0].name.trim().chars().take(60).collect::<String>();
        let base = if base.is_empty() {
            "Prefab".to_owned()
        } else {
            base
        };
        let mut name = base.clone();
        let mut suffix = 2;
        while self.prefabs.values().any(|prefab| prefab.name == name) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        let id = Uuid::new_v4();
        self.prefabs.insert(
            id,
            Prefab {
                name,
                entities,
                source,
            },
        );
        Ok(id)
    }

    pub fn instantiate_prefab(&mut self, id: Uuid, position: Vec3) -> SceneResult<Uuid> {
        self.validate()?;
        if !position.is_finite() {
            return Err("Prefab position must be finite.".into());
        }
        let prefab = self
            .prefabs
            .get(&id)
            .ok_or("Prefab is no longer in the scene.")?;
        let listener_exists = self
            .entities
            .iter()
            .any(|object| object.audio_listener.is_some());
        let root = Uuid::new_v4();
        let copies = prefab.copies(root, position, listener_exists, Uuid::new_v4);
        self.entities
            .extend(
                copies
                    .into_iter()
                    .zip(&prefab.entities)
                    .map(|(mut object, template)| {
                        object.prefab = Some(PrefabLink {
                            asset: id,
                            instance: root,
                            node: template.uuid,
                            overrides: 0,
                        });
                        object
                    }),
            );
        Ok(root)
    }

    pub fn detach_prefab(&mut self, root: Uuid) {
        for object in &mut self.entities {
            if object.prefab.is_some_and(|link| link.instance == root) {
                object.prefab = None;
            }
        }
    }

    pub fn remove_prefab(&mut self, id: Uuid) {
        self.prefabs.remove(&id);
        for object in &mut self.entities {
            if object.prefab.is_some_and(|link| link.asset == id) {
                object.prefab = None;
            }
        }
    }

    pub fn reset_prefab_overrides(&mut self, id: Uuid) {
        let Some(object) = self.entities.iter_mut().find(|object| object.uuid == id) else {
            return;
        };
        let Some(link) = &mut object.prefab else {
            return;
        };
        let Some(template) = self
            .prefabs
            .get(&link.asset)
            .and_then(|prefab| prefab.entities.iter().find(|node| node.uuid == link.node))
        else {
            return;
        };
        if object.uuid != link.instance {
            object.transform.translation = template.transform.translation;
        }
        object.transform.rotation = template.transform.rotation;
        object.transform.scale = template.transform.scale;
        object.material = template.material;
        object.texture = template.texture;
        object.color = template.color;
        link.overrides = 0;
    }

    pub(crate) fn validate_prefab_links(&self) -> SceneResult<()> {
        if self.entities.iter().all(|object| object.prefab.is_none()) {
            return Ok(());
        }
        let objects: HashMap<_, _> = self
            .entities
            .iter()
            .map(|object| (object.uuid, object))
            .collect();
        let templates: HashMap<_, _> = self
            .prefabs
            .iter()
            .flat_map(|(&asset, prefab)| {
                prefab
                    .entities
                    .iter()
                    .map(move |node| ((asset, node.uuid), node.parent))
            })
            .collect();
        let mut nodes = HashSet::new();
        let mut sizes = HashMap::<Uuid, usize>::new();
        for object in &self.entities {
            let Some(link) = object.prefab else {
                continue;
            };
            let valid = templates
                .get(&(link.asset, link.node))
                .is_some_and(|&parent_node| {
                    link.overrides & !OVERRIDE_FLAGS == 0
                        && objects
                            .get(&link.instance)
                            .and_then(|root| root.prefab)
                            .is_some_and(|root| {
                                root.asset == link.asset
                                    && root.instance == link.instance
                                    && templates.get(&(root.asset, root.node)) == Some(&None)
                            })
                        && nodes.insert((link.instance, link.node))
                        && match parent_node {
                            None => object.uuid == link.instance,
                            Some(node) => object
                                .parent
                                .and_then(|id| objects.get(&id))
                                .and_then(|parent| parent.prefab)
                                .is_some_and(|parent| {
                                    parent.instance == link.instance
                                        && parent.asset == link.asset
                                        && parent.node == node
                                }),
                        }
                });
            if !valid {
                return Err(format!("Invalid prefab link on {}", object.name).into());
            }
            *sizes.entry(link.instance).or_default() += 1;
        }
        for (root, size) in sizes {
            let object = objects[&root];
            let link = object.prefab.expect("validated instance root");
            if size != self.prefabs[&link.asset].entities.len() {
                return Err(format!("Incomplete prefab instance on {}", object.name).into());
            }
        }
        Ok(())
    }

    pub fn prefab_source(&self, id: Uuid) -> Option<Uuid> {
        let prefab = self.prefabs.get(&id)?;
        let source = prefab.source.unwrap_or(prefab.entities.first()?.uuid);
        self.entities
            .iter()
            .any(|object| object.uuid == source && object.prefab.is_none())
            .then_some(source)
    }

    pub fn sync_prefabs(&mut self, before: &Scene) -> SceneResult<()> {
        if self.prefabs.is_empty() {
            return Ok(());
        }
        self.track_prefab_overrides(before);
        let hierarchy = self.hierarchy()?;
        let mut changes = Vec::new();
        for (&id, prefab) in &self.prefabs {
            let Some(old) = before.prefabs.get(&id) else {
                continue;
            };
            let mut updated = prefab.clone();
            if let Some(source) = prefab.source
                && self
                    .entities
                    .iter()
                    .any(|object| object.uuid == source && object.prefab.is_none())
            {
                updated.entities = self.capture_prefab(source, &hierarchy, true)?;
            }
            if updated.name != old.name || updated.entities != old.entities {
                changes.push((id, old, updated));
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        let mut pending = self.clone();
        for (id, old, updated) in changes {
            pending.update_instances(id, old, &updated);
            pending.prefabs.insert(id, updated);
        }
        pending.validate()?;
        *self = pending;
        Ok(())
    }

    fn update_instances(&mut self, asset: Uuid, old: &Prefab, updated: &Prefab) {
        let old_nodes: HashMap<_, _> = old.entities.iter().map(|node| (node.uuid, node)).collect();
        let new_nodes: HashSet<_> = updated.entities.iter().map(|node| node.uuid).collect();
        let mut instances = HashMap::<Uuid, HashMap<Uuid, usize>>::new();
        for (index, object) in self.entities.iter().enumerate() {
            if let Some(link) = object.prefab.filter(|link| link.asset == asset) {
                instances
                    .entry(link.instance)
                    .or_default()
                    .insert(link.node, index);
            }
        }
        let mut remove = HashSet::new();
        for (root, members) in instances {
            let ids: HashMap<_, _> = updated
                .entities
                .iter()
                .map(|node| {
                    let id = members
                        .get(&node.uuid)
                        .map_or_else(Uuid::new_v4, |&index| self.entities[index].uuid);
                    (node.uuid, id)
                })
                .collect();
            let removed: HashMap<_, _> = members
                .iter()
                .filter(|(node, _)| !new_nodes.contains(node))
                .map(|(&node, &index)| (self.entities[index].uuid, (node, index)))
                .collect();
            let mut protected = HashSet::new();
            for (object_id, &(node, index)) in &removed {
                let object = &self.entities[index];
                if self.main_camera == Some(*object_id)
                    || self
                        .training_range
                        .as_ref()
                        .is_some_and(|range| range.uses(*object_id))
                    || !matches_template(object, old_nodes[&node])
                {
                    protect_ancestors(*object_id, &removed, &self.entities, &mut protected);
                }
            }
            for object in &self.entities {
                if let Some(parent) = object.parent
                    && removed.contains_key(&parent)
                    && object.prefab.is_none_or(|link| link.instance != root)
                {
                    protect_ancestors(parent, &removed, &self.entities, &mut protected);
                }
            }
            for (id, (_, index)) in removed {
                if protected.contains(&id) {
                    self.entities[index].prefab = None;
                } else {
                    remove.insert(id);
                }
            }
            for node in &updated.entities {
                let parent = node.parent.map(|parent| ids[&parent]);
                if let Some(&index) = members.get(&node.uuid) {
                    let object = &mut self.entities[index];
                    let previous = old_nodes[&node.uuid];
                    let is_root = object.uuid == root;
                    if is_root {
                        if object.name == previous.name || object.name == old.name {
                            object.name.clone_from(&updated.name);
                        }
                    } else {
                        if object.name == previous.name {
                            object.name.clone_from(&node.name);
                        }
                        object.parent = parent;
                    }
                    inherit(object, previous, node);
                    if object.parent.is_some() {
                        object.body = None;
                        object.character = None;
                    }
                } else {
                    let mut copy = node.clone();
                    copy.uuid = ids[&node.uuid];
                    copy.parent = parent;
                    copy.audio_listener = None;
                    copy.prefab = Some(PrefabLink {
                        asset,
                        instance: root,
                        node: node.uuid,
                        overrides: 0,
                    });
                    self.entities.push(copy);
                }
            }
        }
        self.entities
            .retain(|object| !remove.contains(&object.uuid));
    }

    fn track_prefab_overrides(&mut self, before: &Scene) {
        let previous: HashMap<_, _> = before
            .entities
            .iter()
            .filter(|object| object.prefab.is_some())
            .map(|object| (object.uuid, object))
            .collect();
        for object in &mut self.entities {
            let Some(link) = &mut object.prefab else {
                continue;
            };
            let Some(old) = previous.get(&object.uuid) else {
                continue;
            };
            let Some(template) = before
                .prefabs
                .get(&link.asset)
                .and_then(|prefab| prefab.entities.iter().find(|node| node.uuid == link.node))
            else {
                continue;
            };
            macro_rules! mark {
                ($bit:ident, $field:ident $(.$part:ident)?) => {
                    if object.$field$(.$part)? != old.$field$(.$part)? {
                        if object.$field$(.$part)? == template.$field$(.$part)? {
                            link.overrides &= !$bit;
                        } else {
                            link.overrides |= $bit;
                        }
                    }
                };
            }
            if link.instance != object.uuid {
                mark!(POSITION, transform.translation);
            }
            mark!(ROTATION, transform.rotation);
            mark!(SCALE, transform.scale);
            mark!(MATERIAL, material);
            mark!(TEXTURE, texture);
            mark!(COLOR, color);
        }
    }
}

fn protect_ancestors(
    mut id: Uuid,
    removed: &HashMap<Uuid, (Uuid, usize)>,
    entities: &[Object],
    protected: &mut HashSet<Uuid>,
) {
    while let Some(&(_, index)) = removed.get(&id) {
        if !protected.insert(id) {
            return;
        }
        let Some(parent) = entities[index].parent else {
            return;
        };
        id = parent;
    }
}

fn matches_template(object: &Object, template: &Object) -> bool {
    if object.prefab.is_some_and(PrefabLink::has_overrides) {
        return false;
    }
    let mut candidate = object.clone();
    candidate.uuid = template.uuid;
    candidate.parent = template.parent;
    candidate.prefab = None;
    candidate.audio_listener = template.audio_listener;
    candidate == *template
}

fn inherit(object: &mut Object, old: &Object, updated: &Object) {
    macro_rules! inherit_fields {
        ($($field:ident $(.$part:ident)?),+ $(,)?) => {$(
            if object.$field$(.$part)? == old.$field$(.$part)? {
                object.$field$(.$part)?.clone_from(&updated.$field$(.$part)?);
            }
        )+};
    }
    inherit_fields!(
        tags,
        spin,
        mesh,
        animation,
        primitive,
        sprite,
        camera,
        light,
        body,
        character,
        script,
        script_fields,
        audio_source,
    );
    let overrides = object.prefab.map_or(0, |link| link.overrides);
    macro_rules! inherit_override {
        ($bit:ident, $field:ident $(.$part:ident)?) => {
            if overrides & $bit == 0 {
                inherit_fields!($field$(.$part)?);
            }
        };
    }
    inherit_override!(ROTATION, transform.rotation);
    inherit_override!(SCALE, transform.scale);
    inherit_override!(MATERIAL, material);
    inherit_override!(TEXTURE, texture);
    inherit_override!(COLOR, color);
    if object
        .prefab
        .is_none_or(|link| object.uuid != link.instance)
    {
        inherit_override!(POSITION, transform.translation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioListener, Material, ScriptValue};
    use futty_core::{Asset, AssetKind, Light, LightKind, Surface};
    use glam::Quat;
    use std::{collections::HashSet, fs};

    #[test]
    fn source_updates_preserve_instance_ids_and_explicit_overrides() {
        let mut scene = Scene::default();
        let source = scene.entities[0].uuid;
        let mut child = Object::cube("Panel".into(), Vec3::Y);
        child.parent = Some(source);
        let node = child.uuid;
        scene.entities.push(child);
        let materials = [Uuid::new_v4(), Uuid::new_v4()];
        for id in materials {
            scene.materials.insert(id, Material::default());
        }
        let asset = scene.create_prefab(source).unwrap();
        let first = scene.instantiate_prefab(asset, Vec3::X * 3.0).unwrap();
        let second = scene.instantiate_prefab(asset, Vec3::X * 6.0).unwrap();
        let member = |scene: &Scene, instance, node| {
            scene
                .entities
                .iter()
                .position(|object| {
                    object
                        .prefab
                        .is_some_and(|link| link.instance == instance && link.node == node)
                })
                .unwrap()
        };
        let a = member(&scene, first, node);
        let b = member(&scene, second, node);
        let (a_id, b_id) = (scene.entities[a].uuid, scene.entities[b].uuid);
        let before = scene.clone();
        scene.entities[a].material = Some(materials[0]);
        scene.entities[a].transform.translation = Vec3::Y * 2.0;
        scene.sync_prefabs(&before).unwrap();
        assert!(scene.entities[a].prefab.unwrap().has_overrides());

        for (height, material) in [(2.0, materials[0]), (3.0, materials[1])] {
            let before = scene.clone();
            scene.entities[0].transform.rotation = Quat::from_rotation_y(height);
            scene.entities[1].transform.translation = Vec3::Y * height;
            scene.entities[1].material = Some(material);
            scene.entities[1].color = [0.1, 0.8, 0.4];
            scene.sync_prefabs(&before).unwrap();
        }
        assert_eq!(scene.entities[a].uuid, a_id);
        assert_eq!(scene.entities[b].uuid, b_id);
        assert_eq!(scene.entities[a].material, Some(materials[0]));
        assert_eq!(scene.entities[a].transform.translation, Vec3::Y * 2.0);
        assert_eq!(scene.entities[a].color, [0.1, 0.8, 0.4]);
        assert_eq!(scene.entities[b].material, Some(materials[1]));
        assert_eq!(scene.entities[b].transform.translation, Vec3::Y * 3.0);
        let first_root = &scene.entities[member(&scene, first, source)];
        assert_eq!(first_root.transform.translation, Vec3::X * 3.0);
        assert_eq!(
            first_root.transform.rotation,
            scene.entities[0].transform.rotation
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("linked.ron");
        scene.save(&path).unwrap();
        assert_eq!(scene, Scene::load(&path).unwrap());

        let before = scene.clone();
        let mut lamp = Object::cube("Lamp".into(), Vec3::Z);
        lamp.parent = Some(source);
        let lamp_node = lamp.uuid;
        scene.entities.push(lamp);
        scene.sync_prefabs(&before).unwrap();
        assert_eq!(
            scene.entities[member(&scene, first, lamp_node)].parent,
            Some(first)
        );
        assert_eq!(
            scene.entities[member(&scene, second, lamp_node)].parent,
            Some(second)
        );

        let before = scene.clone();
        scene.entities.retain(|object| object.uuid != node);
        scene.sync_prefabs(&before).unwrap();
        assert!(scene.entities.iter().all(|object| object.uuid != b_id));
        let detached = scene
            .entities
            .iter()
            .find(|object| object.uuid == a_id)
            .unwrap();
        assert!(detached.prefab.is_none());
        assert_eq!(detached.material, Some(materials[0]));
        scene.remove_prefab(asset);
        assert!(scene.entities.iter().all(|object| object.prefab.is_none()));
        assert!(scene.entities.iter().any(|object| object.uuid == first));
        scene.validate().unwrap();
    }

    #[test]
    fn prefab_captures_only_its_branch_and_instances_keep_settings_with_fresh_ids() {
        let mut scene = Scene::default();
        let root = scene.entities[0].uuid;
        scene.entities[0].transform.translation = Vec3::new(4.0, 2.0, 1.0);
        scene.entities[0].transform.rotation = Quat::from_rotation_y(0.3);
        scene.entities[0].audio_listener = Some(AudioListener { volume: 1.0 });
        let script = Uuid::new_v4();
        let material = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Move".into(),
            kind: AssetKind::Script,
        });
        scene
            .script_sources
            .insert(script, "fn on_update(entity, dt) {}".into());
        scene.materials.insert(
            material,
            Material {
                surface: Surface {
                    roughness: 0.2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut child = Object::cube("Lamp".into(), Vec3::Y * 2.0);
        child.parent = Some(root);
        child.light = Some(Light::new(LightKind::Point));
        child.script = Some(script);
        child
            .script_fields
            .insert("speed".into(), ScriptValue::Number(3.0));
        child.material = Some(material);
        let mut grandchild = Object::cube("Lens".into(), Vec3::X);
        grandchild.parent = Some(child.uuid);
        scene.entities.extend([grandchild, child]);
        scene.entities.push(Object::cube("Other".into(), Vec3::Z));
        let prefab = scene.create_prefab(root).unwrap();
        let template = scene.prefabs[&prefab].clone();
        assert_eq!(template.entities.len(), 3);
        assert_eq!(template.entities[0].transform.translation, Vec3::ZERO);
        let first = scene.instantiate_prefab(prefab, Vec3::X * 8.0).unwrap();
        let second = scene.instantiate_prefab(prefab, Vec3::X * 16.0).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            scene
                .entities
                .iter()
                .filter(|object| object.audio_listener.is_some())
                .count(),
            1
        );
        let ids: HashSet<_> = scene.entities.iter().map(|object| object.uuid).collect();
        assert_eq!(ids.len(), scene.entities.len());
        for copies in scene.entities[4..].as_chunks::<3>().0 {
            assert_eq!(copies[1].parent, Some(copies[0].uuid));
            assert_eq!(copies[2].parent, Some(copies[1].uuid));
            assert_eq!(
                copies[0].transform.rotation,
                scene.entities[0].transform.rotation
            );
            assert_eq!(copies[1].light, template.entities[1].light);
            assert_eq!(copies[1].script_fields, template.entities[1].script_fields);
            assert_eq!(copies[1].material, Some(material));
        }
        scene.entities[4].color = [0.0; 3];
        assert_eq!(scene.prefabs[&prefab], template);
        scene.validate().unwrap();
        scene.entities.clear();
        assert!(
            scene
                .referenced_objects()
                .any(|object| object.script == Some(script) && object.material == Some(material))
        );
        assert_eq!(crate::Runtime::new(&scene).unwrap().extract().count(), 0);
    }

    #[test]
    fn prefab_preserves_detached_world_shape_and_rejects_shear_without_partial_edits() {
        let mut scene = Scene::default();
        let parent = scene.entities[0].uuid;
        scene.entities[0].transform.scale = Vec3::splat(2.0);
        scene.entities[0].transform.rotation = Quat::from_rotation_y(0.4);
        let mut child = Object::cube("Child".into(), Vec3::X);
        child.transform.rotation = Quat::from_rotation_z(0.5);
        child.parent = Some(parent);
        let child_id = child.uuid;
        scene.entities.push(child);
        let mut expected =
            scene.entities[0].transform.matrix() * scene.entities[1].transform.matrix();
        expected.w_axis = Vec3::ZERO.extend(1.0);
        let id = scene.create_prefab(child_id).unwrap();
        assert!(
            scene.prefabs[&id].entities[0]
                .transform
                .matrix()
                .abs_diff_eq(expected, 0.0001)
        );
        scene.entities[0].transform.scale = Vec3::new(2.0, 1.0, 1.0);
        let before = scene.clone();
        assert!(scene.create_prefab(child_id).is_err());
        assert_eq!(scene, before);
        assert!(
            scene
                .instantiate_prefab(id, Vec3::splat(f32::INFINITY))
                .is_err()
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn prefab_round_trip_rejects_bad_hierarchies_and_missing_dependencies_before_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("prefabs.ron");
        let mut scene = Scene::default();
        let id = scene.create_prefab(scene.entities[0].uuid).unwrap();
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        let saved = fs::read(&path).unwrap();
        let valid = scene.prefabs[&id].clone();
        scene.prefabs.get_mut(&id).unwrap().entities[0].parent = Some(Uuid::new_v4());
        assert!(scene.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), saved);
        scene.prefabs.insert(id, valid.clone());
        scene.prefabs.get_mut(&id).unwrap().entities[0].script = Some(Uuid::new_v4());
        assert!(scene.save(&path).is_err());
        scene.prefabs.insert(id, valid);
        scene.prefabs.get_mut(&id).unwrap().entities.clear();
        assert!(scene.instantiate_prefab(id, Vec3::ZERO).is_err());
        assert_eq!(fs::read(&path).unwrap(), saved);
    }
}
