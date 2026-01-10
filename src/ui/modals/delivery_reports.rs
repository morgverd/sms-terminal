use crate::error::AppError;
use crate::modals::{AppModal, ModalResponse};
use crate::theme::Theme;
use crate::types::AppAction;
use crate::ui::modals::loading::LoadingModal;
use crate::ui::modals::{ModalComponent, ModalLoadBehaviour, ModalUtils};
use crate::ui::views::ViewStateRequest;
use chrono::{DateTime, Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use sms_client::error::ClientError;
use sms_client::types::http::HttpPaginationOptions;
use sms_client::types::sms::{SmsDeliveryReport, SmsDeliveryReportStatusCategory, SmsMessage};
use std::time::SystemTime;

/// A delivery report entry with pre-computed fields for efficient rendering.
#[derive(Debug, Clone, PartialEq)]
struct ReportEntry {
    timestamp: Option<DateTime<Local>>,
    status_category: SmsDeliveryReportStatusCategory,
}
impl ReportEntry {
    /// Create a synthetic "Sent" entry.
    fn sent(timestamp: SystemTime) -> Self {
        Self {
            timestamp: Some(timestamp.into()),
            status_category: SmsDeliveryReportStatusCategory::Sent,
        }
    }

    /// Create an entry from an API delivery report.
    fn from_api(report: &SmsDeliveryReport) -> Self {
        Self {
            timestamp: report
                .created_at
                .and_then(|ts| Local.timestamp_opt(i64::from(ts), 0).single()),
            status_category: SmsDeliveryReportStatusCategory::from(report.status),
        }
    }

    fn icon(&self) -> &'static str {
        match self.status_category {
            SmsDeliveryReportStatusCategory::Sent => "📤",
            SmsDeliveryReportStatusCategory::Received => "✅",
            SmsDeliveryReportStatusCategory::Retrying => "🔄",
            SmsDeliveryReportStatusCategory::Failed => "❌",
        }
    }

    fn style(&self, theme: &Theme) -> Style {
        match self.status_category {
            SmsDeliveryReportStatusCategory::Sent => Style::default().fg(theme.text_accent),
            SmsDeliveryReportStatusCategory::Received => Style::default()
                .fg(theme.text_accent)
                .add_modifier(Modifier::BOLD),
            SmsDeliveryReportStatusCategory::Retrying => Style::default().fg(theme.text_muted),
            SmsDeliveryReportStatusCategory::Failed => {
                theme.error_style.add_modifier(Modifier::BOLD)
            }
        }
    }

    fn to_timeline_entry(&self, theme: &Theme) -> Line<'static> {
        let time_str = self
            .timestamp
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_string());

        let style = self.style(theme);

        Line::from(vec![
            Span::styled(format!("{} ", self.icon()), style),
            Span::styled(format!("{time_str} "), theme.secondary_style),
            Span::styled(self.status_category.to_string(), style),
        ])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryReportsModal {
    message: SmsMessage,
    reports: Option<Vec<ReportEntry>>,
}
impl DeliveryReportsModal {
    pub const MAX_REPORTS_USIZE: usize = 10;
    pub const MAX_REPORTS_U16: u16 = 10;

    /// Create uninitialized modal, which will trigger it to load once set active.
    pub fn new(message: SmsMessage) -> Self {
        Self {
            message,
            reports: None,
        }
    }

    /// Create an initialized modal with a set of delivery reports.
    pub fn with_reports(message: SmsMessage, api_reports: Vec<SmsDeliveryReport>) -> Self {
        let mut reports: Vec<ReportEntry> = api_reports.iter().map(ReportEntry::from_api).collect();

        // Add synthetic "sent" report if available
        if let Some(timestamp) = message.created_at() {
            reports.push(ReportEntry::sent(timestamp));
        }

        // Sort by timestamp (newest first), None values last
        reports.sort_by(|a, b| match (a.timestamp, b.timestamp) {
            (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        Self {
            message,
            reports: Some(reports),
        }
    }

    fn render_timeline(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = self
            .reports
            .as_ref()
            .map(|reports| {
                reports
                    .iter()
                    .take(Self::MAX_REPORTS_USIZE)
                    .map(|r| r.to_timeline_entry(theme))
                    .collect()
            })
            .unwrap_or_else(|| vec![Line::raw("Loading...")]);

        // Pad to consistent height
        lines.resize_with(Self::MAX_REPORTS_USIZE, || Line::raw(""));
        lines.push(Line::raw(""));
        lines
    }
}
impl ModalComponent for DeliveryReportsModal {
    fn handle_key(&mut self, key: KeyEvent) -> Option<ModalResponse> {
        match key.code {
            KeyCode::Esc => Some(ModalResponse::Dismissed),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        ModalUtils::render_base(
            frame,
            "Delivery Reports",
            |frame, area, theme| {
                let sections = Layout::vertical([
                    Constraint::Length(1),                      // Top padding
                    Constraint::Min(Self::MAX_REPORTS_U16 * 2), // Timeline
                    Constraint::Min(1),                         // Middle padding
                    Constraint::Length(1),                      // Help text
                ])
                .split(area);

                let timeline_paragraph =
                    Paragraph::new(self.render_timeline(theme)).alignment(Alignment::Left);
                frame.render_widget(timeline_paragraph, sections[1]);

                let help = Paragraph::new("(Esc) close")
                    .style(theme.primary_style)
                    .alignment(Alignment::Center);
                frame.render_widget(help, sections[2]);
            },
            theme,
            50,
            Self::MAX_REPORTS_U16 + 10,
        );
    }

    fn load(&self) -> ModalLoadBehaviour {
        if self.reports.is_some() {
            return ModalLoadBehaviour::None;
        }

        let message = self.message.clone();
        ModalLoadBehaviour::Function(Box::new(move |ctx| {
            tokio::spawn(async move {
                let pagination =
                    HttpPaginationOptions::default().with_limit(Self::MAX_REPORTS_USIZE as u64);
                let reports = match ctx
                    .0
                    .get_delivery_reports(
                        message
                            .message_id
                            .expect("SmsMessage is missing required message_id!"),
                        Some(pagination),
                    )
                    .await
                {
                    Ok(reports) => reports,
                    Err(e) => {
                        let _ = ctx.1.send(AppAction::SetViewState {
                            state: ViewStateRequest::from(AppError::from(ClientError::from(e))),
                            dismiss_modal: true,
                        });
                        return;
                    }
                };

                let modal = AppModal::new(
                    "delivery_reports",
                    DeliveryReportsModal::with_reports(message, reports),
                );
                let _ = ctx.1.send(AppAction::SetModal(Some(modal)));
            });

            let modal = AppModal::new(
                "delivery_reports_loading",
                LoadingModal::new("Loading delivery reports..."),
            );
            (Some(AppAction::SetModal(Some(modal))), true)
        }))
    }
}
