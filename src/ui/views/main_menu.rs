use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::AppAction;
use crate::ui::views::ViewStateRequest;
use crate::ui::{centered_rect, ViewBase};

pub struct MenuItem {
    pub label: String,
    pub description: String,
    pub action_fn: Box<dyn Fn() -> AppAction>,
    pub key_hint: String,
}
impl MenuItem {
    pub fn new<F>(label: &str, description: &str, action_fn: F, key_hint: &str) -> Self
    where
        F: Fn() -> AppAction + 'static,
    {
        Self {
            label: label.to_string(),
            description: description.to_string(),
            action_fn: Box::new(action_fn),
            key_hint: key_hint.to_string(),
        }
    }

    pub fn view(label: &str, description: &str, state: ViewStateRequest, key_hint: &str) -> Self {
        Self::new(
            label,
            description,
            move || AppAction::SetViewState {
                state: state.clone(),
                dismiss_modal: false,
            },
            key_hint,
        )
    }

    pub fn execute(&self) -> AppAction {
        (self.action_fn)()
    }
}

pub struct MainMenuView {
    menu_items: Vec<MenuItem>,
    selected_index: usize,
}
impl MainMenuView {
    pub fn new() -> Self {
        let menu_items = vec![
            MenuItem::view(
                "Phonebook",
                "Send and receive messages from contacts",
                ViewStateRequest::Phonebook,
                "P",
            ),
            MenuItem::view(
                "Device Info",
                "View device signal strength, battery level and other info",
                ViewStateRequest::DeviceInfo,
                "D",
            ),
            MenuItem::new("Exit", "Close the terminal", || AppAction::Exit, "Q"),
        ];

        Self {
            menu_items,
            selected_index: 0,
        }
    }

    fn select_next(&mut self) {
        if !self.menu_items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.menu_items.len();
        }
    }

    fn select_previous(&mut self) {
        if !self.menu_items.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.menu_items.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    fn get_selected_action(&self) -> Option<AppAction> {
        self.menu_items
            .get(self.selected_index)
            .map(MenuItem::execute)
    }

    fn handle_key_shortcut(&self, key: char) -> Option<AppAction> {
        self.menu_items
            .iter()
            .find(|item| {
                item.key_hint
                    .to_lowercase()
                    .starts_with(key.to_ascii_lowercase())
            })
            .map(MenuItem::execute)
    }
}
impl ViewBase for MainMenuView {
    type Context<'ctx> = ();

    async fn load(&mut self, _ctx: Self::Context<'_>) -> AppResult<()> {
        self.selected_index = 0;
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent, _ctx: Self::Context<'_>) -> Option<AppAction> {
        match key.code {
            KeyCode::Char('c' | 'C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(AppAction::Exit)
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.get_selected_action(),
            KeyCode::Down => {
                self.select_next();
                None
            }
            KeyCode::Up => {
                self.select_previous();
                None
            }
            KeyCode::Char(c) => {
                // Handle keyboard shortcuts
                self.handle_key_shortcut(c)
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme, _ctx: Self::Context<'_>) {
        let area = centered_rect(60, 50, frame.area());
        frame.render_widget(Clear, area);

        // Main container
        let title = format!(" SMS Terminal v{} ", crate::PKG_VERSION);
        let block = Block::bordered()
            .title(title)
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Main layout
        let layout = Layout::vertical([
            Constraint::Length(2), // Top spacing
            Constraint::Length(1), // Welcome text
            Constraint::Length(2), // Spacing after welcome
            Constraint::Min(0),    // Menu items (flexible)
            Constraint::Length(2), // Spacing before help
            Constraint::Length(1), // Theme controls hint
            Constraint::Length(1), // Help text
            Constraint::Length(1), // Bottom spacing
        ])
        .split(inner);

        // Welcome
        let welcome = Paragraph::new("Select an option to continue:")
            .style(
                Style::default()
                    .fg(theme.text_accent)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        frame.render_widget(welcome, layout[1]);

        let item_height = 4;
        let menu_area = layout[3];
        let total_menu_height = self.menu_items.len() * item_height;

        // Center the menu items vertically within the menu area
        let menu_start_y = if menu_area.height as usize > total_menu_height {
            (menu_area.height as usize - total_menu_height) / 2
        } else {
            0
        };

        // Render each menu item
        for (i, item) in self.menu_items.iter().enumerate() {
            let y_offset = menu_start_y + (i * item_height);
            let item_rect = Rect {
                x: menu_area.x + 4, // + horizontal padding
                y: menu_area.y + u16::try_from(y_offset).unwrap_or(0),
                width: menu_area.width.saturating_sub(8), // - padding
                height: u16::try_from(item_height).unwrap_or(0),
            };

            let is_selected = i == self.selected_index;
            if is_selected {
                let selection_bg = Rect {
                    x: menu_area.x + 2,
                    y: item_rect.y,
                    width: menu_area.width.saturating_sub(4),
                    height: 2, // Only cover the title and description lines
                };
                let bg_block =
                    Block::new().style(Style::default().bg(theme.text_accent).fg(Color::Black));
                frame.render_widget(bg_block, selection_bg);
            }

            // Item layout
            let item_layout = Layout::vertical([
                Constraint::Length(1), // Main label line
                Constraint::Length(1), // Description line
                Constraint::Length(1), // Empty line for spacing
                Constraint::Length(1), // Separator line
            ])
            .split(item_rect);

            let label_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.primary_style.add_modifier(Modifier::BOLD)
            };

            let label_text = format!("  ({})  {}", item.key_hint, item.label);
            let label = Paragraph::new(label_text).style(label_style);
            frame.render_widget(label, item_layout[0]);

            // Description
            let desc_text = format!("       {}", item.description);
            let desc_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::ITALIC)
            };

            let description = Paragraph::new(desc_text).style(desc_style);
            frame.render_widget(description, item_layout[1]);

            // Visual separator between items (except for the last one)
            if i < self.menu_items.len() - 1 {
                let separator_width = (item_rect.width as usize).saturating_sub(8);
                let separator_text = "─".repeat(separator_width);
                let separator = Paragraph::new(separator_text)
                    .style(Style::default().fg(theme.text_muted))
                    .alignment(Alignment::Center);
                frame.render_widget(separator, item_layout[3]);
            }
        }

        // Controls hint
        let help_text = "↑↓ navigate, (Enter) select, (Ctrl+C) to quit";
        let help = Paragraph::new(help_text)
            .style(
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center);
        frame.render_widget(help, layout[5]);

        // Theme hint
        let theme_hint = Paragraph::new("(F10) change theme color, (F11) toggle background fill")
            .style(
                Style::default()
                    .fg(theme.text_muted)
                    .add_modifier(Modifier::ITALIC),
            )
            .alignment(Alignment::Center);
        frame.render_widget(theme_hint, layout[6]);
    }
}
