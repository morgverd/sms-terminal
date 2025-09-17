use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Table, TableState,
};
use ratatui::Frame;
use sms_client::http::types::HttpPaginationOptions;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthStr;

use crate::app::AppContext;
use crate::error::{AppError, AppResult};
use crate::modals::AppModal;
use crate::theme::Theme;
use crate::types::{AppAction, SmsMessage};
use crate::ui::modals::delivery_reports::DeliveryReportsModal;
use crate::ui::ViewBase;
use crate::ui::views::ViewStateRequest;

// Pages of 20 items, load next (max-5)
const ITEM_HEIGHT: usize = 4;
const LOAD_THRESHOLD: usize = 5;
const MESSAGES_PER_PAGE: u64 = 20;

pub struct MessagesView {
    context: AppContext,
    state: TableState,
    messages: Vec<SmsMessage>,
    longest_item_lens: (u16, u16, u16, u16),
    scroll_state: ScrollbarState,
    is_loading: bool,
    has_more: bool,
    reversed: bool,
    current_offset: u64,
    total_messages: usize,
    is_selected_outgoing: bool
}
impl MessagesView {
    pub fn with_context(context: AppContext) -> Self {
        Self {
            context,
            state: TableState::default(),
            messages: Vec::new(),
            longest_item_lens: (10, 10, 20, 50),
            scroll_state: ScrollbarState::new(0),
            is_loading: false,
            has_more: true,
            reversed: false,
            current_offset: 0,
            total_messages: 0,
            is_selected_outgoing: false
        }
    }

    pub fn add_live_message(&mut self, message: &SmsMessage) {
        if self.messages.iter().any(|m| m.message_id == message.message_id) {
            return;
        }

        self.messages.insert(0, message.clone());
        self.total_messages = self.messages.len();
        self.update_constraints();
        self.scroll_state = ScrollbarState::new((self.messages.len() - 1) * ITEM_HEIGHT);
    }

    fn reset(&mut self) {
        self.current_offset = 0;
        self.has_more = true;
        self.is_selected_outgoing = false;
        self.messages.clear();
        self.state = TableState::default();
    }

    async fn reload(&mut self, phone_number: &str) -> AppResult<()> {
        self.reset();
        self.load_messages(phone_number).await
    }

    async fn load_messages(&mut self, phone_number: &str) -> AppResult<()> {
        if self.is_loading {
            return Ok(());
        }
        let pagination = HttpPaginationOptions::default()
            .with_limit(MESSAGES_PER_PAGE)
            .with_offset(self.current_offset)
            .with_reverse(self.reversed);

        self.is_loading = true;
        let result = self.context.0.as_ref().get_messages(phone_number, Some(pagination)).await;
        self.is_loading = false;

        match result {
            Ok(messages) => {
                let new_messages: Vec<SmsMessage> = messages.iter().map(SmsMessage::from).collect();

                let count = new_messages.len();
                if count > 0 {
                    self.handle_new_messages(new_messages);
                }

                // If there is still a full page, there could be more results
                self.has_more = count == MESSAGES_PER_PAGE as usize;
                Ok(())
            }
            Err(e) => Err(AppError::HttpError(e))
        }
    }

    fn handle_new_messages(&mut self, new_messages: Vec<SmsMessage>) {
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
    }

    fn update_constraints(&mut self) {
        let id_len = self.messages
            .iter()
            .map(|m| m.identifier.width())
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

    async fn check_load_more(&mut self, phone_number: &str) -> AppResult<()> {
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

    fn next_row(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let current = self.state.selected().unwrap_or(0);
        let next = (current + 1).min(self.messages.len() - 1);

        if next != current {
            self.state.select(Some(next));
            self.scroll_state = self.scroll_state.position(next * ITEM_HEIGHT);
            self.update_selection(next);
        }
    }

    fn previous_row(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let current = self.state.selected().unwrap_or(0);
        let previous = current.saturating_sub(1);

        if previous != current {
            self.state.select(Some(previous));
            self.scroll_state = self.scroll_state.position(previous * ITEM_HEIGHT);
            self.update_selection(previous);
        }
    }

    fn update_selection(&mut self, idx: usize) {
        self.is_selected_outgoing = self.messages.get(idx)
            .map(|m| m.is_outgoing)
            .unwrap_or(false);
    }

    fn next_column(&mut self) {
        self.state.select_next_column();
    }

    fn previous_column(&mut self) {
        self.state.select_previous_column();
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
            "(↑/↓) navigate | (←/→) columns | (Ctrl+R) order".to_string(),
            if self.is_selected_outgoing {
                "(Esc) back | (r) reload | (c) compose SMS | (m) delivery reports".to_string()
            } else {
                "(Esc) back | (r) reload | (c) compose SMS".to_string()
            }
        ];

        // Add sort order indicator
        let order_indicator = if self.reversed {
            "↓ Oldest First"
        } else {
            "↑ Newest First"
        };
        if !self.messages.is_empty() {
            let status = if self.is_loading {
                "⟳ Loading more..."
            } else if self.has_more {
                "More available ↓"
            } else {
                "All loaded ✓"
            };

            footer_lines.push(format!(
                "💬 {} | ✉️ {} messages | {} | {}",
                phone_number,
                self.total_messages,
                order_indicator,
                status
            ));
        } else if self.is_loading {
            footer_lines.push("⟳ Loading messages...".to_string());
        } else if !phone_number.is_empty() {
            footer_lines.push(format!("💬 {} | No messages found | {}", phone_number, order_indicator));
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
}
impl ViewBase for MessagesView {
    type Context<'ctx> = (&'ctx String, bool);

    async fn load<'ctx>(&mut self, ctx: Self::Context<'ctx>) -> AppResult<()> {
        self.reversed = ctx.1;
        self.reload(ctx.0).await?;
        self.is_selected_outgoing = self.messages.first().map(|m| m.is_outgoing).unwrap_or(false);
        Ok(())
    }

    async fn handle_key<'ctx>(&mut self, key: KeyEvent, ctx: Self::Context<'ctx>) -> Option<AppAction> {
        let view_state = match key.code {
            KeyCode::Esc => {
                self.reset();
                Some(ViewStateRequest::Phonebook)
            },
            KeyCode::Char('c') | KeyCode::Char('C') => {
                Some(ViewStateRequest::Compose {
                    phone_number: ctx.0.to_string()
                })
            },
            KeyCode::Char('r') | KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset();
                Some(ViewStateRequest::Messages { phone_number: ctx.0.to_string(), reversed: !self.reversed })
            },
            KeyCode::Char('r') | KeyCode::Char('R') => {
                match self.reload(ctx.0).await {
                    Ok(()) => None,
                    Err(e) => Some(ViewStateRequest::from(e))
                }
            },
            KeyCode::Char('m') | KeyCode::Char('M') => {
                let message = self.messages.get(
                    self.state.selected()?
                )?;
                if !message.is_outgoing {
                    return None;
                }

                // Show uninitialized delivery report, which will trigger it's loading.
                let modal = AppModal::new("delivery_reports", DeliveryReportsModal::new(message.clone()));
                return Some(AppAction::ShowModal(modal))
            },
            KeyCode::Down => {
                self.next_row();
                match self.check_load_more(ctx.0).await {
                    Ok(()) => None,
                    Err(e) => Some(ViewStateRequest::from(e))
                }
            },
            KeyCode::Up => {
                self.previous_row();
                None
            },
            KeyCode::Right => {
                self.next_column();
                None
            },
            KeyCode::Left => {
                self.previous_column();
                None
            },
            _ => None
        };

        // If a view state is retuned, make it into a state change.
        view_state.map(|state| AppAction::SetViewState {
            state,
            dismiss_modal: false
        })
    }
    fn render<'ctx>(&mut self, frame: &mut Frame, theme: &Theme, ctx: Self::Context<'ctx>) {
        let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(5)]);
        let rects = layout.split(frame.area());

        self.render_table(frame, rects[0], theme);
        self.render_scrollbar(frame, rects[0]);
        self.render_footer(frame, rects[1], ctx.0, theme);
    }
}