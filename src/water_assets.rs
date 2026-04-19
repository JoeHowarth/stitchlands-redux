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
