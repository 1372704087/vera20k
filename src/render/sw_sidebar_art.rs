//! Phobos `[SIDE] SuperWeaponSidebar.*PCX` artwork and Ares `SidebarPCX`
//! superweapon cameos.
//!
//! Phobos loads each PCX through `PhobosPCXFile::GetSurface` and blits it
//! without scaling (`SWColumnClass::Draw`, `ToggleSWButtonClass::Draw`,
//! `SWButtonClass::Draw`). VERA packs the side images and the optional
//! per-superweapon cameos into RGBA atlases so each layer is one batch texture.

use std::collections::HashMap;
use std::hash::Hash;

use crate::assets::asset_manager::AssetManager;
use crate::assets::pcx_file::PcxFile;
use crate::render::batch::{BatchRenderer, BatchTexture};
use crate::render::gpu::GpuContext;
use crate::rules::ruleset::RuleSet;

const ART_PADDING: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwSidebarArtKind {
    On,
    Off,
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct SwSidebarArtEntry {
    pub uv_origin: [f32; 2],
    pub uv_size: [f32; 2],
    pub pixel_size: [f32; 2],
}

pub struct SwSidebarArt {
    pub texture: BatchTexture,
    entries: HashMap<SwSidebarArtKind, SwSidebarArtEntry>,
}

impl SwSidebarArt {
    pub fn entry(&self, kind: SwSidebarArtKind) -> Option<&SwSidebarArtEntry> {
        self.entries.get(&kind)
    }

    fn size(&self, kind: SwSidebarArtKind) -> Option<[i32; 2]> {
        self.entry(kind).map(|entry| {
            [
                entry.pixel_size[0].round() as i32,
                entry.pixel_size[1].round() as i32,
            ]
        })
    }

    pub fn top_size(&self) -> Option<[i32; 2]> {
        self.size(SwSidebarArtKind::Top)
    }

    pub fn bottom_size(&self) -> Option<[i32; 2]> {
        self.size(SwSidebarArtKind::Bottom)
    }

    /// Phobos sizes the toggle gadget from the larger loaded On/Off image.
    pub fn toggle_size(&self) -> Option<[i32; 2]> {
        let on = self.size(SwSidebarArtKind::On);
        let off = self.size(SwSidebarArtKind::Off);
        match (on, off) {
            (Some(a), Some(b)) => Some([a[0].max(b[0]), a[1].max(b[1])]),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

/// Ares `[SuperWeapon] SidebarPCX` cameos, keyed by superweapon section.
#[cfg(feature = "ares")]
pub struct SwSidebarCameoArt {
    pub texture: BatchTexture,
    entries: HashMap<String, SwSidebarArtEntry>,
}

#[cfg(feature = "ares")]
impl SwSidebarCameoArt {
    pub fn entry(&self, type_id: &str) -> Option<&SwSidebarArtEntry> {
        self.entries.get(&type_id.to_ascii_uppercase())
    }
}

pub fn build_sw_sidebar_art(
    gpu: &GpuContext,
    batch: &BatchRenderer,
    asset_manager: &AssetManager,
    rules: &RuleSet,
    side_index: i32,
) -> Option<SwSidebarArt> {
    let art = rules.side_sw_sidebar_art(side_index)?;
    let images = [
        (SwSidebarArtKind::On, art.on_pcx.as_deref()),
        (SwSidebarArtKind::Off, art.off_pcx.as_deref()),
        (SwSidebarArtKind::Top, art.top_pcx.as_deref()),
        (SwSidebarArtKind::Center, art.center_pcx.as_deref()),
        (SwSidebarArtKind::Bottom, art.bottom_pcx.as_deref()),
    ]
    .into_iter()
    .filter_map(|(kind, name)| Some((kind, load_pcx(asset_manager, name?)?)))
    .collect();
    let (texture, entries) = pack_atlas(gpu, batch, images)?;
    log::info!("SuperWeaponSidebar art atlas: {} entries", entries.len());
    Some(SwSidebarArt { texture, entries })
}

/// All registered superweapons with an Ares `SidebarPCX=`, packed once at
/// presentation load because the type set is fixed for the match.
#[cfg(feature = "ares")]
pub fn build_sw_sidebar_cameo_art(
    gpu: &GpuContext,
    batch: &BatchRenderer,
    asset_manager: &AssetManager,
    rules: &RuleSet,
) -> Option<SwSidebarCameoArt> {
    let images = rules
        .super_weapon_order
        .iter()
        .filter_map(|id| {
            let name = rules.super_weapon(id)?.sidebar_pcx()?;
            let image = load_pcx(asset_manager, name)?;
            Some((id.to_ascii_uppercase(), image))
        })
        .collect();
    let (texture, entries) = pack_atlas(gpu, batch, images)?;
    log::info!(
        "SuperWeaponSidebar SidebarPCX atlas: {} entries",
        entries.len()
    );
    Some(SwSidebarCameoArt { texture, entries })
}

struct LoadedImage {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn load_pcx(asset_manager: &AssetManager, name: &str) -> Option<LoadedImage> {
    let bytes = asset_manager.get_ref(name)?;
    let image = match PcxFile::from_bytes(bytes) {
        Ok(image) => image,
        Err(error) => {
            log::warn!("SuperWeaponSidebar PCX {name} did not load: {error}");
            return None;
        }
    };
    Some(LoadedImage {
        rgba: image.to_rgba(None),
        width: u32::from(image.width),
        height: u32::from(image.height),
    })
}

fn pack_atlas<K: Eq + Hash>(
    gpu: &GpuContext,
    batch: &BatchRenderer,
    images: Vec<(K, LoadedImage)>,
) -> Option<(BatchTexture, HashMap<K, SwSidebarArtEntry>)> {
    if images.is_empty() {
        return None;
    }
    let width = images
        .iter()
        .map(|(_, image)| image.width)
        .max()
        .unwrap_or(1);
    let height = images
        .iter()
        .map(|(_, image)| image.height + ART_PADDING)
        .sum::<u32>()
        .saturating_sub(ART_PADDING)
        .max(1);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut entries = HashMap::new();
    let mut y = 0u32;
    for (key, image) in images {
        for row in 0..image.height {
            let src = (row * image.width * 4) as usize;
            let dst = (((y + row) * width) * 4) as usize;
            rgba[dst..dst + (image.width * 4) as usize]
                .copy_from_slice(&image.rgba[src..src + (image.width * 4) as usize]);
        }
        entries.insert(
            key,
            SwSidebarArtEntry {
                uv_origin: [0.0, y as f32 / height as f32],
                uv_size: [
                    image.width as f32 / width as f32,
                    image.height as f32 / height as f32,
                ],
                pixel_size: [image.width as f32, image.height as f32],
            },
        );
        y += image.height + ART_PADDING;
    }
    Some((batch.create_texture(gpu, &rgba, width, height), entries))
}
