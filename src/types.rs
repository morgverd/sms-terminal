use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};
use sms_types::sms::SmsMessage;

use crate::modals::AppModal;
use crate::ui::notifications::NotificationType;
use crate::ui::views::ViewStateRequest;

#[derive(Debug, PartialEq)]
pub enum AppAction {
    SetViewState {
        state: ViewStateRequest,
        dismiss_modal: bool,
    },
    SetModal(Option<AppModal>),
    HandleMessage(SmsMessage),
    ShowNotification(NotificationType),
    ShowError {
        message: String,
        dismissible: bool,
    },
    Exit,

    /// Unimplemented, but left to hopefully spur me into finishing
    /// it since it is the only thing showing warnings on compile!
    DeliveryFailure(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPress {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}
impl From<KeyEvent> for KeyPress {
    fn from(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers,
        }
    }
}

/// Prevent long key presses etc from sending multiple `KeyPress` events.
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
