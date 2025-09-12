use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::TerminalConfig;
use crate::error::AppError;
use crate::theme::ThemeManager;
use crate::types::{AppState, KeyDebouncer, KeyPress, SmsMessage, DEBOUNCE_DURATION};
use crate::ui::error::ErrorView;
use crate::ui::messages_table::MessagesView;
use crate::ui::phone_input::PhoneInputView;
use crate::ui::sms_input::SmsInputView;

/// TODO: WebSocket integration!
#[derive(Debug, Clone)]
pub enum LiveMessage {
    NewMessage(SmsMessage),
    Error(String),
}

pub struct App {
    input_buffer: String,
    sms_text_buffer: String,
    app_state: AppState,
    key_debouncer: KeyDebouncer,
    theme_manager: ThemeManager,
    phone_input_view: PhoneInputView,
    messages_view: Arc<RwLock<MessagesView>>,
    sms_input_view: Arc<RwLock<SmsInputView>>,
    error_view: ErrorView,
    message_receiver: mpsc::UnboundedReceiver<LiveMessage>,
    message_sender: mpsc::UnboundedSender<LiveMessage>,
    current_phone_for_sms: String
}
impl App {
    pub fn new(config: TerminalConfig) -> Result<Self> {
        let client = sms_client::Client::new(config.client)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();
        Ok(Self {
            input_buffer: String::new(),
            sms_text_buffer: String::new(),
            app_state: AppState::InputPhone,
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            theme_manager: ThemeManager::with_preset(config.theme),
            phone_input_view: PhoneInputView::new(),
            messages_view: Arc::new(RwLock::new(MessagesView::new(client.http_arc()))),
            sms_input_view: Arc::new(RwLock::new(SmsInputView::new())),
            error_view: ErrorView::new(),
            message_receiver: rx,
            message_sender: tx,
            current_phone_for_sms: String::new()
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.spawn_live_message_listener();

        loop {
            terminal.draw(|frame| self.render(frame))?;
            self.process_live_messages();
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

    /// TODO: IMPLEMENT!
    fn spawn_live_message_listener(&self) {
        // let messages_view = Arc::clone(&self.messages_view);
        // let sender = self.message_sender.clone();
        //
        // tokio::spawn(async move {
        //     let mut interval = tokio::time::interval(Duration::from_secs(5));
        //
        //     loop {
        //         interval.tick().await;
        //
        //         let should_listen = {
        //             let view = messages_view.lock().unwrap();
        //             !view.current_phone().is_empty()
        //         };
        //
        //         if should_listen {
        //
        //         }
        //     }
        // });
    }

    fn process_live_messages(&mut self) {
        while let Ok(msg) = self.message_receiver.try_recv() {
            match msg {
                LiveMessage::NewMessage(sms_message) => {
                    if matches!(self.app_state, AppState::ViewMessages) {
                        self.messages_view.write().unwrap().add_live_message(sms_message);
                    }
                }
                LiveMessage::Error(error) => {
                    if matches!(self.app_state, AppState::ViewMessages) {
                        self.messages_view.write().unwrap().set_error_message(Some(error));
                    }
                }
            }
        }
    }

    async fn handle_state_transitions(&mut self) {
        if let AppState::ViewMessages = self.app_state {
            let should_load = {
                self.messages_view.read().unwrap().should_load_initial()
            };

            if should_load {
                match self.messages_view.write().unwrap().load_messages(None).await {
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

        match &self.app_state {
            AppState::InputPhone => self.handle_input_phone(key).await,
            AppState::ViewMessages => self.handle_view_messages(key).await,
            AppState::ComposeSms => self.handle_compose_sms(key).await,
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
                    self.messages_view.write().unwrap().set_phone(&self.input_buffer);
                    self.app_state = AppState::ViewMessages;
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

    async fn handle_view_messages(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.app_state = AppState::InputPhone;
                self.key_debouncer.reset();
                self.messages_view.write().unwrap().reset();
            },
            KeyCode::Char('c') => {
                let phone = {
                    self.messages_view.read()
                        .unwrap()
                        .current_phone()
                        .to_string()
                };
                self.current_phone_for_sms = phone;
                self.sms_text_buffer.clear();
                self.app_state = AppState::ComposeSms;
                self.sms_input_view.write().unwrap().set_cursor_position(0, 0);
            },
            KeyCode::Char('r') => {
                match self.messages_view.write().unwrap().reload().await {
                    Ok(()) => {},
                    Err(e) => {
                        self.app_state = AppState::Error(e.to_string());
                    }
                }
            },
            KeyCode::Down => {
                self.messages_view.write().unwrap().next_row().await;
            },
            KeyCode::Up => {
                self.messages_view.write().unwrap().previous_row().await;
            },
            KeyCode::Right => {
                self.messages_view.write().unwrap().next_column();
            },
            KeyCode::Left => {
                self.messages_view.write().unwrap().previous_column();
            },
            _ => {}
        }

        false
    }

    async fn handle_compose_sms(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.app_state = AppState::ViewMessages;
                self.sms_text_buffer.clear();
            },
            KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.sms_text_buffer.is_empty() {
                    // TODO: Show confirmation popup, then send SMS.
                    self.app_state = AppState::ViewMessages;
                    self.sms_text_buffer.clear();
                }
            },
            KeyCode::Enter => {
                self.sms_text_buffer.push('\n');
                self.sms_input_view.write().unwrap().move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Backspace => {
                let mut input = self.sms_input_view.write().unwrap();
                if input.cursor_position() > 0 {
                    let pos = input.cursor_position();
                    self.sms_text_buffer.remove(pos - 1);
                    input.move_cursor_left();
                }
            },
            KeyCode::Delete => {
                let input = self.sms_input_view.read().unwrap();
                if input.cursor_position() < self.sms_text_buffer.len() {
                    let pos = input.cursor_position();
                    self.sms_text_buffer.remove(pos);
                }
            },
            KeyCode::Left => {
                self.sms_input_view.write().unwrap().move_cursor_left();
            },
            KeyCode::Right => {
                self.sms_input_view.write().unwrap().move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Home => {
                self.sms_input_view.write().unwrap().move_cursor_to_start();
            },
            KeyCode::End => {
                self.sms_input_view.write().unwrap().move_cursor_to_end(self.sms_text_buffer.len());
            },
            KeyCode::Char(c) => {
                let mut view = self.sms_input_view.write().unwrap();
                let pos = view.cursor_position();
                self.sms_text_buffer.insert(pos, c);
                view.move_cursor_right(self.sms_text_buffer.len());
            },
            _ => {}
        }

        false
    }

    fn handle_error(&mut self, key: KeyEvent) -> bool {
        key.code == KeyCode::Esc
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        match &self.app_state {
            AppState::InputPhone => {
                self.phone_input_view.render(frame, &self.input_buffer, theme);
            },
            AppState::ViewMessages => {
                let mut view = self.messages_view.write().unwrap();
                view.render(frame, theme);
            },
            AppState::ComposeSms => {
                let char_count = self.sms_text_buffer.chars().count();
                self.sms_input_view.read().unwrap().render(
                    frame,
                    &self.current_phone_for_sms,
                    &self.sms_text_buffer,
                    char_count,
                    theme
                );
            },
            AppState::Error(msg) => {
                self.error_view.render(frame, msg, theme);
            }
        }
    }
}