// Whole-pixel zoom of the match frame.
//
// Each source pixel becomes a `factor` x `factor` block of the window, the
// first one at `origin`. The draw's scissor covers exactly the scaled frame;
// the rest of the window was cleared to black.

struct Params {
    origin: vec2f,   // window pixel where source pixel (0, 0) lands
    factor: f32,     // whole-pixel scale
    _pad: f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4f {
    // One triangle that covers the whole target.
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);
    return vec4f(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    // `position.xy` is the pixel centre, so the floor names the block.
    let texel = vec2i(floor((position.xy - params.origin) / params.factor));
    let last = vec2i(textureDimensions(t_source)) - 1;
    return textureLoad(t_source, clamp(texel, vec2i(0), last), 0);
}
