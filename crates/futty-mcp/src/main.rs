use std::{
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
};

use futty_core::{Camera, CameraLens};
use futty_runtime::{Object, Scene};
use glam::Vec3;
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
struct Futty {
    root: PathBuf,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    /// Existing .ron scene path relative to the configured Futty workspace.
    path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewScene {
    /// New .ron scene path relative to the workspace. Its parent directory must exist.
    path: String,
    name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Background {
    Sky,
    Solid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetBackground {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    background: Background,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddCamera {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    name: String,
    /// World-space camera position [x, y, z].
    position: [f32; 3],
    /// World-space point the camera should face [x, y, z].
    target: [f32; 3],
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetMainCamera {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    /// Camera UUID from scene_inspect or scene_add_camera.
    camera_id: String,
}

#[tool_handler]
impl ServerHandler for Futty {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("futty-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use workspace-relative .ron paths with forward slashes. Mutating tools edit saved scene files. Save and close an open scene in Futty before editing it through MCP, then reopen it.")
    }
}

#[tool_router]
impl Futty {
    #[tool(
        description = "Create a new Futty scene with the default cube and sky. Refuses to overwrite an existing file."
    )]
    fn scene_create(
        &self,
        Parameters(input): Parameters<NewScene>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, false)?;
        let mut scene = Scene::default();
        scene.name = input.name;
        scene.save_new(&path).map_err(io_error)?;
        Ok(scene_info(&scene))
    }

    #[tool(
        description = "Read a saved Futty scene's background, object count, and cameras, including the active game camera.",
        annotations(read_only_hint = true)
    )]
    fn scene_inspect(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        Ok(scene_info(&scene))
    }

    #[tool(description = "Set sky or solid as the background of a saved Futty scene.")]
    fn scene_set_background(
        &self,
        Parameters(input): Parameters<SetBackground>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            scene.sky = matches!(input.background, Background::Sky);
            Ok(scene_info(scene))
        })
    }

    #[tool(
        description = "Add a perspective camera to a saved Futty scene and aim it at a world-space point. The first camera becomes the main camera."
    )]
    fn scene_add_camera(
        &self,
        Parameters(input): Parameters<AddCamera>,
    ) -> Result<CallToolResult, ErrorData> {
        let eye = Vec3::from(input.position);
        let Some(direction) = (Vec3::from(input.target) - eye).try_normalize() else {
            return Err(ErrorData::invalid_params(
                "Camera position and target must define a finite direction",
                None,
            ));
        };
        let rotation = Camera {
            eye,
            yaw: direction.x.atan2(-direction.z),
            pitch: direction.y.clamp(-1.0, 1.0).asin(),
        }
        .view()
        .rotation;
        self.edit_scene(&input.path, |scene| {
            let mut object = Object::cube(input.name, eye);
            object.spin = false;
            object.camera = Some(CameraLens::default());
            object.transform.rotation = rotation;
            let id = object.uuid;
            let main = scene.camera_index(None).is_none();
            if main {
                scene.main_camera = Some(id);
            }
            scene.entities.push(object);
            Ok(CallToolResult::structured(
                json!({"camera_id": id, "main": main}),
            ))
        })
    }

    #[tool(
        description = "Choose which existing camera is used to play or export a saved Futty scene."
    )]
    fn scene_set_main_camera(
        &self,
        Parameters(input): Parameters<SetMainCamera>,
    ) -> Result<CallToolResult, ErrorData> {
        let id = Uuid::parse_str(&input.camera_id)
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        self.edit_scene(&input.path, |scene| {
            if !scene
                .entities
                .iter()
                .any(|object| object.uuid == id && object.camera.is_some())
            {
                return Err(ErrorData::invalid_params(
                    "No camera with this UUID exists in the scene",
                    None,
                ));
            }
            scene.main_camera = Some(id);
            Ok(CallToolResult::structured(json!({"main_camera": id})))
        })
    }
}

impl Futty {
    fn scene_path(&self, value: &str, existing: bool) -> Result<PathBuf, ErrorData> {
        let relative = Path::new(value);
        if relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("ron")
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(ErrorData::invalid_params(
                "Use a workspace-relative .ron path without parent traversal",
                None,
            ));
        }

        let joined = self.root.join(relative);
        let path = if existing {
            let file = joined.canonicalize().map_err(io_error)?;
            if !file.is_file()
                || file.extension().and_then(|extension| extension.to_str()) != Some("ron")
            {
                return Err(ErrorData::invalid_params(
                    "Expected a .ron scene file",
                    None,
                ));
            }
            file
        } else {
            let parent = joined
                .parent()
                .unwrap_or(&self.root)
                .canonicalize()
                .map_err(io_error)?;
            parent.join(relative.file_name().expect(".ron path has a file name"))
        };
        if !path.starts_with(&self.root) {
            return Err(ErrorData::invalid_params(
                "The scene path is outside the configured workspace",
                None,
            ));
        }
        Ok(path)
    }

    fn edit_scene(
        &self,
        relative: &str,
        edit: impl FnOnce(&mut Scene) -> Result<CallToolResult, ErrorData>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(relative, true)?;
        let mut scene = Scene::load(&path).map_err(io_error)?;
        let result = edit(&mut scene)?;
        scene.save(&path).map_err(io_error)?;
        Ok(result)
    }

    fn lock_edits(&self) -> std::io::Result<File> {
        let target = self.root.join("target");
        fs::create_dir_all(&target)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(target.join("futty-mcp.lock"))?;
        lock.lock()?;
        Ok(lock)
    }
}

fn scene_info(scene: &Scene) -> CallToolResult {
    let cameras: Vec<_> = scene
        .entities
        .iter()
        .filter_map(|object| {
            object.camera.map(|lens| {
                json!({
                    "id": object.uuid,
                    "name": object.name,
                    "parent": object.parent,
                    "local_position": object.transform.translation,
                    "local_rotation": object.transform.rotation,
                    "lens": lens,
                })
            })
        })
        .collect();
    CallToolResult::structured(json!({
        "name": scene.name,
        "background": if scene.sky { "sky" } else { "solid" },
        "object_count": scene.entities.len(),
        "main_camera": scene.camera_index(None).map(|index| scene.entities[index].uuid),
        "cameras": cameras,
    }))
}

fn io_error(error: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("Usage: futty-mcp <workspace>")?;
    let root = PathBuf::from(root).canonicalize()?;
    if !root.is_dir() {
        return Err("Workspace must be a directory".into());
    }
    Futty { root }.serve(stdio()).await?.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_camera_and_background_without_overwriting_on_create() {
        let temp = tempfile::tempdir().unwrap();
        let server = Futty {
            root: temp.path().canonicalize().unwrap(),
        };
        let path = "main.futty.scene.ron";
        let create = |name: &str| {
            server.scene_create(Parameters(NewScene {
                path: path.into(),
                name: name.into(),
            }))
        };
        create("Main").unwrap();
        assert!(create("Overwrite").is_err());
        assert!(server.scene_path("../outside.ron", false).is_err());

        server
            .scene_set_background(Parameters(SetBackground {
                path: path.into(),
                background: Background::Solid,
            }))
            .unwrap();
        server
            .scene_add_camera(Parameters(AddCamera {
                path: path.into(),
                name: "Camera".into(),
                position: [0.0, 0.0, 3.0],
                target: [0.0, 0.0, 0.0],
            }))
            .unwrap();

        let scene = Scene::load(&server.root.join(path)).unwrap();
        assert_eq!(scene.name, "Main");
        assert!(!scene.sky);
        let camera = &scene.entities[scene.camera_index(None).unwrap()];
        assert_eq!(scene.main_camera, Some(camera.uuid));
        assert!((camera.transform.rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::NEG_Z, 1e-5));
    }
}
