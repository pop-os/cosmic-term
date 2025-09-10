use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{NamedColor, Rgb},
};
use hex_color::HexColor;
use palette::{FromColor, Okhsl, encoding::Srgb, rgb::Rgb as PRgb};
use std::{collections::HashMap, fs};

use crate::config::{
    COSMIC_THEME_DARK, COSMIC_THEME_LIGHT, ColorScheme, ColorSchemeAnsi, ColorSchemeKind,
};

// Fill missing dim/bright colors with derived values from normal ones.
#[allow(dead_code)]
struct ColorDerive {
    dim_saturation_adjustment: f32,
    dim_lightness_adjustment: f32,
    bright_saturation_adjustment: f32,
    bright_lightness_adjustment: f32,
}

#[allow(dead_code)]
impl ColorDerive {
    fn new() -> Self {
        Self {
            // The dim flag/escape code is also sometimes described as faint.
            // So we reduce lightness and saturation to get both effects.
            dim_saturation_adjustment: -0.2,
            dim_lightness_adjustment: -0.2,
            // Normal colors are usually saturated enough. So we default this to 0.0
            // to avoid pushing colors towards white.
            bright_saturation_adjustment: 0.0,
            bright_lightness_adjustment: 0.10,
        }
    }

    fn with_dim_lightness_adjustment(self, dim_lightness_adjustment: f32) -> Self {
        Self {
            dim_lightness_adjustment,
            ..self
        }
    }

    fn rgb_to_okhsl(c: Rgb) -> Okhsl {
        let p_rgb = PRgb::<Srgb, u8>::new(c.r, c.g, c.b).into_format::<f32>();
        Okhsl::from_color(p_rgb)
    }

    fn okhsl_to_rgb(c: Okhsl) -> Rgb {
        let p_rgb = PRgb::<Srgb, _>::from_color(c).into_format::<u8>();
        let (r, g, b) = p_rgb.into_components();
        Rgb { r, g, b }
    }

    fn color_adj(rgb: Rgb, saturation_adj: f32, lightness_adj: f32) -> Rgb {
        let mut okhsl = Self::rgb_to_okhsl(rgb);

        okhsl.saturation = (okhsl.saturation + saturation_adj).clamp(0.0, 1.0);
        okhsl.lightness = (okhsl.lightness + lightness_adj).clamp(0.0, 1.0);

        Self::okhsl_to_rgb(okhsl)
    }

    fn brighten(&self, rgb: Rgb) -> Rgb {
        let saturation_adj = self.bright_saturation_adjustment;
        let lightness_adj = self.bright_lightness_adjustment;
        Self::color_adj(rgb, saturation_adj, lightness_adj)
    }

    fn dim_and_faint(&self, rgb: Rgb) -> Rgb {
        let saturation_adj = self.dim_saturation_adjustment;
        let lightness_adj = self.dim_lightness_adjustment;
        Self::color_adj(rgb, saturation_adj, lightness_adj)
    }

    fn fill_missing_brights(&self, colors: &mut Colors) {
        macro_rules! populate {
            ($($normal:ident$(,)?)+) => {
                paste::paste!{
                    $(
                        if colors[NamedColor::[<Bright $normal>]].is_none() {
                            match colors[NamedColor::$normal] {
                                None => panic!("tried to derive bright color from {} which is not set", stringify!($normal)),
                                Some(rgb) => colors[NamedColor::[<Bright $normal>]] = Some(self.brighten(rgb)),
                            }
                        }
                    )+
                }
            };
        }

        populate! { Foreground, Black, Red, Green, Yellow, Blue, Magenta, Cyan, White };
    }

    fn fill_missing_dims(&self, colors: &mut Colors) {
        macro_rules! populate {
            ($($normal:ident$(,)?)+) => {
                paste::paste!{
                    $(
                        if colors[NamedColor::[<Dim $normal>]].is_none() {
                            match colors[NamedColor::$normal] {
                                None => panic!("tried to derive dim color from {} which is not set", stringify!($normal)),
                                Some(rgb) => colors[NamedColor::[<Dim $normal>]] = Some(self.dim_and_faint(rgb)),
                            }
                        }
                    )+
                }
            };
        }

        populate! { Foreground, Black, Red, Green, Yellow, Blue, Magenta, Cyan, White };
    }
}

fn auto_colors() -> Colors {
    let mut colors = Colors::default();

    // These colors come from `ransid`: https://gitlab.redox-os.org/redox-os/ransid/-/blob/master/src/color.rs
    /* Indexed colors */
    for value in 16..=231 {
        let convert = |value: u8| -> u8 {
            match value {
                0 => 0,
                _ => value * 0x28 + 0x28,
            }
        };

        let r = convert((value - 16) / 36 % 6);
        let g = convert((value - 16) / 6 % 6);
        let b = convert((value - 16) % 6);
        colors[value as usize] = Some(Rgb { r, g, b });
    }

    /* Grays */
    for value in 232..=255 {
        let gray = (value - 232) * 10 + 8;
        colors[value as usize] = Some(Rgb {
            r: gray,
            g: gray,
            b: gray,
        });
    }

    colors
}

impl From<&ColorScheme> for Colors {
    fn from(color_scheme: &ColorScheme) -> Self {
        let mut colors = auto_colors();

        let encode_rgb = |rgb_opt: Option<HexColor>| -> Option<Rgb> {
            let rgb = rgb_opt?;
            Some(Rgb {
                r: rgb.r,
                g: rgb.g,
                b: rgb.b,
            })
        };

        // Set normal colors
        colors[NamedColor::Black] = encode_rgb(color_scheme.normal.black);
        colors[NamedColor::Red] = encode_rgb(color_scheme.normal.red);
        colors[NamedColor::Green] = encode_rgb(color_scheme.normal.green);
        colors[NamedColor::Yellow] = encode_rgb(color_scheme.normal.yellow);
        colors[NamedColor::Blue] = encode_rgb(color_scheme.normal.blue);
        colors[NamedColor::Magenta] = encode_rgb(color_scheme.normal.magenta);
        colors[NamedColor::Cyan] = encode_rgb(color_scheme.normal.cyan);
        colors[NamedColor::White] = encode_rgb(color_scheme.normal.white);

        // Set bright colors
        colors[NamedColor::BrightBlack] = encode_rgb(color_scheme.bright.black);
        colors[NamedColor::BrightRed] = encode_rgb(color_scheme.bright.red);
        colors[NamedColor::BrightGreen] = encode_rgb(color_scheme.bright.green);
        colors[NamedColor::BrightYellow] = encode_rgb(color_scheme.bright.yellow);
        colors[NamedColor::BrightBlue] = encode_rgb(color_scheme.bright.blue);
        colors[NamedColor::BrightMagenta] = encode_rgb(color_scheme.bright.magenta);
        colors[NamedColor::BrightCyan] = encode_rgb(color_scheme.bright.cyan);
        colors[NamedColor::BrightWhite] = encode_rgb(color_scheme.bright.white);

        // Set dim colors
        colors[NamedColor::DimBlack] = encode_rgb(color_scheme.dim.black);
        colors[NamedColor::DimRed] = encode_rgb(color_scheme.dim.red);
        colors[NamedColor::DimGreen] = encode_rgb(color_scheme.dim.green);
        colors[NamedColor::DimYellow] = encode_rgb(color_scheme.dim.yellow);
        colors[NamedColor::DimBlue] = encode_rgb(color_scheme.dim.blue);
        colors[NamedColor::DimMagenta] = encode_rgb(color_scheme.dim.magenta);
        colors[NamedColor::DimCyan] = encode_rgb(color_scheme.dim.cyan);
        colors[NamedColor::DimWhite] = encode_rgb(color_scheme.dim.white);

        // Set special colors
        colors[NamedColor::Foreground] = encode_rgb(color_scheme.foreground);
        colors[NamedColor::Background] = encode_rgb(color_scheme.background);
        colors[NamedColor::Cursor] = encode_rgb(color_scheme.cursor);
        colors[NamedColor::BrightForeground] = encode_rgb(color_scheme.bright_foreground);
        colors[NamedColor::DimForeground] = encode_rgb(color_scheme.dim_foreground);

        colors
    }
}

impl From<(&str, &Colors)> for ColorScheme {
    fn from(tuple: (&str, &Colors)) -> Self {
        let (name, colors) = tuple;

        let encode_rgb = |rgb_opt: Option<Rgb>| -> Option<HexColor> {
            let rgb = rgb_opt?;
            Some(HexColor::rgb(rgb.r, rgb.g, rgb.b))
        };

        Self {
            name: name.to_string(),
            foreground: encode_rgb(colors[NamedColor::Foreground]),
            background: encode_rgb(colors[NamedColor::Background]),
            cursor: encode_rgb(colors[NamedColor::Cursor]),
            bright_foreground: encode_rgb(colors[NamedColor::BrightForeground]),
            dim_foreground: encode_rgb(colors[NamedColor::DimForeground]),
            normal: ColorSchemeAnsi {
                black: encode_rgb(colors[NamedColor::Black]),
                red: encode_rgb(colors[NamedColor::Red]),
                green: encode_rgb(colors[NamedColor::Green]),
                yellow: encode_rgb(colors[NamedColor::Yellow]),
                blue: encode_rgb(colors[NamedColor::Blue]),
                magenta: encode_rgb(colors[NamedColor::Magenta]),
                cyan: encode_rgb(colors[NamedColor::Cyan]),
                white: encode_rgb(colors[NamedColor::White]),
            },
            bright: ColorSchemeAnsi {
                black: encode_rgb(colors[NamedColor::BrightBlack]),
                red: encode_rgb(colors[NamedColor::BrightRed]),
                green: encode_rgb(colors[NamedColor::BrightGreen]),
                yellow: encode_rgb(colors[NamedColor::BrightYellow]),
                blue: encode_rgb(colors[NamedColor::BrightBlue]),
                magenta: encode_rgb(colors[NamedColor::BrightMagenta]),
                cyan: encode_rgb(colors[NamedColor::BrightCyan]),
                white: encode_rgb(colors[NamedColor::BrightWhite]),
            },
            dim: ColorSchemeAnsi {
                black: encode_rgb(colors[NamedColor::DimBlack]),
                red: encode_rgb(colors[NamedColor::DimRed]),
                green: encode_rgb(colors[NamedColor::DimGreen]),
                yellow: encode_rgb(colors[NamedColor::DimYellow]),
                blue: encode_rgb(colors[NamedColor::DimBlue]),
                magenta: encode_rgb(colors[NamedColor::DimMagenta]),
                cyan: encode_rgb(colors[NamedColor::DimCyan]),
                white: encode_rgb(colors[NamedColor::DimWhite]),
            },
        }
    }
}

pub fn cosmic_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x1B1B1B));
    colors[NamedColor::Red] = Some(encode_rgb(0xF16161));
    colors[NamedColor::Green] = Some(encode_rgb(0x7CB987));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xDDC74C));
    colors[NamedColor::Blue] = Some(encode_rgb(0x6296BE));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xBE6DEE));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x49BAC8));
    colors[NamedColor::White] = Some(encode_rgb(0xBEBEBE));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x808080));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF8985));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x97D5A0));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFAE365));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x7DB1DA));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xD68EFF));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x49BAC8));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xC4C4C4));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    // Background comes from theme settings: colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

pub fn cosmic_light() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x292929));
    colors[NamedColor::Red] = Some(encode_rgb(0x8C151F));
    colors[NamedColor::Green] = Some(encode_rgb(0x145129));
    colors[NamedColor::Yellow] = Some(encode_rgb(0x624000));
    colors[NamedColor::Blue] = Some(encode_rgb(0x003F5F));
    colors[NamedColor::Magenta] = Some(encode_rgb(0x6D169C));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x004F57));
    colors[NamedColor::White] = Some(encode_rgb(0xBEBEBE));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x808080));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0x9D2329));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x235D34));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0x714B00));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x054B6F));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0x7A28A9));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x005C5D));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xD7D7D7));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::Black];
    // Background comes from theme settings: colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Cursor] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightBlack];

    // Fill missing dim colors
    ColorDerive::new()
        // With light backgrounds, the dim and faint descriptions are at odds!
        // To make the color fainter, we would need to increase lightness not decrease it!
        // But other terminals seem to still dim colors in light themes. So we dim too, but
        // not by much, since normal colors are dim enough already.
        .with_dim_lightness_adjustment(-0.07)
        .fill_missing_dims(&mut colors);

    colors
}

// Get builtin themes
pub fn terminal_themes() -> HashMap<(String, ColorSchemeKind), Colors> {
    let mut themes = HashMap::new();
    themes.insert(
        (COSMIC_THEME_DARK.to_string(), ColorSchemeKind::Dark),
        cosmic_dark(),
    );
    themes.insert(
        (COSMIC_THEME_LIGHT.to_string(), ColorSchemeKind::Light),
        cosmic_light(),
    );
    themes
}

// Helper function to export builtin themes to theme files
#[allow(dead_code)]
pub fn export() {
    for ((name, _color_scheme_kind), theme) in terminal_themes() {
        let color_scheme = ColorScheme::from((name.as_str(), &theme));

        // Ensure conversion to and from ColorScheme matches original theme
        {
            let theme_conv = Colors::from(&color_scheme);
            for i in 0..alacritty_terminal::term::color::COUNT {
                assert_eq!(theme[i], theme_conv[i]);
            }
        }

        let ron = match ron::ser::to_string_pretty(&color_scheme, ron::ser::PrettyConfig::new()) {
            Ok(ok) => ok,
            Err(err) => {
                log::error!("failed to export {name:?}: {err}");
                continue;
            }
        };

        let path = format!("color-schemes/{name}.ron");
        match fs::write(&path, ron) {
            Ok(()) => {
                log::info!("exported {path:?}");
            }
            Err(err) => {
                log::error!("failed to esport {path:?}: {err}");
            }
        }
    }
}
