struct Uniforms {
    // [min_time, max_time, min_val, max_val]
    bounds: vec4<f32>,
    color: vec4<f32>,
    // line mode:    [point_count_f32, viewport_width_px, viewport_height_px, line_thickness_px]
    // scatter mode: [point_size_px,   viewport_width_px, viewport_height_px, line_thickness_px]
    params: vec4<f32>,
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
    let t = data[idx * 2u];
    let v = data[idx * 2u + 1u];

    let t_norm = (t - uniforms.bounds.x) / (uniforms.bounds.y - uniforms.bounds.x);
    let v_norm = (v - uniforms.bounds.z) / (uniforms.bounds.w - uniforms.bounds.z);

    return vec2<f32>(t_norm * 2.0 - 1.0, v_norm * 2.0 - 1.0);
}

// Thick polyline with miter joins.
//
// Each instance draws the quad for segment [instance_idx, instance_idx+1].
// It reads the previous point (instance_idx-1) and next point (instance_idx+2)
// from the storage buffer to compute miter angles at each endpoint, so adjacent
// quads share exact vertices — no gaps at joins.
//
// uniforms.params.x must carry the total point count (as f32) so the shader
// can avoid out-of-bounds reads at the first and last segments.
@vertex
fn vs_miter(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> LineVertexOutput {
    let count       = u32(uniforms.params.x);
    let viewport    = max(uniforms.params.yz, vec2<f32>(1.0, 1.0));
    let half_thick  = max(uniforms.params.w * 0.5, 0.5);

    let p0 = sample_clip_position(instance_idx);
    let p1 = sample_clip_position(instance_idx + 1u);

    // Work in tile-pixel space for numerically stable direction/normal maths.
    let p0_px = p0 * (viewport * 0.5);
    let p1_px = p1 * (viewport * 0.5);

    let seg     = p1_px - p0_px;
    let seg_len = length(seg);
    var curr_dir = vec2<f32>(1.0, 0.0);
    if seg_len > 1e-4 {
        curr_dir = seg / seg_len;
    }
    let curr_norm = vec2<f32>(-curr_dir.y, curr_dir.x);

    // Miter at the START vertex (shared with the end of the previous segment).
    var start_off = curr_norm * half_thick;
    if instance_idx > 0u {
        let prev_px = sample_clip_position(instance_idx - 1u) * (viewport * 0.5);
        let pd      = p0_px - prev_px;
        let pd_len  = length(pd);
        if pd_len > 1e-4 {
            let prev_norm = vec2<f32>(-(pd.y / pd_len), pd.x / pd_len);
            let miter     = normalize(prev_norm + curr_norm);
            let scale     = min(1.0 / max(dot(miter, curr_norm), 0.1), 4.0);
            start_off     = miter * (half_thick * scale);
        }
    }

    // Miter at the END vertex (shared with the start of the next segment).
    var end_off = curr_norm * half_thick;
    if instance_idx + 2u < count {
        let next_px = sample_clip_position(instance_idx + 2u) * (viewport * 0.5);
        let nd      = next_px - p1_px;
        let nd_len  = length(nd);
        if nd_len > 1e-4 {
            let next_norm = vec2<f32>(-(nd.y / nd_len), nd.x / nd_len);
            let miter     = normalize(curr_norm + next_norm);
            let scale     = min(1.0 / max(dot(miter, curr_norm), 0.1), 4.0);
            end_off       = miter * (half_thick * scale);
        }
    }

    // Convert pixel-space offsets back to clip space.
    let s = vec2<f32>(start_off.x * 2.0 / viewport.x, start_off.y * 2.0 / viewport.y);
    let e = vec2<f32>(end_off.x   * 2.0 / viewport.x, end_off.y   * 2.0 / viewport.y);

    var clip: vec2<f32>;
    if vertex_idx == 0u {
        clip = p0 - s;
    } else if vertex_idx == 1u {
        clip = p0 + s;
    } else if vertex_idx == 2u {
        clip = p1 - e;
    } else {
        clip = p1 + e;
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
    let center       = sample_clip_position(instance_idx);
    let half_size_px = uniforms.params.x * 0.5;
    let viewport     = max(uniforms.params.yz, vec2<f32>(1.0, 1.0));
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
    out.local_pos     = local_pos;
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
