//! Shared texture assets used by the water rendering pipeline.
//!
//! Mirrors the globals bound by `Verse/TexGame.cs:18-28` and
//! `Verse/WaterInfo.cs:26,50` in the RimWorld decompile, plus the
//! per-terrain surface ramps referenced from `Terrain_Water.xml`. All
//! assets are resolved once at app startup; missing assets are hard errors so
//! visual checks do not silently run against placeholder textures.

use anyhow::Context;
use image::RgbaImage;
use log::info;

use crate::assets::AssetResolver;

/// Packed paths. Kept RimWorld-native so they grep against the decompile;
/// the packed resolver matches on basename anyway (see
/// `src/assets/variants.rs`).
pub const RIPPLE_TEX_PATH: &str = "Other/Ripples";
pub const WATER_REFLECTION_TEX_PATH: &str = "Other/WaterReflection";
pub const WATER_SHALLOW_RAMP_PATH: &str = "Terrain/Surfaces/WaterShallowRamp";
pub const WATER_DEEP_RAMP_PATH: &str = "Terrain/Surfaces/WaterDeepRamp";
pub const WATER_CHEST_DEEP_RAMP_PATH: &str = "Terrain/Surfaces/WaterChestDeepRamp";

/// Which packed ramp to sample in the water-surface shader. Layout matches
/// the order the renderer uploads the three ramps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterRampKind {
    Shallow = 0,
    Deep = 1,
    ChestDeep = 2,
}

/// Shader-side parameters derived from a `TerrainDef`. These get packed into
/// the per-instance `tint` vec4 the water pipelines read in the vertex stage:
/// `tint.r = depth_const`, `tint.g = ramp_kind_as_float`, `tint.b = use_offset`.
/// Depth constant is arbitrary per the plan — shallow water writes a smaller
/// depth value into the offscreen RT than deep, giving the surface shader's
/// ramp lookup a distinguishable X coordinate.
#[derive(Debug, Clone, Copy)]
pub struct WaterShaderParams {
    pub depth_const: f32,
    pub ramp_kind: WaterRampKind,
    pub use_offset: bool,
}

impl WaterShaderParams {
    pub fn to_tint(self) -> [f32; 4] {
        [
            self.depth_const,
            self.ramp_kind as u32 as f32,
            if self.use_offset { 1.0 } else { 0.0 },
            0.0,
        ]
    }
}

/// Map a water `TerrainDef` to the per-instance shader parameters. The
/// depth constants are the approximations in
/// `plans/water-rendering/plan.md` §5 Phase 3; `_UseWaterOffset` is read
/// from `water_depth_shader_parameters`. Non-water defs return `None` —
/// callers shouldn't reach this for dry terrain, but it's nice not to
/// panic.
pub fn water_shader_params(def: &crate::defs::TerrainDef) -> Option<WaterShaderParams> {
    def.water_depth_shader.as_ref()?;
    let ramp_kind = match def.def_name.as_str() {
        "WaterDeep" | "WaterOceanDeep" => WaterRampKind::Deep,
        "WaterMovingChestDeep" => WaterRampKind::ChestDeep,
        _ => WaterRampKind::Shallow,
    };
    let depth_const = match ramp_kind {
        WaterRampKind::Shallow => 0.35,
        WaterRampKind::Deep => 0.75,
        WaterRampKind::ChestDeep => 0.9,
    };
    let use_offset = def
        .water_depth_shader_parameters
        .iter()
        .any(|(name, value)| name == "_UseWaterOffset" && *value > 0.0);
    Some(WaterShaderParams {
        depth_const,
        ramp_kind,
        use_offset,
    })
}

/// All shared water textures resolved at boot.
pub struct WaterAssets {
    pub ripple: RgbaImage,
    pub reflection: RgbaImage,
    pub shallow_ramp: RgbaImage,
    pub deep_ramp: RgbaImage,
    pub chest_deep_ramp: RgbaImage,
}

impl WaterAssets {
    pub fn load(resolver: &mut AssetResolver) -> anyhow::Result<Self> {
        let loaded = Self {
            ripple: resolve_required(resolver, RIPPLE_TEX_PATH)?,
            reflection: resolve_required(resolver, WATER_REFLECTION_TEX_PATH)?,
            shallow_ramp: resolve_required(resolver, WATER_SHALLOW_RAMP_PATH)?,
            deep_ramp: resolve_required(resolver, WATER_DEEP_RAMP_PATH)?,
            chest_deep_ramp: resolve_required(resolver, WATER_CHEST_DEEP_RAMP_PATH)?,
        };
        info!(
            "water assets loaded: ripple={}x{} reflection={}x{} ramps(shallow={}x{}, deep={}x{}, chest_deep={}x{})",
            loaded.ripple.width(),
            loaded.ripple.height(),
            loaded.reflection.width(),
            loaded.reflection.height(),
            loaded.shallow_ramp.width(),
            loaded.shallow_ramp.height(),
            loaded.deep_ramp.width(),
            loaded.deep_ramp.height(),
            loaded.chest_deep_ramp.width(),
            loaded.chest_deep_ramp.height(),
        );
        Ok(loaded)
    }
}

fn resolve_required(resolver: &mut AssetResolver, path: &str) -> anyhow::Result<RgbaImage> {
    let resolved = resolver
        .resolve_texture_path(path)
        .with_context(|| format!("resolving water asset '{path}'"))?;
    if resolved.used_fallback() {
        anyhow::bail!("missing water asset texture '{path}'");
    }
    Ok(resolved.image)
}
