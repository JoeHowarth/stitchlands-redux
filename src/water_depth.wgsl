// First half of the two-pass water pipeline. Writes a single float in the
// R channel of the R16Float offscreen RT: the per-type depth constant
// (tint.r, set by `water_shader_params`) modulated by a noise-mask sample
// of `_AlphaAddTex` (RoughAlphaAdd). The surface shader then samples this
// RT in screen space to pick a ramp color and soften the shore.
//
// Phase 3a keeps the math deliberately small — no ripple, no flow offset,
// no sun/moon math. Phase 3b/c add those.

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
  // Base depth per water type (tint.r), roughened by the noise mask so the
  // shore reads as irregular when the surface pass does its smoothstep.
  let depth = in.tint.r * mix(0.75, 1.0, noise);
  return vec4<f32>(depth, 0.0, 0.0, 0.0);
}
