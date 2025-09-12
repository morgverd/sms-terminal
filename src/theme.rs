use ratatui::style::{Color, Style};
use ratatui::style::palette::tailwind;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum PresetTheme {
    Blue,
    Zinc,
    Emerald,
    Indigo,
    Red,
    Pink
}
impl PresetTheme {
    pub fn palette(&self) -> tailwind::Palette {
        match self {
            PresetTheme::Blue => tailwind::BLUE,
            PresetTheme::Zinc => tailwind::ZINC,
            PresetTheme::Emerald => tailwind::EMERALD,
            PresetTheme::Indigo => tailwind::INDIGO,
            PresetTheme::Red => tailwind::RED,
            PresetTheme::Pink => tailwind::PINK
        }
    }

    pub fn variants() -> &'static [PresetTheme] {
        &[
            PresetTheme::Blue,
            PresetTheme::Zinc,
            PresetTheme::Emerald,
            PresetTheme::Indigo,
            PresetTheme::Red,
            PresetTheme::Pink,
        ]
    }
}
impl Default for PresetTheme {
    fn default() -> Self {
        PresetTheme::Blue
    }
}

#[derive(Clone)]
pub struct Theme {
    // Base colors
    pub bg: Color,
    pub fg: Color,

    // Component colors
    pub header_bg: Color,
    pub header_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub border_error: Color,

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
    pub input_cursor: Color,
}
impl Theme {
    pub fn new(palette: &tailwind::Palette) -> Self {
        Self {
            // Base colors
            bg: tailwind::SLATE.c950,
            fg: tailwind::SLATE.c200,

            // Component colors
            header_bg: palette.c900,
            header_fg: tailwind::SLATE.c200,
            border: palette.c400,
            border_focused: palette.c500,
            border_error: tailwind::RED.c500,

            // Text colors
            text_primary: tailwind::SLATE.c200,
            text_secondary: tailwind::SLATE.c400,
            text_muted: tailwind::SLATE.c500,
            text_accent: palette.c400,
            text_error: tailwind::RED.c400,

            // Table specific
            row_normal_bg: tailwind::SLATE.c950,
            row_alt_bg: tailwind::SLATE.c900,
            row_selected_fg: palette.c400,
            column_selected_fg: palette.c400,
            cell_selected_fg: palette.c600,

            // Input specific
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
    themes: Vec<Theme>,
    current_index: usize,
}
impl ThemeManager {
    pub fn with_preset(preset: PresetTheme) -> Self {
        let themes = PresetTheme::variants()
            .iter()
            .map(|preset| Theme::from(preset))
            .collect();

        let current_index = PresetTheme::variants()
            .iter()
            .position(|&p| std::mem::discriminant(&p) == std::mem::discriminant(&preset))
            .unwrap_or(0);

        Self {
            themes,
            current_index
        }
    }

    pub fn current(&self) -> &Theme {
        &self.themes[self.current_index]
    }

    pub fn next(&mut self) {
        self.current_index = (self.current_index + 1) % self.themes.len();
    }
}