use std::collections::HashMap;

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
}

impl Prefab {
    pub fn hierarchy(&self) -> SceneResult<Hierarchy> {
        Hierarchy::new(&self.entities)
    }
}

impl Scene {
    pub fn referenced_objects(&self) -> impl Iterator<Item = &Object> {
        self.entities
            .iter()
            .chain(self.prefabs.values().flat_map(|prefab| &prefab.entities))
    }

    pub fn create_prefab(&mut self, root: Uuid) -> SceneResult<Uuid> {
        self.validate()?;
        let index = self
            .entities
            .iter()
            .position(|object| object.uuid == root)
            .ok_or("Select an object to save as a prefab.")?;
        let hierarchy = self.hierarchy()?;
        let start = hierarchy
            .order
            .iter()
            .position(|&(i, _)| i == index)
            .unwrap();
        let depth = hierarchy.order[start].1;
        let mut entities: Vec<_> = hierarchy
            .order
            .iter()
            .skip(start)
            .take_while(|&&(i, d)| i == index || d > depth)
            .map(|&(i, _)| self.entities[i].clone())
            .collect();
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
        self.prefabs.insert(id, Prefab { name, entities });
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
        let remap: HashMap<_, _> = prefab
            .entities
            .iter()
            .map(|object| (object.uuid, Uuid::new_v4()))
            .collect();
        let listener_exists = self
            .entities
            .iter()
            .any(|object| object.audio_listener.is_some());
        let mut copies: Vec<_> = prefab
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
        copies[0].name = prefab.name.clone();
        let root = copies[0].uuid;
        // shortcut: instances are independent snapshots; add override tracking before supporting linked prefab updates.
        self.entities.extend(copies);
        Ok(root)
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
