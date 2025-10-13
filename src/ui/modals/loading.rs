use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::prelude::{Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::modals::ModalResponse;
use crate::theme::Theme;
use crate::ui::modals::{ModalComponent, ModalUtils};

#[derive(Debug, Clone, PartialEq)]
pub struct LoadingModal {
    pub message: String,
    pub frame_count: usize,
}
impl LoadingModal {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            frame_count: 0,
        }
    }

    fn get_spinner_char(&self) -> char {
        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let index = self.frame_count % spinner_chars.len();
        spinner_chars[index]
    }
}
impl ModalComponent for LoadingModal {
    fn handle_key(&mut self, _key: KeyEvent) -> Option<ModalResponse> {
        None
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        self.frame_count = self.frame_count.wrapping_add(1);
        ModalUtils::render_base(
            frame,
            "Please Wait",
            |frame, area, theme| {
                let layout = Layout::vertical([
                    Constraint::Length(1), // Top spacer
                    Constraint::Length(1), // Spinner + message line
                    Constraint::Length(1), // Bottom spacer
                ])
                .split(area);

                let spinner = Paragraph::new(format!(
                    "{} {}",
                    self.get_spinner_char(),
                    self.message.trim()
                ))
                .style(
                    Style::default()
                        .fg(theme.text_accent)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center);
                frame.render_widget(spinner, layout[1]);
            },
            theme,
            50,
            10,
        );
    }
}
