use eframe::egui;
use eframe::glow;
use glow::HasContext;
use std::sync::{Arc, Mutex};

struct GridResources {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    vertex_count: i32,
    shader_program: glow::Program,
}

pub struct Scene3D {
    resources: Arc<Mutex<Option<GridResources>>>,

    yaw: f32,
    pitch: f32,
    distance: f32,
    target: glam::Vec3,
}

impl Scene3D {
    pub fn new() -> Self {
        Self {
            resources: Arc::new(Mutex::new(None)),
            yaw: 45.0f32.to_radians(),
            pitch: 30.0f32.to_radians(),
            distance: 100.0,
            target: glam::Vec3::ZERO,
        }
    }

    fn init_gl(gl: &glow::Context) -> GridResources {
        unsafe {
            let vertex_shader_source = r#"
                #version 330 core
                layout(location = 0) in vec3 position;
                layout(location = 1) in vec3 color;
                
                uniform mat4 view_proj;
                
                out vec3 v_color;
                
                void main() {
                    gl_Position = view_proj * vec4(position, 1.0);
                    v_color = color;
                }
            "#;

            let fragment_shader_source = r#"
                #version 330 core
                in vec3 v_color;
                out vec4 frag_color;
                
                void main() {
                    frag_color = vec4(v_color, 1.0);
                }
            "#;

            let vertex_shader = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(vertex_shader, vertex_shader_source);
            gl.compile_shader(vertex_shader);

            if !gl.get_shader_compile_status(vertex_shader) {
                panic!(
                    "Vertex shader error: {}",
                    gl.get_shader_info_log(vertex_shader)
                );
            }

            let fragment_shader = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(fragment_shader, fragment_shader_source);
            gl.compile_shader(fragment_shader);

            if !gl.get_shader_compile_status(fragment_shader) {
                panic!(
                    "Fragment shader error: {}",
                    gl.get_shader_info_log(fragment_shader)
                );
            }

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                panic!("Shader program error: {}", gl.get_program_info_log(program));
            }

            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            let grid_size = 500.0;
            let grid_spacing = 5.0;
            let num_lines = (grid_size / grid_spacing) as i32;

            let mut vertices = Vec::new();
            let grid_color = [0.3, 0.3, 0.3];
            let axis_color_x = [0.8, 0.2, 0.2];
            let axis_color_y = [0.2, 0.8, 0.2];

            // Grid lines parallel to X axis (along East)
            for i in -num_lines..=num_lines {
                let y = i as f32 * grid_spacing;
                let color = if i == 0 { axis_color_y } else { grid_color };

                // Line from (-grid_size, y, 0) to (grid_size, y, 0)
                vertices.extend_from_slice(&[-grid_size, y, 0.0, color[0], color[1], color[2]]);
                vertices.extend_from_slice(&[grid_size, y, 0.0, color[0], color[1], color[2]]);
            }

            // Grid lines parallel to Y axis (along North)
            for i in -num_lines..=num_lines {
                let x = i as f32 * grid_spacing;
                let color = if i == 0 { axis_color_x } else { grid_color };

                // Line from (x, -grid_size, 0) to (x, grid_size, 0)
                vertices.extend_from_slice(&[x, -grid_size, 0.0, color[0], color[1], color[2]]);
                vertices.extend_from_slice(&[x, grid_size, 0.0, color[0], color[1], color[2]]);
            }

            let vertex_count = vertices.len() as i32 / 6;

            let vao = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao));

            let vbo = gl.create_buffer().unwrap();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(
                0,
                3,
                glow::FLOAT,
                false,
                6 * std::mem::size_of::<f32>() as i32,
                0,
            );

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(
                1,
                3,
                glow::FLOAT,
                false,
                6 * std::mem::size_of::<f32>() as i32,
                3 * std::mem::size_of::<f32>() as i32,
            );

            gl.bind_vertex_array(None);

            GridResources {
                vao,
                vbo,
                vertex_count,
                shader_program: program,
            }
        }
    }

    fn compute_view_proj_matrix(&self, aspect: f32) -> glam::Mat4 {
        let height = -self.distance * self.pitch.sin();
        let ground_dist = self.distance * self.pitch.cos();

        let camera_x = ground_dist * self.yaw.cos();
        let camera_y = ground_dist * self.yaw.sin();
        let camera_z = height;

        let eye = self.target + glam::Vec3::new(camera_x, camera_y, camera_z);
        let up = glam::Vec3::new(0.0, 0.0, -1.0);

        let view = glam::Mat4::look_at_rh(eye, self.target, up);
        let proj = glam::Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 10000.0);

        proj * view
    }

    pub fn render(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let response = ui.interact(rect, ui.id().with("3d_scene"), egui::Sense::drag());

        if response.dragged() {
            let delta = response.drag_delta();
            self.yaw += delta.x * 0.01;
            self.pitch += delta.y * 0.01;
            self.pitch = self.pitch.clamp(0.0, std::f32::consts::FRAC_PI_2);
            ui.ctx().request_repaint();
        }

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.distance *= 1.0 - (scroll * 0.001);
                self.distance = self.distance.clamp(10.0, 1000.0);
                ui.ctx().request_repaint();
            }
        }

        let aspect = rect.width() / rect.height();
        let view_proj = self.compute_view_proj_matrix(aspect);
        let resources_ref = self.resources.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                let gl = painter.gl();

                let mut resources_guard = resources_ref.lock().unwrap();
                if resources_guard.is_none() {
                    *resources_guard = Some(Self::init_gl(gl));
                }

                if let Some(ref resources) = *resources_guard {
                    unsafe {
                        gl.enable(glow::DEPTH_TEST);
                        gl.depth_func(glow::LESS);
                        gl.clear(glow::DEPTH_BUFFER_BIT);

                        gl.use_program(Some(resources.shader_program));

                        let view_proj_loc =
                            gl.get_uniform_location(resources.shader_program, "view_proj");
                        if let Some(loc) = view_proj_loc {
                            gl.uniform_matrix_4_f32_slice(
                                Some(&loc),
                                false,
                                &view_proj.to_cols_array(),
                            );
                        }

                        // Draw grid
                        gl.bind_vertex_array(Some(resources.vao));
                        gl.draw_arrays(glow::LINES, 0, resources.vertex_count);
                        gl.bind_vertex_array(None);

                        // Disable depth test for egui rendering
                        gl.disable(glow::DEPTH_TEST);
                    }
                }
            })),
        };

        ui.painter().add(callback);
    }
}

impl Default for Scene3D {
    fn default() -> Self {
        Self::new()
    }
}
