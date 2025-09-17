use std::sync::Arc;
use std::time::Duration;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::style::Color;
use sms_client::Client;
use sms_client::http::HttpClient;
use sms_client::ws::types::WebsocketMessage;
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::TerminalConfig;
use crate::error::{AppError, AppResult};
use crate::modals::{AppModal, ModalLoadBehaviour};
use crate::theme::ThemeManager;
use crate::types::{KeyDebouncer, KeyPress, AppAction, SmsMessage, DEBOUNCE_DURATION};
use crate::ui::ViewBase;
use crate::ui::notifications::{NotificationType, NotificationsView};
use crate::ui::views::{ViewManager, ViewStateRequest};

pub type AppActionSender = mpsc::UnboundedSender<AppAction>;
pub type AppContext = (Arc<HttpClient>, AppActionSender);

pub struct App {
    view_manager: ViewManager,
    notifications: NotificationsView,
    current_modal: Option<AppModal>,
    theme_manager: ThemeManager,
    key_debouncer: KeyDebouncer,
    message_receiver: mpsc::UnboundedReceiver<AppAction>,
    message_sender: mpsc::UnboundedSender<AppAction>,
    sms_client: Client,
    websocket_enabled: bool,
    render_views: bool,

    #[cfg(feature = "sentry")]
    sentry_enabled: bool
}
impl App {
    pub fn new(config: TerminalConfig) -> Result<Self> {
        let client = Client::new(config.client)
            .map_err(|e| AppError::ConfigError(e.to_string()))?;

        let (tx, rx) = mpsc::unbounded_channel();
        let context: AppContext = (client.http_arc(), tx.clone());

        Ok(Self {
            view_manager: ViewManager::new(context)?,
            notifications: NotificationsView::new(),
            current_modal: None,
            theme_manager: ThemeManager::with_preset(config.theme),
            key_debouncer: KeyDebouncer::new(DEBOUNCE_DURATION),
            message_receiver: rx,
            message_sender: tx,
            sms_client: client,
            websocket_enabled: config.websocket,
            render_views: true,

            #[cfg(feature = "sentry")]
            sentry_enabled: config.sentry.is_some()
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
                icon: "❌".to_string(),
                title: "WebSocket Disabled".to_string(),
                message: "Live updates will not show!".to_string(),
            };
            self.notifications.add_notification(notification);
        };

        // If we're running a +sentry build, we're expecting to run in some managed env
        // where the sentry dsn is always set. Therefore, if it isn't show a warning.
        #[cfg(feature = "sentry")]
        if !self.sentry_enabled {
            let notification = NotificationType::GenericMessage {
                color: Color::Yellow,
                icon: "❌".to_string(),
                title: "Sentry Inactive".to_string(),
                message: "Sentry feature is compiled, but is not configured!".to_string(),
            };
            self.notifications.add_notification(notification);
        }

        // Transition into starting state (which may be an error!)
        self.transition_view(ViewStateRequest::default()).await;

        let mut ticker = interval(Duration::from_millis(30));
        loop {
            // Process all actions from the channel
            while let Ok(action) = self.message_receiver.try_recv() {
                if self.handle_app_action(action).await {
                    return Ok(());
                }
            }

            terminal.draw(|frame| {
                let theme = self.theme_manager.current();

                // Views (bottom)
                if self.render_views {
                    self.view_manager.render(frame, &theme);
                }

                // Modals
                if let Some(modal) = &mut self.current_modal {
                    modal.render(frame, &theme);
                }

                // Notifications (top)
                self.notifications.render(frame, &theme, ());
            })?;

            // Poll for key input
            while event::poll(Duration::from_millis(0))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if let Some(action) = self.get_key_action(key).await {
                        if self.handle_app_action(action).await {
                            return Ok(());
                        }
                    }
                }
            }

            // Yield back to runtime (for messages from websocket)
            ticker.tick().await;
        }
    }

    async fn transition_view(&mut self, request: ViewStateRequest) {
        self.view_manager.transition_to(request).await;
        self.key_debouncer.reset();

        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::SetTitle(
                format!("SMS Terminal v{} ｜ {}", crate::VERSION, self.view_manager)
            ),
        );
    }

    async fn handle_app_action(&mut self, action: AppAction) -> bool {
        match action {
            AppAction::SetViewState { state, dismiss_modal } => {

                // Allow the state change to dismiss the current modal.
                // This is useful for transitioning out of a loading state.
                if self.current_modal.is_some() && dismiss_modal {
                    self.set_modal(None);
                }
                self.transition_view(state).await;
            },
            AppAction::SetModal(modal) => self.set_modal(modal),
            AppAction::Exit => return true,
            AppAction::HandleIncomingMessage(sms_message) => {

                // Try to add the incoming message to the current view
                let msg = SmsMessage::from(&sms_message);
                let show_notification = !self.view_manager.try_add_message(&msg);

                // Show incoming notification if not suppressed by view
                if show_notification && !sms_message.is_outgoing {
                    let notification = NotificationType::IncomingMessage {
                        phone: sms_message.phone_number.clone(),
                        content: msg.content
                    };
                    self.notifications.add_notification(notification);
                }
            },
            AppAction::DeliveryFailure(_) => unimplemented!("Oops!"),
            AppAction::ShowNotification(notification) => {
                self.notifications.add_notification(notification)
            },
            AppAction::ShowError { message, dismissible } => {

                // If another error is being displayed, only overwrite it if
                // that one is dismissable but this one isn't. Otherwise, ignore.
                if self.view_manager.should_show_error(dismissible) {
                    self.transition_view(ViewStateRequest::Error { message, dismissible }).await;
                }
            }
        };

        false
    }

    async fn get_key_action(&mut self, key: KeyEvent) -> Option<AppAction> {
        let key_press = KeyPress::from(key);
        if !self.key_debouncer.should_process(&key_press) {
            return None;
        }

        // TODO: FIND BETTER KEYS.
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
            let response = self.view_manager.handle_modal_response(modal, key);
            if response.is_some() {

                // Dismiss the current modal if some response was returned.
                self.set_modal(None);
            }
            return response;
        }

        // Handle notification interactions
        if let Some(response) = self.notifications.handle_key(key, ()).await {
            return Some(response);
        }

        // Delegate to current view
        self.view_manager.handle_key(key).await
    }

    fn set_modal(&mut self, modal: Option<AppModal>) {
        // Allow the modal to determine if background views should render.
        self.render_views = modal.as_ref()
            .map(|m| m.should_render_views())
            .unwrap_or(true);

        if let Some(ref modal) = modal {

            // Call modal loader, which can take the current AppContext for async loading.
            // This is to ensure that the render + async loop is never blocked.
            match modal.load() {
                ModalLoadBehaviour::Function(cb) => {
                    let (action, should_block) = cb((self.sms_client.http_arc(), self.message_sender.clone()));
                    if let Some(action) = action {
                        let _ = self.message_sender.send(action);
                    }
                    if should_block {
                        return;
                    }
                },
                _ => { }
            }
        }

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