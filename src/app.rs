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
use crate::modals::AppModal;
use crate::theme::ThemeManager;
use crate::types::{ViewState, KeyDebouncer, KeyPress, AppAction, SmsMessage, DEBOUNCE_DURATION};
use crate::ui::{ModalResponderComponent, ViewBase};
use crate::ui::notification::{NotificationType, NotificationView};
use crate::ui::views::error::ErrorView;
use crate::ui::views::messages::MessagesView;
use crate::ui::views::phonebook::PhonebookView;
use crate::ui::views::compose::ComposeView;

pub type AppContext = (Arc<HttpClient>, mpsc::UnboundedSender<AppAction>);

pub struct App {
    view_state: ViewState,
    current_modal: Option<AppModal>,
    key_debouncer: KeyDebouncer,
    theme_manager: ThemeManager,
    phonebook_view: PhonebookView,
    messages_view: MessagesView,
    compose_view: ComposeView,
    error_view: ErrorView,
    notification_view: NotificationView,
    message_receiver: mpsc::UnboundedReceiver<AppAction>,
    message_sender: mpsc::UnboundedSender<AppAction>,
    sms_client: Client,
    websocket_enabled: bool,
    render_views: bool
}
impl App {
    pub fn new(config: TerminalConfig) -> Result<Self> {
        let client = Client::new(config.client)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let context: AppContext = (client.http_arc(), tx.clone());

        Ok(Self {
            view_state: ViewState::Phonebook,
            current_modal: None,
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            theme_manager: ThemeManager::with_preset(config.theme),
            phonebook_view: PhonebookView::with_context(context.clone()),
            messages_view: MessagesView::with_context(context.clone()),
            compose_view: ComposeView::with_context(context),
            error_view: ErrorView::new(),
            notification_view: NotificationView::new(),
            message_receiver: rx,
            message_sender: tx,
            sms_client: client,
            websocket_enabled: config.websocket,
            render_views: true
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
        self.transition_state(ViewState::Phonebook).await;

        loop {
            terminal.draw(|frame| self.render(frame))?;
            while let Ok(action) = self.message_receiver.try_recv() {
                self.handle_app_action(action).await;
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    // Transition the state, handling error format easily.
                    // This is also the only direct way to quit, by returning true in the key response.
                    if let Some(action) = self.get_key_action(key).await {
                        if self.handle_app_action(action).await {
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
        if self.render_views {
            match &self.view_state {
                ViewState::Phonebook => self.phonebook_view.render(frame, theme, ()),
                ViewState::Messages { phone_number, reversed } => self.messages_view.render(frame, theme, (phone_number, *reversed)),
                ViewState::Compose { phone_number } => self.compose_view.render(frame, theme, phone_number),
                ViewState::Error { message, dismissible } => self.error_view.render(frame, theme, (message, *dismissible))
            }
        }

        // Render modal on top of main view
        if let Some(modal) = &mut self.current_modal {
            modal.render(frame, theme);
        }

        // Render notifications on top of everything
        self.notification_view.render(frame, theme, ());
    }

    async fn transition_state(&mut self, new_state: ViewState) {
        let result = match &new_state {
            ViewState::Phonebook => self.phonebook_view.load(()).await,
            ViewState::Messages { phone_number, reversed } => self.messages_view.load((phone_number, *reversed)).await,
            ViewState::Compose { phone_number } => self.compose_view.load(phone_number).await,
            _ => Ok(())
        };

        // Get the actual state by first checking if any of
        // the view transition results returned an error.
        let actual_state = if let Err(e) = result {
            ViewState::from(e)
        } else {
            new_state
        };

        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::SetTitle(format!("SMS Terminal v{} ｜ {}", crate::VERSION, actual_state)),
        );

        self.view_state = actual_state;
        self.key_debouncer.reset();
    }

    async fn handle_app_action(&mut self, response: AppAction) -> bool {
        match response {
            AppAction::SetAppState(new_state) => {

                // TODO: SOLVE THIS!!
                // if matches!(self.current_modal, Some(AppModal::Loading { .. })) {
                //     self.set_modal(None);
                // }
                self.transition_state(new_state).await
            }
            AppAction::ShowModal(modal) => {
                self.set_modal(Some(modal));
            },
            AppAction::Exit => return true,
            AppAction::HandleIncomingMessage(sms_message) => {
                if let Err(e) = self.handle_new_message(sms_message).await {
                    self.transition_state(ViewState::from(e)).await;
                }
            },
            AppAction::DeliveryFailure(_) => unimplemented!("Oops!"),
            AppAction::ShowNotification(notification) => self.notification_view.add_notification(notification),
            AppAction::ShowError { message, dismissible } => {

                // If another error is being displayed, only overwrite it if
                // that one is dismissable but this one isn't. Otherwise, ignore.
                let allowed = match &self.view_state {
                    ViewState::Error { dismissible: existing_dismissable, .. } => {
                        *existing_dismissable || !dismissible
                    },
                    _ => true
                };
                if allowed {
                    self.transition_state(ViewState::Error { message, dismissible }).await;
                }
            }
        };

        false
    }

    async fn get_key_action(&mut self, key: KeyEvent) -> Option<AppAction> {
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
            let Some(response) = modal.handle_key(key) else {
                return None;
            };

            // Route response to appropriate view based on ID
            let response = match modal.id.as_str() {
                "confirm_sms_send" => self.compose_view.handle_modal_response(response, modal.metadata.clone()),
                "edit_friendly_name" => self.phonebook_view.handle_modal_response(response, modal.metadata.clone()),
                id => Some(AppAction::ShowError {
                    message: format!("Got unknown modal response ID {}!", id),
                    dismissible: true
                })
            };

            // Clear current modal before submitting response
            // This also passively handles Dismissed
            self.set_modal(None);
            return response;
        }

        // Handle notification interactions
        if let Some(response) = self.notification_view.handle_key(key, ()).await {
            return Some(response);
        }

        // View handlers
        match &self.view_state {
            ViewState::Phonebook => self.phonebook_view.handle_key(key, ()).await,
            ViewState::Messages { phone_number, reversed } => self.messages_view.handle_key(key, (phone_number, *reversed)).await,
            ViewState::Compose { phone_number } => self.compose_view.handle_key(key, phone_number).await,
            ViewState::Error { message, dismissible } => self.error_view.handle_key(key, (message, *dismissible)).await
        }
    }

    async fn handle_new_message(&mut self, sms_message: SmsStoredMessage) -> AppResult<()> {
        // Only add the message if we're viewing messages for the same phone number.
        let msg = SmsMessage::from(&sms_message);
        let mut show_notification = true;
        match &self.view_state {
            ViewState::Messages { phone_number, .. } if phone_number == sms_message.phone_number.as_str() => {
                self.messages_view.add_live_message(msg.clone());
                show_notification = false;
            }
            _ => { }
        }

        // Push to phone list view always so it maintains order.
        self.phonebook_view.push_new_number(
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

    fn set_modal(&mut self, modal: Option<AppModal>) {
        // Allow the modal to determine if background views should render.
        self.render_views = modal.as_ref()
            .map(|m| m.should_render_views())
            .unwrap_or(true);

        self.current_modal = modal;
    }

    async fn start_sms_websocket(&self) -> AppResult<()> {
        let ws_sender = self.message_sender.clone();
        self.sms_client.on_message_simple(move |message| {
            match message {
                WebsocketMessage::IncomingMessage(sms) | WebsocketMessage::OutgoingMessage(sms) => {
                    let _ = ws_sender.send(AppAction::HandleIncomingMessage(sms));
                },
                WebsocketMessage::ModemStatusUpdate { previous, current } => {
                    let notification = NotificationType::OnlineStatus { previous, current };
                    let _ = ws_sender.send(AppAction::ShowNotification(notification));
                },
                WebsocketMessage::WebsocketConnectionUpdate { connected, reconnect } => {
                    let notification = NotificationType::WebSocketConnectionUpdate { connected, reconnect };
                    let _ = ws_sender.send(AppAction::ShowNotification(notification));
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
            let _ = task_sender.send(AppAction::ShowError { message, dismissible });
        });

        Ok(())
    }
}