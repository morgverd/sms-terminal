use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(clap::ValueEnum, Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum PresetTheme {
    #[default]
    Emerald = 0,
    Blue = 1,
    Zinc = 2,
    Indigo = 3,
    Red = 4,
    Amber = 5,
    Pink = 6,
}
impl PresetTheme {
    pub const COUNT: usize = 7;

    pub const VARIANTS: &'static [PresetTheme] = &[
        PresetTheme::Emerald,
        PresetTheme::Blue,
        PresetTheme::Zinc,
        PresetTheme::Indigo,
        PresetTheme::Red,
        PresetTheme::Amber,
        PresetTheme::Pink,
    ];

    pub const fn palette(self) -> tailwind::Palette {
        match self {
            PresetTheme::Emerald => tailwind::EMERALD,
            PresetTheme::Blue => tailwind::BLUE,
            PresetTheme::Zinc => tailwind::ZINC,
            PresetTheme::Indigo => tailwind::INDIGO,
            PresetTheme::Red => tailwind::RED,
            PresetTheme::Amber => tailwind::AMBER,
            PresetTheme::Pink => tailwind::PINK,
        }
    }

    #[inline]
    pub const fn as_index(self) -> usize {
        self as usize
    }
}

pub struct Theme {
    // Base colors
    pub bg: Color,

    // Component colors
    pub header_bg: Color,
    pub header_fg: Color,
    pub border: Color,

    // Text colors
    pub text_primary: Color,
    pub text_muted: Color,
    pub text_accent: Color,
    pub text_error: Color,

    // Table specific
    pub row_normal_bg: Color,
    pub row_alt_bg: Color,
    pub row_selected_fg: Color,
    pub column_selected_fg: Color,
    pub cell_selected_fg: Color,

    // Input specific
    pub input_cursor: Color,

    // Styles
    pub primary_style: Style,
    pub secondary_style: Style,
    pub accent_style: Style,
    pub error_style: Style,
    pub border_style: Style,
    pub border_focused_style: Style,
    pub input_style: Style,
}
impl Theme {
    #[inline]
    pub fn new(palette: &tailwind::Palette) -> Self {
        Self::with_mode(palette, false)
    }

    pub fn with_mode(palette: &tailwind::Palette, modify_background: bool) -> Self {
        if modify_background {
            Self::themed_background(palette)
        } else {
            Self::static_background(palette)
        }
    }

    #[inline(never)]
    fn themed_background(palette: &tailwind::Palette) -> Self {
        let bg = palette.c950;
        let text_primary = palette.c100;
        let text_secondary = palette.c300;
        let text_accent = palette.c400;
        let text_error = tailwind::RED.c400;
        let border = palette.c400;
        let border_focused = palette.c500;
        let input_bg = palette.c900;
        let input_fg = palette.c200;

        Self {
            // Base
            bg,

            // Component
            header_bg: palette.c900,
            header_fg: palette.c100,
            border,

            // Text
            text_primary,
            text_muted: palette.c400,
            text_accent,
            text_error,

            // Table
            row_normal_bg: palette.c950,
            row_alt_bg: palette.c900,
            row_selected_fg: palette.c300,
            column_selected_fg: palette.c300,
            cell_selected_fg: palette.c500,

            // Input
            input_cursor: palette.c400,

            // Styles
            primary_style: Style::default().fg(text_primary).bg(bg),
            secondary_style: Style::default().fg(text_secondary).bg(bg),
            accent_style: Style::default().fg(text_accent),
            error_style: Style::default().fg(text_error),
            border_style: Style::default().fg(border),
            border_focused_style: Style::default().fg(border_focused),
            input_style: Style::default().fg(input_fg).bg(input_bg),
        }
    }

    #[inline(never)]
    fn static_background(palette: &tailwind::Palette) -> Self {
        const SLATE_950: Color = tailwind::SLATE.c950;
        const SLATE_200: Color = tailwind::SLATE.c200;
        const SLATE_400: Color = tailwind::SLATE.c400;
        const SLATE_500: Color = tailwind::SLATE.c500;
        const SLATE_900: Color = tailwind::SLATE.c900;
        const RED_400: Color = tailwind::RED.c400;

        let text_accent = palette.c400;
        let border = palette.c400;
        let border_focused = palette.c500;
        let input_fg = palette.c300;

        Self {
            // Base
            bg: SLATE_950,

            // Component
            header_bg: palette.c900,
            header_fg: SLATE_200,
            border,

            // Text
            text_primary: SLATE_200,
            text_muted: SLATE_500,
            text_accent,
            text_error: RED_400,

            // Table
            row_normal_bg: SLATE_950,
            row_alt_bg: SLATE_900,
            row_selected_fg: palette.c400,
            column_selected_fg: palette.c400,
            cell_selected_fg: palette.c600,

            // Input
            input_cursor: palette.c500,

            // Styles
            primary_style: Style::default().fg(SLATE_200).bg(SLATE_950),
            secondary_style: Style::default().fg(SLATE_400).bg(SLATE_950),
            accent_style: Style::default().fg(text_accent),
            error_style: Style::default().fg(RED_400),
            border_style: Style::default().fg(border),
            border_focused_style: Style::default().fg(border_focused),
            input_style: Style::default().fg(input_fg).bg(SLATE_900),
        }
    }
}

impl From<&PresetTheme> for Theme {
    #[inline]
    fn from(preset: &PresetTheme) -> Self {
        Self::new(&preset.palette())
    }
}

pub struct ThemeManager {
    modify_background: bool,
    static_themes: [Option<Rc<Theme>>; PresetTheme::COUNT],
    dynamic_themes: [Option<Rc<Theme>>; PresetTheme::COUNT],
    current_preset: PresetTheme,
    current_theme: Rc<Theme>,
}
impl ThemeManager {
    pub fn with_preset(preset: PresetTheme) -> Self {
        const NONE: Option<Rc<Theme>> = None;

        let modify_background = true;
        let current_theme = Rc::new(Theme::with_mode(&preset.palette(), modify_background));

        let mut dynamic_themes = [NONE; PresetTheme::COUNT];
        dynamic_themes[preset.as_index()] = Some(current_theme.clone());

        Self {
            modify_background,
            static_themes: [NONE; PresetTheme::COUNT],
            dynamic_themes,
            current_preset: preset,
            current_theme,
        }
    }

    #[inline]
    pub fn current(&self) -> &Rc<Theme> {
        &self.current_theme
    }

    #[inline]
    pub fn next(&mut self) {
        let next_index = (self.current_preset as u8 + 1) % PresetTheme::COUNT as u8;
        self.current_preset = PresetTheme::VARIANTS[next_index as usize];
        self.update_current_theme();
    }

    #[inline]
    pub fn toggle_modify_background(&mut self) {
        self.modify_background = !self.modify_background;
        self.update_current_theme();
    }

    fn update_current_theme(&mut self) {
        let index = self.current_preset.as_index();
        let theme_cache = if self.modify_background {
            &mut self.dynamic_themes
        } else {
            &mut self.static_themes
        };

        self.current_theme = theme_cache[index]
            .get_or_insert_with(|| {
                Rc::new(Theme::with_mode(
                    &self.current_preset.palette(),
                    self.modify_background,
                ))
            })
            .clone();
    }
}
