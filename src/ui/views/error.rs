use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::AppAction;
use crate::ui::views::ViewStateRequest;
use crate::ui::{centered_rect, ViewBase};

pub struct ErrorView;
impl ErrorView {
    pub fn new() -> Self {
        Self
    }
}
impl ViewBase for ErrorView {
    type Context<'ctx> = (&'ctx String, bool);

    async fn load(&mut self, _ctx: Self::Context<'_>) -> AppResult<()> {
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent, ctx: Self::Context<'_>) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc if ctx.1 => Some(AppAction::SetViewState {
                state: ViewStateRequest::default(),
                dismiss_modal: false,
            }),
            KeyCode::Char('c' | 'C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(AppAction::Exit)
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme, ctx: Self::Context<'_>) {
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
            Constraint::Length(1), // Spacing
            Constraint::Min(3),    // Error message
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Control hints
        ])
        .split(inner);

        // Error message with proper styling
        let error_text = Paragraph::new(ctx.0.to_string())
            .style(theme.error_style())
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        frame.render_widget(error_text, layout[1]);

        // Control hints
        let help_text = if ctx.1 {
            "(Esc) dismiss, (Ctrl+C) quit"
        } else {
            "(Ctrl+C) quit"
        };
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[3]);
    }
}
