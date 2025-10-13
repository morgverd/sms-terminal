pub mod modals;
pub mod notifications;
pub mod views;

use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::Frame;

use crate::error::AppResult;
use crate::modals::{AppModal, ModalResponse};
use crate::theme::Theme;
use crate::types::AppAction;

pub trait ViewBase {
    type Context<'ctx>;

    async fn load(&mut self, ctx: Self::Context<'_>) -> AppResult<()>;
    async fn handle_key(&mut self, key: KeyEvent, ctx: Self::Context<'_>) -> Option<AppAction>;
    fn render(&mut self, frame: &mut Frame, theme: &Theme, ctx: Self::Context<'_>);
}

pub trait ModalResponderComponent {
    /// Handle a modal response with its associated metadata.
    /// Returns a `KeyResponse` if the app state should change.
    fn handle_modal_response(
        &mut self,
        modal: &mut AppModal,
        response: ModalResponse,
    ) -> Option<AppAction>;
}

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
