// Main-pass water surface shader. Samples the offscreen R16Float depth RT
// written by `water_depth.wgsl` in screen space, uses the sampled depth as
// an X coordinate into a per-type ramp texture to pick a color, and uses a
// second noise sample (same `_AlphaAddTex`) to soften the shore alpha.
//
// Phase 3a: no ripple, no reflection, no flow. Base color comes straight
// from the ramp; alpha is a smoothstep of depth, jittered by the noise.

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
  let base_color = sample_ramp(ramp_idx, clamp(d, 0.0, 1.0));

  // Near-shore softening. Depth falls off at the mask-roughened cell edge
  // (see `water_depth.wgsl`), so low-d pixels end up transparent-ish;
  // mid-d pixels pick up the full ramp tint.
  let shore_noise = textureSample(alpha_add_tex, alpha_add_sampler, in.cell_uv).r;
  let alpha = smoothstep(0.05, 0.35, d) * mix(0.9, 1.0, shore_noise);

  return vec4<f32>(base_color, alpha);
}
