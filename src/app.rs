use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::time::Duration;
use ratatui::style::Color;
use sms_client::Client;
use sms_client::types::SmsStoredMessage;
use sms_client::ws::types::WebsocketMessage;
use tokio::sync::mpsc;

use crate::TerminalConfig;
use crate::error::{AppError, AppResult};
use crate::theme::ThemeManager;
use crate::types::{AppState, KeyDebouncer, KeyPress, KeyResponse, SmsMessage, DEBOUNCE_DURATION};
use crate::ui::error::ErrorView;
use crate::ui::messages_table::MessagesTableView;
use crate::ui::notification::{NotificationType, NotificationView};
use crate::ui::phone_input::PhoneInputView;
use crate::ui::sms_input::SmsInputView;

#[derive(Debug, Clone)]
pub enum LiveEvent {
    NewMessage(SmsStoredMessage),
    SendFailure(String),
    ShowNotification(NotificationType),
    ShowError {
        message: String,
        dismissible: bool
    }
}

pub struct App {
    app_state: AppState,
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
        Ok(Self {
            app_state: AppState::InputPhone,
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            theme_manager: ThemeManager::with_preset(config.theme),
            phone_input_view: PhoneInputView::with_http(client.http_arc()),
            messages_view: MessagesTableView::with_http(client.http_arc()),
            sms_input_view: SmsInputView::new(),
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

        // Transition into starting state and get SMS HTTP client
        if !self.transition_state(AppState::InputPhone).await {
            return Err(AppError::ViewError("Could not transition into initial view!").into());
        }

        loop {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_live_events().await?;

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

    async fn transition_state(&mut self, new_state: AppState) -> bool {
        let result = match &new_state {
            AppState::InputPhone => self.phone_input_view.load().await,
            AppState::ViewMessages { phone_number, reversed } => self.messages_view.load(phone_number, *reversed).await,
            AppState::ComposeSms { .. } => {
                self.sms_input_view.load();
                Ok(())
            },
            _ => Ok(())
        };

        // Get the actual state by first checking if any of
        // the view transition results returned an error.
        let (actual_state, is_successful) = if let Err(e) = result {
            (AppState::from(e), false)
        } else {
            (new_state, true)
        };

        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::SetTitle(format!("SMS Terminal v{} ｜ {}", crate::VERSION, actual_state)),
        );

        self.app_state = actual_state;
        self.key_debouncer.reset();

        is_successful
    }

    async fn handle_key_response(&mut self, response: KeyResponse) -> bool {
        match response {
            KeyResponse::SetAppState(state) => {
                let _ = self.transition_state(state).await;
            },
            KeyResponse::SendMessage(message, state) => {

                // Send the SMS message as provided
                let notification = match self.sms_client.http_arc().send_sms(&message).await {
                    Ok(response) => {

                        // Convert message and response into a stored message
                        // format so it can be pushed to other views.
                        let _ = self.handle_new_message(
                            SmsStoredMessage::from((message, response))
                        ).await;

                        NotificationType::GenericMessage {
                            color: Color::Green,
                            title: "Message Sent".to_string(),
                            message: format!("Message #{} was sent (ref {})!", response.message_id, response.reference_id),
                        }
                    },
                    Err(e) => {
                        NotificationType::GenericMessage {
                            color: Color::Red,
                            title: "Send Failure".to_string(),
                            message: e.to_string()
                        }
                    }
                };

                // Show resulting notification and change to result state
                self.notification_view.add_notification(notification);
                let _ = self.transition_state(state).await;
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

        // Global theme switching with F10. This was such a pain to make
        // but a coworker said it looked cool, so I stuck with it throughout.
        // This MUST remain uppercase T, since shift modifies it before here!
        if key.code == KeyCode::F(10) {
            self.theme_manager.next();
            return None;
        }
        if key.code == KeyCode::F(11) {
            self.theme_manager.toggle_modify_background();
            return None;
        }

        // Handle notification interactions first (priority)
        if let Some(response) = self.notification_view.handle_key(key) {
            return Some(response);
        }

        // State specific key handlers
        match &self.app_state {
            AppState::InputPhone => self.phone_input_view.handle_key(key),
            AppState::ViewMessages { phone_number, .. } => self.messages_view.handle_key(key, phone_number).await,
            AppState::ComposeSms { phone_number } => self.sms_input_view.handle_key(key, phone_number),
            AppState::Error { dismissible, .. } => self.error_view.handle_key(key, *dismissible)
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        match &self.app_state {
            AppState::InputPhone => self.phone_input_view.render(frame, theme),
            AppState::ViewMessages { phone_number, .. } => self.messages_view.render(frame, phone_number, theme),
            AppState::ComposeSms { phone_number } => self.sms_input_view.render(frame, phone_number, theme),
            AppState::Error { message, dismissible } => self.error_view.render(frame, message, *dismissible, theme)
        }

        // Render notifications on top of everything
        self.notification_view.render(frame, theme);
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

    async fn handle_live_events(&mut self) -> AppResult<()> {
        while let Ok(msg) = self.message_receiver.try_recv() {
            match msg {
                LiveEvent::NewMessage(sms_message) => self.handle_new_message(sms_message).await?,
                LiveEvent::SendFailure(_) => unimplemented!("Oops!"),
                LiveEvent::ShowNotification(notification) => self.notification_view.add_notification(notification),
                LiveEvent::ShowError { message, dismissible } => {
                    let _ = self.transition_state(AppState::Error { message, dismissible });
                }
            }
        }

        Ok(())
    }
}