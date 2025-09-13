pub mod error;
pub mod messages_table;
pub mod phone_input;
pub mod sms_input;
pub mod notification;
pub mod dialog;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::KeyResponse;

pub trait View {
    type Context: Default;

    async fn load(&mut self, ctx: Self::Context) -> AppResult<()>;
    async fn handle_key(&mut self, key: KeyEvent, ctx: Self::Context) -> Option<KeyResponse>;
    fn render(&mut self, frame: &mut Frame, theme: &Theme, ctx: Self::Context);
}

/// Helper function to create a centered rectangle
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
        .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
        .split(popup_layout[1])[1]
}
