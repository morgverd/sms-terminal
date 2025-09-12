use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use std::time::{Duration, Instant};
use sms_client::types::ModemStatusUpdateState;
use crate::theme::Theme;

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
            NotificationType::WebSocketConnectionUpdate { .. } => None
        }
    }
}

struct NotificationStyle {
    icon: &'static str,
    title: &'static str,
    border_color: Color,
    title_color: Color
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

    pub fn first(&self) -> Option<&NotificationMessage> {
        self.notifications.first()
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

    pub fn clear_all(&mut self) {
        self.notifications.clear();
    }

    pub fn has_notifications(&self) -> bool {
        !self.notifications.is_empty()
    }

    fn get_notification_style(&self, notification: &NotificationMessage, theme: &Theme) -> NotificationStyle {
        match &notification.notification_type {
            NotificationType::IncomingMessage { .. } => NotificationStyle {
                icon: "📨",
                title: "New Message",
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
                    title: "Status Change",
                    border_color: color,
                    title_color: color
                }
            },
            NotificationType::SendFailure { .. } => NotificationStyle {
                icon: "❌",
                title: "Send Failed",
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
                    title,
                    border_color: color,
                    title_color: color
                }
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        if self.notifications.is_empty() {
            return;
        }

        let area = frame.area();
        let mut y_offset = 1;
        let mut is_top = true;

        for notification in self.notifications.iter() {
            let style = self.get_notification_style(notification, theme);
            let height = self.calculate_notification_height(notification, is_top);

            // Position notifications from top-right
            let width = area.width.min(55);
            let x = area.width.saturating_sub(width).saturating_sub(1);
            let y = y_offset;

            if y + height > area.height.saturating_sub(1) {
                break;
            }

            // Apply fade effect for older notifications
            let opacity_modifier = if is_top {
                Modifier::empty()
            } else {
                Modifier::DIM
            };

            let popup_area = Rect::new(x, y, width, height);
            self.render_notification(frame, notification, popup_area, &style, theme, opacity_modifier, is_top);

            y_offset += height + 1;
            is_top = false;
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
            NotificationType::WebSocketConnectionUpdate { .. } => 3
        };

        // Add extra height for empty line separator and controls hint if its the top notification.
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
        style: &NotificationStyle,
        theme: &Theme,
        opacity_modifier: Modifier,
        is_top: bool,
    ) {
        frame.render_widget(Clear, area);
        let title = format!(" {} {} ", style.icon, style.title);
        let block = Block::bordered()
            .title(title)
            .title_style(Style::default().fg(style.title_color))
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(style.border_color).add_modifier(opacity_modifier));

        let lines = self.build_notification_content(notification, theme, opacity_modifier, is_top);
        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: true })
            .style(Style::default().add_modifier(opacity_modifier));

        frame.render_widget(paragraph, area);
    }

    fn build_notification_content(
        &self,
        notification: &NotificationMessage,
        theme: &Theme,
        opacity_modifier: Modifier,
        is_top: bool,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let base_style = Style::default().fg(theme.text_primary).add_modifier(opacity_modifier);
        let accent_style = Style::default().fg(theme.text_accent).add_modifier(opacity_modifier);
        let muted_style = Style::default().fg(theme.text_muted).add_modifier(opacity_modifier);

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
            }
        }

        // Show controls hint only for the most recent notification
        if is_top {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "(Space) dismiss • (Enter) view",
                Style::default().fg(theme.text_muted).add_modifier(Modifier::ITALIC)
            )));
        }

        lines
    }
}
impl Default for NotificationView {
    fn default() -> Self {
        Self::new()
    }
}