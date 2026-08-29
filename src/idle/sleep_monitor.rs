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
            match Connection::system().await {
                Ok(conn) => {
                    info!("Bound to System D-Bus logind for Sleep/Suspend tracking");
                    
                    let rule = "type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'";
                    if let Err(e) = conn
                        .call_method(
                            Some("org.freedesktop.DBus"),
                            "/org/freedesktop/DBus",
                            Some("org.freedesktop.DBus"),
                            "AddMatch",
                            &rule,
                        )
                        .await
                    {
                        warn!("Could not attach sleep match rule via DBus: {}", e);
                    }

                    let mut stream = zbus::MessageStream::from(&conn);
                    while let Some(msg_res) = stream.next().await {
                        if let Ok(msg) = msg_res {
                            if let Some(member) = msg.header().member() {
                                if member.as_str() == "PrepareForSleep" {
                                    if let Ok(active) = msg.body().deserialize::<bool>() {
                                        if active {
                                            info!("System entering sleep mode");
                                            let _ = tx.send(SleepSignal::GoingToSleep).await;
                                        } else {
                                            info!("System resumed from sleep");
                                            let _ = tx.send(SleepSignal::WakingUp).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("System D-Bus unavailable for logind monitor: {}", e);
                }
            }
        });
    }
}
