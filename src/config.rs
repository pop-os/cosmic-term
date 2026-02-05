// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    theme,
};
use cosmic_text::{Metrics, Stretch, Weight};
use hex_color::HexColor;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::{fl, localize::LANGUAGE_SORTER, shortcuts::Shortcuts};

pub const CONFIG_VERSION: u64 = 1;
pub const COSMIC_THEME_DARK: &str = "COSMIC Dark";
pub const COSMIC_THEME_LIGHT: &str = "COSMIC Light";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    Light,
    System,
}

impl AppTheme {
    pub fn theme(&self) -> theme::Theme {
        match self {
            Self::Dark => {
                let mut t = theme::system_dark();
                t.theme_type.prefer_dark(Some(true));
                t
            }
            Self::Light => {
                let mut t = theme::system_light();
                t.theme_type.prefer_dark(Some(false));
                t
            }
            Self::System => theme::system_preference(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ColorSchemeKind {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ColorSchemeId(pub u64);

//TODO: there is a lot of extra code to keep the exported color scheme clean,
//consider how to reduce this
fn de_color_opt<'de, D>(deserializer: D) -> Result<Option<HexColor>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let hex_color: HexColor = Deserialize::deserialize(deserializer)?;
    Ok(Some(hex_color))
}

fn ser_color_opt<S>(hex_color_opt: &Option<HexColor>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::Error as _;
    match hex_color_opt {
        Some(hex_color) => Serialize::serialize(hex_color, serializer),
        None => Err(S::Error::custom("ser_color_opt called with None")),
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorSchemeAnsi {
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub black: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub red: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub green: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub yellow: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub blue: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub magenta: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub cyan: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub white: Option<HexColor>,
}

impl ColorSchemeAnsi {
    pub fn is_empty(&self) -> bool {
        self.black.is_none()
            && self.red.is_none()
            && self.green.is_none()
            && self.yellow.is_none()
            && self.blue.is_none()
            && self.magenta.is_none()
            && self.cyan.is_none()
            && self.white.is_none()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorScheme {
    pub name: String,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub foreground: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub background: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub bright_foreground: Option<HexColor>,
    #[serde(
        deserialize_with = "de_color_opt",
        serialize_with = "ser_color_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub dim_foreground: Option<HexColor>,
    #[serde(skip_serializing_if = "ColorSchemeAnsi::is_empty")]
    pub normal: ColorSchemeAnsi,
    #[serde(skip_serializing_if = "ColorSchemeAnsi::is_empty")]
    pub bright: ColorSchemeAnsi,
    #[serde(skip_serializing_if = "ColorSchemeAnsi::is_empty")]
    pub dim: ColorSchemeAnsi,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProfileId(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub syntax_theme_dark: String,
    #[serde(default)]
    pub syntax_theme_light: String,
    #[serde(default)]
    pub tab_title: String,
    #[serde(default)]
    pub working_directory: String,
    #[serde(default)]
    pub drain_on_exit: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: fl!("new-profile"),
            command: String::new(),
            syntax_theme_dark: COSMIC_THEME_DARK.to_string(),
            syntax_theme_light: COSMIC_THEME_LIGHT.to_string(),
            tab_title: String::new(),
            working_directory: String::new(),
            drain_on_exit: false,
        }
    }
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    pub app_theme: AppTheme,
    pub color_schemes_dark: BTreeMap<ColorSchemeId, ColorScheme>,
    pub color_schemes_light: BTreeMap<ColorSchemeId, ColorScheme>,
    pub font_name: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub dim_font_weight: u16,
    pub bold_font_weight: u16,
    pub font_stretch: u16,
    pub font_size_zoom_step_mul_100: u16,
    pub opacity: u8,
    pub profiles: BTreeMap<ProfileId, Profile>,
    pub show_headerbar: bool,
    pub use_bright_bold: bool,
    pub syntax_theme_dark: String,
    pub syntax_theme_light: String,
    pub focus_follow_mouse: bool,
    pub default_profile: Option<ProfileId>,
    #[serde(default)]
    pub shortcuts_custom: Shortcuts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            bold_font_weight: Weight::BOLD.0,
            color_schemes_dark: BTreeMap::new(),
            color_schemes_light: BTreeMap::new(),
            dim_font_weight: Weight::NORMAL.0,
            focus_follow_mouse: false,
            font_name: "Noto Sans Mono".to_string(),
            font_size: 14,
            font_size_zoom_step_mul_100: 100,
            font_stretch: Stretch::Normal.to_number(),
            font_weight: Weight::NORMAL.0,
            opacity: 100,
            profiles: BTreeMap::new(),
            show_headerbar: true,
            syntax_theme_dark: COSMIC_THEME_DARK.to_string(),
            syntax_theme_light: COSMIC_THEME_LIGHT.to_string(),
            use_bright_bold: false,
            default_profile: None,
            shortcuts_custom: Shortcuts::default(),
        }
    }
}

impl Config {
    pub fn color_schemes(
        &self,
        color_scheme_kind: ColorSchemeKind,
    ) -> &BTreeMap<ColorSchemeId, ColorScheme> {
        match color_scheme_kind {
            ColorSchemeKind::Dark => &self.color_schemes_dark,
            ColorSchemeKind::Light => &self.color_schemes_light,
        }
    }

    pub fn color_schemes_mut(
        &mut self,
        color_scheme_kind: ColorSchemeKind,
    ) -> &mut BTreeMap<ColorSchemeId, ColorScheme> {
        match color_scheme_kind {
            ColorSchemeKind::Dark => &mut self.color_schemes_dark,
            ColorSchemeKind::Light => &mut self.color_schemes_light,
        }
    }

    pub fn color_scheme_kind(&self) -> ColorSchemeKind {
        if self.app_theme.theme().theme_type.is_dark() {
            ColorSchemeKind::Dark
        } else {
            ColorSchemeKind::Light
        }
    }

    // Get a sorted and adjusted for duplicates list of color scheme names and ids
    pub fn color_scheme_names(
        &self,
        color_scheme_kind: ColorSchemeKind,
    ) -> Vec<(String, ColorSchemeId)> {
        let color_schemes = self.color_schemes(color_scheme_kind);
        let mut color_scheme_names =
            Vec::<(String, ColorSchemeId)>::with_capacity(color_schemes.len());
        for (color_scheme_id, color_scheme) in color_schemes {
            let mut name = color_scheme.name.clone();

            let mut copies = 1;
            while color_scheme_names.iter().any(|x| x.0 == name) {
                copies += 1;
                name = format!("{} ({})", color_scheme.name, copies);
            }

            color_scheme_names.push((name, *color_scheme_id));
        }
        color_scheme_names.sort_by(|a, b| LANGUAGE_SORTER.compare(&a.0, &b.0));
        color_scheme_names
    }

    fn font_size_adjusted(&self, zoom_adj: i8) -> f32 {
        let font_size = f32::from(self.font_size).max(1.0);
        let adj = f32::from(zoom_adj);
        let adj_step = f32::from(self.font_size_zoom_step_mul_100) / 100.0;
        (font_size + adj * adj_step).max(1.0)
    }

    // Calculate metrics from font size
    pub fn metrics(&self, zoom_adj: i8) -> Metrics {
        let font_size = self.font_size_adjusted(zoom_adj);
        let line_height = (font_size * 1.4).ceil();
        Metrics::new(font_size, line_height)
    }

    pub fn opacity_ratio(&self) -> f32 {
        f32::from(self.opacity) / 100.0
    }

    // Get a sorted and adjusted for duplicates list of profile names and ids
    pub fn profile_names(&self) -> Vec<(String, ProfileId)> {
        let mut profile_names = Vec::<(String, ProfileId)>::with_capacity(self.profiles.len());
        for (profile_id, profile) in &self.profiles {
            let mut name = profile.name.clone();

            let mut copies = 1;
            while profile_names.iter().any(|x| x.0 == name) {
                copies += 1;
                name = format!("{} ({})", profile.name, copies);
            }

            profile_names.push((name, *profile_id));
        }
        profile_names.sort_by(|a, b| LANGUAGE_SORTER.compare(&a.0, &b.0));
        profile_names
    }

    // Get current syntax theme based on dark mode
    pub fn syntax_theme(&self, profile_id_opt: Option<ProfileId>) -> (String, ColorSchemeKind) {
        let color_scheme_kind = self.color_scheme_kind();
        let theme_name = match profile_id_opt.and_then(|profile_id| self.profiles.get(&profile_id))
        {
            Some(profile) => match color_scheme_kind {
                ColorSchemeKind::Dark => profile.syntax_theme_dark.clone(),
                ColorSchemeKind::Light => profile.syntax_theme_light.clone(),
            },
            None => match color_scheme_kind {
                ColorSchemeKind::Dark => self.syntax_theme_dark.clone(),
                ColorSchemeKind::Light => self.syntax_theme_light.clone(),
            },
        };
        (theme_name, color_scheme_kind)
    }

    pub fn typed_font_stretch(&self) -> Stretch {
        macro_rules! populate_num_typed_map {
            ($($stretch:ident,)+) => {
                let mut map = BTreeMap::new();
                $(map.insert(Stretch::$stretch.to_number(), Stretch::$stretch);)+
                map
            };
        }

        static NUM_TO_TYPED_MAP: OnceLock<BTreeMap<u16, Stretch>> = OnceLock::new();

        NUM_TO_TYPED_MAP.get_or_init(|| {
            populate_num_typed_map! {
                UltraCondensed, ExtraCondensed, Condensed, SemiCondensed,
                Normal, SemiExpanded, Expanded, ExtraExpanded, UltraExpanded,
            }
        })[&self.font_stretch]
    }
}
