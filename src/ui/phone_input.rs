use std::sync::Arc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph};
use ratatui::Frame;
use sms_client::error::ClientError;
use sms_client::http::HttpClient;
use sms_client::http::types::HttpPaginationOptions;
use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::{AppState, KeyResponse};
use super::centered_rect;

pub struct PhoneInputView {
    http_client: Arc<HttpClient>,
    recent_contacts: Vec<(String, String)>, // (phone, name)
    selected_contact: Option<usize>,
    input_buffer: String,
    max_contacts: usize
}
impl PhoneInputView {
    pub fn with_http(http_client: Arc<HttpClient>) -> Self {

        // TODO: Fetch the latest contacts from client!
        let recent_contacts = vec![];
        Self {
            http_client,
            recent_contacts,
            selected_contact: None,
            input_buffer: String::new(),
            max_contacts: 10
        }
    }

    pub fn push_new_number(&mut self, phone_number: String) {
        if let Some(pos) = self.recent_contacts.iter().position(|(key, _)| *key == phone_number) {
            // If found, move to the front.
            let item = self.recent_contacts.remove(pos);
            self.recent_contacts.insert(0, item);
        } else {
            // If not found, insert at front.
            self.recent_contacts.insert(0, (phone_number.to_string(), "Unknown!".to_string()));

            if self.recent_contacts.len() > self.max_contacts {
                self.recent_contacts.truncate(self.max_contacts);
            }
        }
    }

    pub async fn load(&mut self) -> AppResult<()> {
        let pagination = HttpPaginationOptions::default().with_limit(self.max_contacts as u64);
        self.recent_contacts = self.http_client.get_latest_numbers(Some(pagination))
            .await
            .map_err(|e| ClientError::from(e))?
            .iter()
            .map(|phone_number| (phone_number.clone(), "Unknown".to_string()))
            .collect();

        // Reset selection if OOB
        if let Some(selected) = self.selected_contact {
            if selected >= self.recent_contacts.len() {
                self.selected_contact = None;
            }
        }
        Ok(())
    }

    fn select_next(&mut self) {
        if self.recent_contacts.is_empty() {
            return;
        }

        match self.selected_contact {
            None => self.selected_contact = Some(0),
            Some(i) => {
                self.selected_contact = Some((i + 1) % self.recent_contacts.len());
            }
        }
    }

    fn select_previous(&mut self) {
        if self.recent_contacts.is_empty() {
            return;
        }

        match self.selected_contact {
            None => self.selected_contact = Some(self.recent_contacts.len() - 1),
            Some(0) => self.selected_contact = Some(self.recent_contacts.len() - 1),
            Some(i) => self.selected_contact = Some(i - 1),
        }
    }

    fn clear_selection(&mut self) {
        self.selected_contact = None;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<KeyResponse> {
        match key.code {
            // Make sure control is held so it's not just a letter input into text box.
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(KeyResponse::Quit);
            },
            KeyCode::Enter => {
                let current_phone = self.selected_contact
                    .and_then(|i| self.recent_contacts.get(i))
                    .map(|(phone, _)| phone.clone());

                if let Some(current_phone) = current_phone {
                    self.input_buffer = current_phone;
                }

                if !self.input_buffer.is_empty() {
                    let phone_number = self.input_buffer.clone();
                    self.input_buffer.clear();

                    return Some(KeyResponse::SetAppState(
                        AppState::view_messages(phone_number)
                    ));
                }
            },
            KeyCode::Down => {
                self.select_next();
                self.input_buffer.clear();
            },
            KeyCode::Up => {
                self.select_previous();
                self.input_buffer.clear();
            },
            KeyCode::Backspace => {
                self.input_buffer.pop();
                self.clear_selection();
            },
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.clear_selection();
            },
            _ => {}
        }

        None
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let area = centered_rect(50, 35, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(" Enter Phone Number ")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut constraints = vec![
            Constraint::Length(1),   // Prompt
            Constraint::Length(3),   // Input box
            Constraint::Length(1),   // Help text
        ];
        if !self.recent_contacts.is_empty() {
            constraints.push(Constraint::Length(1)); // Spacing
            constraints.push(Constraint::Length(1)); // Recent contacts header

            // Get height for contacts box
            let contacts_height = std::cmp::min(self.recent_contacts.len(), 8) as u16;
            constraints.push(Constraint::Length(contacts_height));
        }
        let layout = Layout::vertical(constraints).split(inner);

        // Prompt
        let prompt = Paragraph::new("Phone number (international format):")
            .style(theme.secondary_style());
        frame.render_widget(prompt, layout[0]);

        // Input box
        let input_text = if self.input_buffer.is_empty() {
            "+1234567890"
        } else {
            &*self.input_buffer
        };

        // If there is no text, mute the text box.
        let input_style = if self.input_buffer.is_empty() {
            Style::default().fg(theme.text_muted)
        } else {
            theme.input_style()
        };

        let input = Paragraph::new(input_text)
            .style(input_style)
            .block(
                Block::bordered()
                    .border_style(if self.input_buffer.is_empty() {
                        theme.border_style()
                    } else {
                        theme.border_focused_style()
                    })
            );
        frame.render_widget(input, layout[1]);

        // Controls help
        let help_text = if self.recent_contacts.is_empty() {
            "(Enter) confirm, (Ctrl+C) quit"
        } else {
            "(Enter) confirm, (Ctrl+C) quit, ↑↓ select contact"
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[2]);

        // Recent contacts section, if there are some
        if !self.recent_contacts.is_empty() {
            let header = Paragraph::new("Recent Contacts:")
                .style(theme.secondary_style());
            frame.render_widget(header, layout[4]);

            let items: Vec<ListItem> = self.recent_contacts
                .iter()
                .enumerate()
                .take(8) // Limit to max 8 items
                .map(|(i, (phone, name))| {
                    let content = format!("{}  {}", phone, name);
                    let style = if Some(i) == self.selected_contact {
                        Style::default().bg(theme.text_accent).fg(Color::Black)
                    } else {
                        Style::default().fg(theme.text_muted)
                    };
                    ListItem::new(content).style(style)
                })
                .collect();

            let list = List::new(items);
            frame.render_widget(list, layout[5]);
        }
    }
}