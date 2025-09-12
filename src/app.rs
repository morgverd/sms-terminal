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
use crate::error::AppError;
use crate::theme::ThemeManager;
use crate::types::{AppState, KeyDebouncer, KeyPress, SmsMessage, DEBOUNCE_DURATION};
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
    ShowError(String)
}

pub struct App {
    input_buffer: String,
    sms_text_buffer: String,
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
            input_buffer: String::new(),
            sms_text_buffer: String::new(),
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
            self.start_sms_websocket(client).await;
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
            self.process_live_events();
            self.handle_state_transitions().await;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    if self.handle_key_event(key).await {
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn start_sms_websocket(&self, client: &Client) {
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
        })
        .await
        .expect("Failed to create websocket listener!");

        // Create websocket worker task.
        let client = client.clone();
        let task_sender = self.message_sender.clone();
        tokio::spawn(async move {

            // Handle early termination or errors on starting.
            let error_message = match client.start_blocking_websocket().await {
                Ok(_) => "The WebSocket has been terminated!".to_string(),
                Err(e) => e.to_string()
            };
            let _ = task_sender.send(LiveEvent::ShowError(error_message));
        });
    }

    fn process_live_events(&mut self) {
        while let Ok(msg) = self.message_receiver.try_recv() {
            match msg {
                LiveEvent::NewMessage(sms_message) => {

                    // Only add the message if we're viewing messages for the same phone number.
                    let msg = SmsMessage::from(&sms_message);
                    let mut show_notification = true;
                    if let AppState::ViewMessages(current_phone) = &self.app_state {
                        if current_phone == sms_message.phone_number.as_str() {
                            self.messages_view.add_live_message(msg.clone(), current_phone);
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
                LiveEvent::ShowError(error_message) => {
                    self.app_state = AppState::Error(error_message);
                }
            }
        }
    }

    async fn handle_state_transitions(&mut self) {
        if let AppState::ViewMessages(phone_number) = &self.app_state {

            // Only load the initial batch of messages after a state change
            // if there are no messages already loaded (prevents returning to
            // this state double loading messages).
            if self.messages_view.should_load_initial(phone_number) {
                match self.messages_view.load_messages(phone_number).await {
                    Ok(()) => {},
                    Err(e) => {
                        self.app_state = AppState::Error(e.to_string());
                    }
                }
            }
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Debounce all key presses.
        let key_press = KeyPress::from(key);
        if !self.key_debouncer.should_process(&key_press) {
            return false;
        }

        // Global theme switching with Shift+T. This was such a pain to make
        // but a coworker said it looked cool, so I stuck with it throughout.
        // This MUST remain uppercase T, since shift modifies it before here!
        if key.code == KeyCode::Char('T') && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.theme_manager.next();
            return false;
        }

        // Handle notification interactions first (priority).
        if self.notification_view.has_notifications() {
            match key.code {
                KeyCode::Char(' ') => {
                    self.notification_view.dismiss_oldest();
                    return false;
                },
                KeyCode::Enter => {
                    // Navigate to the most recent notification's conversation if it can be viewed
                    if let Some(phone_number) = self.notification_view.get_first()
                        .filter(|n| n.can_view())
                        .and_then(|n| n.get_phone_number())
                    {
                        self.notification_view.dismiss_all();
                        self.app_state = AppState::ViewMessages(phone_number);
                        self.key_debouncer.reset();
                        return false;
                    }
                },
                _ => { }
            }
        }

        match &self.app_state {
            AppState::InputPhone => self.handle_input_phone(key).await,
            AppState::ViewMessages(phone_number) => {
                self.handle_view_messages(key, phone_number.clone().as_str()).await
            },
            AppState::ComposeSms(phone_number) => {
                self.handle_compose_sms(key, phone_number.clone().as_str()).await
            },
            AppState::Error(_) => self.handle_error(key),
        }
    }

    async fn handle_input_phone(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Make sure control is held so it's not just a letter input into text box.
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return true;
            },
            KeyCode::Enter => {
                // Check if a contact is selected first
                if let Some(selected_phone) = self.phone_input_view.get_selected_phone() {
                    self.input_buffer = selected_phone;
                }

                if !self.input_buffer.is_empty() {
                    self.app_state = AppState::ViewMessages(self.input_buffer.clone());
                    self.key_debouncer.reset();
                }
            },
            KeyCode::Down => {
                self.phone_input_view.select_next();
                // Clear input buffer when navigating contacts
                self.input_buffer.clear();
            },
            KeyCode::Up => {
                self.phone_input_view.select_previous();
                // Clear input buffer when navigating contacts
                self.input_buffer.clear();
            },
            KeyCode::Backspace => {
                self.input_buffer.pop();
                // Clear selection when typing
                self.phone_input_view.clear_selection();
            },
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                // Clear selection when typing
                self.phone_input_view.clear_selection();
            },
            _ => {}
        }
        false
    }

    async fn handle_view_messages(&mut self, key: KeyEvent, phone_number: &str) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.app_state = AppState::InputPhone;
                self.key_debouncer.reset();
                self.messages_view.reset();
            },
            KeyCode::Char('c') => {
                self.sms_text_buffer.clear();
                self.app_state = AppState::ComposeSms(phone_number.to_string());
                self.sms_input_view.set_cursor_position(0, 0);
            },
            KeyCode::Char('r') => {
                match self.messages_view.reload(phone_number).await {
                    Ok(()) => {},
                    Err(e) => {
                        self.app_state = AppState::Error(e.to_string());
                    }
                }
            },
            KeyCode::Down => {
                self.messages_view.next_row().await;
                // Check if we need to load more after moving
                if let Err(e) = self.messages_view.check_load_more(phone_number).await {
                    self.messages_view.set_error_message(Some(e.to_string()));
                }
            },
            KeyCode::Up => {
                self.messages_view.previous_row().await;
            },
            KeyCode::Right => {
                self.messages_view.next_column();
            },
            KeyCode::Left => {
                self.messages_view.previous_column();
            },
            _ => {}
        }

        false
    }

    async fn handle_compose_sms(&mut self, key: KeyEvent, phone_number: &str) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.app_state = AppState::ViewMessages(phone_number.to_string());
                self.sms_text_buffer.clear();
            },
            KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.sms_text_buffer.is_empty() {
                    // TODO: Show confirmation popup, then send SMS.
                    self.app_state = AppState::ViewMessages(phone_number.to_string());
                    self.sms_text_buffer.clear();
                }
            },
            KeyCode::Enter => {
                self.sms_text_buffer.push('\n');
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Backspace => {
                if self.sms_input_view.cursor_position() > 0 {
                    let pos = self.sms_input_view.cursor_position();
                    self.sms_text_buffer.remove(pos - 1);
                    self.sms_input_view.move_cursor_left();
                }
            },
            KeyCode::Delete => {
                if self.sms_input_view.cursor_position() < self.sms_text_buffer.len() {
                    let pos = self.sms_input_view.cursor_position();
                    self.sms_text_buffer.remove(pos);
                }
            },
            KeyCode::Left => {
                self.sms_input_view.move_cursor_left();
            },
            KeyCode::Right => {
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Home => {
                self.sms_input_view.move_cursor_to_start();
            },
            KeyCode::End => {
                self.sms_input_view.move_cursor_to_end(self.sms_text_buffer.len());
            },
            KeyCode::Char(c) => {
                let pos = self.sms_input_view.cursor_position();
                self.sms_text_buffer.insert(pos, c);
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            },
            _ => {}
        }

        false
    }

    fn handle_error(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.app_state = AppState::InputPhone;
                false
            },
            _ => false
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        match &self.app_state {
            AppState::InputPhone => {
                self.phone_input_view.render(frame, &self.input_buffer, theme);
            },
            AppState::ViewMessages(phone_number) => {
                self.messages_view.render(frame, phone_number, theme);
            },
            AppState::ComposeSms(phone_number) => {
                let char_count = self.sms_text_buffer.chars().count();
                self.sms_input_view.render(
                    frame,
                    phone_number,
                    &self.sms_text_buffer,
                    char_count,
                    theme
                );
            },
            AppState::Error(msg) => {
                self.error_view.render(frame, msg, theme);
            }
        }

        // Render notifications on top of everything
        self.notification_view.render(frame, theme);
    }
}