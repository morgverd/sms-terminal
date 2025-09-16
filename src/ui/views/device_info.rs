use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;
use ratatui::style::palette::tailwind;

use sms_client::error::ClientError;
use sms_client::http::types::{HttpSmsDeviceInfoData, HttpModemSignalStrengthResponse, HttpModemBatteryLevelResponse};

use crate::app::AppContext;
use crate::error::AppResult;
use crate::theme::Theme;
use crate::types::{ViewState, AppAction};
use crate::ui::{centered_rect, ViewBase};

pub struct DeviceInfoView {
    context: AppContext,
    device_info: Option<HttpSmsDeviceInfoData>
}
impl DeviceInfoView {
    pub fn with_context(context: AppContext) -> Self {
        Self {
            context,
            device_info: None
        }
    }

    fn get_signal_strength_percentage(&self, signal: &HttpModemSignalStrengthResponse) -> u8 {
        // Convert RSSI (0-31) to percentage
        // RSSI 0 = -113 dBm (worst), RSSI 31 = -51 dBm (best)
        // RSSI 99 = unknown
        if signal.rssi == 99 {
            0 // Unknown signal
        } else if signal.rssi > 31 {
            100 // Cap at 100% for invalid values
        } else {
            ((signal.rssi as f32 / 31.0) * 100.0) as u8
        }
    }

    fn get_signal_quality_text(&self, signal: &HttpModemSignalStrengthResponse) -> (&'static str, Color) {
        if signal.rssi == 99 {
            return ("Unknown", Color::Gray);
        }

        match self.get_signal_strength_percentage(signal) {
            90..=100 => ("Excellent", Color::Green),
            70..=89 => ("Good", tailwind::LIME.c400),
            50..=69 => ("Fair", Color::Yellow),
            25..=49 => ("Poor", tailwind::ORANGE.c400),
            _ => ("Very Poor", Color::Red),
        }
    }

    fn get_battery_status_text(&self, battery: &HttpModemBatteryLevelResponse) -> &'static str {
        match battery.status {
            0 => "Not Charging",
            1 => "Charging",
            2 => "No Battery",
            _ => "Unknown",
        }
    }

    fn render_battery(&self, battery: &HttpModemBatteryLevelResponse, theme: &Theme) -> Vec<Line<'static>> {
        let battery_level = battery.charge.min(100); // Ensure within 0-100 range

        // Battery design with clear separation between outline and fill
        let battery_top =    "┌──────────────┐ ";
        let battery_tip =    "│              │█";
        let battery_body =   "│              │█";
        let battery_body2 =  "│              │█";
        let battery_bottom = "└──────────────┘ ";

        // Calculate fill level (out of 14 characters)
        let filled_chars = ((battery_level as f32 / 100.0) * 14.0) as usize;

        // Create battery outline and fill separately
        let create_battery_line = |_outline: &str| -> Vec<Span<'static>> {
            let mut spans = Vec::new();

            // Left border
            spans.push(Span::styled("│", Style::default().fg(theme.border)));

            // Battery fill
            for i in 0..14 {
                if i < filled_chars {
                    let fill_color = match battery_level {
                        81..=100 => Color::Green,
                        61..=80 => tailwind::LIME.c400,
                        41..=60 => Color::Yellow,
                        21..=40 => tailwind::ORANGE.c400,
                        _ => Color::Red,
                    };
                    spans.push(Span::styled("█", Style::default().fg(fill_color)));
                } else {
                    spans.push(Span::raw(" "));
                }
            }

            // Right border and terminal
            spans.push(Span::styled("│", Style::default().fg(theme.border)));
            spans.push(Span::styled("█", Style::default().fg(theme.border)));

            spans
        };

        let outline_style = Style::default().fg(theme.border);

        // Show charging indicator if charging
        let status_indicator = if battery.status == 1 { "⚡ " } else { "" };

        vec![
            Line::from(vec![Span::styled(battery_top, outline_style)]),
            Line::from(create_battery_line(battery_tip)),
            Line::from(create_battery_line(battery_body)),
            Line::from(create_battery_line(battery_body2)),
            Line::from(vec![Span::styled(battery_bottom, outline_style)]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("{}{}% • {}", status_indicator, battery_level, format!("{:.2}V", battery.voltage)),
                    theme.accent_style().add_modifier(Modifier::BOLD)
                )
            ]),
            Line::from(vec![
                Span::styled(
                    self.get_battery_status_text(battery),
                    Style::default().fg(theme.text_muted)
                )
            ]),
        ]
    }

    fn render_signal_bars(&self, signal: &HttpModemSignalStrengthResponse, theme: &Theme) -> Vec<Line<'static>> {
        let signal_rssi = if signal.rssi == 99 { 0 } else { signal.rssi.min(31) };
        let signal_percentage = self.get_signal_strength_percentage(signal);

        // Convert RSSI to bars (0-5)
        let bars = if signal.rssi == 99 {
            0
        } else if signal_rssi == 0 {
            0
        } else {
            ((signal_rssi as f32 / 31.0) * 5.0).ceil() as usize
        };

        let bar_heights = [1, 2, 3, 4, 5];
        let (quality_text, signal_color) = self.get_signal_quality_text(signal);
        let filled_style = Style::default().fg(signal_color);
        let empty_style = Style::default().fg(theme.text_muted);

        let mut lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(""),
        ];

        // Build signal bars from bottom up
        for row in (0..5).rev() {
            let mut spans = Vec::new();
            spans.push(Span::raw("  ")); // Leading space

            for (bar_idx, &height) in bar_heights.iter().enumerate() {
                let should_fill = bars > bar_idx && row >= (5 - height);
                let style = if should_fill { filled_style } else { empty_style };

                spans.push(Span::styled("███", style));
                spans.push(Span::raw(" "));
            }

            lines[row] = Line::from(spans);
        }

        // Add signal details with clearer formatting
        lines.push(Line::from(""));
        if signal.rssi == 99 {
            lines.push(Line::from(vec![
                Span::styled(
                    "Signal Unknown",
                    Style::default().fg(theme.text_muted).add_modifier(Modifier::ITALIC)
                )
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ({}%)", quality_text, signal_percentage),
                    theme.accent_style().add_modifier(Modifier::BOLD)
                )
            ]));
        }

        lines
    }

    fn render_technical_details(&self, signal: &HttpModemSignalStrengthResponse, theme: &Theme) -> Vec<Line<'static>> {
        // More accurate RSSI to dBm conversion
        // Leading space to avoid needing to reformat in the Span. Looks silly though :(
        let rssi_dbm = if signal.rssi == 99 {
            " Unknown".to_string()
        } else {
            let dbm = -113 + (signal.rssi.min(31) as i16 * 2);
            format!(" {} dBm", dbm)
        };

        let ber_text = if signal.ber == 99 {
            " Unknown".to_string()
        } else if signal.ber <= 7 {
            format!(" {}", signal.ber)
        } else {
            " Invalid".to_string()
        };

        vec![
            Line::from(vec![
                Span::styled("RSSI:", theme.secondary_style()),
                Span::styled(rssi_dbm, theme.accent_style()),
            ]),
            Line::from(vec![
                Span::styled("BER:", theme.secondary_style()),
                Span::styled(ber_text, theme.accent_style()),
            ]),
            Line::from(vec![
                Span::styled("Raw RSSI:", theme.secondary_style()),
                Span::styled(
                    if signal.rssi == 99 { " Unknown".to_string() } else { format!(" {}/31", signal.rssi) },
                    Style::default().fg(theme.text_muted)
                ),
            ]),
        ]
    }
}
impl ViewBase for DeviceInfoView {
    type Context<'ctx> = ();

    async fn load<'ctx>(&mut self, _ctx: Self::Context<'ctx>) -> AppResult<()> {
        self.device_info = Some(self.context.0.get_device_info().await.map_err(|e| ClientError::from(e))?);
        Ok(())
    }

    async fn handle_key<'ctx>(&mut self, key: KeyEvent, _ctx: Self::Context<'ctx>) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc => {
                Some(AppAction::SetViewState {
                    state: ViewState::Phonebook,
                    dismiss_modal: false
                })
            },
            KeyCode::Char('r') | KeyCode::Char('R') => {
                match self.load(()).await {
                    Ok(_) => None,
                    Err(e) => Some(AppAction::SetViewState {
                        state: ViewState::from(e),
                        dismiss_modal: true
                    })
                }
            },
            _ => None
        }
    }

    fn render<'ctx>(&mut self, frame: &mut Frame, theme: &Theme, _ctx: Self::Context<'ctx>) {
        let area = centered_rect(75, 60, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::bordered()
            .title(" 📱 Device Information ")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_focused_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // If we're loading, show nothing.
        // TODO: Maybe actually show something.
        let Some(ref device_info) = self.device_info else {
            return;
        };

        let main_layout = Layout::vertical([
            Constraint::Length(1),   // Top spacing
            Constraint::Length(4),   // Phone number section
            Constraint::Length(1),   // Spacing
            Constraint::Length(10),  // Battery and Signal section
            Constraint::Length(1),   // Spacing
            Constraint::Length(6),   // Operator and technical details
            Constraint::Min(1),      // Flexible spacing
            Constraint::Length(1),   // Help text
        ]).split(inner);

        // Phone number
        if let Some(ref phone_number) = device_info.phone_number {
            let phone_section = Layout::vertical([
                Constraint::Length(1),   // Label
                Constraint::Length(2),   // Number with decoration
                Constraint::Length(1),   // Separator
            ]).split(main_layout[1]);

            let phone_label = Paragraph::new("📞 Phone Number")
                .style(theme.secondary_style().add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);
            frame.render_widget(phone_label, phone_section[0]);

            let phone_display = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("╰─── {} ───╯", phone_number),
                        theme.accent_style().add_modifier(Modifier::BOLD)
                    )
                ])
            ])
                .alignment(Alignment::Center);
            frame.render_widget(phone_display, phone_section[1]);

            let separator = Paragraph::new("━".repeat((phone_section[2].width as usize).min(60)))
                .style(theme.border_style())
                .alignment(Alignment::Center);
            frame.render_widget(separator, phone_section[2]);
        }

        // Battery and Signal
        let metrics_layout = Layout::horizontal([
            Constraint::Percentage(50),  // Battery
            Constraint::Percentage(50),  // Signal
        ]).split(main_layout[3]);

        // Battery
        if let Some(ref battery) = device_info.battery {
            let battery_section = Layout::vertical([
                Constraint::Length(2),   // Label
                Constraint::Length(8),   // Battery visual
            ]).split(metrics_layout[0]);

            let battery_label = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("🔋 Battery Status", theme.secondary_style().add_modifier(Modifier::BOLD))
                ]),
                Line::from("")
            ])
                .alignment(Alignment::Center);
            frame.render_widget(battery_label, battery_section[0]);

            let battery_visual = Paragraph::new(self.render_battery(battery, theme))
                .alignment(Alignment::Center);
            frame.render_widget(battery_visual, battery_section[1]);
        }

        // Signal
        if let Some(ref signal) = device_info.signal {
            let signal_section = Layout::vertical([
                Constraint::Length(2),   // Label
                Constraint::Length(8),   // Signal bars
            ]).split(metrics_layout[1]);

            let signal_label = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("📶 Signal Strength", theme.secondary_style().add_modifier(Modifier::BOLD))
                ]),
                Line::from("")
            ])
                .alignment(Alignment::Center);
            frame.render_widget(signal_label, signal_section[0]);

            let signal_visual = Paragraph::new(self.render_signal_bars(signal, theme))
                .alignment(Alignment::Center);
            frame.render_widget(signal_visual, signal_section[1]);
        }

        // Operator and technical details
        let details_layout = Layout::horizontal([
            Constraint::Percentage(50),  // Operator info
            Constraint::Percentage(50),  // Technical details
        ]).split(main_layout[5]);

        // Operator
        let operator_name = device_info.network_operator.as_ref()
            .map(|op| op.operator.clone())
            .or_else(|| device_info.service_provider.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let operator_info = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("📡 Network Operator", theme.secondary_style().add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(operator_name, theme.accent_style().add_modifier(Modifier::BOLD))
            ]),
        ])
            .alignment(Alignment::Center);
        frame.render_widget(operator_info, details_layout[0]);

        // Technical details
        if let Some(ref signal) = device_info.signal {
            let tech_details = Paragraph::new({
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("⚙️ Technical Details", theme.secondary_style().add_modifier(Modifier::BOLD))
                    ]),
                    Line::from("")
                ];
                lines.extend(self.render_technical_details(signal, theme));
                lines
            })
                .alignment(Alignment::Center);
            frame.render_widget(tech_details, details_layout[1]);
        }

        // Help text
        let help_text = "(r) refresh, (Esc) back";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(help, main_layout[7]);
    }
}