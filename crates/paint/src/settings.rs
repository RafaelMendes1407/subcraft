//! Brush, material and stroke option types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use subcraft_core::{ChannelKind, ChannelValue, ResourceId};
use subcraft_doc::{Projection, TextureSet};

/// How the brush footprint is laid onto the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    /// Tangent frame at the hit; footprint measured by 3D distance (wraps around curvature).
    #[default]
    TangentWrap,
    /// Tangent plane at the hit; footprint projected onto the plane (depth-limited by the radius).
    TangentPlanar,
    /// Screen-space disc projected from the camera.
    Camera,
    /// Disc in UV space around the hit UV (no 3D visibility test, like painting in the 2D view).
    Uv,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushSettings {
    /// Brush diameter in percent of the mesh bounding-box diagonal (object space).
    pub size: f32,
    /// Coverage each stamp adds (0..1).
    pub flow: f32,
    /// Maximum opacity a single stroke can reach, however many stamps overlap (0..1).
    pub stroke_opacity: f32,
    /// Stamp distance as a fraction of the brush diameter.
    pub spacing: f32,
    /// Fraction of the radius at full strength before the falloff starts (0..1).
    pub hardness: f32,
    pub angle_deg: f32,
    /// Footprint aspect (1 = round; < 1 squashes along the brush's local y axis).
    pub roundness: f32,
    /// Add the screen-space stroke direction to the angle.
    pub follow_path: bool,
    /// Random size reduction per stamp (0..1, fraction of the size).
    pub size_jitter: f32,
    /// Random flow reduction per stamp (0..1).
    pub flow_jitter: f32,
    /// Random angle offset per stamp, ± degrees.
    pub angle_jitter_deg: f32,
    /// Random screen offset per stamp, in brush diameters.
    pub position_jitter: f32,
    pub alignment: Alignment,
    pub backface_culling: bool,
    /// Surfaces turned further than this from the projection direction are not painted.
    pub backface_angle_deg: f32,
    pub seed: u32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            size: 5.0,
            flow: 1.0,
            stroke_opacity: 1.0,
            spacing: 0.1,
            hardness: 0.5,
            angle_deg: 0.0,
            roundness: 1.0,
            follow_path: false,
            size_jitter: 0.0,
            flow_jitter: 0.0,
            angle_jitter_deg: 0.0,
            position_jitter: 0.0,
            alignment: Alignment::TangentWrap,
            backface_culling: true,
            backface_angle_deg: 90.0,
            seed: 1,
        }
    }
}

impl BrushSettings {
    /// World (object-space) radius for a mesh with bounding-box diagonal `diagonal`.
    pub fn radius(&self, diagonal: f32) -> f32 {
        self.size.max(0.0) * 0.01 * diagonal * 0.5
    }
}

/// Brush strength at normalized distance `d` (0 = center, 1 = rim).
#[inline]
pub fn falloff(d: f32, hardness: f32) -> f32 {
    if d >= 1.0 {
        return 0.0;
    }
    let h = hardness.clamp(0.0, 1.0);
    if d <= h || h >= 1.0 {
        return 1.0;
    }
    let t = (d - h) / (1.0 - h);
    1.0 - t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrushChannel {
    pub enabled: bool,
    /// Linear value for color channels.
    pub value: ChannelValue,
}

/// How a normal stroke interacts with normals already painted on this layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalPaintBlend {
    #[default]
    Replace,
    Detail,
}

/// What a paint stroke deposits per channel; masks receive `mask_value`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushMaterial {
    pub channels: BTreeMap<ChannelKind, BrushChannel>,
    /// Optional image sources; channel enable switches still apply. Values remain available
    /// when an image is cleared. Resource identity never depends on a file path.
    pub textures: BTreeMap<ChannelKind, ResourceId>,
    pub projection: Projection,
    /// Repeat images; when false, coordinates outside the image are transparent.
    pub texture_tiling: bool,
    pub normal_blend: NormalPaintBlend,
    /// Project the channel images from the view the stroke started in (like a slide projector)
    /// instead of through `projection`'s UV / planar / tri-planar mapping. `projection`'s
    /// scale, rotation and offset still place the image on the screen.
    pub camera_projection: bool,
    /// Grayscale painted into mask rasters.
    pub mask_value: f32,
}

impl Default for BrushMaterial {
    fn default() -> Self {
        Self { channels: BTreeMap::new(), textures: BTreeMap::new(), projection: Projection::default(), texture_tiling: true,
            normal_blend: NormalPaintBlend::Replace, camera_projection: false, mask_value: 1.0 }
    }
}

impl BrushMaterial {
    /// Every texture-set channel enabled at its default paint value; white for masks.
    pub fn new(ts: &TextureSet) -> Self {
        let channels = ts.channel_kinds().map(|k| (k, BrushChannel { enabled: true, value: k.default_paint_value() })).collect();
        Self { channels, ..Self::default() }
    }

    /// Only the given channels enabled, at the given values.
    pub fn with(values: &[(ChannelKind, ChannelValue)]) -> Self {
        let channels = values.iter().map(|&(k, value)| (k, BrushChannel { enabled: true, value })).collect();
        Self { channels, ..Self::default() }
    }

    pub fn set(&mut self, kind: ChannelKind, value: ChannelValue) {
        self.channels.insert(kind, BrushChannel { enabled: true, value });
    }

    pub fn disable(&mut self, kind: ChannelKind) {
        if let Some(c) = self.channels.get_mut(&kind) {
            c.enabled = false;
        }
    }
}

/// Stroke-level options that shape the stamp path rather than the footprint.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StrokeOptions {
    /// Lazy mouse: the brush trails the cursor by this many pixels (0 = off).
    pub lazy_distance_px: f32,
    /// Mirror planes (X, Y, Z) through the mesh bounds center.
    pub symmetry: [bool; 3],
    /// Copies rotated around an axis (combines with the mirror planes; ADR 0042).
    pub radial: RadialSymmetry,
}

/// Most stroke copies (view + mirrors + radial) one stroke paints; each needs a view-sized
/// depth map, so the radial count shrinks when mirror planes multiply it (ADR 0042).
pub const MAX_SYMMETRY_COPIES: u32 = 64;

/// Radial symmetry: the stroke repeated `count` times around an axis line, copy `k` rotated by
/// `k * 360 / count` degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RadialSymmetry {
    pub enabled: bool,
    /// Rotation axis: 0 = X, 1 = Y, 2 = Z.
    pub axis: u8,
    /// Total copies around the axis, including the original (2..=64 when enabled).
    pub count: u32,
    /// Offset of the axis line from the mesh bounds center (object units).
    pub center_offset: [f32; 3],
}

impl Default for RadialSymmetry {
    fn default() -> Self {
        Self { enabled: false, axis: 1, count: 4, center_offset: [0.0; 3] }
    }
}

impl RadialSymmetry {
    /// Copies actually painted around the axis (1 = off) next to `mirror_planes` enabled
    /// mirror planes: clamped to 1..=64 and so that `count * 2^mirror_planes` stays within
    /// [`MAX_SYMMETRY_COPIES`].
    pub fn effective_count(&self, mirror_planes: usize) -> u32 {
        if !self.enabled { return 1; }
        let limit = (MAX_SYMMETRY_COPIES >> mirror_planes.min(6)).max(1);
        self.count.clamp(1, MAX_SYMMETRY_COPIES).min(limit)
    }

    /// Unit vector of the rotation axis.
    pub fn axis_dir(&self) -> glam::Vec3 {
        let mut d = glam::Vec3::ZERO;
        d[(self.axis as usize).min(2)] = 1.0;
        d
    }
}

impl StrokeOptions {
    /// Number of copies (including the original) every stamp of the stroke produces.
    pub fn copies(&self) -> u32 {
        let mirrors = self.symmetry.iter().filter(|&&m| m).count();
        self.radial.effective_count(mirrors) << mirrors
    }

    /// Whether any symmetry (mirror or radial) is active.
    pub fn has_symmetry(&self) -> bool {
        self.copies() > 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrokeMode {
    #[default]
    Paint,
    /// Reduces paint alpha; on a mask raster, returns texels toward the mask's untouched value.
    Erase,
    /// Drags the paint under the brush along the stroke (enabled channels only).
    Smudge,
    /// Paints what lies at the clone source, offset from where the stroke starts.
    Clone,
}

/// What a paint stroke writes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaintTarget {
    /// The channel images of a paint layer (enabled brush channels only).
    Content,
    /// The base raster of the layer's mask.
    MaskBase,
    /// The rasters of a Paint effect in the layer's content stack (works on fill layers too).
    ContentEffect(subcraft_core::EffectId),
    /// The raster of a Paint effect in the layer's mask stack.
    MaskEffect(subcraft_core::EffectId),
}

impl PaintTarget {
    /// Whether strokes change the mask (grayscale `mask_value`) rather than material channels.
    pub fn is_mask(self) -> bool {
        matches!(self, PaintTarget::MaskBase | PaintTarget::MaskEffect(_))
    }
}

/// Polygon-fill selection grown from the triangle under the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolygonFillMode {
    Triangle,
    MeshElement,
    UvIsland,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_copies_are_clamped_by_mirror_planes() {
        let r = RadialSymmetry { enabled: true, count: 64, ..RadialSymmetry::default() };
        assert_eq!(r.effective_count(0), 64);
        assert_eq!(r.effective_count(1), 32);
        assert_eq!(r.effective_count(3), 8);
        assert_eq!(RadialSymmetry { count: 0, ..r }.effective_count(0), 1);
        assert_eq!(RadialSymmetry { count: 500, ..r }.effective_count(0), 64);
        assert_eq!(RadialSymmetry { enabled: false, ..r }.effective_count(0), 1);
        let all = StrokeOptions { symmetry: [true; 3], radial: r, ..StrokeOptions::default() };
        assert_eq!(all.copies(), MAX_SYMMETRY_COPIES);
        assert_eq!(StrokeOptions { radial: RadialSymmetry { count: 6, ..r }, ..StrokeOptions::default() }.copies(), 6);
        assert!(!StrokeOptions::default().has_symmetry());
        assert_eq!(RadialSymmetry { axis: 9, ..r }.axis_dir(), glam::Vec3::Z);
    }


    #[test]
    fn falloff_shape() {
        assert_eq!(falloff(0.0, 0.5), 1.0);
        assert_eq!(falloff(0.5, 0.5), 1.0);
        assert!((falloff(0.75, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(falloff(1.0, 0.5), 0.0);
        assert_eq!(falloff(0.99, 1.0), 1.0);
        let mut prev = 1.0;
        for i in 0..=100 {
            let v = falloff(i as f32 / 100.0, 0.2);
            assert!(v <= prev + 1e-6);
            prev = v;
        }
    }
}
