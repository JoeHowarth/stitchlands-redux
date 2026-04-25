use anyhow::{Result, bail};
use glam::Vec2;

use crate::defs::RgbaColor;
use crate::world::RenderState;

const DEFAULT_SKY_SHADOW_COLOR: RgbaColor = RgbaColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const DEFAULT_SHADOW_ALPHA_SCALE: f32 = 0.55;
const SHADOW_DAY_NIGHT_THRESHOLD: f32 = 0.6;
const SHADOW_GLOW_LERP_SPAN: f32 = 0.15;
const DAY_SHADOW_Z_BASE: f32 = -1.5;
const NIGHT_SHADOW_Z_BASE: f32 = -0.9;
const SHADOW_MAX_LENGTH: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyShadowState {
    pub shadow_vector: Vec2,
    pub sun_glow: f32,
    pub shadow_strength: f32,
    pub shadow_color: RgbaColor,
    pub shadow_alpha_scale: f32,
}

pub fn sky_shadow_state(render: &RenderState) -> Result<SkyShadowState> {
    match (
        render.shadow_vector,
        render.shadow_color,
        render.day_percent,
    ) {
        (Some(shadow_vector), Some(shadow_color), day_percent) => {
            let (sun_glow, shadow_strength) = day_percent
                .map(derived_sun_glow_and_shadow_strength)
                .unwrap_or((1.0, 1.0));
            Ok(SkyShadowState {
                shadow_vector,
                sun_glow,
                shadow_strength,
                shadow_color,
                shadow_alpha_scale: shadow_color.a.clamp(0.0, 1.0),
            })
        }
        (shadow_vector, shadow_color, Some(day_percent)) => {
            let day_percent = day_percent.clamp(0.0, 1.0);
            let (sun_glow, shadow_strength) = derived_sun_glow_and_shadow_strength(day_percent);
            let shadow_vector =
                shadow_vector.unwrap_or_else(|| derived_shadow_vector(day_percent, sun_glow));
            let (shadow_color, shadow_alpha_scale) = match shadow_color {
                Some(shadow_color) => (shadow_color, shadow_color.a.clamp(0.0, 1.0)),
                None => (
                    derived_shadow_color(shadow_strength),
                    DEFAULT_SHADOW_ALPHA_SCALE * shadow_strength,
                ),
            };
            Ok(SkyShadowState {
                shadow_vector,
                sun_glow,
                shadow_strength,
                shadow_color,
                shadow_alpha_scale,
            })
        }
        (Some(_), None, None) => {
            bail!("render.shadow_vector requires render.day_percent or render.shadow_color")
        }
        (None, Some(_), None) => {
            bail!("render.shadow_color requires render.day_percent or render.shadow_vector")
        }
        (None, None, None) => {
            bail!("shadow overlays require render.day_percent or complete explicit shadow state")
        }
    }
}

fn derived_sun_glow_and_shadow_strength(day_percent: f32) -> (f32, f32) {
    let sun_glow = (1.0 - (day_percent - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    let shadow_strength =
        ((sun_glow - SHADOW_DAY_NIGHT_THRESHOLD).abs() / SHADOW_GLOW_LERP_SPAN).clamp(0.0, 1.0);
    (sun_glow, shadow_strength)
}

fn derived_shadow_vector(day_percent: f32, sun_glow: f32) -> Vec2 {
    let t = if sun_glow > SHADOW_DAY_NIGHT_THRESHOLD {
        day_percent
    } else if day_percent > 0.5 {
        inverse_lerp(0.5, 1.0, day_percent) * 0.5
    } else {
        0.5 + inverse_lerp(0.0, 0.5, day_percent) * 0.5
    };
    let z_base = if sun_glow > SHADOW_DAY_NIGHT_THRESHOLD {
        DAY_SHADOW_Z_BASE
    } else {
        NIGHT_SHADOW_Z_BASE
    };
    let x = lerp(-SHADOW_MAX_LENGTH, SHADOW_MAX_LENGTH, t);
    let z = z_base - 2.5 * (x * x / 100.0);
    Vec2::new(x, z)
}

fn derived_shadow_color(shadow_strength: f32) -> RgbaColor {
    lerp_color(RgbaColor::WHITE, DEFAULT_SKY_SHADOW_COLOR, shadow_strength)
}

fn inverse_lerp(min: f32, max: f32, value: f32) -> f32 {
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

fn lerp(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t
}

fn lerp_color(from: RgbaColor, to: RgbaColor, t: f32) -> RgbaColor {
    RgbaColor {
        r: lerp(from.r, to.r, t),
        g: lerp(from.g, to.g, t),
        b: lerp(from.b, to.b, t),
        a: lerp(from.a, to.a, t),
    }
}

#[cfg(test)]
mod tests {
    use super::sky_shadow_state;
    use crate::defs::RgbaColor;
    use crate::world::RenderState;

    fn render_state(
        day_percent: Option<f32>,
        shadow_vector: Option<glam::Vec2>,
        shadow_color: Option<RgbaColor>,
    ) -> RenderState {
        RenderState {
            roofs: Vec::new(),
            fog: Vec::new(),
            snow_depth: Vec::new(),
            day_percent,
            sky_glow: None,
            shadow_color,
            shadow_vector,
            glow_sources: Vec::new(),
        }
    }

    #[test]
    fn derives_daytime_morning_noon_and_evening_vectors() {
        let morning = sky_shadow_state(&render_state(Some(0.35), None, None)).unwrap();
        let noon = sky_shadow_state(&render_state(Some(0.5), None, None)).unwrap();
        let evening = sky_shadow_state(&render_state(Some(0.65), None, None)).unwrap();

        assert!((morning.shadow_vector.x + 4.5).abs() < 0.001);
        assert!((morning.shadow_vector.y + 2.00625).abs() < 0.001);
        assert!(noon.shadow_vector.x.abs() < 0.001);
        assert!((noon.shadow_vector.y + 1.5).abs() < 0.001);
        assert!((evening.shadow_vector.x - 4.5).abs() < 0.001);
        assert!((evening.shadow_vector.y + 2.00625).abs() < 0.001);
    }

    #[test]
    fn derives_night_vector_from_wrapped_moon_path() {
        let midnight = sky_shadow_state(&render_state(Some(0.0), None, None)).unwrap();
        let late_night = sky_shadow_state(&render_state(Some(0.75), None, None)).unwrap();

        assert!(midnight.shadow_vector.x.abs() < 0.001);
        assert!((midnight.shadow_vector.y + 0.9).abs() < 0.001);
        assert!((late_night.shadow_vector.x + 7.5).abs() < 0.001);
        assert!((late_night.shadow_vector.y + 2.30625).abs() < 0.001);
    }

    #[test]
    fn explicit_complete_shadow_state_does_not_require_day_percent() {
        let state = sky_shadow_state(&render_state(
            None,
            Some(glam::Vec2::new(0.5, -0.25)),
            Some(RgbaColor {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 0.4,
            }),
        ))
        .unwrap();

        assert_eq!(state.shadow_vector, glam::Vec2::new(0.5, -0.25));
        assert_eq!(state.shadow_color.r, 0.1);
        assert_eq!(state.shadow_alpha_scale, 0.4);
    }

    #[test]
    fn partial_overrides_require_day_percent_and_override_only_their_field() {
        let vector_only = sky_shadow_state(&render_state(
            Some(0.5),
            Some(glam::Vec2::new(0.5, -0.25)),
            None,
        ))
        .unwrap();
        assert_eq!(vector_only.shadow_vector, glam::Vec2::new(0.5, -0.25));
        assert!(vector_only.shadow_color.r < 0.001);

        let color_only = sky_shadow_state(&render_state(
            Some(0.5),
            None,
            Some(RgbaColor {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 0.4,
            }),
        ))
        .unwrap();
        assert!(color_only.shadow_vector.x.abs() < 0.001);
        assert_eq!(color_only.shadow_color.r, 0.1);
        assert_eq!(color_only.shadow_alpha_scale, 0.4);
    }

    #[test]
    fn missing_required_shadow_inputs_error() {
        assert!(sky_shadow_state(&render_state(None, Some(glam::Vec2::ONE), None)).is_err());
        assert!(sky_shadow_state(&render_state(None, None, Some(RgbaColor::WHITE))).is_err());
        assert!(sky_shadow_state(&render_state(None, None, None)).is_err());
    }

    #[test]
    fn shadow_strength_comes_from_derived_sun_glow() {
        let threshold = sky_shadow_state(&render_state(Some(0.3), None, None)).unwrap();
        let noon = sky_shadow_state(&render_state(Some(0.5), None, None)).unwrap();

        assert!(threshold.shadow_strength < 0.001);
        assert!((threshold.sun_glow - 0.6).abs() < 0.001);
        assert!((noon.shadow_strength - 1.0).abs() < 0.001);
        assert!((noon.sun_glow - 1.0).abs() < 0.001);
    }
}
