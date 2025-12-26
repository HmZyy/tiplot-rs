struct Uniforms {
    // [min_time, max_time, min_val, max_val]
    bounds: vec4<f32>, 
    color: vec4<f32>,
    params: vec4<f32>,  // [point_size, unused, unused, unused]
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> data: array<f32>; // T, V, T, V interleaved

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
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

    var out: VertexOutput;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return uniforms.color;
}
