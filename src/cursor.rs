// SPDX-License-Identifier: GPL-3.0-only

use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{CursorShape, NamedColor, Rgb},
};
use hex_color::HexColor;

use crate::config::{Config, CursorBlinkSetting, CursorColorSource, CursorStyleSetting};

/// Snapshot of cursor-related config passed to renderers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorSettings {
    pub color_source: CursorColorSource,
    pub color_custom: Option<hex_color::HexColor>,
    pub style: CursorStyleSetting,
    pub unfocused_style: CursorStyleSetting,
    pub blink: CursorBlinkSetting,
    pub blink_interval_ms: u16,
}

impl From<&Config> for CursorSettings {
    fn from(config: &Config) -> Self {
        Self {
            color_source: config.cursor_color_source,
            color_custom: config.cursor_color_custom,
            style: config.cursor_style,
            unfocused_style: config.cursor_unfocused_style,
            blink: config.cursor_blink,
            blink_interval_ms: config.cursor_blink_interval_ms,
        }
    }
}

impl CursorStyleSetting {
    pub const fn to_shape(self) -> CursorShape {
        match self {
            Self::FollowTerminal => CursorShape::Block,
            Self::Block => CursorShape::Block,
            Self::Beam => CursorShape::Beam,
            Self::Underline => CursorShape::Underline,
            Self::HollowBlock => CursorShape::HollowBlock,
        }
    }
}

pub fn effective_shape(
    settings: &CursorSettings,
    is_focused: bool,
    terminal_shape: CursorShape,
) -> CursorShape {
    if terminal_shape == CursorShape::Hidden {
        return CursorShape::Hidden;
    }

    let setting = if is_focused {
        settings.style
    } else {
        settings.unfocused_style
    };

    match setting {
        CursorStyleSetting::FollowTerminal => terminal_shape,
        fixed => fixed.to_shape(),
    }
}

pub fn should_blink(settings: &CursorSettings, is_focused: bool, terminal_blinking: bool) -> bool {
    if !is_focused {
        return false;
    }

    match settings.blink {
        CursorBlinkSetting::Never => false,
        CursorBlinkSetting::Always => true,
        CursorBlinkSetting::RespectTerminal => terminal_blinking,
    }
}

/// Default custom cursor color seeded from the active color scheme.
pub fn scheme_cursor_hex(colors: &Colors) -> Option<HexColor> {
    colors[NamedColor::Cursor]
        .or_else(|| colors[NamedColor::Foreground])
        .map(|rgb| HexColor::rgb(rgb.r, rgb.g, rgb.b))
}

pub fn effective_cursor_rgb(settings: &CursorSettings, colors: &Colors) -> Option<Rgb> {
    match settings.color_source {
        CursorColorSource::ColorScheme => colors[alacritty_terminal::vte::ansi::NamedColor::Cursor],
        CursorColorSource::Custom => settings.color_custom.map(|hex| Rgb {
            r: hex.r,
            g: hex.g,
            b: hex.b,
        }),
    }
}
