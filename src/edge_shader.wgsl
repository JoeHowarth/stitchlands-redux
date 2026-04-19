// Terrain-edge overlay shader. Samples the neighbor terrain texture inside the
// current cell and alpha-fades from the edge(s) matching `edge_mask` inward.
// The noise texture provides a variation mask for FadeRough / Water edges;
// `Hard` edges ignore noise and alpha-clip at a fixed threshold.

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
  @location(0) pos: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) world_pos: vec3<f32>,
  @location(3) size: vec2<f32>,
  @location(4) edge_mask: vec4<f32>,
  @location(5) tint: vec4<f32>,
  @location(6) edge_params: vec4<f32>,
};

struct VsOut {
  @builtin(position) clip_pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) tint: vec4<f32>,
  @location(2) edge_mask: vec4<f32>,
  @location(3) edge_params: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  let world = vec3<f32>(
    in.world_pos.x + in.pos.x * in.size.x,
    in.world_pos.y + in.pos.y * in.size.y,
    in.world_pos.z
  );
  out.clip_pos = camera.view_proj * vec4<f32>(world, 1.0);
  out.uv = in.uv;
  out.tint = in.tint;
  out.edge_mask = in.edge_mask;
  out.edge_params = in.edge_params;
  return out;
}

const FADE_WIDTH: f32 = 0.35;
const NOISE_SCALE: f32 = 2.5;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  // Quad vertices map uv.y=1 to the bottom (south) and uv.y=0 to the top
  // (north), matching the sprite shader's convention. `local` re-orients so
  // local.y grows northward — aligning mask.x with "north-edge strength".
  let local = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
  let sN = in.edge_mask.x * (1.0 - smoothstep(0.0, FADE_WIDTH, 1.0 - local.y));
  let sE = in.edge_mask.y * (1.0 - smoothstep(0.0, FADE_WIDTH, 1.0 - local.x));
  let sS = in.edge_mask.z * (1.0 - smoothstep(0.0, FADE_WIDTH, local.y));
  let sW = in.edge_mask.w * (1.0 - smoothstep(0.0, FADE_WIDTH, local.x));
  let alpha_dir = max(max(sN, sE), max(sS, sW));

  let base = textureSample(sprite_tex, sprite_sampler, in.uv);
  let noise_uv = local * NOISE_SCALE + in.edge_params.xy;
  let noise = textureSample(noise_tex, noise_sampler, noise_uv).r;

  let edge_type = u32(in.edge_params.z + 0.5);
  var alpha: f32;
  switch edge_type {
    case 0u: { alpha = select(0.0, 1.0, alpha_dir > 0.5); }
    case 1u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }
    case 2u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }
    default: { alpha = 0.0; }
  }

  var color = base * in.tint;
  color.a = color.a * alpha;
  return color;
}
