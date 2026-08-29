use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
use zbus::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySignal {
    IdleThresholdPassed,
    UserActivityResumed,
}

pub struct IdleDetector {
    threshold: Duration,
    tx: mpsc::Sender<ActivitySignal>,
}

impl IdleDetector {
    pub fn new(threshold: Duration, tx: mpsc::Sender<ActivitySignal>) -> Self {
        Self { threshold, tx }
    }

    pub async fn start(self) {
        tokio::spawn(async move {
            match Connection::session().await {
                Ok(conn) => {
                    info!("Successfully bound to Session D-Bus for activity tracking");
                    Self::poll_loop(conn, self.threshold, self.tx).await;
                }
                Err(e) => {
                    error!("Failed to establish D-Bus session: {}. Activity detector unavailable.", e);
                }
            }
        });
    }

    async fn poll_loop(conn: Connection, threshold: Duration, tx: mpsc::Sender<ActivitySignal>) {
        let mut is_idle = false;
        let mut ticker = tokio::time::interval(Duration::from_secs(1));

        loop {
            ticker.tick().await;

            let idle_time = Self::query_session_idle(&conn).await;
            if let Some(idle_ms) = idle_time {
                let duration = Duration::from_millis(idle_ms);
                if duration >= threshold && !is_idle {
                    is_idle = true;
                    let _ = tx.send(ActivitySignal::IdleThresholdPassed).await;
                } else if duration < threshold && is_idle {
                    is_idle = false;
                    let _ = tx.send(ActivitySignal::UserActivityResumed).await;
                }
            }
        }
    }

    async fn query_session_idle(conn: &Connection) -> Option<u64> {
        // Strategy A: Standard FreeDesktop ScreenSaver
        let s_reply: std::result::Result<u32, _> = conn
            .call_method(
                Some("org.freedesktop.ScreenSaver"),
                "/org/freedesktop/ScreenSaver",
                Some("org.freedesktop.ScreenSaver"),
                "GetSessionIdleTime",
                &(),
            )
            .await
            .and_then(|r| r.body().deserialize());

        if let Ok(ms) = s_reply {
            return Some(ms as u64);
        }

        // Strategy B: GNOME / Mutter Idle Monitor
        let m_reply: std::result::Result<u64, _> = conn
            .call_method(
                Some("org.gnome.Mutter.IdleMonitor"),
                "/org/gnome/Mutter/IdleMonitor/Core",
                Some("org.gnome.Mutter.IdleMonitor"),
                "GetIdletime",
                &(),
            )
            .await
            .and_then(|r| r.body().deserialize());

        if let Ok(ms) = m_reply {
            return Some(ms);
        }

        // Strategy C: KDE ScreenSaver
        let k_reply: std::result::Result<u32, _> = conn
            .call_method(
                Some("org.kde.screensaver"),
                "/ScreenSaver",
                Some("org.freedesktop.ScreenSaver"),
                "GetSessionIdleTime",
                &(),
            )
            .await
            .and_then(|r| r.body().deserialize());

        if let Ok(ms) = k_reply {
            return Some(ms as u64);
        }

        None
    }
}
