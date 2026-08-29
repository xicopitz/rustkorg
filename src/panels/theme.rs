use egui::Color32;

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub bg_primary: Color32,
    pub bg_secondary: Color32,
    pub bg_tertiary: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent_blue: Color32,
    pub accent_orange: Color32,
    pub accent_red: Color32,
    pub accent_green: Color32,
    pub accent_cyan: Color32,
    pub accent_violet: Color32,
    pub accent_gold: Color32,
    pub accent_pink: Color32,
    pub border: Color32,
}

pub const DEFAULT: ThemePalette = ThemePalette {
    bg_primary: Color32::from_rgb(12, 14, 22),
    bg_secondary: Color32::from_rgb(22, 27, 38),
    bg_tertiary: Color32::from_rgb(34, 42, 58),
    text_primary: Color32::from_rgb(240, 244, 255),
    text_secondary: Color32::from_rgb(157, 174, 204),
    text_muted: Color32::from_rgb(116, 128, 150),
    accent_blue: Color32::from_rgb(92, 168, 255),
    accent_orange: Color32::from_rgb(255, 166, 94),
    accent_red: Color32::from_rgb(255, 109, 109),
    accent_green: Color32::from_rgb(92, 214, 166),
    accent_cyan: Color32::from_rgb(86, 215, 238),
    accent_violet: Color32::from_rgb(163, 122, 255),
    accent_gold: Color32::from_rgb(255, 201, 92),
    accent_pink: Color32::from_rgb(255, 130, 182),
    border: Color32::from_rgb(52, 61, 80),
};

pub const MIDNIGHT: ThemePalette = ThemePalette {
    bg_primary: Color32::from_rgb(9, 11, 18),
    bg_secondary: Color32::from_rgb(18, 22, 31),
    bg_tertiary: Color32::from_rgb(30, 36, 48),
    text_primary: Color32::from_rgb(236, 239, 246),
    text_secondary: Color32::from_rgb(149, 162, 184),
    text_muted: Color32::from_rgb(107, 119, 142),
    accent_blue: Color32::from_rgb(83, 146, 255),
    accent_orange: Color32::from_rgb(255, 156, 92),
    accent_red: Color32::from_rgb(255, 98, 98),
    accent_green: Color32::from_rgb(88, 200, 164),
    accent_cyan: Color32::from_rgb(83, 207, 238),
    accent_violet: Color32::from_rgb(160, 108, 255),
    accent_gold: Color32::from_rgb(255, 196, 82),
    accent_pink: Color32::from_rgb(255, 123, 186),
    border: Color32::from_rgb(42, 48, 62),
};

pub const SUNSET: ThemePalette = ThemePalette {
    bg_primary: Color32::from_rgb(24, 15, 19),
    bg_secondary: Color32::from_rgb(43, 23, 31),
    bg_tertiary: Color32::from_rgb(60, 34, 42),
    text_primary: Color32::from_rgb(255, 240, 244),
    text_secondary: Color32::from_rgb(226, 170, 180),
    text_muted: Color32::from_rgb(168, 115, 126),
    accent_blue: Color32::from_rgb(116, 154, 255),
    accent_orange: Color32::from_rgb(255, 154, 84),
    accent_red: Color32::from_rgb(255, 101, 120),
    accent_green: Color32::from_rgb(124, 214, 170),
    accent_cyan: Color32::from_rgb(128, 223, 255),
    accent_violet: Color32::from_rgb(195, 120, 255),
    accent_gold: Color32::from_rgb(255, 196, 94),
    accent_pink: Color32::from_rgb(255, 119, 167),
    border: Color32::from_rgb(94, 59, 67),
};

pub const OCEAN: ThemePalette = ThemePalette {
    bg_primary: Color32::from_rgb(7, 18, 22),
    bg_secondary: Color32::from_rgb(13, 30, 35),
    bg_tertiary: Color32::from_rgb(21, 46, 52),
    text_primary: Color32::from_rgb(234, 247, 252),
    text_secondary: Color32::from_rgb(148, 212, 224),
    text_muted: Color32::from_rgb(96, 141, 156),
    accent_blue: Color32::from_rgb(80, 176, 255),
    accent_orange: Color32::from_rgb(255, 170, 95),
    accent_red: Color32::from_rgb(255, 120, 120),
    accent_green: Color32::from_rgb(88, 214, 180),
    accent_cyan: Color32::from_rgb(90, 222, 238),
    accent_violet: Color32::from_rgb(136, 136, 255),
    accent_gold: Color32::from_rgb(255, 208, 102),
    accent_pink: Color32::from_rgb(255, 140, 201),
    border: Color32::from_rgb(39, 73, 81),
};

pub const BG_PRIMARY: Color32 = DEFAULT.bg_primary;
pub const BG_SECONDARY: Color32 = DEFAULT.bg_secondary;
pub const BG_TERTIARY: Color32 = DEFAULT.bg_tertiary;
pub const TEXT_PRIMARY: Color32 = DEFAULT.text_primary;
pub const TEXT_SECONDARY: Color32 = DEFAULT.text_secondary;
pub const TEXT_MUTED: Color32 = DEFAULT.text_muted;
pub const ACCENT_BLUE: Color32 = DEFAULT.accent_blue;
pub const ACCENT_ORANGE: Color32 = DEFAULT.accent_orange;
pub const ACCENT_RED: Color32 = DEFAULT.accent_red;
pub const ACCENT_GREEN: Color32 = DEFAULT.accent_green;
pub const ACCENT_CYAN: Color32 = DEFAULT.accent_cyan;
pub const ACCENT_VIOLET: Color32 = DEFAULT.accent_violet;
pub const ACCENT_GOLD: Color32 = DEFAULT.accent_gold;
pub const ACCENT_PINK: Color32 = DEFAULT.accent_pink;
pub const BORDER: Color32 = DEFAULT.border;

pub fn palette_for_name(name: &str) -> ThemePalette {
    match name.to_ascii_lowercase().as_str() {
        "default" | "dark" => DEFAULT,
        "midnight" => MIDNIGHT,
        "sunset" => SUNSET,
        "ocean" => OCEAN,
        "light" => ThemePalette {
            bg_primary: Color32::from_rgb(245, 247, 251),
            bg_secondary: Color32::from_rgb(233, 237, 245),
            bg_tertiary: Color32::from_rgb(220, 225, 235),
            text_primary: Color32::from_rgb(20, 24, 31),
            text_secondary: Color32::from_rgb(71, 92, 118),
            text_muted: Color32::from_rgb(108, 122, 141),
            accent_blue: Color32::from_rgb(36, 110, 214),
            accent_orange: Color32::from_rgb(220, 132, 48),
            accent_red: Color32::from_rgb(201, 69, 82),
            accent_green: Color32::from_rgb(28, 154, 108),
            accent_cyan: Color32::from_rgb(26, 154, 197),
            accent_violet: Color32::from_rgb(127, 89, 227),
            accent_gold: Color32::from_rgb(204, 160, 54),
            accent_pink: Color32::from_rgb(215, 87, 160),
            border: Color32::from_rgb(194, 201, 214),
        },
        _ => DEFAULT,
    }
}
