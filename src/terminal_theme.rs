use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{NamedColor, Rgb},
};

use palette::{encoding::Srgb, rgb::Rgb as PRgb, Okhsl, FromColor, num::Abs};
use std::collections::HashMap;

// Fill missing dim/bright colors with derived values from normal ones.
struct ColorDerive {
    dim_saturation_adjustment: f32,
    dim_lightness_adjustment: f32,
    bright_saturation_adjustment: f32,
    bright_lightness_adjustment: f32,
}

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
        Self { dim_lightness_adjustment, ..self }
    }

    fn rgb_to_okhsl(c: Rgb) -> Okhsl {
        let p_rgb = PRgb::<Srgb, u8>::new(c.r, c.g, c.b)
            .into_format::<f32>();
        Okhsl::from_color(p_rgb)
    }

    fn okhsl_to_rgb(c: Okhsl) -> Rgb {
        let p_rgb = PRgb::<Srgb, _>::from_color(c)
            .into_format::<u8>();
        let (r, g, b) = p_rgb.into_components();
        Rgb{r, g, b}
    }

    fn color_adj(rgb: Rgb, saturation_adj: f32, lightness_adj: f32) -> Rgb {
        let mut okhsl = Self::rgb_to_okhsl(rgb);

        okhsl.saturation = (okhsl.saturation + saturation_adj)
            .max(0.0)
            .min(1.0);
        okhsl.lightness = (okhsl.lightness + lightness_adj)
            .max(0.0)
            .min(1.0);

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

        populate!{ Foreground, Black, Red, Green, Yellow, Blue, Magenta, Cyan, White };
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

        populate!{ Foreground, Black, Red, Green, Yellow, Blue, Magenta, Cyan, White };
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

fn cosmic_dark() -> Colors {
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
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn cosmic_light() -> Colors {
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
    colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
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

fn gruvbox_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x282828));
    colors[NamedColor::Red] = Some(encode_rgb(0xcc241d));
    colors[NamedColor::Green] = Some(encode_rgb(0x98971a));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xd79921));
    colors[NamedColor::Blue] = Some(encode_rgb(0x458588));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xb16286));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x689d6a));
    colors[NamedColor::White] = Some(encode_rgb(0xa89984));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x928374));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xfb4934));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0xb8bb26));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xfabd2f));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x83a598));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xd3869b));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x8ec07c));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xebdbb2));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.15)
        .fill_missing_dims(&mut colors);

    colors
}

fn one_half_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x282c34));
    colors[NamedColor::Red] = Some(encode_rgb(0xe06c75));
    colors[NamedColor::Green] = Some(encode_rgb(0x98c379));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xe5c07b));
    colors[NamedColor::Blue] = Some(encode_rgb(0x61afef));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xc678dd));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x56b6c2));
    colors[NamedColor::White] = Some(encode_rgb(0xdcdfe4));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x5d677a));

    // Set this before filling bright colors (including BrightForeground)
    colors[NamedColor::Foreground] = colors[NamedColor::White];

    let color_derive = ColorDerive::new();

    // Fill missing bright colors
    color_derive.fill_missing_brights(&mut colors);

    // Set the rest of special colors
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];

    // Fill missing dim colors
    color_derive.fill_missing_dims(&mut colors);

    colors
}

fn pop_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |r: u8, g: u8, b: u8| -> Rgb { Rgb { r, g, b } };

    // Pop colors (from pop-desktop gsettings)
    colors[NamedColor::Black] = Some(encode_rgb(51, 51, 51));
    colors[NamedColor::Red] = Some(encode_rgb(204, 0, 0));
    colors[NamedColor::Green] = Some(encode_rgb(78, 154, 6));
    colors[NamedColor::Yellow] = Some(encode_rgb(196, 160, 0));
    colors[NamedColor::Blue] = Some(encode_rgb(52, 101, 164));
    colors[NamedColor::Magenta] = Some(encode_rgb(117, 80, 123));
    colors[NamedColor::Cyan] = Some(encode_rgb(6, 152, 154));
    colors[NamedColor::White] = Some(encode_rgb(211, 215, 207));
    colors[NamedColor::BrightBlack] = Some(encode_rgb(136, 128, 124));
    colors[NamedColor::BrightRed] = Some(encode_rgb(241, 93, 34));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(115, 196, 143));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(255, 206, 81));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(72, 185, 199));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(173, 127, 168));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(52, 226, 226));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(238, 238, 236));

    // Set special colors
    // Pop colors (from pop-desktop gsettings)
    colors[NamedColor::Foreground] = Some(encode_rgb(242, 242, 242));
    colors[NamedColor::Background] = Some(encode_rgb(51, 51, 51));
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.05)
        .fill_missing_dims(&mut colors);

    colors
}

pub fn terminal_themes() -> HashMap<String, Colors> {
    let mut themes = HashMap::new();
    themes.insert("COSMIC Dark".to_string(), cosmic_dark());
    themes.insert("COSMIC Light".to_string(), cosmic_light());
    themes.insert("gruvbox-dark".to_string(), gruvbox_dark());
    themes.insert("OneHalfDark".to_string(), one_half_dark());
    themes.insert("Pop Dark".to_string(), pop_dark());
    themes
}
