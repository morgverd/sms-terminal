use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::widgets::{Paragraph, Wrap};

use crate::theme::Theme;
use crate::ui::modals::{ModalButtonComponentStyles, ModalComponent, ModalButtonComponent};

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
}
impl ModalComponent for ConfirmationModal {

    fn handle_key(&mut self, key: KeyEvent) -> Option<bool> {
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
    fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        let button_styles = ModalButtonComponentStyles::from_theme(theme);
        let styled_buttons = ModalButtonComponent::create_yes_no_buttons(&button_styles);

        Self::render_base(
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

    fn get_input(&self) -> Option<&str> {
        None
    }
}