use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use sms_client::config::ClientConfig;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

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
    messages_view: Arc<Mutex<MessagesView>>,
    phone_input_view: PhoneInputView,
    sms_input_view: SmsInputView,
    error_view: ErrorView,
    message_receiver: mpsc::UnboundedReceiver<LiveMessage>,
    message_sender: mpsc::UnboundedSender<LiveMessage>,
    current_phone_for_sms: String
}
impl App {
    pub fn new() -> Result<Self> {
        let config = ClientConfig::http_only("http://192.168.1.21:3000").with_auth("test");
        let client = sms_client::Client::new(config)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();

        Ok(Self {
            input_buffer: String::new(),
            sms_text_buffer: String::new(),
            app_state: AppState::InputPhone,
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            theme_manager: ThemeManager::new(),
            messages_view: Arc::new(Mutex::new(MessagesView::new(client.http_arc()))),
            phone_input_view: PhoneInputView::new(),
            sms_input_view: SmsInputView::new(),
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
                        let mut view = self.messages_view.lock().unwrap();
                        view.add_live_message(sms_message);
                    }
                }
                LiveMessage::Error(error) => {
                    if matches!(self.app_state, AppState::ViewMessages) {
                        let mut view = self.messages_view.lock().unwrap();
                        view.set_error_message(Some(error));
                    }
                }
            }
        }
    }

    async fn handle_state_transitions(&mut self) {
        if let AppState::ViewMessages = self.app_state {
            let should_load = {
                let view = self.messages_view.lock().unwrap();
                view.should_load_initial()
            };

            if should_load {
                let mut view = self.messages_view.lock().unwrap();
                match view.load_messages(None).await {
                    Ok(()) => {},
                    Err(e) => {
                        self.app_state = AppState::Error(e.to_string());
                    }
                }
            }
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Global theme switching with Shift+T. This was such a pain to make
        // but a coworker said it looked cool, so I stuck with it throughout.
        if key.code == KeyCode::Char('T') && key.modifiers.contains(KeyModifiers::SHIFT) {
            let key_press = KeyPress::from(key);
            if self.key_debouncer.should_process(&key_press) {
                self.theme_manager.next();
                return false;
            }
        }

        match &self.app_state {
            AppState::InputPhone => self.handle_input_phone(key),
            AppState::ViewMessages => self.handle_view_messages(key).await,
            AppState::ComposeSms => self.handle_compose_sms(key).await,
            AppState::Error(_) => self.handle_error(key),
        }
    }

    fn handle_input_phone(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                let key_press = KeyPress::from(key);
                if self.key_debouncer.should_process(&key_press) {
                    return true;
                }
            }
            KeyCode::Enter => {
                let key_press = KeyPress::from(key);
                if self.key_debouncer.should_process(&key_press) {
                    // Check if a contact is selected first
                    if let Some(selected_phone) = self.phone_input_view.get_selected_phone() {
                        self.input_buffer = selected_phone;
                    }

                    if !self.input_buffer.is_empty() {
                        let mut view = self.messages_view.lock().unwrap();
                        view.set_phone(&self.input_buffer);
                        self.app_state = AppState::ViewMessages;
                        self.key_debouncer.reset();
                    }
                }
            }
            KeyCode::Down => {
                let key_press = KeyPress::from(key);
                if self.key_debouncer.should_process(&key_press) {
                    self.phone_input_view.select_next();
                    // Clear input buffer when navigating contacts
                    self.input_buffer.clear();
                }
            }
            KeyCode::Up => {
                let key_press = KeyPress::from(key);
                if self.key_debouncer.should_process(&key_press) {
                    self.phone_input_view.select_previous();
                    // Clear input buffer when navigating contacts
                    self.input_buffer.clear();
                }
            }
            KeyCode::Backspace => {
                let key_press = KeyPress::from(key);
                if self.key_debouncer.should_process(&key_press) {
                    self.input_buffer.pop();
                    // Clear selection when typing
                    self.phone_input_view.clear_selection();
                }
            }
            KeyCode::Char(c) => {
                if key.kind == KeyEventKind::Press && self.input_buffer.len() < 20 {
                    self.input_buffer.push(c);
                    // Clear selection when typing
                    self.phone_input_view.clear_selection();
                }
            }
            _ => {}
        }
        false
    }

    async fn handle_view_messages(&mut self, key: KeyEvent) -> bool {
        let key_press = KeyPress::from(key);

        if !self.key_debouncer.should_process(&key_press) {
            return false;
        }

        // TODO: Make a state cleanup fn?
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('n') => {
                self.input_buffer.clear();
                self.app_state = AppState::InputPhone;
                let mut view = self.messages_view.lock().unwrap();
                view.reset();
                self.key_debouncer.reset();
            }
            KeyCode::Char('c') => {
                let phone = {
                    let view = self.messages_view.lock().unwrap();
                    view.current_phone().to_string()
                };
                self.current_phone_for_sms = phone;
                self.sms_text_buffer.clear();
                self.sms_input_view.set_cursor_position(0, 0);
                self.app_state = AppState::ComposeSms;
            }
            KeyCode::Char('r') => {
                let mut view = self.messages_view.lock().unwrap();
                match view.reload().await {
                    Ok(()) => {},
                    Err(e) => {
                        self.app_state = AppState::Error(e.to_string());
                    }
                }
            }
            KeyCode::Down => {
                let mut view = self.messages_view.lock().unwrap();
                view.next_row().await;
            }
            KeyCode::Up => {
                let mut view = self.messages_view.lock().unwrap();
                view.previous_row().await;
            }
            KeyCode::Right => {
                let mut view = self.messages_view.lock().unwrap();
                view.next_column();
            }
            KeyCode::Left => {
                let mut view = self.messages_view.lock().unwrap();
                view.previous_column();
            }
            _ => {}
        }

        false
    }

    async fn handle_compose_sms(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.app_state = AppState::ViewMessages;
                self.sms_text_buffer.clear();
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.sms_text_buffer.is_empty() {
                    // TODO: Implement actual SMS sending
                    self.app_state = AppState::ViewMessages;
                    self.sms_text_buffer.clear();
                }
            }
            KeyCode::Enter => {
                self.sms_text_buffer.push('\n');
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            }
            KeyCode::Backspace => {
                if self.sms_input_view.cursor_position() > 0 {
                    let pos = self.sms_input_view.cursor_position();
                    self.sms_text_buffer.remove(pos - 1);
                    self.sms_input_view.move_cursor_left();
                }
            }
            KeyCode::Delete => {
                if self.sms_input_view.cursor_position() < self.sms_text_buffer.len() {
                    let pos = self.sms_input_view.cursor_position();
                    self.sms_text_buffer.remove(pos);
                }
            }
            KeyCode::Left => {
                self.sms_input_view.move_cursor_left();
            }
            KeyCode::Right => {
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            }
            KeyCode::Home => {
                self.sms_input_view.move_cursor_to_start();
            }
            KeyCode::End => {
                self.sms_input_view.move_cursor_to_end(self.sms_text_buffer.len());
            }
            KeyCode::Char(c) => {
                let pos = self.sms_input_view.cursor_position();
                self.sms_text_buffer.insert(pos, c);
                self.sms_input_view.move_cursor_right(self.sms_text_buffer.len());
            }
            _ => {}
        }

        false
    }

    fn handle_error(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Esc {
            let key_press = KeyPress::from(key);
            if self.key_debouncer.should_process(&key_press) {
                return true;
            }
        }
        false
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.current();

        match &self.app_state {
            AppState::InputPhone => {
                self.phone_input_view.render(frame, &self.input_buffer, theme);
            }
            AppState::ViewMessages => {
                let mut view = self.messages_view.lock().unwrap();
                // Updated to pass theme as parameter
                view.render(frame, theme);
            }
            AppState::ComposeSms => {
                let char_count = self.sms_text_buffer.chars().count();
                self.sms_input_view.render(
                    frame,
                    &self.current_phone_for_sms,
                    &self.sms_text_buffer,
                    char_count,
                    theme
                );
            }
            AppState::Error(msg) => {
                self.error_view.render(frame, msg, theme);
            }
        }
    }
}