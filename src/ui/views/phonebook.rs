use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use sms_client::error::ClientError;
use sms_client::http::types::{HttpPaginationOptions, LatestNumberFriendlyNamePair};

use crate::app::AppContext;
use crate::error::AppResult;
use crate::modals::{AppModal, ModalMetadata, ModalResponse};
use crate::theme::Theme;
use crate::types::{AppAction};
use crate::ui::{centered_rect, ModalResponderComponent, ViewBase};
use crate::ui::modals::text_input::TextInputModal;
use crate::ui::notifications::NotificationType;
use crate::ui::views::ViewStateRequest;

pub struct PhonebookView {
    context: AppContext,
    recent_contacts: Vec<LatestNumberFriendlyNamePair>, // (phone, friendly name)
    selected_contact: Option<usize>,
    input_buffer: String,
    max_contacts: usize
}
impl PhonebookView {
    pub fn with_context(context: AppContext) -> Self {
        let recent_contacts = vec![];
        Self {
            context,
            recent_contacts,
            selected_contact: None,
            input_buffer: String::new(),
            max_contacts: 14
        }
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

    fn get_max_phone_length(&self) -> usize {
        self.recent_contacts
            .iter()
            .map(|(phone, _)| phone.len())
            .max()
            .unwrap_or(0)
    }
}
impl ViewBase for PhonebookView {
    type Context<'ctx> = ();

    async fn load<'ctx>(&mut self, _ctx: Self::Context<'ctx>) -> AppResult<()> {
        if !self.recent_contacts.is_empty() {
            return Ok(());
        }

        // Request first page of latest contacts.
        let pagination = HttpPaginationOptions::default().with_limit(self.max_contacts as u64);
        self.recent_contacts = self.context.0.get_latest_numbers(Some(pagination))
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

    async fn handle_key<'ctx>(&mut self, key: KeyEvent, _ctx: Self::Context<'ctx>) -> Option<AppAction> {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(AppAction::Exit);
            },
            KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let selected = self.selected_contact?;
                let (phone, name) = self.recent_contacts.get(selected)?;

                let mut ui = TextInputModal::new("Edit Friendly Name", format!("Name for {}", phone))
                    .with_max_length(50);

                if let Some(existing) = name {
                    ui = ui.with_initial_value(existing);
                }

                // Include selected phone number in modal metadata for the response!
                let modal = AppModal::new("edit_friendly_name", ui)
                    .with_metadata(ModalMetadata::PhoneNumber(phone.clone()));

                return Some(AppAction::ShowModal(modal));
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

                    return Some(AppAction::SetViewState {
                        state: ViewStateRequest::view_messages(&*phone_number),
                        dismiss_modal: false
                    });
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

    fn render<'ctx>(&mut self, frame: &mut Frame, theme: &Theme, _ctx: Self::Context<'ctx>) {
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
            "(Enter) confirm, ↑↓ select, (Ctrl+E) edit name, (Ctrl+C) quit"
        } else {
            "(Enter) confirm, ↑↓ select contact, (Ctrl+C) quit"
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

            let max_phone_length = self.get_max_phone_length();
            let items: Vec<ListItem> = self.recent_contacts
                .iter()
                .enumerate()
                .map(|(i, (phone, name))| {
                    let content = if let Some(friendly_name) = name {
                        // Pad the phone number to align the separators
                        format!("{:width$} ｜ {}", phone, friendly_name, width = max_phone_length)
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
impl ModalResponderComponent for PhonebookView {
    fn handle_modal_response(&mut self, response: ModalResponse, metadata: ModalMetadata) -> Option<AppAction> {
        let phone_number = match metadata {
            ModalMetadata::PhoneNumber(phone_number) => phone_number,
            _ => return None
        };
        let friendly_name = match response {
            ModalResponse::TextInput(friendly_name) => friendly_name?,
            _ => return None
        };

        let http_client = self.context.0.clone();
        let cloned_phone = phone_number.to_string();
        let cloned_name = friendly_name.clone();
        let sender = self.context.1.clone();

        tokio::spawn(async move {
            if let Err(_) = http_client.set_friendly_name(&cloned_phone, Some(cloned_name)).await {

                // If the edit failed, show a notification.
                // It's not worth changing to the error state just over a failed friendly name change.
                let notification = NotificationType::GenericMessage {
                    color: Color::Red,
                    icon: "❌".to_string(),
                    title: "Edit Failed".to_string(),
                    message: format!("Failed to change friendly name for {}", cloned_phone),
                };
                let _ = sender.send(AppAction::ShowNotification(notification));
            }
        });

        // Update local cache
        if let Some(contact) = self.recent_contacts.iter_mut()
            .find(|(p, _)| *p == phone_number) {
            contact.1 = Some(friendly_name.to_string());
        }

        None
    }
}