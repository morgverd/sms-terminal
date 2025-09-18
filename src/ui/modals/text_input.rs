use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Paragraph};
use crate::modals::ModalResponse;
use crate::theme::Theme;
use crate::ui::modals::{ModalButtonComponentStyles, ModalComponent, ModalButtonComponent, ModalUtils};

/// Text input with OK/Cancel buttons
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputModal {
    pub title: String,
    pub prompt: String,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub selected_ok: bool,
    pub placeholder: String,
    pub max_length: Option<usize>
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
            max_length: None
        }
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
}
impl ModalComponent for TextInputModal {

    fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResponse> {
        match key.code {
            KeyCode::Esc => {
                return Some(ModalResponse::Dismissed)
            },
            KeyCode::Tab => {
                self.selected_ok = !self.selected_ok;
            },
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(ModalResponse::TextInput(Some(self.input_buffer.clone())))
            },
            KeyCode::Enter => {
                return if self.selected_ok && !self.input_buffer.trim().is_empty() {
                    Some(ModalResponse::TextInput(Some(self.input_buffer.clone())))
                } else if !self.selected_ok {
                    Some(ModalResponse::Dismissed)
                } else {
                    None
                }
            },
            KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                self.selected_ok = true;
            },
            KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                self.selected_ok = false;
            },
            KeyCode::Left => {
                self.cursor_position = self.cursor_position.saturating_sub(1);
            },
            KeyCode::Right => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
            },
            KeyCode::Home => {
                self.cursor_position = 0;
            },
            KeyCode::End => {
                self.cursor_position = self.input_buffer.len();
            },
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            },
            KeyCode::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
            },
            KeyCode::Char(c) => {
                if let Some(max) = self.max_length {
                    if self.input_buffer.len() >= max {
                        return None;
                    }
                }
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            _ => { }
        }

        None
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let available_height = frame.area().height.saturating_sub(4); // Leave some margin
        let modal_height = Self::BASE_HEIGHT.max(available_height.min(25)); // Cap at reasonable max

        let button_styles = ModalButtonComponentStyles::from_theme(theme);
        let styled_buttons = ModalButtonComponent::create_ok_cancel_buttons(&button_styles);

        ModalUtils::render_base(
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
                ModalUtils::render_buttons(frame, layout[layout_index], &styled_buttons, selected_index);
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
            40,
            modal_height
        );
    }
}