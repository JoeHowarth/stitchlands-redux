// First half of the two-pass water pipeline. Writes a single float in the
// R channel of the R16Float offscreen RT: a **type-independent** shore
// indicator — ~1.0 inside water, 0 outside (non-water cells never draw
// here), with noise-mask jitter so the shore reads as irregular when the
// surface pass does its smoothstep. Per-water-type color variation lives
// in the ramp choice, not in this value; mixing shallow-vs-deep into the
// same RT produces a visible brown band at shallow↔deep cell boundaries
// via linear-sampler bleed.
//
// Phase 3a/3c: no ripple, no flow offset, no sun/moon math. 3b adds those.

struct Camera {
  view_proj: mat4x4<f32>,
  frame_time_seconds: f32,
  screen_width: f32,
  screen_height: f32,
  _pad0: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var alpha_add_tex: texture_2d<f32>;

@group(1) @binding(1)
var alpha_add_sampler: sampler;

struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) world_pos: vec3<f32>,
  @location(3) size: vec2<f32>,
  @location(4) tint: vec4<f32>,
  @location(5) uv_rect: vec4<f32>,
};

struct VsOut {
  @builtin(position) clip_pos: vec4<f32>,
  @location(0) cell_uv: vec2<f32>,
  @location(1) tint: vec4<f32>,
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
  out.cell_uv = in.uv;
  out.tint = in.tint;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let noise = textureSample(alpha_add_tex, alpha_add_sampler, in.cell_uv).r;
  // Uniform across water types — type-specific color differences come
  // from the ramp choice + per-type reflection strength in the surface
  // pass. The noise jitter gives the shore an irregular roughened edge.
  let depth = mix(0.75, 1.0, noise);
  return vec4<f32>(depth, 0.0, 0.0, 0.0);
}
