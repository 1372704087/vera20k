//! Phobos building placement preview artwork.
//!
//! Native identity: `TacticalClass_DrawPlacement_PlacementPreview @ 0x6D528A`
//! (`src/Ext/BuildingType/Hooks.cpp`). The image is
//! `PlacementPreview.Shape` when it loads, otherwise the building's buildup
//! SHP at `Frames / 2 - 1`, otherwise the body SHP at frame 0. The draw is
//! centered at the placement cell plus `PlacementPreview.Offset`; `Remap=yes`
//! uses the placing house's drawer (theater unit palette with its colour
//! ramp), and `Remap=no` uses `PlacementPreview.Palette` or the unit palette.
//! Translucency is the per-type/global `TranslucencyLevel`.
//!
//! VERA bakes the resolved palette into the strip's RGBA once per (type,
//! shape, palette, house colour) and draws it through the placement overlay
//! material, which carries the instance alpha.

use std::collections::HashMap;

use crate::assets::asset_manager::AssetManager;
use crate::assets::pal_file::Palette;
use crate::assets::shp_file::ShpFile;
use crate::render::batch::{BatchRenderer, BatchTexture, SpriteInstance};
use crate::render::selection_overlay::CenteredShapeStrip;
use crate::rules::house_colors::{HouseColorIndex, NO_REMAP};
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::production::BuildingPlacementPreview;

#[derive(Default)]
pub(crate) struct PlacementPreviewArt {
    cache: HashMap<PreviewKey, Option<CachedPreview>>,
    current: Option<PreviewKey>,
    unit_palette: Option<Palette>,
    unit_palette_loaded: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct PreviewKey {
    type_id: String,
    shape: Option<String>,
    palette: Option<String>,
    remap: Option<HouseColorIndex>,
    theater_name: String,
    theater_ext: String,
}

struct CachedPreview {
    strip: CenteredShapeStrip,
    /// Buildup's last first-half frame, or 0 for a custom shape/body.
    default_frame: usize,
}

impl PlacementPreviewArt {
    /// Map loads replace the theater and house colours; drop the old strips.
    pub(crate) fn reset(&mut self) {
        self.cache.clear();
        self.current = None;
        self.unit_palette = None;
        self.unit_palette_loaded = false;
    }

    /// The texture belonging to the instance most recently returned by
    /// [`Self::build_instance`].
    pub(crate) fn current_texture(&self) -> Option<&BatchTexture> {
        self.cache
            .get(self.current.as_ref()?)?
            .as_ref()
            .map(|cached| cached.strip.texture())
    }

    /// Build the one preview instance for this frame, loading and retaining
    /// the resolved SHP on first use.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_instance(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &BatchRenderer,
        assets: &AssetManager,
        rules: &RuleSet,
        type_id: &str,
        object: &ObjectType,
        theater_name: &str,
        theater_ext: &str,
        remap_house: Option<HouseColorIndex>,
        point: (f32, f32),
        alpha: f32,
    ) -> Option<SpriteInstance> {
        let key = PreviewKey {
            type_id: type_id.to_owned(),
            shape: object.placement_preview_shape().map(str::to_owned),
            palette: object.placement_preview_palette().map(str::to_owned),
            remap: remap_house,
            theater_name: theater_name.to_owned(),
            theater_ext: theater_ext.to_owned(),
        };
        if !self.cache.contains_key(&key) {
            let loaded = self.load_preview(
                device,
                queue,
                batch,
                assets,
                rules,
                type_id,
                object,
                theater_name,
                theater_ext,
                remap_house,
            );
            self.cache.insert(key.clone(), loaded);
        }
        let cached = self.cache.get(&key)?.as_ref()?;
        let frames = cached.strip.frame_count();
        if frames == 0 {
            return None;
        }
        let last = frames - 1;
        let frame = object
            .placement_preview_shape_frame()
            .map_or(cached.default_frame, |frame| frame.max(0) as usize)
            .min(last);
        let mut instance = cached.strip.instance(frame, point, 0.0)?;
        instance.alpha = alpha;
        self.current = Some(key);
        Some(instance)
    }

    #[allow(clippy::too_many_arguments)]
    fn load_preview(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &BatchRenderer,
        assets: &AssetManager,
        rules: &RuleSet,
        type_id: &str,
        object: &ObjectType,
        theater_name: &str,
        theater_ext: &str,
        remap_house: Option<HouseColorIndex>,
    ) -> Option<CachedPreview> {
        if !self.unit_palette_loaded {
            self.unit_palette = load_unit_palette(assets, theater_ext);
            self.unit_palette_loaded = true;
        }
        let unit_palette = self
            .unit_palette
            .clone()
            .or_else(|| load_named_palette(assets, "palette.pal"))?;
        let palette = if object.placement_preview_remap() {
            match remap_house {
                Some(house) if house != NO_REMAP => {
                    unit_palette.with_house_colors(rules.house_color_ramps.ramp(house))
                }
                _ => unit_palette,
            }
        } else {
            object
                .placement_preview_palette()
                .and_then(|name| {
                    let name = with_extension(&theater_name_with_suffix(name, theater_ext), "pal");
                    load_named_palette(assets, &name)
                })
                .unwrap_or(unit_palette)
        };

        // A missing or unreadable custom Shape falls through to the building's
        // own buildup/body, as Phobos' null TheaterSpecificSHP does.
        if let Some(name) = object.placement_preview_shape() {
            let name = with_extension(&theater_name_with_suffix(name, theater_ext), "shp");
            if let Some(strip) = load_strip(device, queue, batch, assets, &[name], &palette) {
                return Some(CachedPreview {
                    strip,
                    default_frame: 0,
                });
            }
        }

        let art = rules.art();
        let entry = art.resolve_metadata_entry(type_id, &object.image);
        let buildup_candidates = entry
            .and_then(|entry| entry.buildup.as_deref())
            .map(|buildup| {
                crate::rules::art_data::building_shp_names(buildup, theater_name).to_vec()
            })
            .unwrap_or_default();
        if let Some(strip) = load_strip(device, queue, batch, assets, &buildup_candidates, &palette)
        {
            // `LoadBuildup`'s last frame is `Frames / 2 - 1`; the SHP's second
            // half is its shadow. One-frame/empty art clamps to frame 0.
            let default_frame = (strip.frame_count() / 2).saturating_sub(1);
            return Some(CachedPreview {
                strip,
                default_frame,
            });
        }

        let image_id = art.resolve_effective_image_id(type_id, &object.image);
        let body_candidates = crate::rules::art_data::building_body_shp_candidates(
            Some(art),
            &object.image,
            &image_id,
            theater_ext,
            theater_name,
        );
        let strip = load_strip(device, queue, batch, assets, &body_candidates, &palette)?;
        Some(CachedPreview {
            strip,
            default_frame: 0,
        })
    }
}

/// `CellClass::Cell2Coord(cell, floor + offset.Z)` projected by
/// `CoordsToClient`, then the offset's X/Y pixels. `preview.rx/ry` is the
/// foundation origin: the native hook's `0x88095C` center plus `0x880960`
/// offset is the marker origin recorded by
/// `tools/spatial_oracle/placement_render.py` (`placement_render.json`).
pub(crate) fn preview_screen_point(
    preview: &BuildingPlacementPreview,
    offset: [i32; 3],
) -> Option<(f32, f32)> {
    let anchor = (preview.rx, preview.ry);
    let level = preview
        .cells
        .iter()
        .find(|cell| cell.coord == anchor)
        .or_else(|| preview.cells.first())
        .map(|cell| cell.level)?;
    let z = i32::from(level)
        .wrapping_mul(crate::util::lepton::GROUND_LEVEL_HEIGHT_LEPTONS)
        .wrapping_add(offset[2]);
    let center = crate::sim::cell_kernel::cell_center(
        crate::sim::cell_kernel::CellCoordinate {
            x: i32::from(preview.rx),
            y: i32::from(preview.ry),
        },
        z,
    );
    let (sx, sy) = crate::util::lepton::absolute_leptons_to_screen(center.x, center.y, center.z);
    Some((sx + offset[0] as f32, sy + offset[1] as f32))
}

/// Phobos `ApplyTheaterSuffixToString` plus the `.shp`/`.pal` extension rule:
/// the first `~~~` becomes the active theater extension (e.g. `tem`).
fn theater_name_with_suffix(name: &str, theater_ext: &str) -> String {
    name.replacen("~~~", theater_ext, 1)
}

fn with_extension(name: &str, extension: &str) -> String {
    if name
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        name.to_owned()
    } else {
        format!("{name}.{extension}")
    }
}

fn load_named_palette(assets: &AssetManager, name: &str) -> Option<Palette> {
    Palette::from_bytes(&assets.get(name)?).ok()
}

fn load_unit_palette(assets: &AssetManager, theater_ext: &str) -> Option<Palette> {
    let ext = theater_ext.to_ascii_lowercase();
    let mut names = vec![format!("unit{ext}.pal"), "unit.pal".to_owned()];
    for fallback in ["unittem.pal", "unitsno.pal", "uniturb.pal", "palette.pal"] {
        names.push(fallback.to_owned());
    }
    names
        .into_iter()
        .find_map(|name| load_named_palette(assets, &name))
}

fn load_strip(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &BatchRenderer,
    assets: &AssetManager,
    candidates: &[String],
    palette: &Palette,
) -> Option<CenteredShapeStrip> {
    for name in candidates {
        let Some(bytes) = assets.get(name) else {
            continue;
        };
        let Ok(shp) = ShpFile::from_bytes(&bytes) else {
            continue;
        };
        if let Some(strip) =
            CenteredShapeStrip::from_shp_on_device(device, queue, batch, &shp, palette)
        {
            return Some(strip);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::production::{BuildingPlacementCell, BuildingPlacementPreview};

    #[test]
    fn theater_suffix_and_extensions_match_phobos() {
        assert_eq!(theater_name_with_suffix("FOO~~~.SHP", "tem"), "FOOtem.SHP");
        assert_eq!(theater_name_with_suffix("FOO.SHP", "tem"), "FOO.SHP");
        assert_eq!(with_extension("FOO", "shp"), "FOO.shp");
        assert_eq!(with_extension("FOO.SHP", "shp"), "FOO.SHP");
        assert_eq!(with_extension("unit~~~", "pal"), "unit~~~.pal");
    }

    #[test]
    fn preview_point_uses_the_cell_centre_and_offset() {
        let preview = BuildingPlacementPreview {
            rx: 10,
            ry: 10,
            width: 2,
            height: 2,
            valid: true,
            in_build_area: true,
            foundation_visible: true,
            reason: None,
            cells: vec![BuildingPlacementCell {
                coord: (10, 10),
                valid: true,
                level: 0,
                slope: 0,
            }],
            wall_autofill_cells: Vec::new(),
        };
        let (sx, sy) = preview_screen_point(&preview, [0, -15, 1]).unwrap();
        // Cell2Coord(10,10) centre = (2688, 2688); the planar projection gives
        // y = (10+10)*30/2/256*15 = 315, the entity frame adds the common +15
        // row bias, and the offset moves it up 15 px (Z=1 lepton rounds to 0).
        assert_eq!(sx, 0.0);
        assert_eq!(sy, 315.0);
    }
}
