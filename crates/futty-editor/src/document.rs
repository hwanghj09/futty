use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
};

use crate::selection::{Selection, selected_tree};
use futty_core::Transform;
use futty_runtime::Scene;
use glam::Mat4;
use uuid::Uuid;

pub struct Document {
    pub scene: Scene,
    pub path: Option<PathBuf>,
    pub selection: Selection,
    saved: Scene,
    undo: VecDeque<History>,
    redo: Vec<History>,
    gesture: Option<Scene>,
    pub prefab_error: Option<String>,
}

struct History {
    scene: Scene,
    selection: Option<Vec<Uuid>>,
}

impl Document {
    pub fn new(scene: Scene, path: Option<PathBuf>) -> Self {
        let mut selection = Selection::default();
        selection.replace(scene.entities.first().map(|object| object.uuid));
        Self {
            saved: scene.clone(),
            scene,
            path,
            selection,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
            prefab_error: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.scene != self.saved
    }

    pub fn saved_asset_path(&self, id: Uuid) -> Option<&std::path::Path> {
        self.path
            .as_deref()
            .filter(|_| self.saved.assets.iter().any(|asset| asset.id == id))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.saved = self.scene.clone();
        self.path = Some(path);
    }

    pub fn delete_selection(&mut self) {
        let Ok(hierarchy) = self.scene.hierarchy() else {
            return;
        };
        let removed: HashSet<_> =
            selected_tree(&self.scene.entities, &hierarchy, self.selection.ids())
                .map(|(index, _)| self.scene.entities[index].uuid)
                .collect();
        if removed.is_empty() {
            return;
        }
        let before = self.history_with_selection();
        let detach: HashSet<_> = self
            .scene
            .entities
            .iter()
            .filter_map(|object| {
                object
                    .prefab
                    .filter(|link| {
                        removed.contains(&object.uuid) && !removed.contains(&link.instance)
                    })
                    .map(|link| link.instance)
            })
            .collect();
        for root in detach {
            self.scene.detach_prefab(root);
        }
        self.scene
            .entities
            .retain(|object| !removed.contains(&object.uuid));
        if self
            .scene
            .main_camera
            .is_some_and(|id| removed.contains(&id))
        {
            self.scene.main_camera = None;
        }
        self.selection.replace(None);
        self.record(before);
    }

    pub fn duplicate_selection(&mut self) {
        let Ok(hierarchy) = self.scene.hierarchy() else {
            return;
        };
        let ids = self.selection.ids();
        let entities = &self.scene.entities;
        let branches: Vec<_> = selected_tree(entities, &hierarchy, ids).collect();
        if branches.is_empty() {
            return;
        }
        let remap: HashMap<_, _> = branches
            .iter()
            .map(|&(index, _)| (entities[index].uuid, Uuid::new_v4()))
            .collect();
        let copies: Vec<_> = branches
            .into_iter()
            .map(|(index, root)| {
                let mut copy = entities[index].clone();
                copy.uuid = remap[&copy.uuid];
                copy.prefab = copy.prefab.and_then(|mut link| {
                    link.instance = *remap.get(&link.instance)?;
                    Some(link)
                });
                copy.audio_listener = None;
                if root {
                    copy.name.push_str(" Copy");
                    copy.transform.translation.x += 1.25;
                } else if let Some(parent) = copy.parent {
                    copy.parent = Some(remap[&parent]);
                }
                copy
            })
            .collect();
        let selected: Vec<_> = ids.iter().filter_map(|id| remap.get(id).copied()).collect();
        let before = self.history_with_selection();
        self.scene.entities.extend(copies);
        self.selection.replace(selected);
        self.record(before);
    }

    pub fn reparent(&mut self, ids: &[Uuid], parent: Option<Uuid>) -> Result<(), String> {
        let hierarchy = self.scene.hierarchy().map_err(|error| error.to_string())?;
        let objects = &self.scene.entities;
        let roots: Vec<_> = selected_tree(objects, &hierarchy, ids).collect();
        if parent.is_some_and(|id| roots.iter().any(|&(index, _)| objects[index].uuid == id)) {
            return Err("An object cannot be parented to itself or its children.".into());
        }
        let worlds = hierarchy.world_matrices(|index| objects[index].transform.matrix());
        let parent_matrix = match parent {
            Some(id) => {
                let index = objects
                    .iter()
                    .position(|object| object.uuid == id)
                    .ok_or("Parent is no longer in the scene.")?;
                worlds[index]
            }
            None => Mat4::IDENTITY,
        };
        let inverse = parent_matrix.inverse();
        let mut updates = Vec::new();
        for (index, root) in roots {
            let object = &objects[index];
            if !root || object.parent == parent {
                continue;
            }
            if parent.is_some() && (object.body.is_some() || object.character.is_some()) {
                return Err(
                    "Rigid bodies and character controllers must stay at the scene root.".into(),
                );
            }
            let local = inverse * worlds[index];
            if !local.is_finite() {
                return Err("The parent transform cannot be inverted.".into());
            }
            let transform = Transform::from_matrix(local)
                .ok_or("This parent would distort the object. Use uniform parent scales.")?;
            updates.push((index, transform));
        }
        if updates.is_empty() {
            return Ok(());
        }
        let before = self.history_with_selection();
        for (index, transform) in updates {
            let object = &self.scene.entities[index];
            if let Some(link) = object.prefab.filter(|link| link.instance != object.uuid) {
                self.scene.detach_prefab(link.instance);
            }
            self.scene.entities[index].parent = parent;
            self.scene.entities[index].transform = transform;
        }
        self.record(before);
        Ok(())
    }

    pub fn create_prefab(&mut self) -> Result<Uuid, String> {
        let root = self
            .selection
            .primary()
            .ok_or("Select an object to save as a prefab.")?;
        let before = self.history_with_selection();
        let id = self
            .scene
            .create_prefab(root)
            .map_err(|error| error.to_string())?;
        self.record(before);
        Ok(id)
    }

    pub fn instantiate_prefab(&mut self, id: Uuid, position: glam::Vec3) -> Result<Uuid, String> {
        let before = self.history_with_selection();
        let root = self
            .scene
            .instantiate_prefab(id, position)
            .map_err(|error| error.to_string())?;
        self.selection.replace(Some(root));
        self.record(before);
        Ok(root)
    }

    pub fn record_frame(&mut self, before: Scene, dragging: bool) {
        if dragging {
            if before != self.scene {
                self.gesture.get_or_insert(before);
            }
            return;
        }
        let before = self.gesture.take().unwrap_or(before);
        self.record(History {
            scene: before,
            selection: None,
        });
    }

    fn history_with_selection(&self) -> History {
        History {
            scene: self.scene.clone(),
            selection: Some(self.selection.ids().to_vec()),
        }
    }

    fn record(&mut self, before: History) {
        if before.scene != self.scene {
            self.prefab_error = self
                .scene
                .sync_prefabs(&before.scene)
                .err()
                .map(|error| error.to_string());
            self.selection.retain(&self.scene.entities);
            if self.undo.len() == 64 {
                self.undo.pop_front();
            }
            self.undo.push_back(before);
            self.redo.clear();
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo.pop_back() {
            let previous = self.restore(entry);
            self.redo.push(previous);
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo.pop() {
            let previous = self.restore(entry);
            self.undo.push_back(previous);
        }
    }

    fn restore(&mut self, entry: History) -> History {
        self.prefab_error = None;
        let scene = std::mem::replace(&mut self.scene, entry.scene);
        let selection = entry.selection.map(|ids| {
            let previous = self.selection.ids().to_vec();
            self.selection.replace(ids);
            previous
        });
        self.selection.retain(&self.scene.entities);
        History { scene, selection }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_edit_and_linked_propagation_undo_and_redo_together() {
        let mut doc = Document::new(Scene::default(), None);
        let id = doc.create_prefab().unwrap();
        let root = doc.instantiate_prefab(id, glam::Vec3::X * 4.0).unwrap();
        doc.duplicate_selection();
        let duplicate = doc.selection.primary().unwrap();
        assert_ne!(root, duplicate);
        let before = doc.scene.clone();
        doc.scene.entities[0].color = [0.1, 0.6, 0.8];
        doc.record_frame(before.clone(), false);
        assert_eq!(doc.scene.entities[1].color, doc.scene.entities[0].color);
        assert_eq!(doc.scene.entities[2].color, doc.scene.entities[0].color);
        assert_eq!(doc.scene.entities[2].prefab.unwrap().instance, duplicate);
        let recolored = doc.scene.clone();
        doc.undo();
        assert_eq!(doc.scene, before);
        doc.redo();
        assert_eq!(doc.scene, recolored);
    }

    #[test]
    fn creating_and_placing_prefabs_undoes_as_single_edits_with_selection_restored() {
        let mut doc = Document::new(Scene::default(), None);
        let original = doc.selection.primary().unwrap();
        let mut child = futty_runtime::Object::cube("Child".into(), glam::Vec3::Y);
        child.parent = Some(original);
        doc.scene.entities.push(child);
        let id = doc.create_prefab().unwrap();
        let before_place = doc.scene.clone();
        let root = doc.instantiate_prefab(id, glam::Vec3::X * 4.0).unwrap();
        assert_eq!(doc.scene.entities.len(), 4);
        assert_eq!(doc.selection.primary(), Some(root));
        let placed = doc.scene.clone();
        doc.undo();
        assert_eq!(doc.scene, before_place);
        assert_eq!(doc.selection.primary(), Some(original));
        doc.redo();
        assert_eq!(doc.scene, placed);
        assert_eq!(doc.selection.primary(), Some(root));
        doc.undo();
        doc.undo();
        assert!(doc.scene.prefabs.is_empty());
        assert_eq!(doc.scene.entities.len(), 2);
        doc.redo();
        assert_eq!(doc.scene, before_place);
    }

    #[test]
    fn drag_undoes_as_one_edit() {
        let mut doc = Document::new(Scene::default(), None);
        for x in [1.0, 2.0, 3.0] {
            let before = doc.scene.clone();
            doc.scene.entities[0].transform.translation.x = x;
            doc.record_frame(before, true);
        }
        doc.record_frame(doc.scene.clone(), false);
        doc.undo();
        assert!(!doc.dirty());
        assert!(!doc.can_undo());
        doc.redo();
        assert_eq!(doc.scene.entities[0].transform.translation.x, 3.0);
    }

    #[test]
    fn duplicate_and_delete_process_each_selected_branch_once() {
        use futty_runtime::{AudioListener, Object};
        use glam::Vec3;

        let mut scene = Scene::default();
        let root = scene.entities[0].uuid;
        scene.entities[0].audio_listener = Some(AudioListener { volume: 1.0 });
        let mut child = Object::cube("Child".into(), Vec3::X);
        child.parent = Some(root);
        let child_id = child.uuid;
        let other = Object::cube("Other".into(), Vec3::Y);
        let other_id = other.uuid;
        scene.entities.extend([child, other]);
        let mut doc = Document::new(scene.clone(), None);
        doc.selection.replace([root, child_id, other_id]);
        doc.duplicate_selection();
        let copies = &doc.scene.entities[3..];
        assert_eq!(copies.len(), 3);
        assert_eq!(copies[1].parent, Some(copies[0].uuid));
        assert!(copies[0].audio_listener.is_none());
        assert_eq!(doc.selection.ids().len(), 3);
        let duplicated = doc.scene.clone();
        doc.delete_selection();
        assert_eq!(doc.scene, scene);
        doc.undo();
        assert_eq!(doc.scene, duplicated);
        assert_eq!(doc.selection.ids().len(), 3);
        doc.undo();
        assert_eq!(doc.scene, scene);
        assert_eq!(doc.selection.ids(), &[root, child_id, other_id]);
    }

    #[test]
    fn reparent_preserves_world_pose_and_rejects_cycles_and_partial_updates() {
        use futty_runtime::Object;
        use glam::{Quat, Vec3};
        let mut scene = Scene::default();
        let parent = scene.entities[0].uuid;
        scene.entities[0].transform.translation = Vec3::X * 3.0;
        scene.entities[0].transform.rotation = Quat::from_rotation_y(0.5);
        scene.entities[0].transform.scale = Vec3::splat(2.0);
        let object = Object::cube("Child".into(), Vec3::new(4.0, 1.0, 2.0));
        let child = object.uuid;
        let before = object.transform.matrix();
        scene.entities.push(object);
        let mut doc = Document::new(scene, None);
        doc.reparent(&[child], Some(parent)).unwrap();
        let world = doc
            .scene
            .hierarchy()
            .unwrap()
            .world_matrices(|i| doc.scene.entities[i].transform.matrix());
        assert!(world[1].abs_diff_eq(before, 1e-5));
        let scene = doc.scene.clone();
        assert!(doc.reparent(&[parent], Some(child)).is_err());
        assert_eq!(doc.scene, scene);
        doc.undo();
        assert!(doc.scene.entities[1].parent.is_none());
        doc.redo();
        doc.reparent(&[child], None).unwrap();
        assert!(
            doc.scene.entities[1]
                .transform
                .matrix()
                .abs_diff_eq(before, 1e-5)
        );

        doc.scene.entities[0].transform.scale = Vec3::new(3.0, 1.0, 2.0);
        let scene = doc.scene.clone();
        assert!(doc.reparent(&[child], Some(parent)).is_err());
        assert_eq!(doc.scene, scene);
    }
}
