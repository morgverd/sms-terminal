use std::fmt::format;
use std::sync::Arc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use sms_client::error::ClientError;
use sms_client::http::HttpClient;
use sms_client::http::types::{HttpPaginationOptions, LatestNumberFriendlyNamePair};

use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::{AppState, KeyResponse, Modal};
use crate::ui::{centered_rect, View};
use crate::ui::dialog::TextInputDialog;

pub struct PhoneInputView {
    http_client: Arc<HttpClient>,
    recent_contacts: Vec<LatestNumberFriendlyNamePair>, // (phone, friendly name)
    selected_contact: Option<usize>,
    input_buffer: String,
    max_contacts: usize,
    pending_friendly_name_edit: Option<String>, // Phone number being edited
}

impl PhoneInputView {
    pub fn with_http(http_client: Arc<HttpClient>) -> Self {
        let recent_contacts = vec![];
        Self {
            http_client,
            recent_contacts,
            selected_contact: None,
            input_buffer: String::new(),
            max_contacts: 14,
            pending_friendly_name_edit: None,
        }
    }

    pub async fn push_new_number(&mut self, phone_number: String) -> AppResult<()> {
        if let Some(pos) = self.recent_contacts.iter().position(|(key, _)| *key == phone_number) {
            // If found, move to the front
            let item = self.recent_contacts.remove(pos);
            self.recent_contacts.insert(0, item);
        } else {
            // Get any existing friendly name
            let friendly_name = self.http_client.get_friendly_name(&phone_number)
                .await
                .map_err(|e| ClientError::from(e))?;

            // If not found, insert at front
            self.recent_contacts.insert(0, (phone_number, friendly_name));
            if self.recent_contacts.len() > self.max_contacts {
                self.recent_contacts.truncate(self.max_contacts);
            }
        }

        Ok(())
    }

    pub async fn handle_modal_response(&mut self, modal_id: String, value: String) -> Option<KeyResponse> {
        if modal_id == "edit_friendly_name" {
            if let Some(phone_number) = &self.pending_friendly_name_edit {
                // Save the friendly name
                let name_to_save = if value.trim().is_empty() {
                    None
                } else {
                    Some(value.trim().to_string())
                };

                // Save to backend asynchronously
                let http_client = self.http_client.clone();
                let phone = phone_number.clone();
                let name = name_to_save.clone();
                tokio::spawn(async move {
                    let _ = http_client.set_friendly_name(&phone, name.as_deref()).await;
                });

                // Update local cache immediately for better UX
                if let Some(contact) = self.recent_contacts.iter_mut()
                    .find(|(p, _)| p == phone_number) {
                    contact.1 = name_to_save;
                }

                self.pending_friendly_name_edit = None;
            }
        }
        None
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
}
impl View for PhoneInputView {
    type Context = ();

    async fn load(&mut self, _ctx: Self::Context) -> AppResult<()> {
        if !self.recent_contacts.is_empty() {
            return Ok(());
        }

        // Request first page of latest contacts.
        let pagination = HttpPaginationOptions::default().with_limit(self.max_contacts as u64);
        self.recent_contacts = self.http_client.get_latest_numbers(Some(pagination))
            .await
            .map_err(|e| ClientError::from(e))?
            .into_iter()
            .collect();

        // Reset selection if OOB
        if let Some(selected) = self.selected_contact {
            if selected >= self.recent_contacts.len() {
                self.selected_contact = None;
            }
        }
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent, _ctx: Self::Context) -> Option<KeyResponse> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(KeyResponse::Quit);
            },
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(selected) = self.selected_contact {
                    if let Some((phone, existing_friendly_name)) = self.recent_contacts.get(selected) {
                        self.pending_friendly_name_edit = Some(phone.clone());

                        // Create text input dialog with current friendly name
                        let mut dialog = TextInputDialog::new("Edit Friendly Name", format!("Name for {}", phone)).with_max_length(50);
                        if let Some(existing) = existing_friendly_name {
                            dialog = dialog.with_initial_value(existing);
                        }

                        return Some(KeyResponse::ShowModal(
                            Modal::from(("edit_friendly_name", dialog))
                        ));
                    }
                }
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
            KeyCode::Char(c) if !c.is_control() => {
                self.input_buffer.push(c);
                self.clear_selection();
            },
            _ => {}
        }

        None
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme, _ctx: Self::Context) {
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
        } else if self.selected_contact.is_some() {
            "(Enter) confirm, (E) edit name, (Ctrl+C) quit, ↑↓ select"
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
                    let content = if let Some(friendly_name) = name {
                        format!("{} ｜ {}", phone, friendly_name)
                    } else {
                        phone.to_string()
                    };

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