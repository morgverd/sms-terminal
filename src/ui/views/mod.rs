mod error;
mod messages;
mod phonebook;
mod compose;
mod device_info;
mod main_menu;

use std::fmt::Display;
use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::app::AppContext;
use crate::error::{AppError, AppResult};
use crate::modals::{ModalMetadata, ModalResponse};
use crate::theme::Theme;
use crate::types::{AppAction, SmsMessage};
use crate::ui::{ModalResponderComponent, ViewBase};

/*
    Quite happy with this, instead of storing every initialized view on the
    App itself the ViewManager now creates views as they are needed based on
    ViewStateRequests (used in AppAction::SetViewState).
 */

/// Public request interface for switching views.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewStateRequest {
    MainMenu,
    Phonebook,
    DeviceInfo,
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
impl ViewStateRequest {

    /// Create ViewState::ViewMessages with a default reversed state.
    pub fn view_messages(phone_number: &str) -> Self {
        Self::Messages { phone_number: phone_number.to_string(), reversed: false }
    }
}
impl Default for ViewStateRequest {
    fn default() -> Self {
        Self::MainMenu
    }
}
impl From<AppError> for ViewStateRequest {
    fn from(error: AppError) -> Self {
        Self::Error {
            message: error.to_string(),
            dismissible: false
        }
    }
}

/// Track the current view, and create
pub struct ViewManager {
    current: CurrentView,
    context: AppContext
}
impl ViewManager {
    pub fn new(context: AppContext) -> AppResult<Self> {
        let current = CurrentView::from_request(ViewStateRequest::DeviceInfo, &context);
        Ok(Self { current, context })
    }

    pub async fn transition_to(&mut self, request: ViewStateRequest) {
        let mut new_view = CurrentView::from_request(request.clone(), &self.context);

        // Attempt to load, showing an ErrorView if it fails.
        if let Err(e) = new_view.load().await {
            new_view = CurrentView::from_request(
                ViewStateRequest::Error {
                    message: e.to_string(),
                    dismissible: false
                },
                &self.context
            );
        }

        self.current = new_view;
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        self.current.handle_key(key).await
    }

    pub fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        self.current.render(frame, theme)
    }

    pub fn handle_modal_response(
        &mut self,
        response: ModalResponse,
        metadata: ModalMetadata,
    ) -> Option<AppAction> {
        self.current.handle_modal_response(response, metadata)
    }

    pub fn try_add_message(&mut self, message: &SmsMessage) -> bool {
        self.current.try_add_message(message)
    }

    pub fn should_show_error(&self, new_dismissible: bool) -> bool {
        match self.current.is_dismissible_error() {
            Some(existing_dismissible) => existing_dismissible || !new_dismissible,
            None => true
        }
    }
}
impl Display for ViewManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.current)
    }
}

/// The CurrentView holds the BaseView itself, and any additional context.
/// It is private, and only used to maintain the state in ViewManager.
/// It's basically just a view factory.
enum CurrentView {
    MainMenu(main_menu::MainMenuView),
    Phonebook(phonebook::PhonebookView),
    DeviceInfo(device_info::DeviceInfoView),
    Messages {
        view: messages::MessagesView,
        phone_number: String,
        reversed: bool
    },
    Compose {
        view: compose::ComposeView,
        phone_number: String
    },
    Error {
        view: error::ErrorView,
        message: String,
        dismissible: bool
    }
}
impl CurrentView {
    fn from_request(request: ViewStateRequest, context: &AppContext) -> Self {
        match request {
            ViewStateRequest::MainMenu => CurrentView::MainMenu(main_menu::MainMenuView::new()),
            ViewStateRequest::Phonebook => CurrentView::Phonebook(phonebook::PhonebookView::with_context(context.clone())),
            ViewStateRequest::DeviceInfo => CurrentView::DeviceInfo(device_info::DeviceInfoView::with_context(context.clone())),
            ViewStateRequest::Messages { phone_number, reversed } =>
                CurrentView::Messages {
                    view: messages::MessagesView::with_context(context.clone()),
                    phone_number,
                    reversed
                },
            ViewStateRequest::Compose { phone_number } =>
                CurrentView::Compose {
                    view: compose::ComposeView::with_context(context.clone()),
                    phone_number
                },
            ViewStateRequest::Error { message, dismissible } =>
                CurrentView::Error {
                    view: error::ErrorView::new(),
                    message,
                    dismissible
                }
        }
    }

    async fn load(&mut self) -> AppResult<()> {
        match self {
            CurrentView::MainMenu(view) => view.load(()).await,
            CurrentView::Phonebook(view) => view.load(()).await,
            CurrentView::DeviceInfo(view) => view.load(()).await,
            CurrentView::Messages { view, phone_number, reversed } => {
                view.load((phone_number, *reversed)).await
            }
            CurrentView::Compose { view, phone_number } => {
                view.load(phone_number).await
            }
            CurrentView::Error { .. } => Ok(()),
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        match self {
            CurrentView::MainMenu(view) => view.handle_key(key, ()).await,
            CurrentView::Phonebook(view) => view.handle_key(key, ()).await,
            CurrentView::DeviceInfo(view) => view.handle_key(key, ()).await,
            CurrentView::Messages { view, phone_number, reversed } => {
                view.handle_key(key, (phone_number, *reversed)).await
            }
            CurrentView::Compose { view, phone_number } => {
                view.handle_key(key, phone_number).await
            }
            CurrentView::Error { view, message, dismissible } => {
                view.handle_key(key, (message, *dismissible)).await
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        match self {
            CurrentView::MainMenu(view) => view.render(frame, theme, ()),
            CurrentView::Phonebook(view) => view.render(frame, theme, ()),
            CurrentView::DeviceInfo(view) => view.render(frame, theme, ()),
            CurrentView::Messages { view, phone_number, reversed } => {
                view.render(frame, theme, (phone_number, *reversed))
            }
            CurrentView::Compose { view, phone_number } => {
                view.render(frame, theme, phone_number)
            }
            CurrentView::Error { view, message, dismissible } => {
                view.render(frame, theme, (message, *dismissible))
            }
        }
    }

    fn handle_modal_response(
        &mut self,
        response: ModalResponse,
        metadata: ModalMetadata,
    ) -> Option<AppAction> {
        match self {
            CurrentView::Phonebook(view) => view.handle_modal_response(response, metadata),
            CurrentView::Compose { view, .. } => view.handle_modal_response(response, metadata),
            _ => match response {

                // If the modal is being dismissed, it doesn't matter if it doesn't have a handler.
                ModalResponse::Dismissed => None,
                _ => Some(AppAction::ShowError {
                    message: "Current view cannot handle modal responses!".to_string(),
                    dismissible: true
                })
            }
        }
    }

    fn try_add_message(&mut self, message: &SmsMessage) -> bool {
        match self {
            CurrentView::Messages { view, phone_number, .. } => {
                if phone_number == &message.phone_number {

                    // Suppress the notification from showing, since we're already
                    // on the view that the notification would take us to anyway.
                    view.add_live_message(message);
                    return true;
                }
            },
            _ => { }
        }

        false
    }

    fn is_dismissible_error(&self) -> Option<bool> {
        match self {
            CurrentView::Error { dismissible, .. } => Some(*dismissible),
            _ => None,
        }
    }
}
impl Display for CurrentView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MainMenu { .. } => write!(f, "Main Menu"),
            Self::Phonebook { .. } => write!(f, "Phonebook"),
            Self::DeviceInfo { .. } => write!(f, "Device Info"),
            Self::Messages { phone_number, .. } => write!(f, "Viewing Messages ｜ {}", phone_number),
            Self::Compose { phone_number, .. } => write!(f, "Composing Message ｜ {}", phone_number),
            Self::Error { dismissible, .. } => write!(f, "{}", if *dismissible { "Fatal Error" } else { "Error" })
        }
    }
}