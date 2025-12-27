use eframe::glow;
use std::collections::HashMap;
use std::sync::Arc;

const VERTEX_SHADER: &str = r#"
#version 330 core
layout(location = 0) in vec2 a_data;
uniform vec4 u_bounds;

void main() {
    float t = a_data.x;
    float v = a_data.y;

    float min_t = u_bounds.x;
    float max_t = u_bounds.y;
    float min_v = u_bounds.z;
    float max_v = u_bounds.w;

    float t_norm = (t - min_t) / (max_t - min_t);
    float v_norm = (v - min_v) / (max_v - min_v);

    float x = t_norm * 2.0 - 1.0;
    float y = v_norm * 2.0 - 1.0;

    gl_Position = vec4(x, y, 0.0, 1.0);
    gl_PointSize = 5.0;
}
"#;

const FRAGMENT_SHADER: &str = r#"
#version 330 core
out vec4 FragColor;
uniform vec4 u_color;

void main() {
    FragColor = u_color;
}
"#;

pub struct TraceGpuResource {
    pub vbo: glow::Buffer,
    pub vao: glow::VertexArray,
    pub count: i32,
}

pub struct PlotRenderer {
    gl: Arc<glow::Context>,
    shader_program: glow::Program,
    buffers: HashMap<String, TraceGpuResource>,
}

impl PlotRenderer {
    pub fn new(gl: Arc<glow::Context>) -> Self {
        unsafe {
            let shader_program = Self::create_shader_program(&gl);

            Self {
                gl,
                shader_program,
                buffers: HashMap::new(),
            }
        }
    }

    unsafe fn create_shader_program(gl: &glow::Context) -> glow::Program {
        use glow::HasContext as _;

        let vertex_shader = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vertex_shader, VERTEX_SHADER);
        gl.compile_shader(vertex_shader);

        if !gl.get_shader_compile_status(vertex_shader) {
            panic!(
                "Vertex shader compilation failed: {}",
                gl.get_shader_info_log(vertex_shader)
            );
        }

        let fragment_shader = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fragment_shader, FRAGMENT_SHADER);
        gl.compile_shader(fragment_shader);

        if !gl.get_shader_compile_status(fragment_shader) {
            panic!(
                "Fragment shader compilation failed: {}",
                gl.get_shader_info_log(fragment_shader)
            );
        }

        let program = gl.create_program().unwrap();
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.link_program(program);

        if !gl.get_program_link_status(program) {
            panic!(
                "Shader program linking failed: {}",
                gl.get_program_info_log(program)
            );
        }

        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);

        program
    }

    pub fn upload_trace(&mut self, topic: &str, col: &str, times: &[f32], values: &[f32]) {
        use glow::HasContext as _;

        let key = format!("{}/{}", topic, col);

        if times.is_empty() || values.is_empty() {
            return;
        }

        // Interleave times and values: [T0, V0, T1, V1, T2, V2, ...]
        let data: Vec<f32> = times
            .iter()
            .zip(values.iter())
            .flat_map(|(t, v)| [*t, *v])
            .collect();

        unsafe {
            let vao = self.gl.create_vertex_array().unwrap();
            self.gl.bind_vertex_array(Some(vao));

            let vbo = self.gl.create_buffer().unwrap();
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&data),
                glow::STATIC_DRAW,
            );

            // Configure vertex attribute (location 0: vec2)
            self.gl.enable_vertex_attrib_array(0);
            self.gl.vertex_attrib_pointer_f32(
                0,                                     // location
                2,                                     // size (vec2)
                glow::FLOAT,                           // type
                false,                                 // normalized
                2 * std::mem::size_of::<f32>() as i32, // stride
                0,                                     // offset
            );

            self.gl.bind_vertex_array(None);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);

            self.buffers.insert(
                key,
                TraceGpuResource {
                    vbo,
                    vao,
                    count: times.len() as i32,
                },
            );
        }
    }

    pub fn render_trace(
        &self,
        topic: &str,
        col: &str,
        bounds: [f32; 4],
        color: [f32; 4],
        scatter_mode: bool,
    ) {
        use glow::HasContext as _;

        let key = format!("{}/{}", topic, col);

        if let Some(trace) = self.buffers.get(&key) {
            unsafe {
                self.gl.use_program(Some(self.shader_program));

                // Set uniforms
                let bounds_loc = self
                    .gl
                    .get_uniform_location(self.shader_program, "u_bounds");
                self.gl.uniform_4_f32(
                    bounds_loc.as_ref(),
                    bounds[0],
                    bounds[1],
                    bounds[2],
                    bounds[3],
                );

                let color_loc = self.gl.get_uniform_location(self.shader_program, "u_color");
                self.gl
                    .uniform_4_f32(color_loc.as_ref(), color[0], color[1], color[2], color[3]);

                // Draw
                self.gl.bind_vertex_array(Some(trace.vao));

                if scatter_mode {
                    self.gl.draw_arrays(glow::POINTS, 0, trace.count);
                } else {
                    self.gl.draw_arrays(glow::LINE_STRIP, 0, trace.count);
                }

                self.gl.bind_vertex_array(None);
            }
        }
    }
}

impl Drop for PlotRenderer {
    fn drop(&mut self) {
        use glow::HasContext as _;

        unsafe {
            for (_, resource) in self.buffers.drain() {
                self.gl.delete_buffer(resource.vbo);
                self.gl.delete_vertex_array(resource.vao);
            }
            self.gl.delete_program(self.shader_program);
        }
    }
}
