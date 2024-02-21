use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{NamedColor, Rgb},
};

use palette::{encoding::Srgb, rgb::Rgb as PRgb, FromColor, Okhsl};
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

        okhsl.saturation = (okhsl.saturation + saturation_adj).max(0.0).min(1.0);
        okhsl.lightness = (okhsl.lightness + lightness_adj).max(0.0).min(1.0);

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

fn tango_palette() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x2E3436));
    colors[NamedColor::Red] = Some(encode_rgb(0xCC0000));
    colors[NamedColor::Green] = Some(encode_rgb(0x4E9A06));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xC4A000));
    colors[NamedColor::Blue] = Some(encode_rgb(0x3465A4));
    colors[NamedColor::Magenta] = Some(encode_rgb(0x75507B));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x06989A));
    colors[NamedColor::White] = Some(encode_rgb(0xD3D7CF));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x555753));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xEF2929));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x8AE234));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFCE94F));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x729FCF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xAD7FA8));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x34E2E2));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xEEEEEC));

    colors
}

fn tango_dark() -> Colors {
    let mut colors = tango_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::White];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.10)
        .fill_missing_dims(&mut colors);

    colors
}

fn tango_light() -> Colors {
    let mut colors = tango_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::Black];
    colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightBlack];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn linux_console_palette() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x000000));
    colors[NamedColor::Red] = Some(encode_rgb(0xAA0000));
    colors[NamedColor::Green] = Some(encode_rgb(0x00AA00));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xAA5500));
    colors[NamedColor::Blue] = Some(encode_rgb(0x0000AA));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xAA00AA));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x00AAAA));
    colors[NamedColor::White] = Some(encode_rgb(0xAAAAAA));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x555555));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF5555));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x55FF55));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFFFF55));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x5555FF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xFF55FF));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x55FFFF));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xFFFFFF));

    colors
}

fn linux_console() -> Colors {
    let mut colors = linux_console_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::White];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.10)
        .fill_missing_dims(&mut colors);

    colors
}

fn xterm_palette() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x000000));
    colors[NamedColor::Red] = Some(encode_rgb(0xCD0000));
    colors[NamedColor::Green] = Some(encode_rgb(0x00CD00));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xCDCD00));
    colors[NamedColor::Blue] = Some(encode_rgb(0x0000EE));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xCD00CD));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x00CDCD));
    colors[NamedColor::White] = Some(encode_rgb(0xE5E5E5));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x7F7F7F));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF0000));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x00FF00));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFFFF00));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x5C5CFF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xFF00FF));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x00FFFF));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xFFFFFF));

    colors
}

fn xterm_dark() -> Colors {
    let mut colors = xterm_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::White];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.12)
        .fill_missing_dims(&mut colors);

    colors
}

fn xterm_light() -> Colors {
    let mut colors = xterm_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::Black];
    colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightBlack];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn rxvt_palette() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x000000));
    colors[NamedColor::Red] = Some(encode_rgb(0xCD0000));
    colors[NamedColor::Green] = Some(encode_rgb(0x00CD00));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xCDCD00));
    colors[NamedColor::Blue] = Some(encode_rgb(0x0000CD));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xCD00CD));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x00CDCD));
    colors[NamedColor::White] = Some(encode_rgb(0xFAEBD7));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x404040));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF0000));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x00FF00));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xFFFF00));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x0000FF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xFF00FF));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x00FFFF));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xFFFFFF));

    colors
}

fn rxvt_dark() -> Colors {
    let mut colors = rxvt_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::BrightForeground] = colors[NamedColor::White];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new()
        // Dim less so colors are readable with default bg
        .with_dim_lightness_adjustment(-0.12)
        .fill_missing_dims(&mut colors);

    colors
}

fn rxvt_light() -> Colors {
    let mut colors = rxvt_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::Black];
    colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightBlack];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn solarized_palette() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x073642));
    colors[NamedColor::Red] = Some(encode_rgb(0xDC322F));
    colors[NamedColor::Green] = Some(encode_rgb(0x859900));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xB58900));
    colors[NamedColor::Blue] = Some(encode_rgb(0x268BD2));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xD33682));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x2AA198));
    colors[NamedColor::White] = Some(encode_rgb(0xEEE8D5));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x002B36));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xCB4B16));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x586E75));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0x657B83));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x839496));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0x6C71C4));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x93A1A1));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xFDF6E3));

    colors
}

fn solarized_dark() -> Colors {
    let mut colors = solarized_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightBlue];
    colors[NamedColor::Background] = colors[NamedColor::BrightBlack];
    colors[NamedColor::BrightForeground] = colors[NamedColor::Blue];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn solarized_light() -> Colors {
    let mut colors = solarized_palette();

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightYellow];
    colors[NamedColor::Background] = colors[NamedColor::BrightWhite];
    colors[NamedColor::BrightForeground] = colors[NamedColor::Yellow];
    colors[NamedColor::Cursor] = colors[NamedColor::Foreground];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

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
    // Background comes from theme settings: colors[NamedColor::Background] = colors[NamedColor::Black];
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

fn selenized_white() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0xEBEBEB));
    colors[NamedColor::Red] = Some(encode_rgb(0xD6000C));
    colors[NamedColor::Green] = Some(encode_rgb(0x1D9700));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xC49700));
    colors[NamedColor::Blue] = Some(encode_rgb(0x0064E4));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xDD0F9D));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x00AD9C));
    colors[NamedColor::White] = Some(encode_rgb(0x878787));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0xCDCDCD));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xBF0000));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x008400));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xAF8500));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x0054CF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xC7008B));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x009A8A));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0x282828));

    // Set special colors
    colors[NamedColor::Background] = Some(encode_rgb(0xFFFFFF));
    colors[NamedColor::Foreground] = Some(encode_rgb(0x474747));
    colors[NamedColor::Cursor] = colors[NamedColor::Black];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn selenized_light() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0xECE3CC));
    colors[NamedColor::Red] = Some(encode_rgb(0xD2212D));
    colors[NamedColor::Green] = Some(encode_rgb(0x489100));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xAD8900));
    colors[NamedColor::Blue] = Some(encode_rgb(0x0072D4));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xCA4898));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x009C8F));
    colors[NamedColor::White] = Some(encode_rgb(0x909995));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0xD5CDB6));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xCC1729));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x428B00));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xA78300));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x006DCE));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xC44392));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x00978A));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0x3A4D53));

    // Set special colors
    colors[NamedColor::Background] = Some(encode_rgb(0xFBF3DB));
    colors[NamedColor::Foreground] = Some(encode_rgb(0x53676D));
    colors[NamedColor::Cursor] = colors[NamedColor::Black];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn selenized_dark() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x184956));
    colors[NamedColor::Red] = Some(encode_rgb(0xFA5750));
    colors[NamedColor::Green] = Some(encode_rgb(0x75B938));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xDBB32D));
    colors[NamedColor::Blue] = Some(encode_rgb(0x4695F7));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xF275BE));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x41C7B9));
    colors[NamedColor::White] = Some(encode_rgb(0x72898F));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x2D5B69));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF665C));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x84C747));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xEBC13D));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x58A3FF));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xFF84CD));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x53D6C7));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xCAD8D9));

    // Set special colors
    colors[NamedColor::Background] = Some(encode_rgb(0x103C48));
    colors[NamedColor::Foreground] = Some(encode_rgb(0xADBCBC));
    colors[NamedColor::Cursor] = colors[NamedColor::White];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

fn selenized_black() -> Colors {
    let mut colors = auto_colors();

    let encode_rgb = |data: u32| -> Rgb {
        Rgb {
            r: (data >> 16) as u8,
            g: (data >> 8) as u8,
            b: data as u8,
        }
    };

    colors[NamedColor::Black] = Some(encode_rgb(0x252525));
    colors[NamedColor::Red] = Some(encode_rgb(0xED4A46));
    colors[NamedColor::Green] = Some(encode_rgb(0x70B433));
    colors[NamedColor::Yellow] = Some(encode_rgb(0xDBB32D));
    colors[NamedColor::Blue] = Some(encode_rgb(0x368AEB));
    colors[NamedColor::Magenta] = Some(encode_rgb(0xEB6EB7));
    colors[NamedColor::Cyan] = Some(encode_rgb(0x3FC5B7));
    colors[NamedColor::White] = Some(encode_rgb(0x777777));

    colors[NamedColor::BrightBlack] = Some(encode_rgb(0x3B3B3B));
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xFF5E56));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x83C746));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xEFC541));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x4F9CFE));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xFF81CA));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x56D8C9));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xDEDEDE));

    // Set special colors
    colors[NamedColor::Background] = Some(encode_rgb(0x181818));
    colors[NamedColor::Foreground] = Some(encode_rgb(0xB9B9B9));
    colors[NamedColor::Cursor] = colors[NamedColor::White];

    // Fill missing dim colors
    ColorDerive::new().fill_missing_dims(&mut colors);

    colors
}

pub fn terminal_themes() -> HashMap<String, Colors> {
    let mut themes = HashMap::new();
    themes.insert("Tango Dark".to_string(), tango_dark());
    themes.insert("Tango Light".to_string(), tango_light());
    themes.insert("XTerm Dark".to_string(), xterm_dark());
    themes.insert("XTerm Light".to_string(), xterm_light());
    themes.insert("Linux Console".to_string(), linux_console());
    themes.insert("Rxvt Dark".to_string(), rxvt_dark());
    themes.insert("Rxvt Light".to_string(), rxvt_light());
    themes.insert("Solarized Dark".to_string(), solarized_dark());
    themes.insert("Solarized Light".to_string(), solarized_light());
    themes.insert("COSMIC Dark".to_string(), cosmic_dark());
    themes.insert("COSMIC Light".to_string(), cosmic_light());
    themes.insert("gruvbox-dark".to_string(), gruvbox_dark());
    themes.insert("OneHalfDark".to_string(), one_half_dark());
    themes.insert("Pop Dark".to_string(), pop_dark());
    themes.insert("Selenized Black".to_string(), selenized_black());
    themes.insert("Selenized Dark".to_string(), selenized_dark());
    themes.insert("Selenized Light".to_string(), selenized_light());
    themes.insert("Selenized White".to_string(), selenized_white());
    themes
}
