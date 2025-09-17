use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::prelude::Color;
use ratatui::style::palette::tailwind;
use sms_client::http::types::HttpOutgoingSmsMessage;
use sms_client::types::SmsStoredMessage;

use crate::app::AppContext;
use crate::error::AppResult;
use crate::modals::{AppModal, ModalMetadata, ModalResponse};
use crate::theme::Theme;
use crate::types::{ViewState, AppAction};
use crate::ui::{centered_rect, ModalResponderComponent, ViewBase};
use crate::ui::modals::confirmation::ConfirmationModal;
use crate::ui::modals::loading::LoadingModal;
use crate::ui::notification::NotificationType;

const BASE_SEND_TIMEOUT: usize = 30;

fn get_sms_count(char_count: usize) -> usize {
    match char_count {
        0 => 0,
        1..=160 => 1,
        _ => {
            // For multipart messages, each part is 153 chars (7 chars for metadata)
            (char_count + 152) / 153
        }
    }
}

pub struct ComposeView {
    context: AppContext,
    cursor_position: usize,
    sms_text_buffer: String,
    is_sending: bool
}
impl ComposeView {

    pub fn with_context(context: AppContext) -> Self {
        Self {
            context,
            cursor_position: 0,
            sms_text_buffer: String::new(),
            is_sending: false
        }
    }

    fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    fn move_cursor_right(&mut self, text_len: usize) {
        if self.cursor_position < text_len {
            self.cursor_position += 1;
        }
    }

    fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    fn move_cursor_to_end(&mut self, text_len: usize) {
        self.cursor_position = text_len;
    }

    fn render_text_with_cursor(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.sms_text_buffer.is_empty() {
            return vec![Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ])];
        }

        let mut lines = Vec::new();
        let text_lines: Vec<&str> = self.sms_text_buffer.lines().collect();

        let mut char_count = 0;
        for line in text_lines.iter() {
            let line_start = char_count;
            let line_end = line_start + line.len();

            let mut spans = Vec::new();

            if self.cursor_position >= line_start && self.cursor_position <= line_end {
                let cursor_pos_in_line = self.cursor_position - line_start;

                if cursor_pos_in_line > 0 {
                    spans.push(Span::raw(line[..cursor_pos_in_line].to_string()));
                }

                if cursor_pos_in_line < line.len() {
                    spans.push(Span::styled(
                        line.chars().nth(cursor_pos_in_line).unwrap().to_string(),
                        Style::default()
                            .fg(theme.bg)
                            .bg(theme.input_cursor)
                            .add_modifier(Modifier::SLOW_BLINK)
                    ));

                    if cursor_pos_in_line + 1 < line.len() {
                        spans.push(Span::raw(line[cursor_pos_in_line + 1..].to_string()));
                    }
                } else {
                    spans.push(Span::styled(
                        "█",
                        Style::default()
                            .fg(theme.input_cursor)
                            .add_modifier(Modifier::SLOW_BLINK)
                    ));
                }
            } else {
                spans.push(Span::raw(line.to_string()));
            }

            lines.push(Line::from(spans));
            char_count = line_end + 1;
        }

        if self.cursor_position == self.sms_text_buffer.len() && self.sms_text_buffer.ends_with('\n') {
            lines.push(Line::from(vec![
                Span::styled("█", Style::default().fg(theme.input_cursor).add_modifier(Modifier::SLOW_BLINK))
            ]));
        }

        lines
    }
}
impl ViewBase for ComposeView {
    type Context<'ctx> = &'ctx String;

    async fn load<'ctx>(&mut self, _ctx: Self::Context<'ctx>) -> AppResult<()> {
        self.cursor_position = 0;
        self.is_sending = false;
        self.sms_text_buffer.clear();
        Ok(())
    }

    async fn handle_key<'ctx>(&mut self, key: KeyEvent, ctx: Self::Context<'ctx>) -> Option<AppAction> {
        // Ignore all keyboard input while sending the message
        if self.is_sending {
            return None;
        }

        match key.code {
            KeyCode::Esc => {
                self.sms_text_buffer.clear();
                return Some(AppAction::SetViewState {
                    state: ViewState::view_messages(ctx),
                    dismiss_modal: false
                });
            },
            KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.sms_text_buffer.is_empty() {

                    // Show a confirmation modal with message send metadata.
                    // This calls handle_modal_response from async loop, which then sends the message.
                    let modal = AppModal::new("confirm_sms_send", ConfirmationModal::new(format!("Send SMS to {}?", ctx)))
                        .with_metadata(ModalMetadata::SendMessage(ctx.to_owned(), self.sms_text_buffer.clone()));

                    return Some(AppAction::ShowModal(modal));
                }
            },
            KeyCode::Enter => {
                self.sms_text_buffer.push('\n');
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    let pos = self.cursor_position;
                    self.sms_text_buffer.remove(pos - 1);
                    self.move_cursor_left();
                }
            },
            KeyCode::Delete => {
                if self.cursor_position < self.sms_text_buffer.len() {
                    let pos = self.cursor_position;
                    self.sms_text_buffer.remove(pos);
                }
            },
            KeyCode::Left => {
                self.move_cursor_left();
            },
            KeyCode::Right => {
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            KeyCode::Home => {
                self.move_cursor_to_start();
            },
            KeyCode::End => {
                self.move_cursor_to_end(self.sms_text_buffer.len());
            },
            KeyCode::Char(c) => {
                let pos = self.cursor_position;
                self.sms_text_buffer.insert(pos, c);
                self.move_cursor_right(self.sms_text_buffer.len());
            },
            _ => {}
        }

        None
    }

    fn render<'ctx>(&mut self, frame: &mut Frame, theme: &Theme, ctx: Self::Context<'ctx>) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(format!(" Compose SMS to {} ", ctx))
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_focused_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Min(10),     // Text area
            Constraint::Length(2),   // Character count
            Constraint::Length(2),   // Help text
        ])
            .split(inner);

        // Text area with cursor
        let text_with_cursor = self.render_text_with_cursor(theme);

        let text_area = Paragraph::new(text_with_cursor)
            .style(theme.input_style())
            .block(
                Block::bordered()
                    .border_style(theme.border_focused_style())
                    .border_type(BorderType::Rounded)
            )
            .wrap(Wrap { trim: false })
            .scroll((0, 0));

        frame.render_widget(text_area, layout[0]);

        // Character counter
        let char_count = self.sms_text_buffer.chars().count();
        let (counter_style, counter_text) = if char_count <= 160 {
            (theme.accent_style().bg(theme.bg), format!("{}/160 (1 SMS)", char_count))
        } else if char_count <= 320 {
            (Style::default().fg(tailwind::YELLOW.c400).bg(theme.bg), format!("{}/320 (2 SMS parts)", char_count))
        } else {
            (theme.error_style().bg(theme.bg), format!("{} ({} SMS parts)", char_count, get_sms_count(char_count)))
        };

        let char_counter = Paragraph::new(counter_text)
            .style(counter_style)
            .alignment(Alignment::Right);
        frame.render_widget(char_counter, layout[1]);

        // Help text
        let help = Paragraph::new("(Enter) new line | (Ctrl+Space) send | (Esc) cancel")
            .style(theme.secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[2]);
    }
}
impl ModalResponderComponent for ComposeView {

    fn handle_modal_response(&mut self, response: ModalResponse, metadata: ModalMetadata) -> Option<AppAction> {
        match response {
            ModalResponse::Confirmed(true) => { },
            _ => return None
        };

        // Ensure it's a SendMessage metadata
        let (phone, content) = match metadata {
            ModalMetadata::SendMessage(phone, content) => (phone, content),
            _ => return None
        };

        let http = self.context.0.clone();
        let sender = self.context.1.clone();

        tokio::spawn(async move {
            let length = content.len();
            let message = HttpOutgoingSmsMessage::simple_message(phone.clone(), content)
                .with_timeout((BASE_SEND_TIMEOUT * get_sms_count(length)) as u32);

            // Send the SMS message
            let notification = match http.send_sms(&message).await {
                Ok(response) => {
                    // Push message to views to ensure its synced even if WebSocket is disabled
                    let stored_message = SmsStoredMessage::from((message, response));
                    let _ = sender.send(AppAction::HandleIncomingMessage(stored_message));

                    NotificationType::GenericMessage {
                        color: Color::Green,
                        icon: "✔️".to_string(),
                        title: "Message Sent".to_string(),
                        message: format!("Message #{} was sent (ref {})!", response.message_id, response.reference_id),
                    }
                },
                Err(e) => {
                    NotificationType::GenericMessage {
                        color: Color::Red,
                        icon: "❌".to_string(),
                        title: "Send Failure".to_string(),
                        message: e.to_string()
                    }
                }
            };

            let _ = sender.send(AppAction::ShowNotification(notification));
            let _ = sender.send(AppAction::SetViewState {
                state: ViewState::view_messages(&phone),
                // Ensure the loading modal is dismissed on this state change.
                dismiss_modal: true
            });
        });

        let modal = AppModal::new("sms_sending", LoadingModal::new("Sending message..."));
        Some(AppAction::ShowModal(modal))
    }
}