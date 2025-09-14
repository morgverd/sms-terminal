use crossterm::event::KeyEvent;
use ratatui::Frame;
use crate::theme::Theme;
use crate::ui::modals::ModalComponent;

#[derive(Debug, Clone, PartialEq)]
pub enum ModalMetadata {
    SendMessage(String, String),
    PhoneNumber(String),
    None
}
impl ModalMetadata {
    pub fn phone(number: impl Into<String>) -> Self {
        Self::PhoneNumber(number.into())
    }

    pub fn as_phone(&self) -> Option<&str> {
        match self {
            Self::SendMessage(phone, _) => Some(phone),
            Self::PhoneNumber(phone) => Some(phone),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ModalResponse {
    Dismissed,
    Confirmed(bool),
    TextInput(Option<String>)
}

#[derive(Debug)]
pub struct AppModal {
    pub id: String,
    pub metadata: ModalMetadata,
    inner: Box<dyn ModalComponent>
}
impl AppModal {
    pub fn new<T: ModalComponent + 'static>(id: impl Into<String>, modal: T) -> Self {
        Self {
            id: id.into(),
            metadata: ModalMetadata::None,
            inner: Box::new(modal)
        }
    }

    pub fn with_metadata(mut self, metadata: ModalMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    #[inline]
    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        self.inner.render(frame, theme)
    }

    #[inline]
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResponse> {
        self.inner.handle_key(key)
    }

    #[inline]
    pub fn should_render_views(&self) -> bool {
        self.inner.should_render_views()
    }
}
impl PartialEq for AppModal {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.metadata == other.metadata
    }
}
