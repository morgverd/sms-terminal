use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    sms_client: Option<Client>
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
            phone_input_view: PhoneInputView::new(),
            messages_view: MessagesTableView::new(client.http_arc()),
            sms_input_view: SmsInputView::new(),
            error_view: ErrorView::new(),
            notification_view: NotificationView::new(),
            message_receiver: rx,
            message_sender: tx,
            sms_client: config.websocket.then(|| client)
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        if let Some(client) = &self.sms_client {
            self.start_sms_websocket(client).await?;
        } else {

            // Show a notification informing the user that their websocket
            // is disabled and therefore live updates will not work.
            let notification = NotificationType::GenericMessage {
                color: Color::Yellow,
                title: "WebSocket Disabled".to_string(),
                message: "Live updates will not show!".to_string(),
            };
            self.notification_view.add_notification(notification);
        }

        loop {
            terminal.draw(|frame| self.render(frame))?;
            self.process_live_events()?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    match self.get_key_response(key).await {
                        Some(KeyResponse::SetAppState(state)) => {

                            // Attempt to transition app state, otherwise show an error.
                            if let Err(e) = self.transition_state(state).await {
                                self.app_state = AppState::from(e);
                            }
                        },
                        Some(KeyResponse::Quit) => return Ok(()),
                        _ => { }
                    }
                }
            }
        }
    }

    async fn transition_state(&mut self, new_state: AppState) -> AppResult<()> {
        match &new_state {
            AppState::ViewMessages(phone_number) => self.messages_view.reload(phone_number).await?,
            AppState::ComposeSms(_) => self.sms_input_view.reload(),
            _ => { }
        };

        self.app_state = new_state;
        self.key_debouncer.reset();
        Ok(())
    }

    async fn get_key_response(&mut self, key: KeyEvent) -> Option<KeyResponse> {
        // Debounce all key presses.
        let key_press = KeyPress::from(key);
        if !self.key_debouncer.should_process(&key_press) {
            return None;
        }

        // Global theme switching with Shift+T. This was such a pain to make
        // but a coworker said it looked cool, so I stuck with it throughout.
        // This MUST remain uppercase T, since shift modifies it before here!
        if key.code == KeyCode::Char('T') && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.theme_manager.next();
            return None;
        }

        // Handle notification interactions first (priority)
        if let Some(response) = self.notification_view.handle_key(key) {
            return Some(response);
        }

        // State specific key handlers
        match &self.app_state {
            AppState::InputPhone => self.phone_input_view.handle_key(key),
            AppState::ViewMessages(phone_number) => self.messages_view.handle_key(key, phone_number).await,
            AppState::ComposeSms(phone_number) => self.sms_input_view.handle_key(key, phone_number),
            AppState::Error { dismissible, .. } => self.error_view.handle_key(key, *dismissible)
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        match &self.app_state {
            AppState::InputPhone => {
                self.phone_input_view.render(frame, theme);
            },
            AppState::ViewMessages(phone_number) => {
                self.messages_view.render(frame, phone_number, theme);
            },
            AppState::ComposeSms(phone_number) => {
                self.sms_input_view.render(frame, phone_number, theme);
            },
            AppState::Error { message, dismissible } => {
                self.error_view.render(frame, message, *dismissible, theme);
            }
        }

        // Render notifications on top of everything
        self.notification_view.render(frame, theme);
    }

    async fn start_sms_websocket(&self, client: &Client) -> AppResult<()> {
        let ws_sender = self.message_sender.clone();
        client.on_message_simple(move |message| {
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
        let client = client.clone();
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

    fn process_live_events(&mut self) -> AppResult<()> {
        while let Ok(msg) = self.message_receiver.try_recv() {
            match msg {
                LiveEvent::NewMessage(sms_message) => {

                    // Only add the message if we're viewing messages for the same phone number.
                    let msg = SmsMessage::from(&sms_message);
                    let mut show_notification = true;
                    if let AppState::ViewMessages(current_phone) = &self.app_state {
                        if current_phone == sms_message.phone_number.as_str() {
                            self.messages_view.add_live_message(msg.clone());
                            show_notification = false;
                        }
                    }

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
                },
                LiveEvent::SendFailure(_) => unimplemented!("Oops!"),
                LiveEvent::ShowNotification(notification) => {
                    self.notification_view.add_notification(notification)
                },
                LiveEvent::ShowError { message, dismissible } => {
                    self.app_state = AppState::Error { message, dismissible };
                }
            }
        }

        Ok(())
    }
}