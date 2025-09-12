use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;
use crate::types::{AppState, KeyResponse};
use super::centered_rect;

pub struct ErrorView;
impl ErrorView {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_key(&mut self, key: KeyEvent, dismissible: bool) -> Option<KeyResponse> {
        match key.code {
            KeyCode::Esc if dismissible => {
                Some(KeyResponse::SetAppState(AppState::InputPhone))
            },
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(KeyResponse::Quit)
            },
            _ => None
        }
    }

    pub fn render(&self, frame: &mut Frame, error_message: &str, dismissible: bool, theme: &Theme) {
        let area = centered_rect(60, 25, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(" Error ")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.error_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Length(1),   // Spacing
            Constraint::Min(3),      // Error message
            Constraint::Length(1),   // Spacing
            Constraint::Length(1),   // Control hints
        ]).split(inner);

        // Error message with proper styling
        let error_text = Paragraph::new(error_message)
            .style(theme.error_style())
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        frame.render_widget(error_text, layout[1]);

        // Control hints
        let help_text = if dismissible { "(Esc) dismiss, (Ctrl+C) quit" } else { "(Ctrl+C) quit" };
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[3]);
    }
}