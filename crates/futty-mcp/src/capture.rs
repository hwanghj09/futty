use std::{path::Path, sync::Mutex};

use futty_core::{AssetKind, Camera};
use futty_render::{Renderer, Targets, draw_objects};
use futty_runtime::Scene;
use image::ImageEncoder;
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use uuid::Uuid;

use crate::{authoring::invalid, io_error};

static CAPTURE: Mutex<()> = Mutex::new(());

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Capture {
    /// Workspace-relative saved scene.
    pub path: String,
    /// New workspace-relative PNG. Its parent folder must exist.
    pub output_path: String,
    /// Camera object UUID. Omit for the scene's main camera.
    #[schemars(with = "Option<String>")]
    pub camera_id: Option<Uuid>,
    /// Pixel width, 1 to 2048. Defaults to 1280.
    pub width: Option<u32>,
    /// Pixel height, 1 to 2048. Defaults to 720.
    pub height: Option<u32>,
}

pub(crate) fn save(
    scene_path: &Path,
    output: &Path,
    input: &Capture,
) -> Result<([u32; 2], Uuid), ErrorData> {
    let size = [input.width.unwrap_or(1280), input.height.unwrap_or(720)];
    if size.iter().any(|value| !(1..=2048).contains(value)) {
        return Err(invalid(
            "Capture dimensions must be between 1 and 2048 pixels",
        ));
    }
    if output.symlink_metadata().is_ok() {
        return Err(invalid("Choose a new PNG path"));
    }
    let _capture = CAPTURE
        .try_lock()
        .map_err(|_| io_error("A camera capture is already in progress"))?;
    let scene = Scene::load(scene_path).map_err(io_error)?;
    let index = match input.camera_id {
        Some(id) => scene
            .entities
            .iter()
            .position(|object| object.uuid == id && object.camera.is_some()),
        None => scene.camera_index(None),
    }
    .ok_or_else(|| invalid("Choose an existing scene camera"))?;
    let hierarchy = scene.hierarchy().map_err(io_error)?;
    let frame = draw_objects(
        &scene,
        &hierarchy,
        None,
        Camera::default(),
        Some(index),
        &[],
        None,
    );
    if scene
        .camera_view(index, &hierarchy, &frame.world, &frame.local)
        .is_none()
    {
        return Err(invalid("The camera world transform is invalid"));
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&Default::default())).map_err(|error| {
            io_error(format!(
                "A graphics adapter is required for camera capture: {error}"
            ))
        })?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&Default::default())).map_err(io_error)?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = Renderer::new(&device, &queue);
    for asset in &scene.assets {
        if !frame
            .instances
            .iter()
            .any(|object| object.mesh == Some(asset.id) || object.texture == Some(asset.id))
        {
            continue;
        }
        let bytes = futty_assets::read_saved(scene_path, asset)
            .map_err(|error| io_error(format!("{}: {error}", asset.name)))?;
        match asset.kind {
            AssetKind::Model => renderer.upload_model(
                &device,
                &queue,
                asset.id,
                futty_assets::model(&bytes).map_err(io_error)?,
            ),
            AssetKind::Texture => {
                let image = futty_assets::png(&bytes).map_err(io_error)?;
                renderer.upload_image(
                    &device,
                    &queue,
                    asset.id,
                    [image.width(), image.height()],
                    image.as_raw(),
                );
            }
            _ => unreachable!("validated scenes use only models and textures in the render frame"),
        }
    }
    let targets = Targets::new(&device, size);
    renderer.draw(
        &device,
        &queue,
        &targets,
        frame.render_scene(size[0] as f32 / size[1] as f32, scene.sky),
    );
    let pixels = targets.read_rgba(&device, &queue);
    if let Some(error) = pollster::block_on(scope.pop()) {
        return Err(io_error(error));
    }
    let pixels = pixels.map_err(io_error)?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid("Capture output requires a parent folder"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    image::codecs::png::PngEncoder::new(temp.as_file_mut())
        .write_image(&pixels, size[0], size[1], image::ExtendedColorType::Rgba8)
        .map_err(io_error)?;
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist_noclobber(output).map_err(io_error)?;
    Ok((size, scene.entities[index].uuid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a graphics adapter"]
    fn captures_selected_camera_to_unaligned_png_without_overwriting() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("Scene.ron");
        let output = folder.path().join("Camera.png");
        let mut scene = Scene::default();
        let first =
            crate::camera_object("Front".into(), [0.0, 1.0, 4.0], [0.0, 0.65, 0.0]).unwrap();
        scene.main_camera = Some(first.uuid);
        let camera =
            crate::camera_object("Side".into(), [4.0, 1.0, 0.0], [0.0, 0.65, 0.0]).unwrap();
        let camera_id = camera.uuid;
        scene.entities.extend([first, camera]);
        scene.save_new(&path).unwrap();
        let input = Capture {
            path: "Scene.ron".into(),
            output_path: "Camera.png".into(),
            camera_id: Some(camera_id),
            width: Some(129),
            height: Some(73),
        };
        assert_eq!(
            save(&path, &output, &input).unwrap(),
            ([129, 73], camera_id)
        );
        let bytes = std::fs::read(&output).unwrap();
        let image = futty_assets::png(&bytes).unwrap();
        assert_eq!(image.dimensions(), (129, 73));
        assert_ne!(image.get_pixel(0, 0), image.get_pixel(64, 36));
        assert!(save(&path, &output, &input).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
    }
}
