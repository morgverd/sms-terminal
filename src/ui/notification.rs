use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::time::{Duration, Instant};
use crossterm::event::{KeyCode, KeyEvent};
use sms_client::types::ModemStatusUpdateState;
use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::{AppState, KeyResponse};
use crate::ui::View;

#[derive(Clone, Debug)]
pub enum NotificationType {
    IncomingMessage {
        phone: String,
        content: String
    },
    OnlineStatus {
        previous: ModemStatusUpdateState,
        current: ModemStatusUpdateState
    },
    SendFailure {
        phone: String,
        content: String,
        error: Option<String>
    },
    WebSocketConnectionUpdate {
        connected: bool,
        reconnect: bool
    },
    GenericMessage {
        color: Color,
        title: String,
        message: String
    }
}

#[derive(Clone)]
pub struct NotificationMessage {
    pub notification_type: NotificationType,
    pub timestamp: Instant
}
impl NotificationMessage {
    pub fn get_phone_number(&self) -> Option<String> {
        match &self.notification_type {
            NotificationType::IncomingMessage { phone, .. } => Some(phone.clone()),
            NotificationType::SendFailure { phone, .. } => Some(phone.clone()),
            NotificationType::OnlineStatus { .. } => None,
            NotificationType::WebSocketConnectionUpdate { .. } => None,
            NotificationType::GenericMessage { .. } => None
        }
    }

    pub fn can_view(&self) -> bool {
        matches!(self.notification_type, NotificationType::IncomingMessage { .. })
    }

    pub fn is_expired(&self, display_duration: Duration) -> bool {
        self.timestamp.elapsed() > display_duration
    }
}

struct NotificationStyle {
    icon: &'static str,
    title: String,
    border_color: Color,
    title_color: Color
}

struct RenderContext<'a> {
    theme: &'a Theme,
    opacity_modifier: Modifier,
    is_top: bool
}

pub struct NotificationView {
    notifications: Vec<NotificationMessage>,
    display_duration: Duration,
    max_notifications: usize
}
impl NotificationView {

    const TEXTWRAP_MAX_WIDTH: usize = 50;
    const INCOMING_MESSAGE_MAX_LINES: usize = 3;

    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            display_duration: Duration::from_secs(15),
            max_notifications: 6
        }
    }

    pub fn add_notification(&mut self, notification_type: NotificationType) {
        let notification = NotificationMessage {
            notification_type,
            timestamp: Instant::now()
        };

        // Push and truncate end to maintain max size.
        self.notifications.insert(0, notification);
        if self.notifications.len() > self.max_notifications {
            self.notifications.truncate(self.max_notifications);
        }
    }

    fn dismiss_all(&mut self) {
        if !self.notifications.is_empty() {
            self.notifications.clear();
        }
    }

    fn dismiss_oldest(&mut self) {
        if !self.notifications.is_empty() {
            self.notifications.pop();
        }
    }

    fn get_notification_style(&self, notification: &NotificationMessage, theme: &Theme) -> NotificationStyle {
        match &notification.notification_type {
            NotificationType::IncomingMessage { .. } => NotificationStyle {
                icon: "📨",
                title: "New Message".to_string(),
                border_color: theme.text_accent,
                title_color: theme.text_accent
            },
            NotificationType::OnlineStatus { current: current_state, .. } => {
                let (icon, color) = match current_state {
                    ModemStatusUpdateState::Online => ("🟢", Color::Green),
                    ModemStatusUpdateState::Offline => ("🔴", Color::Red),
                    ModemStatusUpdateState::Startup | ModemStatusUpdateState::ShuttingDown => ("🟡", Color::Yellow)
                };
                NotificationStyle {
                    icon,
                    title: "Status Change".to_string(),
                    border_color: color,
                    title_color: color
                }
            },
            NotificationType::SendFailure { .. } => NotificationStyle {
                icon: "❌",
                title: "Send Failed".to_string(),
                border_color: Color::Red,
                title_color: Color::Red
            },
            NotificationType::WebSocketConnectionUpdate { connected, reconnect } => {
                let (icon, title, color) = match (connected, reconnect) {
                    (true, _) => ("🔗", "WebSocket Connected", Color::Green),
                    (false, true) => ("🔄", "WebSocket Reconnecting", Color::Yellow),
                    (false, false) => ("⚠️", "WebSocket Disconnected", Color::Red),
                };
                NotificationStyle {
                    icon,
                    title: title.into(),
                    border_color: color,
                    title_color: color
                }
            },
            NotificationType::GenericMessage { color, title, .. } => NotificationStyle {
                icon: "❌",
                title: title.into(),
                border_color: color.clone(),
                title_color: color.clone()
            }
        }
    }

    fn calculate_notification_height(&self, notification: &NotificationMessage, is_top: bool) -> u16 {
        let base_height = match &notification.notification_type {
            NotificationType::IncomingMessage { content, .. } => {
                let content_lines = (content.len() / 45).max(1).min(3);
                5 + content_lines as u16
            },
            NotificationType::OnlineStatus { .. } => 3,
            NotificationType::SendFailure { .. } => unimplemented!(),
            NotificationType::WebSocketConnectionUpdate { .. } => 3,
            NotificationType::GenericMessage { .. } => 3
        };

        // Add extra height for empty line separator and controls hint if it's the top notification.
        if is_top {
            base_height + 2
        } else {
            base_height
        }
    }

    fn render_notification(
        &self,
        frame: &mut Frame,
        notification: &NotificationMessage,
        area: Rect,
        ctx: &RenderContext
    ) {
        frame.render_widget(Clear, area);

        let style = self.get_notification_style(notification, ctx.theme);
        let title = format!(" {} {} ", style.icon, style.title);
        let block = Block::bordered()
            .title(title)
            .title_style(Style::default().fg(style.title_color))
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(style.border_color).add_modifier(ctx.opacity_modifier));

        let lines = self.build_notification_content(notification, ctx);
        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .style(Style::default().add_modifier(ctx.opacity_modifier));

        frame.render_widget(paragraph, area);
    }

    fn build_notification_content(
        &self,
        notification: &NotificationMessage,
        ctx: &RenderContext
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let base_style = Style::default().fg(ctx.theme.text_primary).add_modifier(ctx.opacity_modifier);
        let accent_style = Style::default().fg(ctx.theme.text_accent).add_modifier(ctx.opacity_modifier);
        let muted_style = Style::default().fg(ctx.theme.text_muted).add_modifier(ctx.opacity_modifier);

        match &notification.notification_type {
            NotificationType::IncomingMessage { phone, content } => {
                lines.push(Line::from(vec![
                    Span::styled("From: ", muted_style),
                    Span::styled(phone.clone(), accent_style),
                ]));
                lines.push(Line::raw(""));

                let wrapped_lines = textwrap::wrap(content, Self::TEXTWRAP_MAX_WIDTH);
                let mut content_lines_added = 0;

                for wrapped_line in wrapped_lines.iter().take(Self::INCOMING_MESSAGE_MAX_LINES) {
                    lines.push(Line::from(Span::styled(wrapped_line.to_string(), base_style)));
                    content_lines_added += 1;
                }

                // Add truncation indicator if there's more content
                if wrapped_lines.len() > Self::INCOMING_MESSAGE_MAX_LINES {
                    lines.push(Line::from(Span::styled("...", muted_style)));
                }

                // Ensure we don't have too few lines (pad if needed) for possible controls line
                while content_lines_added < 1 {
                    lines.push(Line::raw(""));
                    content_lines_added += 1;
                }
            },
            NotificationType::OnlineStatus { previous: previous_state, current: current_state } => {
                lines.push(Line::from(vec![
                    Span::styled(previous_state.to_string(), muted_style),
                    Span::styled(" → ", muted_style),
                    Span::styled(current_state.to_string(), accent_style),
                ]));
            },
            NotificationType::SendFailure { .. } => unimplemented!(),
            NotificationType::WebSocketConnectionUpdate { connected, reconnect } => {
                let status_text = match (connected, reconnect) {
                    (true, _) => "WebSocket connection established",
                    (false, true) => "WebSocket disconnected, attempting to reconnect...",
                    (false, false) => "WebSocket connection lost",
                };
                lines.push(Line::from(Span::styled(status_text.to_string(), base_style)));
            },
            NotificationType::GenericMessage { message, .. } => {
                lines.push(Line::from(Span::styled(message.clone(), base_style)));
            }
        }

        // Show controls hint only for the most recent notification
        if ctx.is_top {
            lines.push(Line::raw(""));

            // Only show "(Enter) view" for notifications that can be viewed
            let controls_text = if notification.can_view() {
                "(F1) dismiss • (F2) view"
            } else {
                "(F1) dismiss"
            };

            lines.push(Line::from(Span::styled(
                controls_text,
                Style::default().fg(ctx.theme.text_muted).add_modifier(Modifier::ITALIC)
            )));
        }

        lines
    }
}
impl View for NotificationView {
    type Context = ();

    async fn load(&mut self, _ctx: Self::Context) -> AppResult<()> {
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent, _ctx: Self::Context) -> Option<KeyResponse> {
        match key.code {
            KeyCode::F(1) => {
                self.dismiss_oldest();
            },
            KeyCode::F(2) => {

                // Navigate to the most recent notification's conversation if it can be viewed
                if let Some(phone_number) = self.notifications.first()
                    .filter(|n| n.can_view())
                    .and_then(|n| n.get_phone_number())
                {
                    self.dismiss_all();

                    let state = AppState::view_messages(phone_number);
                    return Some(KeyResponse::SetAppState(state));
                }
            },
            _ => { }
        }

        None
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme, _ctx: Self::Context) {

        // TODO: Should be calling this way less. No need to enforce expiry every frame.
        self.notifications.retain(|notification| !notification.is_expired(self.display_duration));
        if self.notifications.is_empty() {
            return;
        }

        let area = frame.area();
        let mut y_offset = 1;
        let mut is_top = true;

        for notification in self.notifications.iter() {
            let ctx = RenderContext {
                theme,
                opacity_modifier: if is_top { Modifier::empty() } else { Modifier::DIM },
                is_top
            };

            // Position notifications from top-right
            let width = area.width.min(55);
            let x = area.width.saturating_sub(width).saturating_sub(1);
            let y = y_offset;

            let height = self.calculate_notification_height(notification, is_top);
            if y + height > area.height.saturating_sub(1) {
                break;
            }

            let popup_area = Rect::new(x, y, width, height);
            self.render_notification(frame, notification, popup_area, &ctx);

            y_offset += height + 1;
            is_top = false;
        }
    }
}
impl Default for NotificationView {
    fn default() -> Self {
        Self::new()
    }
}