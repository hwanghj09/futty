use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Scene, valid_scene_id};

fn filename(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

pub const SUFFIX: &str = ".futty.project.ron";
pub const MAX_SCENES: usize = 64;

pub fn is_project(path: &Path) -> bool {
    filename(path).ends_with(SUFFIX)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SceneList {
    pub start: String,
    pub scenes: BTreeMap<String, String>,
}

impl SceneList {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        let mut paths = HashSet::new();
        if self.scenes.len() > MAX_SCENES
            || !self.scenes.contains_key(&self.start)
            || self.scenes.iter().any(|(id, path)| {
                !valid_scene_id(id)
                    || !path.ends_with(".ron")
                    || path.contains(['\\', ':'])
                    || path
                        .split('/')
                        .any(|part| part.is_empty() || part == "." || part == "..")
                    || !paths.insert(path)
            })
        {
            return Err("Invalid scene list. Use unique IDs and relative .ron paths, and choose a start scene.".into());
        }
        Ok(())
    }

    pub fn resolve(&self, root: &Path, id: &str) -> Result<PathBuf, Box<dyn Error>> {
        let relative = self
            .scenes
            .get(id)
            .ok_or_else(|| format!("Unknown scene ID {id:?}"))?;
        let path = root
            .join(relative)
            .canonicalize()
            .map_err(|error| format!("{id}: {error}"))?;
        if !path.starts_with(root) || !path.is_file() {
            return Err(format!("Scene {id:?} must be a file inside the project folder").into());
        }
        Ok(path)
    }

    pub fn add(&mut self, root: &Path, path: &Path) -> Result<String, Box<dyn Error>> {
        let path = path.canonicalize()?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Choose a scene inside the project folder")?;
        let relative = relative
            .to_str()
            .ok_or("Scene paths must be UTF-8")?
            .replace('\\', "/");
        let filename = filename(&path);
        let id = filename
            .strip_suffix(".futty.scene.ron")
            .or_else(|| filename.strip_suffix(".ron"))
            .ok_or("Choose a scene .ron file")?
            .to_owned();
        if self.scenes.contains_key(&id) {
            return Err(format!("Scene ID {id:?} is already registered").into());
        }
        Scene::load(&path)?;
        self.scenes.insert(id.clone(), relative);
        if self.start.is_empty() {
            self.start.clone_from(&id);
        }
        self.validate()?;
        Ok(id)
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    scenes: SceneList,
}

#[derive(Clone)]
pub struct Project {
    path: PathBuf,
    pub root: PathBuf,
    manifest: Manifest,
}

impl Project {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let path = path.canonicalize()?;
        let root = path.parent().ok_or("Missing project folder")?.to_path_buf();
        let manifest: Manifest = ron::from_str(&fs::read_to_string(&path)?)?;
        if manifest.format != 1 {
            return Err("Unsupported Futty project format".into());
        }
        manifest.scenes.validate()?;
        Ok(Self {
            path,
            root,
            manifest,
        })
    }

    pub fn create(path: &Path, first: &Path) -> Result<Self, Box<dyn Error>> {
        let root = project_root(path)?;
        let mut scenes = SceneList {
            start: String::new(),
            scenes: BTreeMap::new(),
        };
        scenes.add(&root, first)?;
        Self::create_with_scenes(path, scenes)
    }

    pub fn create_with_scenes(path: &Path, scenes: SceneList) -> Result<Self, Box<dyn Error>> {
        let root = project_root(path)?;
        let path = root.join(path.file_name().ok_or("Missing project filename")?);
        scenes.validate()?;
        for id in scenes.scenes.keys() {
            Scene::load(&scenes.resolve(&root, id)?)?;
        }
        let project = Self {
            path,
            root,
            manifest: Manifest { format: 1, scenes },
        };
        project.save(false)?;
        Ok(project)
    }

    pub fn save(&self, replace: bool) -> Result<(), Box<dyn Error>> {
        self.list().validate()?;
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        file.write_all(ron::ser::to_string_pretty(&self.manifest, Default::default())?.as_bytes())?;
        file.as_file().sync_all()?;
        if replace {
            file.persist(&self.path)?;
        } else {
            file.persist_noclobber(&self.path)?;
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        filename(&self.path)
            .strip_suffix(SUFFIX)
            .unwrap_or("Project")
    }

    pub fn list(&self) -> &SceneList {
        &self.manifest.scenes
    }

    pub fn list_mut(&mut self) -> &mut SceneList {
        &mut self.manifest.scenes
    }

    pub fn set_start(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        Scene::load(&self.resolve(id)?)?;
        self.manifest.scenes.start = id.into();
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn resolve(&self, id: &str) -> Result<PathBuf, Box<dyn Error>> {
        self.list().resolve(&self.root, id)
    }
}

fn project_root(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    Ok(path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?)
}
