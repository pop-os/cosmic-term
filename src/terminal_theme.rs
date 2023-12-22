use alacritty_terminal::{
    ansi::NamedColor,
    term::color::{Colors, Rgb},
};
use std::collections::HashMap;

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
    /*TODO
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

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
    /*TODO
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

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
    /*TODO
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

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
    colors[NamedColor::BrightRed] = Some(encode_rgb(0xe06c75));
    colors[NamedColor::BrightGreen] = Some(encode_rgb(0x98c379));
    colors[NamedColor::BrightYellow] = Some(encode_rgb(0xe5c07b));
    colors[NamedColor::BrightBlue] = Some(encode_rgb(0x61afef));
    colors[NamedColor::BrightMagenta] = Some(encode_rgb(0xc678dd));
    colors[NamedColor::BrightCyan] = Some(encode_rgb(0x56b6c2));
    colors[NamedColor::BrightWhite] = Some(encode_rgb(0xdcdfe4));

    // Set special colors
    colors[NamedColor::Foreground] = colors[NamedColor::BrightWhite];
    colors[NamedColor::Background] = colors[NamedColor::Black];
    colors[NamedColor::Cursor] = colors[NamedColor::BrightWhite];
    /*TODO
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

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
    /*TODO
    colors[NamedColor::DimBlack] = colors[NamedColor::];
    colors[NamedColor::DimRed] = colors[NamedColor::];
    colors[NamedColor::DimGreen] = colors[NamedColor::];
    colors[NamedColor::DimYellow] = colors[NamedColor::];
    colors[NamedColor::DimBlue] = colors[NamedColor::];
    colors[NamedColor::DimMagenta] = colors[NamedColor::];
    colors[NamedColor::DimCyan] = colors[NamedColor::];
    colors[NamedColor::DimWhite] = colors[NamedColor::];
    */
    colors[NamedColor::BrightForeground] = colors[NamedColor::BrightWhite];
    //TODO colors[NamedColor::DimForeground] = colors[NamedColor::];

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
