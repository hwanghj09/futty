use std::f32::consts::{PI, TAU};

use futty_assets::Vertex;
use futty_core::Primitive;
use glam::Vec3;

pub fn mesh(primitive: Primitive) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    match primitive {
        Primitive::Cube => return super::cube_mesh(),
        Primitive::Plane => {
            for (normal, sign) in [(Vec3::Y, 1.0), (Vec3::NEG_Y, -1.0)] {
                let base = vertices.len() as u32;
                for uv in [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]] {
                    vertices.push(Vertex {
                        position: [uv[0] - 0.5, 0.0, sign * (uv[1] - 0.5)],
                        normal: normal.to_array(),
                        uv,
                    });
                }
                indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        Primitive::Sphere => {
            let rows: Vec<_> = (0..=16)
                .map(|row| {
                    let v = row as f32 / 16.0;
                    let (radius, y) = (PI * v).sin_cos();
                    let radius = if row == 16 { 0.0 } else { radius };
                    (0.5 * radius, -0.5 * y, radius, -y, 1.0 - v)
                })
                .collect();
            surface(&rows, &mut vertices, &mut indices);
        }
        Primitive::Cylinder | Primitive::Cone => {
            let cylinder = primitive == Primitive::Cylinder;
            let top_radius = if cylinder { 0.5 } else { 0.0 };
            let slope = 0.5 - top_radius;
            surface(
                &[
                    (0.5, -0.5, 1.0, slope, 1.0),
                    (top_radius, 0.5, 1.0, slope, 0.0),
                ],
                &mut vertices,
                &mut indices,
            );
            cap(-0.5, &mut vertices, &mut indices);
            if cylinder {
                cap(0.5, &mut vertices, &mut indices);
            }
        }
    }
    (vertices, indices)
}

const SIDES: u32 = 32;

fn surface(rows: &[(f32, f32, f32, f32, f32)], vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
    let base = vertices.len() as u32;
    for &(radius, height, normal_radius, normal_y, v) in rows {
        for side in 0..=SIDES {
            let u = side as f32 / SIDES as f32;
            let (sin, cos) = (TAU * u).sin_cos();
            vertices.push(Vertex {
                position: [radius * cos, height, radius * sin],
                normal: Vec3::new(normal_radius * cos, normal_y, normal_radius * sin)
                    .normalize()
                    .to_array(),
                uv: [u, v],
            });
        }
    }
    for (row, pair) in rows.windows(2).enumerate() {
        for side in 0..SIDES {
            let bottom = base + row as u32 * (SIDES + 1) + side;
            let top = bottom + SIDES + 1;
            if pair[0].0 > 0.0 {
                indices.extend([bottom, top, bottom + 1]);
            }
            if pair[1].0 > 0.0 {
                indices.extend([bottom + 1, top, top + 1]);
            }
        }
    }
}

fn cap(height: f32, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
    let base = vertices.len() as u32;
    let normal = (Vec3::Y * height.signum()).to_array();
    vertices.push(Vertex {
        position: [0.0, height, 0.0],
        normal,
        uv: [0.5, 0.5],
    });
    for side in 0..=SIDES {
        let (sin, cos) = (TAU * side as f32 / SIDES as f32 * -height.signum()).sin_cos();
        vertices.push(Vertex {
            position: [0.5 * cos, height, 0.5 * sin],
            normal,
            uv: [0.5 + 0.5 * cos, 0.5 + 0.5 * sin],
        });
    }
    for side in 0..SIDES {
        indices.extend([base, base + side + 1, base + side + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_meshes_match_bounds_and_have_outward_triangles() {
        for primitive in Primitive::ALL {
            let (vertices, indices) = mesh(primitive);
            let [min, max] = primitive.bounds();
            for vertex in &vertices {
                let position = Vec3::from(vertex.position);
                assert!(position.cmpge(min).all() && position.cmple(max).all());
            }
            for &[a, b, c] in indices.as_chunks::<3>().0 {
                let [a, b, c] = [a, b, c].map(|i| &vertices[i as usize]);
                let origin = Vec3::from(a.position);
                let face = (Vec3::from(b.position) - origin).cross(Vec3::from(c.position) - origin);
                assert!(face.dot(Vec3::from(a.normal)) > 0.0, "{primitive:?}");
            }
        }
    }
}
