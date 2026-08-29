use notify_rust::{Notification, Timeout};
use crate::engine::types::BreakKind;
use tracing::info;

pub struct NotificationEngine;

impl NotificationEngine {
    pub fn send_warning(mins_remaining: u32, kind: BreakKind) {
        let break_str = match kind {
            BreakKind::Micro => "micro-pause",
            BreakKind::Macro => "rest break",
        };

        info!("Sending pre-break warning: {} mins for {:?}", mins_remaining, kind);
        let _ = Notification::new()
            .appname("Rest Time")
            .summary("Rest Time")
            .body(&format!("Upcoming {}: Starting in {} minutes.", break_str, mins_remaining))
            .icon("appointment-soon")
            .timeout(Timeout::Milliseconds(4000))
            .show();
    }

    pub fn send_final_warning(secs_remaining: u32) {
        info!("Sending final break warning: {} secs", secs_remaining);
        let _ = Notification::new()
            .appname("Rest Time")
            .summary("Rest Time")
            .body(&format!("Break begins in {} seconds. Finish your current task.", secs_remaining))
            .icon("appointment-soon")
            .timeout(Timeout::Milliseconds(3000))
            .show();
    }
}
