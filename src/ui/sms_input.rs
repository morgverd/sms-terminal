use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::style::palette::tailwind;

use crate::theme::Theme;
use super::centered_rect;

pub struct SmsInputView {
    cursor_position: usize,
    multiline_mode: bool,
}
impl SmsInputView {
    pub fn new() -> Self {
        Self {
            cursor_position: 0,
            multiline_mode: true,
        }
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn set_cursor_position(&mut self, pos: usize, text_len: usize) {
        self.cursor_position = pos.min(text_len);
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self, text_len: usize) {
        if self.cursor_position < text_len {
            self.cursor_position += 1;
        }
    }

    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    pub fn move_cursor_to_end(&mut self, text_len: usize) {
        self.cursor_position = text_len;
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        phone_number: &str,
        message_text: &str,
        char_count: usize,
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
            Constraint::Length(2),   // Instructions
            Constraint::Min(10),     // Text area
            Constraint::Length(2),   // Character count
            Constraint::Length(2),   // Help text
        ])
            .split(inner);

        // Instructions
        let instructions = Paragraph::new("Type your message below:")
            .style(theme.secondary_style())
            .alignment(Alignment::Left);
        frame.render_widget(instructions, layout[0]);

        // Text area with cursor
        let text_with_cursor = self.render_text_with_cursor(message_text, theme);

        let text_area = Paragraph::new(text_with_cursor)
            .style(theme.input_style())
            .block(
                Block::bordered()
                    .border_style(theme.border_focused_style())
                    .border_type(BorderType::Rounded)
            )
            .wrap(Wrap { trim: false })
            .scroll((0, 0));

        frame.render_widget(text_area, layout[1]);

        // Character counter
        // We can send massive messages since SMS-API supports message concatenation,
        // so the limit doesn't actually stop anything it just shows that it's a bit long.
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
        frame.render_widget(char_counter, layout[2]);

        // Help text
        let help_text = if self.multiline_mode {
            "Ctrl+Enter to send | Esc to cancel | Enter for new line"
        } else {
            "Enter to send | Esc to cancel | Ctrl+Enter for new line"
        };

        let help = Paragraph::new(help_text)
            .style(theme.secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[3]);
    }

    fn render_text_with_cursor(&self, text: &str, theme: &Theme) -> Vec<Line<'static>> {
        if text.is_empty() {
            return vec![Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ])];
        }

        let mut lines = Vec::new();
        let text_lines: Vec<&str> = text.lines().collect();

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

        if self.cursor_position == text.len() && text.ends_with('\n') {
            lines.push(Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ]));
        }

        lines
    }
}