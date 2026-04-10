struct Uniforms {
    // [min_time, max_time, min_val, max_val]
    bounds: vec4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,  // [point_size_px, viewport_width_px, viewport_height_px, line_thickness_px]
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> data: array<f32>; // T, V, T, V interleaved

struct LineVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct ScatterVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
};

fn sample_clip_position(idx: u32) -> vec2<f32> {
    // Data is interleaved: [Time0, Val0, Time1, Val1, ...]
    let t = data[idx * 2u];
    let v = data[idx * 2u + 1u];

    let min_t = uniforms.bounds.x;
    let max_t = uniforms.bounds.y;
    let min_v = uniforms.bounds.z;
    let max_v = uniforms.bounds.w;

    // Normalize to 0..1
    let t_norm = (t - min_t) / (max_t - min_t);
    let v_norm = (v - min_v) / (max_v - min_v);

    // Map to Clip Space -1..1
    let x = t_norm * 2.0 - 1.0;
    let y = v_norm * 2.0 - 1.0;

    return vec2<f32>(x, y);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> LineVertexOutput {
    let p0 = sample_clip_position(instance_idx);
    let p1 = sample_clip_position(instance_idx + 1u);
    let viewport = max(uniforms.params.yz, vec2<f32>(1.0, 1.0));
    let delta_px = (p1 - p0) * viewport * 0.5;
    let delta_len = length(delta_px);
    var dir_px = vec2<f32>(1.0, 0.0);
    if delta_len > 1e-4 {
        dir_px = delta_px / delta_len;
    }
    let half_thickness = max(uniforms.params.w * 0.5, 0.5);
    let tangent_clip = vec2<f32>(
        dir_px.x * half_thickness * 2.0 / viewport.x,
        dir_px.y * half_thickness * 2.0 / viewport.y,
    );
    let normal_clip = vec2<f32>(
        -dir_px.y * half_thickness * 2.0 / viewport.x,
        dir_px.x * half_thickness * 2.0 / viewport.y,
    );
    let p0_ext = p0 - tangent_clip;
    let p1_ext = p1 + tangent_clip;
    var clip = p0_ext - normal_clip;
    if vertex_idx == 1u {
        clip = p0_ext + normal_clip;
    } else if vertex_idx == 2u {
        clip = p1_ext - normal_clip;
    } else {
        clip = p1_ext + normal_clip;
    }

    var out: LineVertexOutput;
    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    return out;
}

@vertex
fn vs_scatter(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> ScatterVertexOutput {
    let center = sample_clip_position(instance_idx);
    let half_size_px = uniforms.params.x * 0.5;
    let viewport = max(uniforms.params.yz, vec2<f32>(1.0, 1.0));
    var local_pos = vec2<f32>(-1.0, -1.0);
    if vertex_idx == 1u {
        local_pos = vec2<f32>(1.0, -1.0);
    } else if vertex_idx == 2u {
        local_pos = vec2<f32>(-1.0, 1.0);
    } else if vertex_idx == 3u {
        local_pos = vec2<f32>(1.0, 1.0);
    }
    let clip_offset = vec2<f32>(
        local_pos.x * half_size_px * 2.0 / viewport.x,
        local_pos.y * half_size_px * 2.0 / viewport.y,
    );

    var out: ScatterVertexOutput;
    out.clip_position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    out.local_pos = local_pos;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return uniforms.color;
}

@fragment
fn fs_scatter(@location(0) local_pos: vec2<f32>) -> @location(0) vec4<f32> {
    if dot(local_pos, local_pos) > 1.0 {
        discard;
    }

    return uniforms.color;
}
