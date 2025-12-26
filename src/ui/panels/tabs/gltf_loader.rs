use glam::Vec3;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Model {
    pub vertices: Vec<Vec3>,
    pub lines: Vec<[u32; 2]>,
}

pub struct ModelCache {
    models: HashMap<String, Model>,
}

impl ModelCache {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn load_model(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.models.contains_key(path) {
            return Ok(());
        }

        let model_path = std::path::PathBuf::from(path);

        if !model_path.exists() {
            return Err(format!(
                "Model file not found: {} (resolved to: {})",
                path,
                model_path.display()
            )
            .into());
        }

        let (document, buffers, _) = gltf::import(path)?;

        let mut all_vertices = Vec::new();
        let mut unique_edges: HashSet<(u32, u32)> = HashSet::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let base_index = all_vertices.len() as u32;
                let mut vert_count = 0;

                if let Some(positions) = reader.read_positions() {
                    for pos in positions {
                        all_vertices.push(Vec3::from(pos));
                        vert_count += 1;
                    }
                } else {
                    continue;
                }

                let indices: Vec<u32> = if let Some(iter) = reader.read_indices() {
                    iter.into_u32().collect()
                } else {
                    (0..vert_count).collect()
                };

                let mut add_edge = |i1: u32, i2: u32| {
                    let a = base_index + i1;
                    let b = base_index + i2;
                    if a < b {
                        unique_edges.insert((a, b));
                    } else {
                        unique_edges.insert((b, a));
                    }
                };

                match primitive.mode() {
                    gltf::mesh::Mode::Triangles => {
                        for chunk in indices.chunks(3) {
                            if chunk.len() == 3 {
                                add_edge(chunk[0], chunk[1]);
                                add_edge(chunk[1], chunk[2]);
                                add_edge(chunk[2], chunk[0]);
                            }
                        }
                    }
                    gltf::mesh::Mode::TriangleStrip => {
                        for i in 0..indices.len().saturating_sub(2) {
                            add_edge(indices[i], indices[i + 1]);
                            add_edge(indices[i + 1], indices[i + 2]);
                            add_edge(indices[i + 2], indices[i]);
                        }
                    }
                    gltf::mesh::Mode::TriangleFan => {
                        for i in 1..indices.len().saturating_sub(1) {
                            add_edge(indices[0], indices[i]);
                            add_edge(indices[i], indices[i + 1]);
                            add_edge(indices[i + 1], indices[0]);
                        }
                    }
                    gltf::mesh::Mode::Lines => {
                        for chunk in indices.chunks(2) {
                            if chunk.len() == 2 {
                                add_edge(chunk[0], chunk[1]);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if all_vertices.is_empty() {
            return Err("Model contains no vertices".into());
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in &all_vertices {
            min = min.min(*v);
            max = max.max(*v);
        }

        let center = (min + max) * 0.5;
        let size = (max - min).max_element();

        if size > 0.0 {
            for v in &mut all_vertices {
                *v = (*v - center) / size;
            }
        }

        let lines: Vec<[u32; 2]> = unique_edges.into_iter().map(|(a, b)| [a, b]).collect();

        self.models.insert(
            path.to_string(),
            Model {
                vertices: all_vertices,
                lines,
            },
        );

        Ok(())
    }

    pub fn get_model(&self, path: &str) -> Option<&Model> {
        self.models.get(path)
    }
}

impl Default for ModelCache {
    fn default() -> Self {
        Self::new()
    }
}
