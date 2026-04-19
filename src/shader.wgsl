struct Camera {
  view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var sprite_tex: texture_2d<f32>;

@group(1) @binding(1)
var sprite_sampler: sampler;

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
  @location(0) uv: vec2<f32>,
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
  out.uv = mix(in.uv_rect.xy, in.uv_rect.zw, in.uv);
  out.tint = in.tint;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let base = textureSample(sprite_tex, sprite_sampler, in.uv);
  return base * in.tint;
}
