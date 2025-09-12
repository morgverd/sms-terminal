use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::style::palette::tailwind;

use crate::theme::Theme;
use crate::types::{AppState, KeyResponse};
use super::centered_rect;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmationState {
    None,
    Confirming { selected_yes: bool }
}

pub struct SmsInputView {
    cursor_position: usize,
    confirmation_state: ConfirmationState,
    sms_text_buffer: String
}
impl SmsInputView {
    pub fn new() -> Self {
        Self {
            cursor_position: 0,
            confirmation_state: ConfirmationState::None,
            sms_text_buffer: String::new()
        }
    }

    pub fn reload(&mut self) {
        self.cursor_position = 0;
        self.sms_text_buffer.clear();
        self.hide_confirmation();
    }

    pub fn handle_key(&mut self, key: KeyEvent, phone_number: &str) -> Option<KeyResponse> {
        // If confirmation dialog is showing, handle its input first
        if self.is_confirming() {
            match key.code {
                KeyCode::Esc => {
                    self.hide_confirmation();
                },
                KeyCode::Left | KeyCode::Right => {
                    self.toggle_confirmation_selection();
                },
                KeyCode::Enter => {
                    if matches!(self.confirmation_state, ConfirmationState::Confirming { selected_yes: true }) {

                        // TODO: Actually send the SMS message!
                        let state = AppState::ViewMessages(phone_number.to_string());
                        return Some(KeyResponse::SetAppState(state));
                    }
                    self.hide_confirmation();
                },
                _ => {}
            }
            return None;
        }

        // Normal SMS input handling
        match key.code {
            KeyCode::Esc => {
                let state = AppState::ViewMessages(phone_number.to_string());
                self.sms_text_buffer.clear();
                return Some(KeyResponse::SetAppState(state));
            },
            KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.sms_text_buffer.is_empty() {
                    // Show confirmation dialog
                    self.show_confirmation();
                }
            },
            KeyCode::Enter => {
                self.sms_text_buffer.push('\n');
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    let pos = self.cursor_position;
                    self.sms_text_buffer.remove(pos - 1);
                    self.move_cursor_left();
                }
            },
            KeyCode::Delete => {
                if self.cursor_position < self.sms_text_buffer.len() {
                    let pos = self.cursor_position;
                    self.sms_text_buffer.remove(pos);
                }
            },
            KeyCode::Left => {
                self.move_cursor_left();
            },
            KeyCode::Right => {
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Home => {
                self.move_cursor_to_start();
            },
            KeyCode::End => {
                self.move_cursor_to_end(self.sms_text_buffer.len());
            },
            KeyCode::Char(c) => {
                let pos = self.cursor_position;
                self.sms_text_buffer.insert(pos, c);
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            _ => {}
        }

        None
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        phone_number: &str,
        theme: &Theme
    ) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(format!(" Compose SMS to {} ", phone_number))
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_focused_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Min(10),     // Text area
            Constraint::Length(2),   // Character count
            Constraint::Length(2),   // Help text
        ])
            .split(inner);

        // Text area with cursor
        let text_with_cursor = self.render_text_with_cursor(theme);

        let text_area = Paragraph::new(text_with_cursor)
            .style(theme.input_style())
            .block(
                Block::bordered()
                    .border_style(theme.border_focused_style())
                    .border_type(BorderType::Rounded)
            )
            .wrap(Wrap { trim: false })
            .scroll((0, 0));

        frame.render_widget(text_area, layout[0]);

        // Character counter
        let char_count = self.sms_text_buffer.chars().count();
        let (counter_style, counter_text) = if char_count <= 160 {
            (theme.accent_style().bg(theme.bg), format!("{}/160 (1 SMS)", char_count))
        } else if char_count <= 320 {
            (Style::default().fg(tailwind::YELLOW.c400).bg(theme.bg), format!("{}/320 (2 SMS parts)", char_count))
        } else {
            let sms_count = (char_count + 159) / 160;
            (theme.error_style().bg(theme.bg), format!("{}/{} ({} SMS parts)", char_count, sms_count * 160, sms_count))
        };

        let char_counter = Paragraph::new(counter_text)
            .style(counter_style)
            .alignment(Alignment::Right);
        frame.render_widget(char_counter, layout[1]);

        // Help text
        let help = Paragraph::new("(Enter) new line | (Ctrl+Space) send | (Esc) cancel")
            .style(theme.secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[2]);

        // Render confirmation dialog if active
        if self.is_confirming() {
            self.render_confirmation_dialog(frame, theme);
        }
    }

    fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    fn move_cursor_right(&mut self, text_len: usize) {
        if self.cursor_position < text_len {
            self.cursor_position += 1;
        }
    }

    fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    fn move_cursor_to_end(&mut self, text_len: usize) {
        self.cursor_position = text_len;
    }

    fn is_confirming(&self) -> bool {
        matches!(self.confirmation_state, ConfirmationState::Confirming { .. })
    }

    fn show_confirmation(&mut self) {
        self.confirmation_state = ConfirmationState::Confirming { selected_yes: false };
    }

    fn hide_confirmation(&mut self) {
        self.confirmation_state = ConfirmationState::None;
    }

    fn toggle_confirmation_selection(&mut self) {
        if let ConfirmationState::Confirming { selected_yes } = &mut self.confirmation_state {
            *selected_yes = !*selected_yes;
        }
    }

    fn render_confirmation_dialog(&self, frame: &mut Frame, theme: &Theme) {
        let dialog_area = centered_rect(50, 15, frame.area()); // Reduced from 30 to 15
        frame.render_widget(Clear, dialog_area);

        let block = Block::bordered()
            .title(" Confirm Send ")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Double)
            .border_style(theme.border_focused_style())
            .style(theme.primary_style());

        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);

        let layout = Layout::vertical([
            Constraint::Length(2),   // Question
            Constraint::Min(1),      // Flexible spacer that takes remaining space
            Constraint::Length(2),   // Buttons
            Constraint::Length(1),   // Help text at bottom
        ])
            .split(inner);

        let question = Paragraph::new("Are you sure you want to send this SMS?")
            .style(theme.primary_style())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        frame.render_widget(question, layout[0]);

        let (yes_style, no_style) = if let ConfirmationState::Confirming { selected_yes } = self.confirmation_state {
            if selected_yes {
                (
                    Style::default()
                        .fg(theme.bg)
                        .bg(theme.text_accent)
                        .add_modifier(Modifier::BOLD),
                    theme.secondary_style()
                )
            } else {
                (
                    theme.secondary_style(),
                    Style::default()
                        .fg(theme.bg)
                        .bg(theme.text_error)
                        .add_modifier(Modifier::BOLD)
                )
            }
        } else {
            (theme.secondary_style(), theme.secondary_style())
        };

        let buttons = Line::from(vec![
            Span::raw("    "),
            Span::styled("  Yes  ", yes_style),
            Span::raw("     "),
            Span::styled("  No  ", no_style),
            Span::raw("    "),
        ]);

        let buttons_paragraph = Paragraph::new(buttons)
            .style(theme.primary_style())
            .alignment(Alignment::Center);
        frame.render_widget(buttons_paragraph, layout[2]);

        let help = Paragraph::new("(←/→) select | (Enter) confirm | (Esc) cancel")
            .style(theme.secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[3]);
    }

    fn render_text_with_cursor(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.sms_text_buffer.is_empty() {
            return vec![Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ])];
        }

        let mut lines = Vec::new();
        let text_lines: Vec<&str> = self.sms_text_buffer.lines().collect();

        let mut char_count = 0;
        for line in text_lines.iter() {
            let line_start = char_count;
            let line_end = line_start + line.len();

            let mut spans = Vec::new();

            if self.cursor_position >= line_start && self.cursor_position <= line_end {
                let cursor_pos_in_line = self.cursor_position - line_start;

                if cursor_pos_in_line > 0 {
                    spans.push(Span::raw(line[..cursor_pos_in_line].to_string()));
                }

                if cursor_pos_in_line < line.len() {
                    spans.push(Span::styled(
                        line.chars().nth(cursor_pos_in_line).unwrap().to_string(),
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.input_cursor)
                            .add_modifier(Modifier::SLOW_BLINK)
                    ));

                    if cursor_pos_in_line + 1 < line.len() {
                        spans.push(Span::raw(line[cursor_pos_in_line + 1..].to_string()));
                    }
                } else {
                    spans.push(Span::styled(
                        "█",
                        Style::default()
                            .fg(theme.input_cursor)
                            .add_modifier(Modifier::SLOW_BLINK)
                    ));
                }
            } else {
                spans.push(Span::raw(line.to_string()));
            }

            lines.push(Line::from(spans));
            char_count = line_end + 1;
        }

        if self.cursor_position == self.sms_text_buffer.len() && self.sms_text_buffer.ends_with('\n') {
            lines.push(Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ]));
        }

        lines
    }
}