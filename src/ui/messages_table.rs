use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, HighlightSpacing, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Table, TableState,
};
use ratatui::Frame;
use sms_client::http::HttpClient;
use sms_client::http::types::HttpPaginationOptions;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

use crate::error::AppError;
use crate::theme::Theme;
use crate::types::SmsMessage;
use super::centered_rect;

const INFO_TEXT: [&str; 2] = [
    "(↑/↓) navigate | (←/→) columns",
    "(Esc) back | (r) reload | (c) compose SMS"
];

// Pages of 20 items, load next (max-5)
const ITEM_HEIGHT: usize = 4;
const LOAD_THRESHOLD: usize = 5;
const MESSAGES_PER_PAGE: u64 = 20;

pub struct MessagesTableView {
    http_client: Arc<HttpClient>,
    state: TableState,
    messages: Vec<SmsMessage>,
    longest_item_lens: (u16, u16, u16, u16),
    scroll_state: ScrollbarState,
    is_loading: bool,
    has_more: bool,
    current_offset: u64,
    total_messages: usize,
    error_message: Option<String>,
    last_loaded_phone: Option<String>,
}
impl MessagesTableView {
    pub fn new(http_client: Arc<HttpClient>) -> Self {
        Self {
            http_client,
            state: TableState::default(),
            messages: Vec::new(),
            longest_item_lens: (10, 10, 20, 50),
            scroll_state: ScrollbarState::new(0),
            is_loading: false,
            has_more: true,
            current_offset: 0,
            total_messages: 0,
            error_message: None,
            last_loaded_phone: None,
        }
    }

    /// Check if we need to load initial messages for a phone number
    pub fn should_load_initial(&self, phone_number: &str) -> bool {
        // Load if:
        // 1. We haven't loaded anything yet for this number
        // 2. The phone number has changed from what we last loaded
        let needs_load = self.last_loaded_phone.as_ref() != Some(&phone_number.to_string());
        needs_load && !self.is_loading
    }

    /// Load the next set of messages for the given phone number
    pub async fn load_messages(&mut self, phone_number: &str) -> Result<(), AppError> {
        if phone_number.is_empty() {
            return Err(AppError::NoPhoneNumber);
        }

        // If phone number changed, reset everything
        if self.last_loaded_phone.as_ref() != Some(&phone_number.to_string()) {
            self.reset_for_new_phone();
            self.last_loaded_phone = Some(phone_number.to_string());
        }

        // Prevent multiple simultaneous loads
        if self.is_loading {
            return Ok(());
        }

        // HTTP pagination
        let pagination = HttpPaginationOptions::default()
            .with_limit(MESSAGES_PER_PAGE)
            .with_offset(self.current_offset);

        self.is_loading = true;
        match self.http_client.as_ref().get_messages(phone_number, Some(pagination)).await {
            Ok(messages) => {
                let new_messages: Vec<SmsMessage> = messages.iter().map(SmsMessage::from).collect();

                let count = new_messages.len();
                if count > 0 {
                    if self.current_offset == 0 {
                        // First load, replace messages and select the first item
                        self.messages = new_messages;
                        self.state.select(Some(0));
                    } else {
                        self.messages.extend(new_messages);
                    }

                    // Update pagination state
                    self.current_offset += MESSAGES_PER_PAGE;
                    self.total_messages = self.messages.len();
                    self.update_constraints();
                    self.scroll_state = ScrollbarState::new((self.messages.len() - 1) * ITEM_HEIGHT);

                    // If there is less than a full page, it must be the last
                    if count < MESSAGES_PER_PAGE as usize {
                        self.has_more = false;
                    }
                } else {
                    // No messages sent in page? Last page must have been exactly the MESSAGES_PER_PAGE
                    self.has_more = false;
                }

                self.is_loading = false;
                self.error_message = None;
                Ok(())
            }
            Err(e) => {
                self.is_loading = false;
                let error_msg = format!("Failed to load messages: {}", e);
                self.error_message = Some(error_msg);
                Err(AppError::HttpError(e.to_string()))
            }
        }
    }

    pub async fn reload(&mut self, phone_number: &str) -> Result<(), AppError> {
        self.reset_pagination();
        self.load_messages(phone_number).await
    }

    pub fn reset(&mut self) {
        self.reset_pagination();
        self.last_loaded_phone = None;
    }

    fn reset_for_new_phone(&mut self) {
        self.reset_pagination();
        // Don't clear last_loaded_phone here as it's managed by the caller
    }

    fn reset_pagination(&mut self) {
        self.current_offset = 0;
        self.has_more = true;
        self.messages.clear();
        self.error_message = None;
        self.state = TableState::default();
    }

    fn update_constraints(&mut self) {
        let id_len = self.messages
            .iter()
            .map(|m| m.id.width())
            .max()
            .unwrap_or(10)
            .min(20);

        let direction_len = 8;
        let timestamp_len = 16;

        let content_len = self.messages
            .iter()
            .map(|m| m.content.lines().map(|l| l.width()).max().unwrap_or(0))
            .max()
            .unwrap_or(50)
            .min(80);

        // Update the longest text item for each column for
        // table_render to try and keep the values roughly centered.
        self.longest_item_lens = (
            id_len as u16,
            direction_len,
            timestamp_len,
            content_len as u16,
        );
    }

    pub async fn check_load_more(&mut self, phone_number: &str) -> Result<(), AppError> {
        // Don't load if already loading, have no more data, or no messages
        if !self.has_more || self.is_loading || self.messages.is_empty() {
            return Ok(());
        }

        if let Some(selected) = self.state.selected() {
            let load_point = self.messages.len().saturating_sub(LOAD_THRESHOLD);
            if selected >= load_point {
                self.load_messages(phone_number).await?;
            }
        }
        Ok(())
    }

    pub async fn next_row(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let current = self.state.selected().unwrap_or(0);
        let next = (current + 1).min(self.messages.len() - 1);

        if next != current {
            self.state.select(Some(next));
            self.scroll_state = self.scroll_state.position(next * ITEM_HEIGHT);
        }
    }

    pub async fn previous_row(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let current = self.state.selected().unwrap_or(0);
        let previous = current.saturating_sub(1);

        if previous != current {
            self.state.select(Some(previous));
            self.scroll_state = self.scroll_state.position(previous * ITEM_HEIGHT);
        }
    }

    pub fn next_column(&mut self) {
        self.state.select_next_column();
    }

    pub fn previous_column(&mut self) {
        self.state.select_previous_column();
    }

    pub fn add_live_message(&mut self, message: SmsMessage, phone_number: &str) {
        // Only add if this view is for the same phone number
        if self.last_loaded_phone.as_ref() != Some(&phone_number.to_string()) {
            return;
        }

        if self.messages.iter().any(|m| m.id == message.id) {
            return;
        }

        self.messages.insert(0, message);
        self.total_messages = self.messages.len();
        self.update_constraints();
        self.scroll_state = ScrollbarState::new((self.messages.len() - 1) * ITEM_HEIGHT);
    }

    pub fn set_error_message(&mut self, error: Option<String>) {
        self.error_message = error;
    }

    /// TODO: Websocket integration!
    pub fn get_last_message_id(&self) -> Option<String> {
        self.messages.first().map(|m| m.id.clone())
    }

    pub fn render(&mut self, frame: &mut Frame, phone_number: &str, theme: &Theme) {
        let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(5)]);
        let rects = layout.split(frame.area());

        self.render_table(frame, rects[0], theme);
        self.render_scrollbar(frame, rects[0]);
        self.render_footer(frame, rects[1], phone_number, theme);

        if let Some(ref error) = self.error_message {
            self.render_error_popup(frame, error, theme);
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let header_style = Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(theme.row_selected_fg);
        let selected_col_style = Style::default().fg(theme.column_selected_fg);
        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(theme.cell_selected_fg);

        let header = ["ID", "Dir", "Time", "Content"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self.messages.iter().enumerate().map(|(i, msg)| {
            let color = match i % 2 {
                0 => theme.row_normal_bg,
                _ => theme.row_alt_bg,
            };

            let item = msg.ref_array();
            item.into_iter()
                .enumerate()
                .map(|(idx, content)| {
                    let text = if idx == 3 && content.len() > 80 {
                        format!("\n{}\n", textwrap::fill(content, 80))
                    } else {
                        format!("\n{}\n", content)
                    };
                    Cell::from(Text::from(text))
                })
                .collect::<Row>()
                .style(Style::new().fg(theme.text_primary).bg(color))
                .height(4)
        });

        let bar = " █ ";
        let t = Table::new(
            rows,
            [
                Constraint::Length(self.longest_item_lens.0 + 1),
                Constraint::Length(self.longest_item_lens.1 + 1),
                Constraint::Length(self.longest_item_lens.2 + 1),
                Constraint::Min(self.longest_item_lens.3),
            ],
        )
            .header(header)
            .row_highlight_style(selected_row_style)
            .column_highlight_style(selected_col_style)
            .cell_highlight_style(selected_cell_style)
            .highlight_symbol(Text::from(vec![
                Line::from(""),
                Line::from(bar),
                Line::from(bar),
                Line::from(""),
            ]))
            .bg(theme.bg)
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(t, area, &mut self.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, phone_number: &str, theme: &Theme) {
        let mut footer_lines = vec![
            INFO_TEXT[0].to_string(),
            INFO_TEXT[1].to_string()
        ];

        if !self.messages.is_empty() {
            let status = if self.is_loading {
                "⟳ Loading more..."
            } else if self.has_more {
                "More available ↓"
            } else {
                "All loaded ✓"
            };

            footer_lines.push(format!(
                "💬 {} | ✉️ {} messages loaded | {}",
                phone_number,
                self.total_messages,
                status
            ));
        } else if self.is_loading {
            footer_lines.push("⟳ Loading messages...".to_string());
        } else if !phone_number.is_empty() {
            footer_lines.push(format!("💬 {} | No messages found", phone_number));
        }

        let info_footer = Paragraph::new(Text::from(footer_lines.join("\n")))
            .style(theme.primary_style())
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(theme.border_focused_style()),
            );
        frame.render_widget(info_footer, area);
    }

    fn render_error_popup(&self, frame: &mut Frame, error: &str, theme: &Theme) {
        let area = centered_rect(60, 20, frame.area());

        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(" Error ")
            .title_alignment(ratatui::layout::Alignment::Center)
            .border_type(BorderType::Thick)
            .border_style(theme.error_style());

        let error_text = Paragraph::new(error)
            .style(theme.error_style())
            .wrap(ratatui::widgets::Wrap { trim: true })
            .block(block);

        frame.render_widget(error_text, area);
    }
}