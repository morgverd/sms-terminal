use std::fmt::Display;
use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sms_client::types::SmsStoredMessage;
use std::time::{Duration, Instant};
use ansi_escape_sequences::strip_ansi;
use unicode_general_category::{get_general_category, GeneralCategory};

use crate::error::AppError;
use crate::ui::modals::ModalComponent;
use crate::ui::notification::NotificationType;

/// A shortened version of a StoredSmsMessage that only
/// stores the information used in messages_table.
#[derive(Clone, Debug)]
pub struct SmsMessage {
    pub id: String,
    pub direction: String,
    pub timestamp: String,
    pub content: String
}
impl SmsMessage {
    pub fn ref_array(&self) -> [&String; 4] {
        [&self.id, &self.direction, &self.timestamp, &self.content]
    }
}
impl From<&SmsStoredMessage> for SmsMessage {
    fn from(value: &SmsStoredMessage) -> Self {

        // Get datetime from timestamp value, or local time if unset / invalid.
        let dt = value.completed_at.or(value.created_at)
            .map(|t| Local.timestamp_opt(t as i64, 0).single())
            .flatten()
            .unwrap_or_else(|| Local::now());

        Self {
            id: value.message_id.to_string(),
            direction: if value.is_outgoing { "← OUT" } else { "→ IN" }.to_string(),
            timestamp: dt.format("%d/%m/%y %H:%M").to_string(),

            // Remove all control characters from being displayed.
            // This includes newlines etc.
            content: strip_ansi(&value.message_content)
                .chars()
                .filter(|c| !c.is_control()
                    && !matches!(
                        get_general_category(*c),
                        GeneralCategory::Format
                            | GeneralCategory::Control
                            | GeneralCategory::Unassigned
                    )
                )
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewState {
    Phonebook,
    Messages {
        phone_number: String,
        reversed: bool
    },
    Compose {
        phone_number: String
    },
    Error {
        message: String,
        dismissible: bool
    }
}
impl ViewState {
    pub fn view_messages(phone_number: &str) -> Self {
        Self::Messages { phone_number: phone_number.to_string(), reversed: false }
    }

    pub fn compose(phone_number: &str) -> Self {
        Self::Compose { phone_number: phone_number.to_string() }
    }
}
impl From<AppError> for ViewState {
    fn from(error: AppError) -> Self {
        ViewState::Error {
            message: error.to_string(),
            dismissible: false
        }
    }
}
impl Display for ViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewState::Phonebook => write!(f, "Phonebook"),
            ViewState::Messages { phone_number, .. } => write!(f, "Viewing Messages ｜ {}", phone_number),
            ViewState::Compose { phone_number, .. } => write!(f, "Composing Message ｜ {}", phone_number),
            ViewState::Error { dismissible, .. } => write!(f, "{}", if *dismissible { "Fatal Error" } else { "Error" })
        }
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum AppModal {
    Confirmation {
        ui: crate::ui::modals::confirmation::ConfirmationModal,
        id: String,
        metadata: ModalMetadata
    },
    TextInput {
        ui: crate::ui::modals::text_input::TextInputModal,
        id: String,
        metadata: ModalMetadata
    },
    Loading {
        ui: crate::ui::modals::loading::LoadingModal,
        metadata: ModalMetadata
    }
}
impl AppModal {

    #[inline]
    pub fn loading(message: impl Into<String>) -> Self {
        Self::Loading {
            ui: crate::ui::modals::loading::LoadingModal::new(message.into()),
            metadata: ModalMetadata::None
        }
    }

    #[inline]
    pub fn with_metadata(mut self, metadata: ModalMetadata) -> Self {
        match &mut self {
            AppModal::Confirmation { metadata: m, .. } => *m = metadata,
            AppModal::TextInput { metadata: m, .. } => *m = metadata,
            AppModal::Loading { metadata: m, .. } => *m = metadata
        }
        self
    }

    #[inline]
    pub fn should_render_views(&self) -> bool {
        match &self {
            AppModal::Loading { ui, .. } => ui.should_render_views(),
            AppModal::TextInput { ui, .. } => ui.should_render_views(),
            AppModal::Confirmation { ui, .. } => ui.should_render_views()
        }
    }
}
impl From<(&'static str, crate::ui::modals::text_input::TextInputModal)> for AppModal {
    fn from((message, ui): (&'static str, crate::ui::modals::text_input::TextInputModal)) -> Self {
        Self::TextInput { ui, id: message.to_string(), metadata: ModalMetadata::None }
    }
}
impl From<(&'static str, crate::ui::modals::confirmation::ConfirmationModal)> for AppModal {
    fn from((message, ui): (&'static str, crate::ui::modals::confirmation::ConfirmationModal)) -> Self {
        Self::Confirmation { ui, id: message.into(), metadata: ModalMetadata::None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalResponse {
    Confirmation {
        modal_id: String,
        confirmed: bool,
    },
    TextInput {
        modal_id: String,
        value: Option<String>
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppAction {
    SetAppState(ViewState),
    ShowModal(AppModal),
    HandleIncomingMessage(SmsStoredMessage),
    ShowNotification(NotificationType),
    ShowError {
        message: String,
        dismissible: bool
    },
    Exit,

    /// Unimplemented, but left to hopefully spur me into finishing
    /// it since it is the only thing showing warnings on compile!
    DeliveryFailure(String)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPress {
    pub code: KeyCode,
    pub modifiers: KeyModifiers
}
impl From<KeyEvent> for KeyPress {
    fn from(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers
        }
    }
}

/// Prevent long key presses etc from sending multiple KeyPress events.
/// This was a particular issue when using WSL for some reason!
pub struct KeyDebouncer {
    last_key: Option<KeyPress>,
    last_time: Instant,
    debounce_duration: Duration,
}
impl KeyDebouncer {
    pub fn new(debounce_duration: Duration) -> Self {
        Self {
            last_key: None,
            last_time: Instant::now(),
            debounce_duration,
        }
    }

    pub fn should_process(&mut self, key: &KeyPress) -> bool {
        let now = Instant::now();

        // If it's a different key, always process it
        if self.last_key.as_ref() != Some(key) {
            self.last_key = Some(key.clone());
            self.last_time = now;
            return true;
        }

        // Same key - check if enough time has passed
        if now.duration_since(self.last_time) >= self.debounce_duration {
            self.last_time = now;
            return true;
        }

        false
    }

    pub fn reset(&mut self) {
        self.last_key = None;
        self.last_time = Instant::now();
    }
}

pub const DEBOUNCE_DURATION: Duration = Duration::from_millis(50);