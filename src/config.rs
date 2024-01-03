// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry},
    theme,
};
use cosmic_text::Metrics;
use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    Light,
    System,
}

impl AppTheme {
    pub fn theme(&self) -> theme::Theme {
        match self {
            Self::Dark => theme::Theme::dark(),
            Self::Light => theme::Theme::light(),
            Self::System => theme::system_preference(),
        }
    }
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    pub app_theme: AppTheme,
    pub font_name: String,
    pub font_size: u16,
    pub font_size_zoom_step_mul_100: u16,
    pub show_headerbar: bool,
    pub syntax_theme_dark: String,
    pub syntax_theme_light: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            font_name: "Fira Mono".to_string(),
            font_size: 14,
            font_size_zoom_step_mul_100: 100,
            show_headerbar: true,
            syntax_theme_dark: "COSMIC Dark".to_string(),
            syntax_theme_light: "COSMIC Light".to_string(),
        }
    }
}

impl Config {
    fn font_size_adjusted(&self, zoom_adj: i8) -> f32 {
        let font_size = f32::from(self.font_size).max(1.0);
        let adj = f32::from(zoom_adj);
        let adj_step = f32::from(self.font_size_zoom_step_mul_100) / 100.0;
        (font_size + adj*adj_step).max(1.0)
    }

    // Calculate metrics from font size
    pub fn metrics(&self, zoom_adj: i8) -> Metrics {
        let font_size = self.font_size_adjusted(zoom_adj);
        let line_height = (font_size * 1.4).ceil();
        Metrics::new(font_size, line_height)
    }

    // Get current syntax theme based on dark mode
    pub fn syntax_theme(&self) -> &str {
        let dark = self.app_theme.theme().theme_type.is_dark();
        if dark {
            &self.syntax_theme_dark
        } else {
            &self.syntax_theme_light
        }
    }
}
