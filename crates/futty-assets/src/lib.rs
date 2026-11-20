mod animation;

pub use animation::{AnimationSample, Clip, Rig};

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    ops::Range,
    path::{Path, PathBuf},
};

use futty_core::{Asset, AssetKind};
use glam::{Mat4, Vec3};
use image::{ImageFormat, ImageReader, RgbaImage};

pub type AssetResult<T> = Result<T, Box<dyn Error>>;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_VERTICES: usize = 1_000_000;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

pub struct Part {
    pub indices: Range<u32>,
    pub color: [f32; 3],
    pub texture: Option<usize>,
}

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub parts: Vec<Part>,
    pub textures: Vec<RgbaImage>,
    pub bounds: [Vec3; 2],
    pub rig: Option<Rig>,
}

pub fn read(path: &Path) -> AssetResult<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Asset exceeds the 64 MiB file limit".into());
    }
    Ok(bytes)
}

fn directory(scene: &Path, create: bool) -> AssetResult<PathBuf> {
    let root = scene
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let mut dir = root.clone();
    for component in ["Assets", "Futty"] {
        dir.push(component);
        if create {
            match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        dir = dir.canonicalize()?;
        if !dir.starts_with(&root) || !dir.is_dir() {
            return Err("Asset folder must be inside the scene folder".into());
        }
    }
    Ok(dir)
}

fn filename(asset: &Asset) -> String {
    format!("{}.{}", asset.id, asset.kind.extension())
}

pub fn read_saved(scene: &Path, asset: &Asset) -> AssetResult<Vec<u8>> {
    let dir = directory(scene, false)?;
    let file = dir.join(filename(asset)).canonicalize()?;
    if !file.starts_with(&dir) || !file.is_file() {
        return Err("Asset file must be inside the asset folder".into());
    }
    read(&file)
}

pub fn save(scene: &Path, asset: &Asset, bytes: &[u8]) -> AssetResult<()> {
    let dir = directory(scene, true)?;
    let path = dir.join(filename(asset));
    if path.exists() {
        return if read_saved(scene, asset)? == bytes {
            Ok(())
        } else {
            Err(format!("Asset UUID collision for {}", asset.name).into())
        };
    }
    write_asset(&path, bytes, false)
}

pub fn save_new(scene: &Path, asset: &Asset, bytes: &[u8]) -> AssetResult<PathBuf> {
    let path = directory(scene, true)?.join(filename(asset));
    write_asset(&path, bytes, false)?;
    Ok(path)
}

pub fn replace_saved_model(
    scene: &Path,
    asset: &Asset,
    expected: Option<&[u8]>,
    bytes: &[u8],
) -> AssetResult<()> {
    if asset.kind != AssetKind::Model {
        return Err("Reimport requires a GLB model asset".into());
    }
    replace_asset(scene, asset, bytes, |current| {
        current.is_none_or(|current| {
            expected.map_or_else(|| model(current).is_err(), |expected| current == expected)
        })
    })
}

pub fn replace_saved(
    scene: &Path,
    asset: &Asset,
    expected: Option<&[u8]>,
    bytes: &[u8],
) -> AssetResult<()> {
    replace_asset(scene, asset, bytes, |current| expected == current)
}

fn replace_asset(
    scene: &Path,
    asset: &Asset,
    bytes: &[u8],
    may_replace: impl FnOnce(Option<&[u8]>) -> bool,
) -> AssetResult<()> {
    let dir = directory(scene, true)?;
    let path = dir.join(filename(asset));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".futty-reimport.lock"))?;
    lock.lock()?;
    match path.symlink_metadata() {
        Ok(_) => {
            let current = read_saved(scene, asset)?;
            if current == bytes {
                return Ok(());
            }
            if !may_replace(Some(&current)) {
                return Err(format!(
                    "{} changed on disk. Reopen the scene before reimporting.",
                    asset.name
                )
                .into());
            }
            write_asset(&path, bytes, true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && may_replace(None) => {
            write_asset(&path, bytes, false)
        }
        Err(error) => Err(error.into()),
    }
}

fn write_asset(path: &Path, bytes: &[u8], replace: bool) -> AssetResult<()> {
    let dir = path.parent().ok_or("Missing asset folder")?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    Ok(())
}

pub fn image(bytes: &[u8], format: ImageFormat) -> AssetResult<RgbaImage> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.into_rgba8())
}

pub fn png(bytes: &[u8]) -> AssetResult<RgbaImage> {
    image(bytes, ImageFormat::Png)
}

pub fn kind(path: &Path) -> Option<AssetKind> {
    let extension = path.extension()?.to_str()?;
    [
        AssetKind::Model,
        AssetKind::Texture,
        AssetKind::Script,
        AssetKind::Wav,
        AssetKind::Ogg,
    ]
    .into_iter()
    .find(|kind| extension.eq_ignore_ascii_case(kind.extension()))
}

pub fn script_source(bytes: &[u8]) -> AssetResult<&str> {
    if bytes.len() > 64 * 1024 {
        return Err("Script exceeds the 64 KiB source limit".into());
    }
    Ok(std::str::from_utf8(bytes)?.trim_start_matches('\u{feff}'))
}

pub fn model_with_clips<'a>(
    bytes: &[u8],
    mut referenced_clips: impl Iterator<Item = (&'a str, u32)>,
) -> AssetResult<Model> {
    let model = model(bytes)?;
    let clips = model.rig.as_ref().map_or(&[][..], Rig::clips);
    if let Some((name, clip)) =
        referenced_clips.find(|(_, index)| !clips.iter().any(|clip| clip.index == *index))
    {
        return Err(format!(
            "{name} uses animation clip {clip}, which is missing from the replacement GLB"
        )
        .into());
    }
    Ok(model)
}

pub fn model(bytes: &[u8]) -> AssetResult<Model> {
    let glb = gltf::Gltf::from_slice(bytes)?;
    let bin = glb
        .blob
        .as_deref()
        .ok_or("Expected a GLB with embedded mesh data")?;
    if glb
        .buffers()
        .any(|b| !matches!(b.source(), gltf::buffer::Source::Bin) || b.length() > bin.len())
        || glb.views().any(|v| {
            v.offset()
                .checked_add(v.length())
                .is_none_or(|end| end > bin.len())
        })
        || glb.accessors().any(|a| a.count() > 3 * MAX_VERTICES)
    {
        return Err("Invalid or oversized GLB buffer".into());
    }
    if let Some(extension) = glb.extensions_required().next() {
        return Err(format!("Unsupported required GLB extension: {extension}").into());
    }
    let scene = glb
        .default_scene()
        .or_else(|| glb.scenes().next())
        .ok_or("GLB has no scene")?;
    let animated = glb.animations().len() != 0 || glb.skins().len() != 0;
    if animated && (glb.animations().len() > 256 || glb.nodes().len() > 4096) {
        return Err("GLB animation node or clip limit exceeded".into());
    }
    let mut slots = vec![None; if animated { glb.nodes().len() } else { 0 }];
    let mut nodes: Vec<_> = scene
        .nodes()
        .map(|node| (node, Mat4::IDENTITY, None))
        .collect();
    let mut visited = HashSet::new();
    let mut textures = HashMap::new();
    let mut result = Model {
        vertices: Vec::new(),
        indices: Vec::new(),
        parts: Vec::new(),
        textures: Vec::new(),
        bounds: [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)],
        rig: animated.then(Rig::default),
    };
    while let Some((node, parent, rig_parent)) = nodes.pop() {
        if !visited.insert(node.index()) {
            return Err("GLB node has more than one parent or forms a cycle".into());
        }
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        let slot = if let Some(rig) = &mut result.rig {
            let slot = rig.nodes.len();
            slots[node.index()] = Some(slot);
            rig.nodes.push(animation::Node::new(&node, rig_parent)?);
            Some(slot)
        } else {
            None
        };
        nodes.extend(node.children().map(|child| (child, world, slot)));
        let Some(mesh) = node.mesh() else { continue };
        let skin = node.skin();
        if skin.is_none() && (!world.is_finite() || world.determinant().abs() < 1e-12) {
            return Err("GLB has an invalid mesh node transform".into());
        }
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err("Only triangle meshes are supported".into());
            }
            let reader = primitive.reader(|_| Some(bin));
            let positions = reader
                .read_positions()
                .ok_or("Invalid GLB position accessor")?;
            let count = positions.len();
            if count == 0 || result.vertices.len() + count > MAX_VERTICES {
                return Err("GLB vertex limit exceeded or mesh is empty".into());
            }
            let positions: Vec<_> = positions.map(Vec3::from).collect();
            let mut indices: Vec<u32> = reader.read_indices().map_or_else(
                || (0..count as u32).collect(),
                |idx| idx.into_u32().collect(),
            );
            if primitive.indices().is_some() && reader.read_indices().is_none()
                || indices.is_empty()
                || !indices.len().is_multiple_of(3)
                || indices.iter().any(|&index| index as usize >= count)
                || result.indices.len() + indices.len() > 3 * MAX_VERTICES
            {
                return Err("Invalid or oversized GLB triangle index data".into());
            }
            let normals: Vec<_> = if let Some(normals) = reader.read_normals() {
                normals.map(Vec3::from).collect()
            } else {
                let mut normals = vec![Vec3::ZERO; count];
                for triangle in indices.as_chunks::<3>().0 {
                    let [a, b, c] = triangle.map(|index| index as usize);
                    let normal = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
                    normals[a] += normal;
                    normals[b] += normal;
                    normals[c] += normal;
                }
                normals.into_iter().map(Vec3::normalize_or_zero).collect()
            };
            let material = primitive.material().pbr_metallic_roughness();
            let base = material.base_color_factor();
            let tex = material.base_color_texture();
            let uv: Vec<_> = reader
                .read_tex_coords(tex.as_ref().map_or(0, |t| t.tex_coord()))
                .map(|coordinates| coordinates.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0; 2]; count]);
            if uv.len() != count
                || normals.len() != count
                || positions.iter().any(|p| !p.is_finite())
                || normals.iter().any(|n| !n.is_finite())
                || uv.iter().flatten().any(|value| !value.is_finite())
            {
                return Err("Invalid GLB vertex data".into());
            }
            let texture = if let Some(tex) = tex {
                if reader.read_tex_coords(tex.tex_coord()).is_none() {
                    return Err("GLB material is missing its UV coordinates".into());
                }
                let source = tex.texture().source();
                let index = if let Some(&index) = textures.get(&source.index()) {
                    index
                } else {
                    let gltf::image::Source::View { view, mime_type } = source.source() else {
                        return Err("GLB textures must be embedded".into());
                    };
                    let format = match mime_type {
                        "image/png" => ImageFormat::Png,
                        "image/jpeg" => ImageFormat::Jpeg,
                        _ => return Err("Unsupported GLB image format".into()),
                    };
                    let decoded =
                        image(&bin[view.offset()..view.offset() + view.length()], format)?;
                    let total = result
                        .textures
                        .iter()
                        .map(|image| image.len())
                        .sum::<usize>();
                    if total + decoded.len() > 128 * 1024 * 1024 {
                        return Err("GLB texture memory limit exceeded".into());
                    }
                    let index = result.textures.len();
                    result.textures.push(decoded);
                    textures.insert(source.index(), index);
                    index
                };
                Some(index)
            } else {
                None
            };
            let vertex_base = result.vertices.len() as u32;
            let start = result.indices.len() as u32;
            if skin.is_none() && world.determinant() < 0.0 {
                for triangle in indices.as_chunks_mut::<3>().0 {
                    triangle.swap(1, 2);
                }
            }
            result
                .indices
                .extend(indices.into_iter().map(|index| index + vertex_base));
            if let Some(rig) = &mut result.rig {
                let deform = if let Some(skin) = &skin {
                    animation::Deform::Skin(
                        skin.index(),
                        animation::influences(&primitive, bin, count, skin.joints().len())?,
                    )
                } else {
                    animation::Deform::Rigid(slot.expect("rig node was recorded"))
                };
                let base = vertex_base as usize;
                rig.groups.push((base..base + count, deform));
                result
                    .vertices
                    .extend(positions.into_iter().zip(normals).zip(uv).map(
                        |((position, normal), uv)| Vertex {
                            position: position.to_array(),
                            normal: normal.to_array(),
                            uv,
                        },
                    ));
            } else {
                let normal_matrix = animation::normal_matrix(world);
                result
                    .vertices
                    .extend(positions.into_iter().zip(normals).zip(uv).map(
                        |((position, normal), uv)| {
                            Vertex {
                                position: world.transform_point3(position).to_array(),
                                normal: normal_matrix
                                    .transform_vector3(normal)
                                    .normalize_or_zero()
                                    .to_array(),
                                uv,
                            }
                        },
                    ));
            }
            result.parts.push(Part {
                indices: start..result.indices.len() as u32,
                color: [base[0], base[1], base[2]],
                texture,
            });
        }
    }
    if result.parts.is_empty() {
        return Err("GLB has no triangle mesh in its default scene".into());
    }
    if let Some(rig) = &mut result.rig {
        result.vertices = rig.finish(&glb, &slots, bin, std::mem::take(&mut result.vertices))?;
        if rig.clips().is_empty() {
            result.rig = None;
        }
    }
    for vertex in &result.vertices {
        let position = Vec3::from(vertex.position);
        if !position.is_finite() {
            return Err("GLB pose produces non-finite geometry".into());
        }
        result.bounds[0] = result.bounds[0].min(position);
        result.bounds[1] = result.bounds[1].max(position);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRATE: &[u8] = include_bytes!("../../../examples/asset-preview/Workshop Crate.glb");
    const GRID: &[u8] = include_bytes!("../../../examples/asset-preview/Workshop Grid.png");

    #[test]
    fn imports_beacon_bot_skin_and_evaluates_clips_with_looping_and_clamping() {
        let bot = model(include_bytes!(
            "../../../examples/animation-playground/Assets/Futty/6c5cb6d0-3ec5-4142-8af3-368c0c033dde.glb"
        )).unwrap();
        let rig = bot.rig.as_ref().unwrap();
        assert_eq!(
            rig.clips()
                .iter()
                .map(|clip| clip.name.as_str())
                .collect::<Vec<_>>(),
            ["Idle", "Walk", "Signal"]
        );
        let sample = |clip, seconds, looping| {
            let mut vertices = Vec::new();
            assert!(rig.sample(
                AnimationSample {
                    clip,
                    seconds,
                    looping
                },
                &mut vertices
            ));
            assert_eq!(vertices.len(), bot.vertices.len());
            assert!(
                vertices
                    .iter()
                    .all(|vertex| Vec3::from(vertex.normal).is_normalized())
            );
            vertices
        };
        let distance = |a: &[Vertex], b: &[Vertex]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| Vec3::from(a.position).distance(Vec3::from(b.position)))
                .fold(0.0f32, f32::max)
        };
        assert!(distance(&bot.vertices, &sample(0, 0.0, false)) < 1e-5);
        assert!(distance(&bot.vertices, &sample(2, 0.61, false)) > 0.7);
        assert!(distance(&sample(2, 1.65, false), &sample(2, 9.0, false)) < 1e-5);
        assert!(
            distance(
                &sample(1, 0.3, true),
                &sample(1, f64::from(rig.clips()[1].duration) + 0.3, true)
            ) < 1e-5
        );
    }

    #[test]
    fn imports_static_parts_and_embedded_texture() {
        let mesh = model(CRATE).unwrap();
        assert_eq!(mesh.parts.len(), 3);
        assert_eq!(mesh.textures.len(), 1);
        assert!(mesh.parts.iter().any(|part| part.texture == Some(0)));
        assert!((mesh.bounds[0].y - 0.12).abs() < 1e-4);
        assert!((mesh.bounds[1].y - 1.05).abs() < 1e-4);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| Vec3::from(vertex.normal).is_normalized())
        );
        assert_eq!(png(GRID).unwrap().dimensions(), (64, 64));
        assert!(model(&CRATE[..CRATE.len() / 2]).is_err());
        assert!(png(&GRID[..10]).is_err());
    }

    #[test]
    fn persists_portable_assets_and_rejects_uuid_collisions() {
        let root = tempfile::tempdir().unwrap();
        let copy = tempfile::tempdir().unwrap();
        let first = root.path().join("scene.ron");
        let second = copy.path().join("scene.ron");
        let asset = Asset {
            id: "c1df83d5-dd33-4d93-b0b8-cef4d2cf45c6".parse().unwrap(),
            name: "Workshop Grid".into(),
            kind: AssetKind::Texture,
        };
        save(&first, &asset, GRID).unwrap();
        save(&first, &asset, GRID).unwrap();
        assert!(save(&first, &asset, b"conflict").is_err());
        save(&second, &asset, &read_saved(&first, &asset).unwrap()).unwrap();
        assert_eq!(read_saved(&second, &asset).unwrap(), GRID);
    }
}
