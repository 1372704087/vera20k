//! Installed Ares 3.0 country presentation state.
//!
//! `HouseTypeExt::Initialize @ 0x1001C0F0` writes the stock flag, status,
//! taunt and observer defaults by the country array index; the unknown branch
//! at `0x1001C413` supplies `rani.pcx`, `GUI:Unknown` and
//! `taunts\tauam%02i.wav`. `LoadFromRulesFile` then reads the Ares country
//! keys, and a country with `ParentCountry=` copies its parent's resolved
//! settings before its own reads. These values are GUI/process state, not
//! simulation CRC inputs.

#[cfg(feature = "ares")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AresCountryPresentation {
    /// `File.Flag=` name, including the extension. Stock/unknown defaults are
    /// the country-index flags and `rani.pcx`.
    pub flag: Option<String>,
    /// `MenuText.Status=` CSF key.
    pub status: Option<String>,
    /// `File.Taunt=` pattern, normally `Taunts\tauam~~.wav` or the stock
    /// `taunts\tauam%02i.wav`.
    pub taunt: Option<String>,
    /// `File.ObserverBackground=` (PCX or SHP name).
    pub observer_background: Option<String>,
    /// `File.ObserverFlag=` (PCX or SHP name).
    pub observer_flag: Option<String>,
    /// `File.ObserverFlagAltPalette=`.
    pub observer_flag_alt_palette: bool,
    /// `ListIndex=`; a negative value hides the country.
    pub list_index: i32,
    /// `RandomSelectionWeight=`.
    pub random_weight: i32,
    /// `AIRandomSelectionWeight=`; initialized from `random_weight`.
    pub ai_random_weight: i32,
}

#[cfg(feature = "ares")]
impl Default for AresCountryPresentation {
    fn default() -> Self {
        Self {
            flag: None,
            status: None,
            taunt: None,
            observer_background: None,
            observer_flag: None,
            observer_flag_alt_palette: false,
            list_index: 100,
            random_weight: 1,
            ai_random_weight: 1,
        }
    }
}

/// Stock rows from `HouseTypeExt::Initialize` (0..9) and the unknown branch.
const STOCK: [(&str, &str, &str, Option<&str>, Option<&str>, bool); 10] = [
    (
        "usai.pcx",
        "STT:PlayerSideAmerica",
        "taunts\\tauam%02i.wav",
        Some("obsalli.shp"),
        Some("usai.shp"),
        false,
    ),
    (
        "japi.pcx",
        "STT:PlayerSideKorea",
        "taunts\\tauko%02i.wav",
        Some("obsalli.shp"),
        Some("japi.shp"),
        false,
    ),
    (
        "frai.pcx",
        "STT:PlayerSideFrance",
        "taunts\\taufr%02i.wav",
        Some("obsalli.shp"),
        Some("frai.shp"),
        false,
    ),
    (
        "geri.pcx",
        "STT:PlayerSideGermany",
        "taunts\\tauge%02i.wav",
        Some("obsalli.shp"),
        Some("geri.shp"),
        false,
    ),
    (
        "gbri.pcx",
        "STT:PlayerSideBritain",
        "taunts\\taubr%02i.wav",
        Some("obsalli.shp"),
        Some("gbri.shp"),
        false,
    ),
    (
        "djbi.pcx",
        "STT:PlayerSideLibya",
        "taunts\\tauli%02i.wav",
        Some("obssovi.shp"),
        Some("djbi.shp"),
        false,
    ),
    (
        "arbi.pcx",
        "STT:PlayerSideIraq",
        "taunts\\tauir%02i.wav",
        Some("obssovi.shp"),
        Some("arbi.shp"),
        false,
    ),
    (
        "lati.pcx",
        "STT:PlayerSideCuba",
        "taunts\\taucu%02i.wav",
        Some("obssovi.shp"),
        Some("lati.shp"),
        false,
    ),
    (
        "rusi.pcx",
        "STT:PlayerSideRussia",
        "taunts\\tauru%02i.wav",
        Some("obssovi.shp"),
        Some("rusi.shp"),
        false,
    ),
    (
        "yrii.pcx",
        "STT:PlayerSideYuriCountry",
        "taunts\\tauyu%02i.wav",
        Some("obsyuri.shp"),
        Some("yrii.shp"),
        true,
    ),
];

#[cfg(feature = "ares")]
impl AresCountryPresentation {
    /// First-phase stock defaults for country array position `index`.
    pub(crate) fn initialize_stock(&mut self, index: usize) {
        if let Some((flag, status, taunt, background, observer_flag, alt_palette)) =
            STOCK.get(index)
        {
            self.flag = Some((*flag).to_owned());
            self.status = Some((*status).to_owned());
            self.taunt = Some((*taunt).to_owned());
            self.observer_background = background.map(str::to_owned);
            self.observer_flag = observer_flag.map(str::to_owned);
            self.observer_flag_alt_palette = *alt_palette;
        } else {
            self.flag = Some("rani.pcx".to_owned());
            self.status = Some("GUI:Unknown".to_owned());
            self.taunt = Some("taunts\\tauam%02i.wav".to_owned());
            self.observer_background = None;
            self.observer_flag = None;
            self.observer_flag_alt_palette = false;
        }
        self.list_index = 100;
        self.random_weight = 1;
        self.ai_random_weight = 1;
    }

    /// `LoadFromRulesFile` file/status reads. Blank image names clear the
    /// current file; the fixed-string status/taunt reads keep it on empty.
    pub(crate) fn read_primary(&mut self, section: &crate::rules::ini_parser::IniSection) {
        for (key, target) in [
            ("File.Flag", &mut self.flag),
            ("File.ObserverFlag", &mut self.observer_flag),
            ("File.ObserverBackground", &mut self.observer_background),
        ] {
            if let Some(raw) = section.raw_value(key) {
                let name = raw.trim();
                *target = (!name.is_empty()).then(|| name.chars().take(31).collect());
            }
        }
        for (key, target) in [
            ("File.Taunt", &mut self.taunt),
            ("MenuText.Status", &mut self.status),
        ] {
            if let Some(value) = section.read_name(key, 0x800) {
                *target = Some(value.to_owned());
            }
        }
        self.observer_flag_alt_palette = section.read_ares_bool(
            "File.ObserverFlagAltPalette",
            self.observer_flag_alt_palette,
        );
    }

    /// `LoadFromINIFile` weight/order reads. `AIRandomSelectionWeight`
    /// defaults to the current `RandomSelectionWeight`, in this order.
    pub(crate) fn read_common(&mut self, section: &crate::rules::ini_parser::IniSection) {
        self.random_weight = section.read_ares_int("RandomSelectionWeight", self.random_weight);
        self.list_index = section.read_ares_int("ListIndex", self.list_index);
        self.ai_random_weight =
            section.read_ares_int("AIRandomSelectionWeight", self.random_weight);
    }

    /// `HouseTypeExt::InheritSettings`: copy the resolved parent settings.
    pub(crate) fn inherit_from(&mut self, parent: &Self) {
        self.flag = parent.flag.clone();
        self.status = parent.status.clone();
        self.taunt = parent.taunt.clone();
        self.observer_background = parent.observer_background.clone();
        self.observer_flag = parent.observer_flag.clone();
        self.observer_flag_alt_palette = parent.observer_flag_alt_palette;
        self.list_index = parent.list_index.wrapping_add(1);
        self.random_weight = parent.random_weight;
        self.ai_random_weight = parent.ai_random_weight;
    }
}
