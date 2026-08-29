use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use gtk4::prelude::*;
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use rest_time_linux::audio::AudioEngine;
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{BreakKind, Event, State, UiEffect};
use rest_time_linux::idle::dbus_detector::{ActivitySignal, IdleDetector};
use rest_time_linux::idle::sleep_monitor::{SleepMonitor, SleepSignal};
use rest_time_linux::notifications::NotificationEngine;
use rest_time_linux::ui::overlay::BreakOverlayManager;
use rest_time_linux::ui::tray::RestTimeTray;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Log Pipeline
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Initializing rest-time-linux enterprise ergonomic daemon");

    // 2. Load Configuration File
    let config = Config::load_or_create()?;

    // 3. Initialize GTK4 Core Context
    gtk4::init()?;
    let app = gtk4::Application::builder()
        .application_id("com.github.rest_time_linux")
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let overlay_manager = BreakOverlayManager::new(&app, config.clone());

    // 4. Initialize Core Async Channels
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(64);
    let (activity_tx, mut activity_rx) = mpsc::channel::<ActivitySignal>(16);
    let (sleep_tx, mut sleep_rx) = mpsc::channel::<SleepSignal>(16);
    let (ui_effect_tx, mut ui_effect_rx) = mpsc::channel::<UiEffect>(32);

    let is_snoozed = Arc::new(AtomicBool::new(false));

    // 5. Spawn Idle Discovery Listener
    let idle_detector = IdleDetector::new(
        Duration::from_secs(config.intervals.idle_threshold_seconds as u64),
        activity_tx,
    );
    idle_detector.start().await;

    // 6. Spawn Sleep/Wake Monitor
    SleepMonitor::spawn(sleep_tx).await;

    // 7. Mount FreeDesktop StatusNotifier Tray
    let tray_service = ksni::TrayService::new(RestTimeTray {
        display_text: format!("{}m", config.intervals.work_duration_mins),
        tooltip_text: "Initializing...".into(),
        is_snoozed: is_snoozed.clone(),
        tx: event_tx.clone(),
    });
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    // 8. Spawn High-Precision 1Hz Clock Loop
    let ticker_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let _ = ticker_tx.send(Event::Tick(Duration::from_secs(1))).await;
        }
    });

    // 9. Dispatch Activity Signals into Engine Events
    let bridge_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Some(signal) = activity_rx.recv().await {
            match signal {
                ActivitySignal::IdleThresholdPassed => {
                    let _ = bridge_tx.send(Event::IdleThresholdTriggered).await;
                }
                ActivitySignal::UserActivityResumed => {
                    let _ = bridge_tx.send(Event::ActivityDetected).await;
                }
            }
        }
    });

    // 10. Dispatch Sleep Signals into Engine Events
    let sleep_bridge_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Some(signal) = sleep_rx.recv().await {
            match signal {
                SleepSignal::GoingToSleep => {
                    let _ = sleep_bridge_tx.send(Event::SystemSuspend).await;
                }
                SleepSignal::WakingUp => {
                    let _ = sleep_bridge_tx.send(Event::SystemResume).await;
                }
            }
        }
    });

    // 11. Core Event Processing Loop in Tokio
    let mut fsm = FsmEngine::new(config.clone());
    let audio_cfg = config.audio.clone();
    let is_snoozed_clone = is_snoozed.clone();
    let final_warn_secs = config.notifications.final_warning_seconds;
    let ui_tx = ui_effect_tx.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let effect = fsm.transition(event);

            // Sync Tray UI Status
            match &fsm.state {
                State::Working { elapsed, total } => {
                    is_snoozed_clone.store(false, Ordering::Relaxed);
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let formatted = format!("{:02}:{:02}", remaining / 60, remaining % 60);
                    tray_handle.update(|tray: &mut RestTimeTray| {
                        tray.display_text = formatted.clone();
                        tray.tooltip_text = format!("Focus Time: {} remaining", formatted);
                    });
                }
                State::IdleMeasuring { idle_elapsed, .. } => {
                    let secs = idle_elapsed.as_secs();
                    tray_handle.update(|tray: &mut RestTimeTray| {
                        tray.tooltip_text = format!("Informal Break: {}s", secs);
                    });
                }
                State::PausedSnooze { resume_at } => {
                    is_snoozed_clone.store(true, Ordering::Relaxed);
                    let remaining = resume_at.saturating_duration_since(std::time::Instant::now()).as_secs();
                    tray_handle.update(|tray: &mut RestTimeTray| {
                        tray.display_text = "PAUSED".into();
                        tray.tooltip_text = format!("Snoozed: {}m remaining", remaining / 60);
                    });
                }
                State::PausedManual => {
                    is_snoozed_clone.store(true, Ordering::Relaxed);
                    tray_handle.update(|tray: &mut RestTimeTray| {
                        tray.display_text = "PAUSED".into();
                        tray.tooltip_text = "Paused Manually".into();
                    });
                }
                State::InBreak { kind, elapsed, total } => {
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let kind_str = match kind {
                        BreakKind::Micro => "Micro-Pause",
                        BreakKind::Macro => "Rest Break",
                    };
                    tray_handle.update(|tray: &mut RestTimeTray| {
                        tray.display_text = format!("{:02}:{:02}", remaining / 60, remaining % 60);
                        tray.tooltip_text = format!("{}: {} remaining", kind_str, tray.display_text);
                    });
                }
                _ => {}
            }

            // Execute Side Effects
            if let Some(eff) = effect {
                match &eff {
                    UiEffect::NotifyPreBreak { minutes_left, kind } => {
                        NotificationEngine::send_warning(*minutes_left, *kind);
                    }
                    UiEffect::TriggerFinalWarning(_kind) => {
                        NotificationEngine::send_final_warning(final_warn_secs);
                    }
                    UiEffect::MountOverlay { .. } => {
                        if audio_cfg.sound_enabled {
                            AudioEngine::play_break_start(audio_cfg.volume);
                        }
                    }
                    UiEffect::BreakComplete => {
                        if audio_cfg.sound_enabled {
                            AudioEngine::play_break_end(audio_cfg.volume);
                        }
                    }
                    UiEffect::AutoCreditResolved => {
                        info!("Auto-credit informal break resolved.");
                    }
                    _ => {}
                }

                let _ = ui_tx.send(eff).await;
            }
        }
    });

    // 12. Main GLib Loop: Handle Overlay UI effects
    let unlock_tx = event_tx.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(effect) = ui_effect_rx.recv().await {
            match effect {
                UiEffect::MountOverlay { kind, total_duration } => {
                    let tx = unlock_tx.clone();
                    overlay_manager.spawn_overlays(kind, total_duration, move || {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(Event::SkipBreak).await;
                        });
                    });
                }
                UiEffect::UpdateOverlayProgress { remaining_secs } => {
                    overlay_manager.update_countdown(remaining_secs);
                }
                UiEffect::DismissOverlay | UiEffect::BreakComplete => {
                    overlay_manager.dismiss();
                }
                _ => {}
            }
        }
    });

    // 13. Run the Persistent GLib Main Loop
    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();
    Ok(())
}
