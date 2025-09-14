use std::sync::Arc;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::time::Duration;
use ratatui::style::Color;
use sms_client::Client;
use sms_client::http::HttpClient;
use sms_client::types::SmsStoredMessage;
use sms_client::ws::types::WebsocketMessage;
use tokio::sync::mpsc;

use crate::TerminalConfig;
use crate::error::{AppError, AppResult};
use crate::theme::ThemeManager;
use crate::types::{AppState, KeyDebouncer, KeyPress, KeyResponse, SmsMessage, DEBOUNCE_DURATION, ModalResponse, Modal, ModalMetadata};
use crate::ui::dialog::Dialog;
use crate::ui::error::ErrorView;
use crate::ui::messages_table::MessagesTableView;
use crate::ui::notification::{NotificationType, NotificationView};
use crate::ui::phone_input::PhoneInputView;
use crate::ui::sms_input::SmsInputView;
use crate::ui::{ModalResponder, View};

#[derive(Debug, Clone)]
pub enum LiveEvent {
    NewMessage(SmsStoredMessage),
    SendFailure(String),
    ShowNotification(NotificationType),
    ShowError {
        message: String,
        dismissible: bool
    },
    ShowLoadingModal(&'static str),
    SetAppState(AppState)
}

pub type AppContext = (Arc<HttpClient>, mpsc::UnboundedSender<LiveEvent>);

pub struct App {
    app_state: AppState,
    current_modal: Option<Modal>,
    key_debouncer: KeyDebouncer,
    theme_manager: ThemeManager,
    phone_input_view: PhoneInputView,
    messages_view: MessagesTableView,
    sms_input_view: SmsInputView,
    error_view: ErrorView,
    notification_view: NotificationView,
    message_receiver: mpsc::UnboundedReceiver<LiveEvent>,
    message_sender: mpsc::UnboundedSender<LiveEvent>,
    sms_client: Client,
    websocket_enabled: bool
}
impl App {
    pub fn new(config: TerminalConfig) -> Result<Self> {
        let client = Client::new(config.client)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let context: AppContext = (client.http_arc(), tx.clone());

        Ok(Self {
            app_state: AppState::InputPhone,
            current_modal: None,
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            theme_manager: ThemeManager::with_preset(config.theme),
            phone_input_view: PhoneInputView::with_context(context.clone()),
            messages_view: MessagesTableView::with_context(context.clone()),
            sms_input_view: SmsInputView::with_context(context),
            error_view: ErrorView::new(),
            notification_view: NotificationView::new(),
            message_receiver: rx,
            message_sender: tx,
            sms_client: client,
            websocket_enabled: config.websocket
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        if self.websocket_enabled {
            self.start_sms_websocket().await?;
        } else {
            // Show a notification informing the user that their websocket
            // is disabled and therefore live updates will not work
            let notification = NotificationType::GenericMessage {
                color: Color::Yellow,
                title: "WebSocket Disabled".to_string(),
                message: "Live updates will not show!".to_string(),
            };
            self.notification_view.add_notification(notification);
        };

        // Transition into starting state (which may be an error!)
        self.transition_state(AppState::InputPhone).await;

        loop {
            terminal.draw(|frame| self.render(frame))?;
            self.process_live_events().await?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    // Transition the state, handling error format easily.
                    // This is also the only direct way to quit, by returning true in the key response.
                    if let Some(response) = self.get_key_response(key).await {
                        if self.handle_key_response(response).await {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        // Render main application view
        match &self.app_state {
            AppState::InputPhone => self.phone_input_view.render(frame, theme, ()),
            AppState::ViewMessages { phone_number, reversed } => self.messages_view.render(frame, theme, (phone_number, *reversed)),
            AppState::ComposeSms { phone_number } => self.sms_input_view.render(frame, theme, phone_number),
            AppState::Error { message, dismissible } => self.error_view.render(frame, theme, (message, *dismissible))
        }

        // Render modal on top of main view
        if let Some(modal) = &mut self.current_modal {
            match modal {
                Modal::Confirmation { dialog, .. } => dialog.render(frame, theme),
                Modal::TextInput { dialog, .. } => dialog.render(frame, theme),
                Modal::Loading { dialog, .. } => dialog.render(frame, theme)
            }
        }

        // Render notifications on top of everything
        self.notification_view.render(frame, theme, ());
    }

    async fn transition_state(&mut self, new_state: AppState) {
        let result = match &new_state {
            AppState::InputPhone => self.phone_input_view.load(()).await,
            AppState::ViewMessages { phone_number, reversed } => self.messages_view.load((phone_number, *reversed)).await,
            AppState::ComposeSms { phone_number } => self.sms_input_view.load(phone_number).await,
            _ => Ok(())
        };

        // Get the actual state by first checking if any of
        // the view transition results returned an error.
        let actual_state = if let Err(e) = result {
            AppState::from(e)
        } else {
            new_state
        };

        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::SetTitle(format!("SMS Terminal v{} ｜ {}", crate::VERSION, actual_state)),
        );

        self.app_state = actual_state;
        self.key_debouncer.reset();
    }

    async fn handle_key_response(&mut self, response: KeyResponse) -> bool {
        match response {
            KeyResponse::SetAppState(state) => self.transition_state(state).await,
            KeyResponse::ShowModal(modal) => {
                self.current_modal = Some(modal);
            },
            KeyResponse::Quit => return true
        };
        false
    }

    async fn get_key_response(&mut self, key: KeyEvent) -> Option<KeyResponse> {
        // Debounce all key presses.
        let key_press = KeyPress::from(key);
        if !self.key_debouncer.should_process(&key_press) {
            return None;
        }

        // Global theme switching with F10
        if key.code == KeyCode::F(10) {
            self.theme_manager.next();
            return None;
        }
        if key.code == KeyCode::F(11) {
            self.theme_manager.toggle_modify_background();
            return None;
        }

        // Handle modal interactions
        if let Some(modal) = &mut self.current_modal {
            let (modal_response, metadata) = match modal {
                Modal::Confirmation { dialog, id, metadata } => {
                    let response = if let Some(confirmed) = dialog.handle_key(key) {
                        Some(ModalResponse::Confirmation {
                            modal_id: id.clone(),
                            confirmed,
                        })
                    } else {
                        None
                    };
                    (response, metadata.clone())
                },
                Modal::TextInput { dialog, id, metadata } => {
                    let response = if let Some(confirmed) = dialog.handle_key(key) {
                        Some(ModalResponse::TextInput {
                            modal_id: id.clone(),
                            value: if confirmed {
                                dialog.get_input().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    (response, metadata.clone())
                },
                Modal::Loading { .. } => return None
            };

            if let Some(response) = modal_response {
                let key_response = self.handle_modal_response(response, metadata).await;
                self.current_modal = None;
                return key_response;
            }
            return None;
        }

        // Handle notification interactions
        if let Some(response) = self.notification_view.handle_key(key, ()).await {
            return Some(response);
        }

        // View handlers
        match &self.app_state {
            AppState::InputPhone => self.phone_input_view.handle_key(key, ()).await,
            AppState::ViewMessages { phone_number, reversed } => self.messages_view.handle_key(key, (phone_number, *reversed)).await,
            AppState::ComposeSms { phone_number } => self.sms_input_view.handle_key(key, phone_number).await,
            AppState::Error { message, dismissible } => self.error_view.handle_key(key, (message, *dismissible)).await
        }
    }

    async fn handle_modal_response(&mut self, response: ModalResponse, metadata: ModalMetadata) -> Option<KeyResponse> {
        match response {
            ModalResponse::Confirmation { modal_id, confirmed } => {
                if !confirmed {
                    return None; // User cancelled
                }
                match modal_id.as_str() {
                    "confirm_sms_send" => {
                        self.sms_input_view.handle_modal_response(
                            modal_id, confirmed, metadata
                        ).await
                    },
                    _ => None
                }
            },
            ModalResponse::TextInput { modal_id, value } => {
                let Some(value) = value else {
                    return None; // User cancelled
                };
                match modal_id.as_str() {
                    "edit_friendly_name" => {
                        self.phone_input_view.handle_modal_response(
                            modal_id, value, metadata
                        ).await
                    },
                    _ => None
                }
            }
        }
    }

    async fn handle_new_message(&mut self, sms_message: SmsStoredMessage) -> AppResult<()> {
        // Only add the message if we're viewing messages for the same phone number.
        let msg = SmsMessage::from(&sms_message);
        let mut show_notification = true;
        match &self.app_state {
            AppState::ViewMessages { phone_number, .. } if phone_number == sms_message.phone_number.as_str() => {
                self.messages_view.add_live_message(msg.clone());
                show_notification = false;
            }
            _ => { }
        }

        // Push to phone list view always so it maintains order.
        self.phone_input_view.push_new_number(
            sms_message.phone_number.clone()
        ).await?;

        // Show a notification for incoming SMS messages.
        // Use the SMSMessage variant for content as it's sanitized.
        // Do not show the notification if we're already viewing those messages.
        if show_notification && !sms_message.is_outgoing {
            let notification = NotificationType::IncomingMessage {
                phone: sms_message.phone_number.clone(),
                content: msg.content
            };
            self.notification_view.add_notification(notification);
        }

        Ok(())
    }

    async fn start_sms_websocket(&self) -> AppResult<()> {
        let ws_sender = self.message_sender.clone();
        self.sms_client.on_message_simple(move |message| {
            match message {
                WebsocketMessage::IncomingMessage(sms) | WebsocketMessage::OutgoingMessage(sms) => {
                    let _ = ws_sender.send(LiveEvent::NewMessage(sms));
                },
                WebsocketMessage::ModemStatusUpdate { previous, current } => {
                    let notification = NotificationType::OnlineStatus { previous, current };
                    let _ = ws_sender.send(LiveEvent::ShowNotification(notification));
                },
                WebsocketMessage::WebsocketConnectionUpdate { connected, reconnect } => {
                    let notification = NotificationType::WebSocketConnectionUpdate { connected, reconnect };
                    let _ = ws_sender.send(LiveEvent::ShowNotification(notification));
                },
                _ => { }
            }
        }).await?;

        // Create websocket worker task.
        let client = self.sms_client.clone();
        let task_sender = self.message_sender.clone();
        tokio::spawn(async move {
            // Handle early termination or errors on starting.
            let (message, dismissible) = match client.start_blocking_websocket().await {
                Ok(_) => ("The WebSocket has been terminated!".to_string(), true),
                Err(e) => (e.to_string(), false)
            };
            let _ = task_sender.send(LiveEvent::ShowError { message, dismissible });
        });

        Ok(())
    }

    async fn process_live_events(&mut self) -> AppResult<()> {
        while let Ok(msg) = self.message_receiver.try_recv() {
            match msg {
                LiveEvent::NewMessage(sms_message) => self.handle_new_message(sms_message).await?,
                LiveEvent::SendFailure(_) => unimplemented!("Oops!"),
                LiveEvent::ShowNotification(notification) => self.notification_view.add_notification(notification),
                LiveEvent::ShowError { message, dismissible } => {

                    // If another error is being displayed, only overwrite it if
                    // that one is dismissable but this one isn't. Otherwise, ignore.
                    let allowed = match &self.app_state {
                        AppState::Error { dismissible: existing_dismissable, .. } => {
                            *existing_dismissable || !dismissible
                        },
                        _ => true
                    };
                    if allowed {
                        self.transition_state(AppState::Error { message, dismissible }).await;
                    }
                },
                LiveEvent::ShowLoadingModal(message) => self.current_modal = Some(Modal::create_loading(message)),
                LiveEvent::SetAppState(new_state) => {
                    if matches!(self.current_modal, Some(Modal::Loading { .. })) {
                        self.current_modal = None;
                    }
                    self.transition_state(new_state).await
                }
            }
        }

        Ok(())
    }
}