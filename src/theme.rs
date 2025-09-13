use ratatui::style::{Color, Style};
use ratatui::style::palette::tailwind;
use serde::{Deserialize, Serialize};

#[derive(clap::ValueEnum, Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PresetTheme {
    Emerald,
    Blue,
    Zinc,
    Indigo,
    Red,
    Amber,
    Pink
}
impl PresetTheme {
    pub fn palette(&self) -> tailwind::Palette {
        match self {
            PresetTheme::Emerald => tailwind::EMERALD,
            PresetTheme::Blue => tailwind::BLUE,
            PresetTheme::Zinc => tailwind::ZINC,
            PresetTheme::Indigo => tailwind::INDIGO,
            PresetTheme::Red => tailwind::RED,
            PresetTheme::Amber => tailwind::AMBER,
            PresetTheme::Pink => tailwind::PINK
        }
    }

    pub fn variants() -> &'static [PresetTheme] {
        &[
            PresetTheme::Emerald,
            PresetTheme::Blue,
            PresetTheme::Zinc,
            PresetTheme::Indigo,
            PresetTheme::Red,
            PresetTheme::Amber,
            PresetTheme::Pink
        ]
    }
}
impl Default for PresetTheme {
    fn default() -> Self {
        PresetTheme::Emerald
    }
}

#[derive(Clone)]
pub struct Theme {
    // Base colors
    pub bg: Color,

    // Component colors
    pub header_bg: Color,
    pub header_fg: Color,
    pub border: Color,
    pub border_focused: Color,

    // Text colors
    pub text_primary: Color,
    pub text_secondary: Color,
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
    pub input_bg: Color,
    pub input_fg: Color,
    pub input_cursor: Color
}
impl Theme {
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

    fn themed_background(palette: &tailwind::Palette) -> Self {
        Self {
            // Base
            bg: palette.c950,

            // Component
            header_bg: palette.c900,
            header_fg: palette.c100,
            border: palette.c400,
            border_focused: palette.c500,

            // Text
            text_primary: palette.c100,
            text_secondary: palette.c300,
            text_muted: palette.c400,
            text_accent: palette.c400,
            text_error: tailwind::RED.c400,

            // Table
            row_normal_bg: palette.c950,
            row_alt_bg: palette.c900,
            row_selected_fg: palette.c300,
            column_selected_fg: palette.c300,
            cell_selected_fg: palette.c500,

            // Input
            input_bg: palette.c900,
            input_fg: palette.c200,
            input_cursor: palette.c400,
        }
    }

    fn static_background(palette: &tailwind::Palette) -> Self {
        Self {
            // Base
            bg: tailwind::SLATE.c950,

            // Component
            header_bg: palette.c900,
            header_fg: tailwind::SLATE.c200,
            border: palette.c400,
            border_focused: palette.c500,

            // Text
            text_primary: tailwind::SLATE.c200,
            text_secondary: tailwind::SLATE.c400,
            text_muted: tailwind::SLATE.c500,
            text_accent: palette.c400,
            text_error: tailwind::RED.c400,

            // Table
            row_normal_bg: tailwind::SLATE.c950,
            row_alt_bg: tailwind::SLATE.c900,
            row_selected_fg: palette.c400,
            column_selected_fg: palette.c400,
            cell_selected_fg: palette.c600,

            // Input
            input_bg: tailwind::SLATE.c900,
            input_fg: palette.c300,
            input_cursor: palette.c500,
        }
    }

    pub fn primary_style(&self) -> Style {
        Style::default().fg(self.text_primary).bg(self.bg)
    }

    pub fn secondary_style(&self) -> Style {
        Style::default().fg(self.text_secondary).bg(self.bg)
    }

    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.text_accent)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.text_error)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.border_focused)
    }

    pub fn input_style(&self) -> Style {
        Style::default().fg(self.input_fg).bg(self.input_bg)
    }
}
impl From<&PresetTheme> for Theme {
    fn from(preset: &PresetTheme) -> Self {
        Self::new(&preset.palette())
    }
}

pub struct ThemeManager {
    modify_background: bool,
    static_themes: Vec<Option<Theme>>,
    dynamic_themes: Vec<Option<Theme>>,
    current_index: usize
}
impl ThemeManager {
    pub fn with_preset(preset: PresetTheme) -> Self {
        let num_themes = PresetTheme::variants().len();

        Self {
            modify_background: true,
            static_themes: vec![None; num_themes],
            dynamic_themes: vec![None; num_themes],
            current_index: PresetTheme::variants()
                .iter()
                .position(|&p| std::mem::discriminant(&p) == std::mem::discriminant(&preset))
                .unwrap_or(0)
        }
    }

    /// Get the current theme, lazily loading it if it doesn't yet exist.
    pub fn current(&mut self) -> &Theme {
        let preset = PresetTheme::variants()[self.current_index];
        let palette = preset.palette();

        let (theme_cache, use_dynamic) = if self.modify_background {
            (&mut self.dynamic_themes, true)
        } else {
            (&mut self.static_themes, false)
        };

        theme_cache[self.current_index]
            .get_or_insert_with(|| Theme::with_mode(&palette, use_dynamic))
    }

    pub fn next(&mut self) {
        self.current_index = (self.current_index + 1) % self.static_themes.len();
    }

    pub fn toggle_modify_background(&mut self) {
        self.modify_background = !self.modify_background;
    }
}