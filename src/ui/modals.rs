use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::centered_rect;

pub trait Modal {
    fn render_base(
        &self,
        frame: &mut Frame,
        title: &str,
        content: impl FnOnce(&mut Frame, Rect, &Theme),
        theme: &Theme,
        width: u16,
        height: u16,
    ) {
        let area = centered_rect(width, height, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(format!(" {} ", title))
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Double)
            .border_style(theme.border_focused_style())
            .style(theme.primary_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        content(frame, inner, theme);
    }

    fn render_buttons(&self, frame: &mut Frame, area: Rect, buttons: &[ModalButton], selected_index: usize) {
        let mut button_spans = Vec::new();

        button_spans.push(Span::raw("    "));
        for (i, button) in buttons.iter().enumerate() {
            if i > 0 {
                button_spans.push(Span::raw("     "));
            }

            let style = button.render_style(i == selected_index);
            button_spans.push(Span::styled(
                format!("  {}  ", button.label),
                style
            ));
        }
        button_spans.push(Span::raw("    "));

        let buttons_line = Line::from(button_spans);
        let buttons_paragraph = Paragraph::new(buttons_line)
            .alignment(Alignment::Center);
        frame.render_widget(buttons_paragraph, area);
    }

    /// Create styled buttons for OK/Cancel pattern
    fn create_ok_cancel_buttons(&self, button_styles: &ButtonStyles) -> [ModalButton; 2] {
        [
            ModalButton::new("OK").with_styles(
                button_styles.primary_normal,
                button_styles.primary_focused
            ),
            ModalButton::new("Cancel").with_styles(
                button_styles.secondary_normal,
                button_styles.secondary_focused
            ),
        ]
    }

    /// Create styled buttons for Yes/No pattern
    fn create_yes_no_buttons(&self, button_styles: &ButtonStyles) -> [ModalButton; 2] {
        [
            ModalButton::new("Yes").with_styles(
                button_styles.primary_normal,
                button_styles.primary_focused
            ),
            ModalButton::new("No").with_styles(
                button_styles.secondary_normal,
                button_styles.secondary_focused
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModalButton {
    pub label: String,
    pub style_normal: Style,
    pub style_focused: Style,
}
impl ModalButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style_normal: Style::default(),
            style_focused: Style::default(),
        }
    }

    pub fn with_styles(mut self, normal: Style, focused: Style) -> Self {
        self.style_normal = normal;
        self.style_focused = focused;
        self
    }

    pub fn render_style(&self, focused: bool) -> Style {
        if focused {
            self.style_focused
        } else {
            self.style_normal
        }
    }
}

#[derive(Debug, Clone)]
pub struct ButtonStyles {
    pub primary_normal: Style,
    pub primary_focused: Style,
    pub secondary_normal: Style,
    pub secondary_focused: Style,
}
impl ButtonStyles {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            primary_normal: theme.secondary_style(),
            primary_focused: Style::default()
                .fg(theme.bg)
                .bg(theme.text_accent)
                .add_modifier(Modifier::BOLD),
            secondary_normal: theme.secondary_style(),
            secondary_focused: Style::default()
                .fg(theme.bg)
                .bg(theme.text_error)
                .add_modifier(Modifier::BOLD),
        }
    }
}

/// Confirmation with Yes/No buttons
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmationModal {
    pub message: String,
    pub selected_yes: bool,
}
impl ConfirmationModal {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            selected_yes: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.selected_yes = !self.selected_yes;
                None
            }
            KeyCode::Enter => Some(self.selected_yes),
            KeyCode::Esc => Some(false),
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.selected_yes = true;
                Some(true)
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.selected_yes = false;
                Some(false)
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let button_styles = ButtonStyles::from_theme(theme);
        let styled_buttons = self.create_yes_no_buttons(&button_styles);

        self.render_base(
            frame,
            "Confirm",
            |frame, area, theme| {
                let layout = Layout::vertical([
                    Constraint::Length(2),   // Message
                    Constraint::Min(1),      // Spacer
                    Constraint::Length(2),   // Buttons
                    Constraint::Length(1),   // Help text
                ])
                    .split(area);

                // Message
                let message = Paragraph::new(self.message.as_str())
                    .style(theme.primary_style())
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false });
                frame.render_widget(message, layout[0]);

                // Buttons
                let selected_index = if self.selected_yes { 0 } else { 1 };
                self.render_buttons(frame, layout[2], &styled_buttons, selected_index);

                // Help text
                let help = Paragraph::new("(←/→) select | (Enter) confirm | (Esc) cancel")
                    .style(theme.secondary_style())
                    .alignment(Alignment::Center);
                frame.render_widget(help, layout[3]);
            },
            theme,
            50,
            15,
        );
    }
}
impl Modal for ConfirmationModal {}

/// Text input with OK/Cancel buttons
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputModal {
    pub title: String,
    pub prompt: String,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub selected_ok: bool,
    pub placeholder: String,
    pub max_length: Option<usize>,
}
impl TextInputModal {

    const BASE_HEIGHT: u16 = 12;
    const MINIMUM_HEIGHT: u16 = 6;

    pub fn new(title: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            input_buffer: String::new(),
            cursor_position: 0,
            selected_ok: true,
            placeholder: String::new(),
            max_length: None,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn with_initial_value(mut self, value: impl Into<String>) -> Self {
        self.input_buffer = value.into();
        self.cursor_position = self.input_buffer.len();
        self
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<bool> {
        match key.code {
            KeyCode::Esc => Some(false),
            KeyCode::Tab => {
                self.selected_ok = !self.selected_ok;
                None
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(self.selected_ok)
            }
            KeyCode::Enter => {
                if self.selected_ok && !self.input_buffer.trim().is_empty() {
                    Some(true)
                } else if !self.selected_ok {
                    Some(false)
                } else {
                    None
                }
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                self.selected_ok = true;
                None
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                self.selected_ok = false;
                None
            }
            KeyCode::Left => {
                self.cursor_position = self.cursor_position.saturating_sub(1);
                None
            }
            KeyCode::Right => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
                None
            }
            KeyCode::Home => {
                self.cursor_position = 0;
                None
            }
            KeyCode::End => {
                self.cursor_position = self.input_buffer.len();
                None
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
                None
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
                None
            }
            KeyCode::Char(c) => {
                if let Some(max) = self.max_length {
                    if self.input_buffer.len() >= max {
                        return None;
                    }
                }
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                None
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let available_height = frame.area().height.saturating_sub(4); // Leave some margin
        let modal_height = Self::BASE_HEIGHT.max(available_height.min(25)); // Cap at reasonable max

        let button_styles = ButtonStyles::from_theme(theme);
        let styled_buttons = self.create_ok_cancel_buttons(&button_styles);

        self.render_base(
            frame,
            &self.title,
            |frame, area, theme| {
                let with_spacer = Self::MINIMUM_HEIGHT + 1; // 7
                let with_counter = with_spacer + 1; // 8 (only if max_length is set)
                let with_help = (if self.max_length.is_some() { with_counter } else { with_spacer }) + 1; // 9 or 8

                // Determine if optional components can be shown
                let show_help = area.height >= with_help;
                let show_counter = self.max_length.is_some() && area.height >= with_counter;
                let show_spacer = area.height >= with_spacer;

                let mut constraints = vec![
                    Constraint::Length(1), // Prompt (fixed)
                    Constraint::Length(3), // Input box (fixed)
                ];

                // Add optional components
                if show_counter {
                    constraints.push(Constraint::Length(1)); // Character count
                }
                if show_spacer {
                    constraints.push(Constraint::Min(1)); // Spacer
                }
                constraints.push(Constraint::Length(2)); // Buttons (fixed)
                if show_help {
                    constraints.push(Constraint::Length(1)); // Help text
                }

                let layout = Layout::vertical(constraints).split(area);
                let mut layout_index = 0;

                // Prompt (fixed)
                let prompt = Paragraph::new(self.prompt.as_str())
                    .style(theme.secondary_style());
                frame.render_widget(prompt, layout[layout_index]);
                layout_index += 1;

                // Input box with cursor
                let input_text = if self.input_buffer.is_empty() {
                    vec![Line::from(vec![
                        Span::styled(&self.placeholder, Style::default().fg(theme.text_muted)),
                        Span::styled("█", Style::default()
                            .fg(theme.input_cursor)
                            .add_modifier(Modifier::SLOW_BLINK))
                    ])]
                } else {
                    self.render_text_with_cursor(theme)
                };

                let input_style = if self.input_buffer.is_empty() {
                    Style::default().fg(theme.text_muted)
                } else {
                    theme.input_style()
                };

                let input = Paragraph::new(input_text)
                    .style(input_style)
                    .block(
                        Block::bordered()
                            .border_style(theme.border_focused_style())
                    );
                frame.render_widget(input, layout[layout_index]);
                layout_index += 1;

                // Character count (only if max_length is set and there's space)
                if show_counter {
                    let count_text = format!("{}/{}", self.input_buffer.len(), self.max_length.unwrap());
                    let count_style = if self.input_buffer.len() >= self.max_length.unwrap() {
                        theme.error_style()
                    } else {
                        theme.secondary_style()
                    };
                    let count = Paragraph::new(count_text)
                        .style(count_style)
                        .alignment(Alignment::Right);
                    frame.render_widget(count, layout[layout_index]);
                    layout_index += 1;
                }
                if show_spacer {
                    layout_index += 1;
                }

                // Buttons
                let selected_index = if self.selected_ok { 0 } else { 1 };
                self.render_buttons(frame, layout[layout_index], &styled_buttons, selected_index);
                layout_index += 1;

                // Help text
                if show_help {
                    let help = Paragraph::new("(Tab/Alt+←→) switch | (Enter) confirm | (Esc) cancel")
                        .style(theme.secondary_style())
                        .alignment(Alignment::Center);
                    frame.render_widget(help, layout[layout_index]);
                }
            },
            theme,
            55,
            modal_height
        );
    }

    fn render_text_with_cursor(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut spans = Vec::new();

        if self.cursor_position > 0 {
            spans.push(Span::raw(self.input_buffer[..self.cursor_position].to_string()));
        }
        if self.cursor_position < self.input_buffer.len() {
            spans.push(Span::styled(
                self.input_buffer.chars().nth(self.cursor_position).unwrap().to_string(),
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.input_cursor)
                    .add_modifier(Modifier::SLOW_BLINK)
            ));

            if self.cursor_position + 1 < self.input_buffer.len() {
                spans.push(Span::raw(self.input_buffer[self.cursor_position + 1..].to_string()));
            }
        } else {
            spans.push(Span::styled(
                "█",
                Style::default()
                    .fg(theme.input_cursor)
                    .add_modifier(Modifier::SLOW_BLINK)
            ));
        }

        vec![Line::from(spans)]
    }

    pub fn get_input(&self) -> &str {
        &self.input_buffer
    }
}
impl Modal for TextInputModal {}