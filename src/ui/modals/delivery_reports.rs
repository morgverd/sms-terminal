use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::prelude::{Line, Modifier, Span, Style};
use chrono::{DateTime, Local, TimeZone};
use ratatui::widgets::Paragraph;
use sms_client::error::ClientError;
use sms_client::http::types::{HttpPaginationOptions, HttpSmsDeliveryReport};
use sms_client::types::{SmsDeliveryReportStatus, SmsDeliveryReportStatusGroup};

use crate::error::AppError;
use crate::modals::{AppModal, ModalResponse};
use crate::theme::Theme;
use crate::types::{AppAction, SmsMessage, ViewState};
use crate::ui::modals::{ModalComponent, ModalLoadBehaviour, ModalUtils};
use crate::ui::modals::loading::LoadingModal;

/// This is to make sure we can always add a 'Sent' report as the
/// first delivery report for each message. Otherwise,
///
#[derive(Debug, Clone, PartialEq)]
enum ReportEntry {
    Sent { timestamp: Option<DateTime<Local>> },
    Api(HttpSmsDeliveryReport),
}
impl ReportEntry {
    fn timestamp(&self) -> Option<DateTime<Local>> {
        match self {
            ReportEntry::Sent { timestamp } => *timestamp,
            ReportEntry::Api(report) => report.created_at
                .and_then(|ts| Local.timestamp_opt(ts as i64, 0).single()),
        }
    }

    fn status_group(&self) -> SmsDeliveryReportStatusGroup {
        match self {
            ReportEntry::Sent { .. } => SmsDeliveryReportStatusGroup::Sent,
            ReportEntry::Api(report) => SmsDeliveryReportStatus::from(report.status).to_status_group(),
        }
    }

    fn display_text(&self) -> &'static str {
        match self.status_group() {
            SmsDeliveryReportStatusGroup::Sent => "Sent",
            SmsDeliveryReportStatusGroup::Received => "Delivered",
            SmsDeliveryReportStatusGroup::PermanentFailure => "Failed",
            SmsDeliveryReportStatusGroup::TemporaryFailure => "Retry",
        }
    }

    fn icon(&self) -> &'static str {
        match self.status_group() {
            SmsDeliveryReportStatusGroup::Sent => "📤",
            SmsDeliveryReportStatusGroup::Received => "✅",
            SmsDeliveryReportStatusGroup::PermanentFailure => "❌",
            SmsDeliveryReportStatusGroup::TemporaryFailure => "🔄",
        }
    }

    fn style(&self, theme: &Theme) -> Style {
        match self.status_group() {
            SmsDeliveryReportStatusGroup::Sent => Style::default().fg(theme.text_accent),
            SmsDeliveryReportStatusGroup::Received => Style::default()
                .fg(theme.text_accent)
                .add_modifier(Modifier::BOLD),
            SmsDeliveryReportStatusGroup::PermanentFailure => theme.error_style()
                .add_modifier(Modifier::BOLD),
            SmsDeliveryReportStatusGroup::TemporaryFailure => Style::default()
                .fg(theme.text_muted),
        }
    }

    fn to_timeline_entry(&self, theme: &Theme) -> Line<'static> {
        let time_str = match self.timestamp() {
            Some(dt) => dt.format("%H:%M:%S").to_string(),
            None => "--:--:--".to_string(),
        };

        let style = self.style(theme);

        Line::from(vec![
            Span::styled(format!("{} ", self.icon()), style),
            Span::styled(format!("{} ", time_str), theme.secondary_style()),
            Span::styled(self.display_text().to_string(), style),
        ])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryReportsModal {
    message: SmsMessage,
    reports: Option<Vec<ReportEntry>>
}
impl DeliveryReportsModal {
    pub const MAX_REPORTS: usize = 10;

    /// Create uninitialized modal, which will trigger it to load once set active.
    pub fn new(message: SmsMessage) -> Self {
        Self {
            message,
            reports: None,
        }
    }

    /// Create an initialized modal with a set of delivery reports.
    pub fn with_reports(message: SmsMessage, api_reports: Vec<HttpSmsDeliveryReport>) -> Self {
        let mut reports = Vec::new();

        // Add synthetic "sent" report if available
        if let Some(timestamp) = message.parse_message_timestamp() {
            reports.push(ReportEntry::Sent { timestamp: Some(timestamp) });
        }

        // Add API reports
        reports.extend(api_reports.into_iter().map(ReportEntry::Api));

        // Sort by timestamp (newest first), None values last
        reports.sort_by(|a, b| {
            match (a.timestamp(), b.timestamp()) {
                (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        Self {
            message,
            reports: Some(reports),
        }
    }

    fn render_timeline(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        match &self.reports {
            Some(reports) => {
                for report in reports.iter().take(Self::MAX_REPORTS) {
                    lines.push(report.to_timeline_entry(theme));
                }
            }
            None => {
                lines.push(Line::raw("Loading..."));
            }
        }

        // Pad to consistent height
        while lines.len() < Self::MAX_REPORTS {
            lines.push(Line::raw(""));
        }

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
                    Constraint::Length(1),                         // Top padding
                    Constraint::Min(Self::MAX_REPORTS as u16 * 2), // Timeline
                    Constraint::Min(1),                            // Middle padding
                    Constraint::Length(1),                         // Help text
                ]).split(area);

                let timeline_paragraph = Paragraph::new(self.render_timeline(theme))
                    .alignment(Alignment::Left);
                frame.render_widget(timeline_paragraph, sections[1]);

                let help = Paragraph::new("(Esc) close")
                    .style(theme.primary_style())
                    .alignment(Alignment::Center);
                frame.render_widget(help, sections[2]);
            },
            theme,
            50,
            (Self::MAX_REPORTS as u16) + 10,
        );
    }

    fn load(&self) -> ModalLoadBehaviour {
        if self.reports.is_some() {
            return ModalLoadBehaviour::None;
        }

        let message = self.message.clone();
        ModalLoadBehaviour::Function(Box::new(move |ctx| {
            tokio::spawn(async move {

                // Get all delivery reports for target message.
                let pagination = HttpPaginationOptions::default().with_limit(Self::MAX_REPORTS as u64);
                let reports = match ctx.0.get_delivery_reports(message.message_id, Some(pagination)).await {
                    Ok(reports) => reports,
                    Err(e) => {
                        let _ = ctx.1.send(AppAction::SetViewState {
                            state: ViewState::from(AppError::from(ClientError::from(e))),
                            dismiss_modal: true
                        });
                        return;
                    }
                };

                let modal = AppModal::new("delivery_reports", DeliveryReportsModal::with_reports(message, reports));
                let _ = ctx.1.send(AppAction::ShowModal(modal));
            });

            // Show temporary loading modal, and block the current (DeliveryReportsModal)
            // from being set. The loader above will then either change view state or modal,
            // which will dismiss the loading modal.
            let modal = AppModal::new("delivery_reports_loading", LoadingModal::new("Loading delivery reports..."));
            (Some(AppAction::ShowModal(modal)), true)
        }))
    }
}