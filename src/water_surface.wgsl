// Main-pass water surface shader. Samples the offscreen R16Float depth RT
// written by `water_depth.wgsl` in screen space, uses the sampled depth as
// an X coordinate into a per-type ramp texture to pick a base (mud-bed)
// color, and blends in a global sky-reflection texture sampled in world
// space — that reflection blend is the lever that turns the earth-toned
// ramps into something that reads as water. `Other/Ripples` adds a small
// animated distortion so the surface is not a static sky overlay.
//
// Phase 3b/3c: ripple distortion + reflection wired.

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

@group(2) @binding(0)
var alpha_add_tex: texture_2d<f32>;

@group(2) @binding(1)
var alpha_add_sampler: sampler;

@group(3) @binding(0)
var shallow_ramp_tex: texture_2d<f32>;

@group(3) @binding(1)
var deep_ramp_tex: texture_2d<f32>;

@group(3) @binding(2)
var chest_deep_ramp_tex: texture_2d<f32>;

@group(3) @binding(3)
var ramp_sampler: sampler;

@group(3) @binding(4)
var reflection_tex: texture_2d<f32>;

@group(3) @binding(5)
var reflection_sampler: sampler;

@group(3) @binding(6)
var ripple_tex: texture_2d<f32>;

@group(3) @binding(7)
var ripple_sampler: sampler;

// World units per sky-reflection tile. Larger = fewer repeats across the
// map (softer sky look); smaller = tighter tiling.
const REFLECT_SCALE: f32 = 8.0;
// World units per ripple tile. Kept larger than a cell so the effect reads as
// a soft moving surface instead of high-frequency texture noise.
const RIPPLE_SCALE: f32 = 3.5;
// UV displacement applied to the reflection lookup. This is intentionally
// subtle; the ramp still carries the water-depth read.
const RIPPLE_REFLECT_STRENGTH: f32 = 0.035;
// Tiny depth perturbation for tonal shimmer in the ramp lookup.
const RIPPLE_DEPTH_STRENGTH: f32 = 0.03;
// Reflection blend strength at the shallowest vs deepest water. Shallow
// water lets more of the mud-bed ramp read through; deep water mirrors
// more sky.
const REFLECT_MIX_SHALLOW: f32 = 0.35;
const REFLECT_MIX_DEEP: f32 = 0.75;
// RimWorld's `Other/WaterReflection` asset is grayscale cloud luminosity,
// not pre-colored. The real shader must multiply it by a sky color; we do
// the same here. Soft daylight sky with a touch of cyan.
const SKY_TINT: vec3<f32> = vec3<f32>(0.45, 0.65, 0.85);

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
  @location(2) world_xy: vec2<f32>,
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
  out.world_xy = world.xy;
  return out;
}

fn sample_ramp(idx: u32, d: f32) -> vec3<f32> {
  // Ramps are 64×64 with gradient variation on both axes. Sample along a
  // diagonal — depth picks X, and we hash Y off the same value so the
  // subtle vertical variation in the RimWorld ramps reads as subsurface
  // tonal jitter rather than a strict 1D strip.
  let uv = vec2<f32>(d, 0.5);
  if (idx == 0u) {
    return textureSample(shallow_ramp_tex, ramp_sampler, uv).rgb;
  } else if (idx == 1u) {
    return textureSample(deep_ramp_tex, ramp_sampler, uv).rgb;
  }
  return textureSample(chest_deep_ramp_tex, ramp_sampler, uv).rgb;
}

fn sample_ripple(world_xy: vec2<f32>, moving_water: f32, time: f32) -> vec2<f32> {
  let still_flow = vec2<f32>(0.018, -0.012);
  let moving_flow = vec2<f32>(0.075, -0.035);
  let flow = mix(still_flow, moving_flow, moving_water);
  let uv_a = world_xy / RIPPLE_SCALE + flow * time;
  let uv_b = world_xy / (RIPPLE_SCALE * 1.7) - vec2<f32>(0.031, 0.047) * time;
  let a = textureSample(ripple_tex, ripple_sampler, uv_a).rg * 2.0 - vec2<f32>(1.0, 1.0);
  let b = textureSample(ripple_tex, ripple_sampler, uv_b).ba * 2.0 - vec2<f32>(1.0, 1.0);
  return (a * 0.65 + b * 0.35);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  let screen_uv = vec2<f32>(
    in.clip_pos.x / camera.screen_width,
    in.clip_pos.y / camera.screen_height,
  );
  let d = textureSample(water_depth_tex, water_depth_sampler, screen_uv).r;
  if (d <= 0.01) {
    discard;
  }

  let ramp_idx = u32(in.tint.g + 0.5);
  let moving_water = step(0.5, in.tint.b);
  let ripple = sample_ripple(in.world_xy, moving_water, camera.frame_time_seconds);
  let shimmer_depth = clamp(d + ripple.x * RIPPLE_DEPTH_STRENGTH * d, 0.0, 1.0);
  let base_color = sample_ramp(ramp_idx, shimmer_depth);

  // Sky reflection in world space — this is the 3c lever that turns the
  // earth-toned ramp output into something blue and water-like. The asset
  // is grayscale cloud luminosity; we multiply by a sky tint here.
  let reflect_uv = in.world_xy / REFLECT_SCALE + ripple * RIPPLE_REFLECT_STRENGTH;
  let sky_lum = textureSample(reflection_tex, reflection_sampler, reflect_uv).r;
  let sky = SKY_TINT * (0.55 + 0.45 * sky_lum);
  let reflect_strength = mix(REFLECT_MIX_SHALLOW, REFLECT_MIX_DEEP, d);
  let rgb = mix(base_color, sky, reflect_strength);

  // Near-shore softening. Depth falls off at the mask-roughened cell edge
  // (see `water_depth.wgsl`), so low-d pixels end up transparent-ish;
  // mid-d pixels pick up the full ramp+reflection tint.
  let shore_noise = textureSample(alpha_add_tex, alpha_add_sampler, in.cell_uv).r;
  let alpha = smoothstep(0.05, 0.35, d) * mix(0.9, 1.0, shore_noise);

  return vec4<f32>(rgb, alpha);
}
