use crossterm::event::KeyEvent;
use ratatui::Frame;
use crate::app::AppContext;
use crate::theme::Theme;
use crate::types::AppAction;
use crate::ui::modals::ModalComponent;

/// Determines how a modal should be loaded after it's set.
/// The views always have priority, and therefore it cannot
/// block the main render or async loops.
pub enum ModalLoadBehaviour {
    Function(Box<dyn FnOnce(AppContext) -> (Option<AppAction>, bool) + Send + Sync>), // return_action, should_block
    None
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalMetadata {
    SendMessage(String, String), // phone_number, message_content
    PhoneNumber(String),
    None
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
    pub fn load(&self) -> ModalLoadBehaviour {
        self.inner.load()
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
