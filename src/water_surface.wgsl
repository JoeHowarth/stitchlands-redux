// Stub for the Phase 2 plumbing smoke test. Draws water cells in the main
// (swapchain) pass, sampling the offscreen water-depth RT in screen space.
// Pixels where the sampled depth is above the threshold are painted bright
// red; everything else alpha-blends to transparent. That's deliberate — it
// proves the depth RT is written in the first pass and sampled in screen
// space by the surface pass. Phase 3 replaces this with the real ramp +
// ripple + reflection + alpha-add math and adds ramp/noise/ripple bindings.

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
var water_depth_tex: texture_2d<f32>;

@group(1) @binding(1)
var water_depth_sampler: sampler;

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
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let screen_uv = vec2<f32>(
    in.clip_pos.x / camera.screen_width,
    in.clip_pos.y / camera.screen_height
  );
  let depth = textureSample(water_depth_tex, water_depth_sampler, screen_uv).r;
  if (depth > 0.5) {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
  }
  return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
