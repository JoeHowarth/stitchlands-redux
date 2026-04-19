// Terrain-edge overlay shader. Consumes a 9-vertex fan (8 perimeter + 1
// center) per neighbor contribution; alpha is interpolated from per-vertex
// values. The noise texture provides a variation mask for FadeRough / Water
// edges; `Hard` edges alpha-clip at a fixed threshold.

struct Camera {
  view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var sprite_tex: texture_2d<f32>;

@group(1) @binding(1)
var sprite_sampler: sampler;

@group(2) @binding(0)
var noise_tex: texture_2d<f32>;

@group(2) @binding(1)
var noise_sampler: sampler;

struct VsIn {
  @location(0) world_pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) alpha: f32,
  @location(3) noise_seed: vec2<f32>,
  @location(4) tint: vec4<f32>,
  @location(5) edge_type: u32,
};

struct VsOut {
  @builtin(position) clip_pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) tint: vec4<f32>,
  @location(2) alpha: f32,
  @location(3) noise_seed: vec2<f32>,
  @location(4) @interpolate(flat) edge_type: u32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  out.clip_pos = camera.view_proj * vec4<f32>(in.world_pos, 1.0);
  out.uv = in.uv;
  out.tint = in.tint;
  out.alpha = in.alpha;
  out.noise_seed = in.noise_seed;
  out.edge_type = in.edge_type;
  return out;
}

const NOISE_SCALE: f32 = 2.5;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let alpha_dir = in.alpha;
  let base = textureSample(sprite_tex, sprite_sampler, in.uv);
  // Noise in cell-local coords: uv.x grows east, 1-uv.y grows north.
  let local = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
  let noise_uv = local * NOISE_SCALE + in.noise_seed;
  let noise = textureSample(noise_tex, noise_sampler, noise_uv).r;

  var alpha: f32;
  switch in.edge_type {
    case 1u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }
    case 2u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }
    default: { alpha = 0.0; }
  }

  var color = base * in.tint;
  color.a = color.a * alpha;
  return color;
}
