struct Camera {
  view_proj: mat4x4<f32>,
};

struct Sprite {
  world_pos: vec3<f32>,
  _pad0: f32,
  size: vec2<f32>,
  _pad1: vec2<f32>,
  tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> sprite: Sprite;

@group(2) @binding(0)
var sprite_tex: texture_2d<f32>;

@group(2) @binding(1)
var sprite_sampler: sampler;

struct VsIn {
  @location(0) pos: vec2<f32>,
  @location(1) uv: vec2<f32>,
};

struct VsOut {
  @builtin(position) clip_pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
  var out: VsOut;
  let world = vec3<f32>(
    sprite.world_pos.x + in.pos.x * sprite.size.x,
    sprite.world_pos.y + in.pos.y * sprite.size.y,
    sprite.world_pos.z
  );
  out.clip_pos = camera.view_proj * vec4<f32>(world, 1.0);
  out.uv = in.uv;
  return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let base = textureSample(sprite_tex, sprite_sampler, in.uv);
  return base * sprite.tint;
}
