use futty_core::AssetKind;
use futty_runtime::{
    Billboard, Body, CharacterController, CharacterMode, Motion, Object, Scene, Sprite, SpriteSheet,
};
use glam::{Vec2, Vec3};
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use uuid::Uuid;

use crate::authoring::invalid;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Tilemap2d {
    #[schemars(with = "String")]
    pub texture_id: Uuid,
    /// Number of atlas columns and rows.
    pub atlas: [u16; 2],
    /// Cell indices, counting from zero; null is empty. Rows run from top to bottom.
    pub rows: Vec<Vec<Option<u16>>>,
    /// Atlas indices that create fixed colliders.
    #[serde(default)]
    pub solid: Vec<u16>,
    /// Center of the top-left cell. Defaults to [0, 0, 0].
    pub origin: Option<[f32; 3]>,
    /// Width and height of a cell, defaulting to 1 world unit.
    pub tile_size: Option<f32>,
}

impl Tilemap2d {
    pub(crate) fn build(&self, scene: &Scene) -> Result<Vec<Object>, ErrorData> {
        if !scene
            .assets
            .iter()
            .any(|asset| asset.id == self.texture_id && asset.kind == AssetKind::Texture)
        {
            return Err(invalid("Tilemap requires a registered PNG texture"));
        }
        let [columns, rows] = self.atlas;
        if !(1..=256).contains(&columns) || !(1..=256).contains(&rows) {
            return Err(invalid("Atlas dimensions must be between 1 and 256"));
        }
        let tile_size = self.tile_size.unwrap_or(1.0);
        let origin = Vec3::from(self.origin.unwrap_or([0.0; 3]));
        if !(0.05..=100.0).contains(&tile_size)
            || !origin.is_finite()
            || origin.abs().max_element() > 10_000.0
        {
            return Err(invalid(
                "Tile size must be between 0.05 and 100 and origin within ±10000",
            ));
        }
        let width = self.rows.first().map_or(0, Vec::len);
        if !(1..=64).contains(&width)
            || !(1..=64).contains(&self.rows.len())
            || self.rows.iter().any(|row| row.len() != width)
        {
            return Err(invalid(
                "Provide a rectangular tile grid up to 64 columns by 64 rows",
            ));
        }
        let atlas_cells = u32::from(columns) * u32::from(rows);
        let count = self.rows.iter().flatten().flatten().count();
        if !(1..=1024).contains(&count)
            || self.solid.len() > 1024
            || self
                .rows
                .iter()
                .flatten()
                .flatten()
                .chain(&self.solid)
                .any(|&cell| u32::from(cell) >= atlas_cells)
        {
            return Err(invalid(
                "Provide 1 to 1024 painted tiles and valid atlas indices",
            ));
        }
        let solid: std::collections::HashSet<_> = self.solid.iter().copied().collect();
        let mut tiles = Vec::with_capacity(count);
        // shortcut: emit editable objects per cell, batch into chunks if larger maps need it
        for (y, row) in self.rows.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                let Some(frame) = cell else { continue };
                let position = origin + Vec3::new(x as f32, -(y as f32), 0.0) * tile_size;
                let mut object = Object::cube(format!("Tile {x},{y}"), position);
                object.spin = false;
                object.color = [1.0; 3];
                object.sprite = Some(Sprite {
                    texture: self.texture_id,
                    size: Vec2::splat(tile_size),
                    billboard: Billboard::Fixed,
                    planar: (scene.dimension == futty_runtime::SceneDimension::TwoD)
                        .then(futty_runtime::Sprite2d::default),
                    sheet: Some(SpriteSheet {
                        columns,
                        rows,
                        first_frame: frame,
                        frames: 1,
                        fps: 1.0,
                        looping: false,
                    }),
                });
                if solid.contains(&frame) {
                    object.body = Some(Body {
                        motion: Motion::Fixed,
                        size: Vec3::new(tile_size, tile_size, 1.0),
                        ..Body::default()
                    });
                }
                tiles.push(object);
            }
        }
        Ok(tiles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_use_atlas_cells_and_only_solid_indices_get_colliders() {
        let texture = Uuid::new_v4();
        let mut scene = Scene::default();
        scene.assets.push(futty_core::Asset {
            id: texture,
            name: "Tiles".into(),
            kind: AssetKind::Texture,
        });
        let mut map = Tilemap2d {
            texture_id: texture,
            atlas: [2, 1],
            rows: vec![vec![None, Some(0)], vec![Some(1), Some(0)]],
            solid: vec![1],
            origin: Some([2.0, 0.0, 1.0]),
            tile_size: Some(0.5),
        };
        let tiles = map.build(&scene).unwrap();
        assert_eq!(tiles.len(), 3);
        assert!(tiles[0].body.is_none());
        assert_eq!(tiles[1].transform.translation, Vec3::new(2.0, -0.5, 1.0));
        assert_eq!(tiles[1].body.unwrap().size, Vec3::new(0.5, 0.5, 1.0));
        assert_eq!(
            tiles[1].sprite.unwrap().sheet.unwrap().uv_rect(0.0),
            [0.5, 0.0, 0.5, 1.0]
        );
        map.rows[0][0] = Some(2);
        assert!(map.build(&scene).is_err());
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Maze2d {
    pub seed: u32,
    /// Number of rooms along X and Y, each between 2 and 8.
    pub rooms: [u8; 2],
    /// Width of a room, passage or wall cell in world units, from 3 to 12.
    pub cell_size: Option<f32>,
    /// World position of the player start. Defaults to [0, 0, 0].
    pub origin: Option<[f32; 3]>,
}

impl Maze2d {
    pub(crate) fn build(self) -> Result<(Object, Object, Vec<Object>), ErrorData> {
        if self.rooms.iter().any(|count| !(2..=8).contains(count)) {
            return Err(invalid("Maze room counts must be between 2 and 8 per axis"));
        }
        let cell = self.cell_size.unwrap_or(3.0);
        let origin = self.origin.unwrap_or([0.0; 3]);
        if !(3.0..=12.0).contains(&cell)
            || origin
                .iter()
                .any(|axis| !(-10_000.0..=10_000.0).contains(axis))
        {
            return Err(invalid(
                "Maze cell size must be between 3 and 12 and origin within ±10000",
            ));
        }
        let [columns, rows] = self.rooms.map(usize::from);
        let [width, height] = [columns * 2 + 1, rows * 2 + 1];
        let open = self.carve(width, height);
        let position =
            |x: f32, y: usize| Vec3::from(origin) + Vec3::new(x - 1.0, y as f32 - 1.0, 0.0) * cell;
        let mut walls = Vec::new();
        for (y, row) in open.chunks_exact(width).enumerate() {
            let mut x = 0;
            while x < width {
                if row[x] {
                    x += 1;
                    continue;
                }
                let run = row[x..].iter().take_while(|&&open| !open).count();
                let center = x as f32 + (run - 1) as f32 * 0.5;
                let mut wall = Object::cube(format!("Maze wall {x},{y}"), position(center, y));
                wall.spin = false;
                wall.color = [0.28, 0.36, 0.48];
                wall.transform.scale = Vec3::new(run as f32 * cell, cell, 1.0);
                wall.body = Some(Body {
                    motion: Motion::Fixed,
                    ..Body::default()
                });
                walls.push(wall);
                x += run;
            }
        }
        let mut start = Object::cube("Maze player".into(), Vec3::from(origin));
        start.spin = false;
        start.tags.push("maze_player".into());
        start.color = [0.38, 0.78, 1.0];
        start.transform.scale = Vec3::splat(0.6);
        start.character = Some(CharacterController {
            mode: CharacterMode::TopDown2d,
            ..CharacterController::default()
        });
        let mut goal = Object::cube("Maze goal".into(), position((width - 2) as f32, height - 2));
        goal.spin = false;
        goal.tags.push("maze_goal".into());
        goal.color = [0.32, 0.95, 0.44];
        goal.transform.scale = Vec3::splat(0.8);
        Ok((start, goal, walls))
    }

    fn carve(&self, width: usize, height: usize) -> Vec<bool> {
        let mut open = vec![false; width * height];
        let mut stack = vec![(1_usize, 1_usize)];
        let mut random = u64::from(self.seed) + 1;
        open[width + 1] = true;
        while let Some(&(x, y)) = stack.last() {
            let candidates: Vec<_> = [
                (x + 2, y),
                (x, y + 2),
                (x.wrapping_sub(2), y),
                (x, y.wrapping_sub(2)),
            ]
            .into_iter()
            .filter(|&(nx, ny)| nx < width && ny < height && !open[ny * width + nx])
            .collect();
            if candidates.is_empty() {
                stack.pop();
                continue;
            }
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let (nx, ny) = candidates[(random % candidates.len() as u64) as usize];
            open[(y + ny) / 2 * width + (x + nx) / 2] = true;
            open[ny * width + nx] = true;
            stack.push((nx, ny));
        }
        open
    }
}
