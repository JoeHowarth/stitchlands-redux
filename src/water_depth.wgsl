// Stub for the Phase 2 plumbing smoke test. Mirrors the quad/instance
// layout of `shader.wgsl` so the same vertex/instance buffers can feed it,
// but the fragment just writes 1.0 into the red channel of the R16Float
// offscreen target. Phase 3 replaces the fragment with the real
// depth-modulation math (constants per water type + `_AlphaAddTex` mask +
// optional `_UseWaterOffset` flow distortion).

struct Camera {
  view_proj: mat4x4<f32>,
  frame_time_seconds: f32,
  screen_width: f32,
  screen_height: f32,
  _pad0: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

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
  // Phase 2 stub: constant 1.0 in R so the surface shader's sample test
  // lights up every water pixel. Phase 3 will modulate this by depth
  // constants, an alpha-add noise mask, and optional flow offset.
  return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}
