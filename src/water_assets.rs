//! Shared texture assets used by the water rendering pipeline.
//!
//! Mirrors the globals bound by `Verse/TexGame.cs:18-28` and
//! `Verse/WaterInfo.cs:26,50` in the RimWorld decompile, plus the
//! per-terrain surface ramps referenced from `Terrain_Water.xml`. All
//! assets are resolved once at app startup; missing assets degrade to
//! small solid-color fallbacks (logged at `warn` level) so the renderer
//! can still boot.

use image::{Rgba, RgbaImage};
use log::{info, warn};

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

/// Shader-side parameters derived from a `TerrainDef`. Packed into the
/// per-instance `tint` vec4 read by both water pipelines:
/// `tint.r = reflect_strength`, `tint.g = ramp_kind_as_float`,
/// `tint.b = use_offset`, `tint.a` unused.
///
/// `reflect_strength` scales the sky-reflection blend in the surface
/// shader. Deep water reflects more sky; shallow lets the mud-bed ramp
/// read through. The depth RT itself is type-independent — it only
/// encodes shore fade. Per-type depth in the RT would linear-bleed at
/// shallow↔deep cell boundaries and paint a visible band there via the
/// ramp UV.
#[derive(Debug, Clone, Copy)]
pub struct WaterShaderParams {
    pub reflect_strength: f32,
    pub ramp_kind: WaterRampKind,
    pub use_offset: bool,
}

impl WaterShaderParams {
    pub fn to_tint(self) -> [f32; 4] {
        [
            self.reflect_strength,
            self.ramp_kind as u32 as f32,
            if self.use_offset { 1.0 } else { 0.0 },
            0.0,
        ]
    }
}

/// Map a water `TerrainDef` to the per-instance shader parameters.
/// Reflection strength values are approximations — shallow water should
/// show the mud bed, deep water should mirror more sky. `_UseWaterOffset`
/// is read from `water_depth_shader_parameters`. Non-water defs return
/// `None` — callers shouldn't reach this for dry terrain, but it's nice
/// not to panic.
pub fn water_shader_params(def: &crate::defs::TerrainDef) -> Option<WaterShaderParams> {
    def.water_depth_shader.as_ref()?;
    let ramp_kind = match def.def_name.as_str() {
        "WaterDeep" | "WaterOceanDeep" => WaterRampKind::Deep,
        "WaterMovingChestDeep" => WaterRampKind::ChestDeep,
        _ => WaterRampKind::Shallow,
    };
    let reflect_strength = match ramp_kind {
        WaterRampKind::Shallow => 0.35,
        WaterRampKind::Deep => 0.65,
        WaterRampKind::ChestDeep => 0.75,
    };
    let use_offset = def
        .water_depth_shader_parameters
        .iter()
        .any(|(name, value)| name == "_UseWaterOffset" && *value > 0.0);
    Some(WaterShaderParams {
        reflect_strength,
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
            ripple: resolve_or_fallback(resolver, RIPPLE_TEX_PATH, solid_gray())?,
            reflection: resolve_or_fallback(resolver, WATER_REFLECTION_TEX_PATH, solid_sky_blue())?,
            shallow_ramp: resolve_or_fallback(
                resolver,
                WATER_SHALLOW_RAMP_PATH,
                solid_shallow_water(),
            )?,
            deep_ramp: resolve_or_fallback(resolver, WATER_DEEP_RAMP_PATH, solid_deep_water())?,
            chest_deep_ramp: resolve_or_fallback(
                resolver,
                WATER_CHEST_DEEP_RAMP_PATH,
                solid_chest_deep_water(),
            )?,
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

fn resolve_or_fallback(
    resolver: &mut AssetResolver,
    path: &str,
    fallback: RgbaImage,
) -> anyhow::Result<RgbaImage> {
    let resolved = resolver.resolve_texture_path(path)?;
    if resolved.used_fallback() {
        warn!("water asset '{path}' not resolved; using solid-color fallback");
        Ok(fallback)
    } else {
        Ok(resolved.image)
    }
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> RgbaImage {
    RgbaImage::from_pixel(1, 1, Rgba([r, g, b, a]))
}

fn solid_gray() -> RgbaImage {
    solid(128, 128, 128, 255)
}

fn solid_sky_blue() -> RgbaImage {
    solid(128, 176, 224, 255)
}

fn solid_shallow_water() -> RgbaImage {
    solid(72, 124, 160, 255)
}

fn solid_deep_water() -> RgbaImage {
    solid(40, 80, 128, 255)
}

fn solid_chest_deep_water() -> RgbaImage {
    solid(28, 56, 96, 255)
}
