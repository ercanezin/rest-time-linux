use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{info, warn};
use zbus::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepSignal {
    GoingToSleep,
    WakingUp,
}

pub struct SleepMonitor;

impl SleepMonitor {
    pub async fn spawn(tx: mpsc::Sender<SleepSignal>) {
        tokio::spawn(async move {
            if let Ok(conn) = Connection::system().await {
                info!("Monitoring org.freedesktop.login1 PrepareForSleep signals");
                
                let rule = "type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'";
                if conn.call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "AddMatch",
                    &rule,
                ).await.is_ok() {
                    let mut stream = zbus::MessageStream::from(&conn);
                    while let Some(Ok(msg)) = stream.next().await {
                        if let Ok(is_sleep) = msg.body().deserialize::<bool>() {
                            if is_sleep {
                                info!("System entering sleep state");
                                let _ = tx.send(SleepSignal::GoingToSleep).await;
                            } else {
                                info!("System resumed from sleep state");
                                let _ = tx.send(SleepSignal::WakingUp).await;
                            }
                        }
                    }
                }
            } else {
                warn!("Unable to connect to System D-Bus for sleep events");
            }
        });
    }
}
