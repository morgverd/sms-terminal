use ratatui::layout::Alignment;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;
use super::centered_rect;

pub struct ErrorView;
impl ErrorView {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame, error_message: &str, theme: &Theme) {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(" Error ")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Thick)
            .border_style(theme.error_style());

        let error_text = Paragraph::new(error_message)
            .style(theme.error_style())
            .wrap(Wrap { trim: true })
            .block(block);

        frame.render_widget(error_text, area);
    }
}