use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::modals::{ModalLoadBehaviour, ModalResponse};
use crate::theme::Theme;
use crate::ui::centered_rect;

pub mod confirmation;
pub mod delivery_reports;
pub mod loading;
pub mod text_input;

pub trait ModalComponent: std::fmt::Debug + Send + Sync {
    /// Handle modal incoming key, and return some response that is pushed back
    /// to the View if it implements `ModalResponderComponent`. If None is returned,
    /// the input is entirely ignored (by both the Modal and active View).
    fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResponse>;

    /// Render the modal, called per frame.
    fn render(&mut self, frame: &mut Frame, theme: &Theme);

    /// Get the modals load behaviour, called once the modal is being set as active.
    /// This can be used to block the modal from loading, or return some `AppAction`.
    fn load(&self) -> ModalLoadBehaviour {
        ModalLoadBehaviour::None
    }

    /// Should views from `AppState` still be rendered whilst Modal is active.
    fn should_render_views(&self) -> bool {
        true
    }
}

pub struct ModalUtils;
impl ModalUtils {
    /// Render the base frame, used by all Modals.
    pub fn render_base<F>(
        frame: &mut Frame,
        title: &str,
        content: F,
        theme: &Theme,
        width: u16,
        height: u16,
    ) where
        F: FnOnce(&mut Frame, Rect, &Theme),
    {
        let area = centered_rect(width, height, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(format!(" {title} "))
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Double)
            .border_style(theme.border_focused_style())
            .style(theme.primary_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        content(frame, inner, theme);
    }

    /// Render modal buttons with a selection index.
    fn render_buttons(
        frame: &mut Frame,
        area: Rect,
        buttons: &[ModalButtonComponent],
        selected_index: usize,
    ) {
        let mut button_spans = Vec::new();

        button_spans.push(Span::raw("    "));
        for (i, button) in buttons.iter().enumerate() {
            if i > 0 {
                button_spans.push(Span::raw("     "));
            }

            let style = button.render_style(i == selected_index);
            button_spans.push(Span::styled(format!("  {}  ", button.label), style));
        }
        button_spans.push(Span::raw("    "));

        let buttons_line = Line::from(button_spans);
        let buttons_paragraph = Paragraph::new(buttons_line).alignment(Alignment::Center);
        frame.render_widget(buttons_paragraph, area);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModalButtonComponent {
    pub label: String,
    pub style_normal: Style,
    pub style_focused: Style,
}
impl ModalButtonComponent {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style_normal: Style::default(),
            style_focused: Style::default(),
        }
    }

    /// Allow Modals to apply theme styles.
    pub fn with_styles(mut self, normal: Style, focused: Style) -> Self {
        self.style_normal = normal;
        self.style_focused = focused;
        self
    }

    /// Get the appropriate theme style depending on if it's focused.
    pub fn render_style(&self, focused: bool) -> Style {
        if focused {
            self.style_focused
        } else {
            self.style_normal
        }
    }

    /// Create styled buttons for OK/Cancel pattern.
    fn create_ok_cancel_buttons(
        button_styles: &ModalButtonComponentStyles,
    ) -> [ModalButtonComponent; 2] {
        [
            ModalButtonComponent::new("OK")
                .with_styles(button_styles.primary_normal, button_styles.primary_focused),
            ModalButtonComponent::new("Cancel").with_styles(
                button_styles.secondary_normal,
                button_styles.secondary_focused,
            ),
        ]
    }

    /// Create styled buttons for Yes/No pattern.
    fn create_yes_no_buttons(
        button_styles: &ModalButtonComponentStyles,
    ) -> [ModalButtonComponent; 2] {
        [
            ModalButtonComponent::new("Yes")
                .with_styles(button_styles.primary_normal, button_styles.primary_focused),
            ModalButtonComponent::new("No").with_styles(
                button_styles.secondary_normal,
                button_styles.secondary_focused,
            ),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ModalButtonComponentStyles {
    pub primary_normal: Style,
    pub primary_focused: Style,
    pub secondary_normal: Style,
    pub secondary_focused: Style,
}
impl ModalButtonComponentStyles {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            primary_normal: theme.secondary_style(),
            primary_focused: Style::default()
                .fg(theme.bg)
                .bg(theme.text_accent)
                .add_modifier(Modifier::BOLD),
            secondary_normal: theme.secondary_style(),
            secondary_focused: Style::default()
                .fg(theme.bg)
                .bg(theme.text_error)
                .add_modifier(Modifier::BOLD),
        }
    }
}
