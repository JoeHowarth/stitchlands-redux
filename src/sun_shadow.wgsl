struct Camera {
  view_proj: mat4x4<f32>,
  frame_time_seconds: f32,
  screen_width: f32,
  screen_height: f32,
  _pad0: f32,
};

struct SunShadow {
  cast_vector: vec4<f32>,
  material_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> sun_shadow: SunShadow;

struct VsIn {
  @location(0) world_pos: vec3<f32>,
  @location(1) color: vec4<f32>,
};

struct VsOut {
  @builtin(position) clip_pos: vec4<f32>,
  @location(0) shadow_height: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  let height = clamp(in.color.a, 0.0, 1.0);
  let displaced = vec3<f32>(
    in.world_pos.x + sun_shadow.cast_vector.x * height,
    in.world_pos.y + sun_shadow.cast_vector.y * height,
    in.world_pos.z
  );

  var out: VsOut;
  out.clip_pos = camera.view_proj * vec4<f32>(displaced, 1.0);
  out.shadow_height = height;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let alpha = clamp(in.shadow_height * sun_shadow.material_color.a, 0.0, 1.0);
  return vec4<f32>(sun_shadow.material_color.rgb * alpha, alpha);
}
