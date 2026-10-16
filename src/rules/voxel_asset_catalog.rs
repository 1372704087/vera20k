//! GPU-independent main-voxel admission shared by gameplay and presentation.
//!
//! Source: fixed ART's Voxel flag, final rules Image/Unit gun configuration,
//! and the match's AssetManager bytes. RuleSet rebuilds this immutable catalog
//! alongside effect assets; replacing ART invalidates it. An unbound catalog
//! admits no type. No renderer cache or per-entity flag can change this input.

use std::collections::BTreeSet;

use crate::assets::asset_manager::AssetManager;
use crate::assets::hva_file::HvaFile;
use crate::assets::vxl_file::VxlFile;
use crate::rules::object_type::ObjectCategory;
use crate::rules::ruleset::RuleSet;
use crate::rules::voxel_frame_catalog::{
    VxlLayer, indexed_turret_count, voxel_gun_basename, voxel_image_id,
};

/// Parsed physical part. Rendering borrows its inputs without owning a second
/// file-admission policy. Building and raw-image callers explicitly permit an
/// absent HVA; native Techno main-body admission requires the complete pair.
pub(crate) struct VoxelPart {
    vxl: VxlFile,
    hva: Option<HvaFile>,
}

impl VoxelPart {
    pub(crate) fn load(assets: &AssetManager, image: &str, require_hva: bool) -> Option<Self> {
        let (vxl, hva) = crate::rules::art_data::voxel_asset_names(image);
        let vxl = VxlFile::from_bytes(assets.get_ref(&vxl)?).ok()?;
        let hva = assets
            .get_ref(&hva)
            .and_then(|bytes| HvaFile::from_bytes(bytes).ok());
        if require_hva && hva.is_none() {
            return None;
        }
        Some(Self { vxl, hva })
    }

    pub(crate) fn vxl(&self) -> &VxlFile {
        &self.vxl
    }

    pub(crate) fn hva(&self) -> Option<&HvaFile> {
        self.hva.as_ref()
    }
}

pub(crate) type MainVoxelParts = (VoxelPart, Vec<(Option<VoxelPart>, Option<VoxelPart>)>);

/// ObjectType5F8110's body and Unit gun loading. TechnoType716090's Voxel
/// declaration gates the call. Main VXL ctor755CD0/755CFE retains an error byte;
/// IsVoxel6F9E10 requires the resulting B0 pointer and error byte zero.
/// Missing/invalid main HVA sets failure5F8245; cleanup5F8A60..5F8AA4 clears
/// B0/B4. Indexed TUR/HVA and present BARL/HVA failures share that cleanup.
///
/// A name without a rules type is an explicit raw-image component boundary
/// (Bullet, debris or auxiliary image), retaining its existing optional HVA.
/// Optional legacy nonindexed gun HVA behavior remains outside this main-body
/// and indexed-pair admission claim.
pub(crate) fn load_main_voxel(
    assets: &AssetManager,
    type_id: &str,
    rules: Option<&RuleSet>,
) -> Option<MainVoxelParts> {
    let object = rules.and_then(|rules| rules.object(type_id));
    if let Some(object) = object
        && (object.category == ObjectCategory::Building
            || !rules?.art().get(&object.image).is_some_and(|art| art.voxel))
    {
        return None;
    }
    let image = voxel_image_id(type_id, rules);
    let body = VoxelPart::load(assets, &image, object.is_some())?;
    let indexed_count = indexed_turret_count(type_id, rules);
    let has_turret = object
        .is_some_and(|object| object.category == ObjectCategory::Vehicle && object.has_turret);
    let mut guns = Vec::new();
    for index in 0..indexed_count.max(i32::from(has_turret)) {
        let turret_name = voxel_gun_basename(&image, VxlLayer::Turret, index);
        let turret = VoxelPart::load(assets, &turret_name, indexed_count > 0);
        if indexed_count > 0 && turret.is_none() {
            return None;
        }
        let barrel = if has_turret {
            let name = voxel_gun_basename(&image, VxlLayer::Barrel, index);
            if assets.get_ref(&format!("{name}.VXL")).is_some() {
                let barrel = VoxelPart::load(assets, &name, indexed_count > 0);
                if indexed_count > 0 && barrel.is_none() {
                    return None;
                }
                barrel
            } else {
                None
            }
        } else {
            None
        };
        guns.push((turret, barrel));
    }
    Some((body, guns))
}

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct VoxelAssetCatalog {
    loaded: BTreeSet<String>,
}

impl VoxelAssetCatalog {
    pub(crate) fn bind(rules: &RuleSet, assets: &AssetManager) -> Self {
        Self {
            loaded: rules
                .all_objects()
                .filter(|object| load_main_voxel(assets, &object.id, Some(rules)).is_some())
                .map(|object| object.id.to_ascii_uppercase())
                .collect(),
        }
    }

    pub(crate) fn contains(&self, type_id: &str) -> bool {
        if type_id.bytes().any(|byte| byte.is_ascii_lowercase()) {
            self.loaded.contains(&type_id.to_ascii_uppercase())
        } else {
            self.loaded.contains(type_id)
        }
    }

    #[cfg(test)]
    pub(crate) fn supply_loaded(&mut self, type_id: &str) {
        self.loaded.insert(type_id.to_ascii_uppercase());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{art_data::ArtRegistry, ini_parser::IniFile};

    fn body_rules(voxel: bool) -> RuleSet {
        let mut rules = RuleSet::from_ini(&IniFile::from_str(
            "[VehicleTypes]\n0=TEST\n[TEST]\nImage=BODY\nStrength=300\n",
        ))
        .unwrap();
        rules.install_art_fixture(ArtRegistry::from_ini(&IniFile::from_str(&format!(
            "[BODY]\nVoxel={voxel}\n"
        ))));
        rules
    }

    #[test]
    fn main_voxel_binding_requires_declared_valid_complete_pair_and_rebuilds() {
        let Some((_, assets)) = crate::rules::retail_ini_fixture::retail_assets() else {
            return;
        };
        let directory =
            crate::map::source::test_support::TestDirectory::new("voxel-main-admission");
        for (source, destination) in [("DEST.VXL", "BODY.VXL"), ("DEST.HVA", "BODY.HVA")] {
            std::fs::write(
                directory.path().join(destination),
                assets.get_ref(source).unwrap(),
            )
            .unwrap();
        }
        let load_assets = || AssetManager::from_loose_root_for_test(directory.path());
        let mut rules = body_rules(true);
        assert!(
            !rules.object_has_loaded_voxel("TEST"),
            "unbound is explicit"
        );
        rules.bind_effect_assets(&load_assets(), "TEM", "TEMPERATE");
        assert!(rules.object_has_loaded_voxel("test"));
        let bound_hash = rules.simulation_config_hash();
        rules.bind_effect_assets(&load_assets(), "TEM", "TEMPERATE");
        assert_eq!(rules.simulation_config_hash(), bound_hash);
        rules.install_art_fixture(ArtRegistry::from_ini(&IniFile::from_str(
            "[BODY]\nVoxel=no\n",
        )));
        assert!(
            !rules.object_has_loaded_voxel("TEST"),
            "ART replacement invalidates"
        );
        rules.bind_effect_assets(&load_assets(), "TEM", "TEMPERATE");
        assert!(
            !rules.object_has_loaded_voxel("TEST"),
            "a physical VXL is not a declaration"
        );
        rules = body_rules(true);
        std::fs::remove_file(directory.path().join("BODY.HVA")).unwrap();
        rules.bind_effect_assets(&load_assets(), "TEM", "TEMPERATE");
        assert!(
            !rules.object_has_loaded_voxel("TEST"),
            "failed HVA unloads B0"
        );
        std::fs::write(
            directory.path().join("BODY.HVA"),
            assets.get_ref("DEST.HVA").unwrap(),
        )
        .unwrap();
        std::fs::write(directory.path().join("BODY.VXL"), b"invalid VXL").unwrap();
        rules.bind_effect_assets(&load_assets(), "TEM", "TEMPERATE");
        assert!(
            !rules.object_has_loaded_voxel("TEST"),
            "malformed VXL is not loaded"
        );
    }
}
