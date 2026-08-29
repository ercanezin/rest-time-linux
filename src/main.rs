use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use rest_time_linux::audio::AudioEngine;
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{Event, State, UiEffect};
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
    let is_in_break = Arc::new(AtomicBool::new(false));

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
        display_text: format!("{:02}:00", config.intervals.work_duration_mins),
        tooltip_text: "Initializing...".into(),
        work_duration_mins: config.intervals.work_duration_mins,
        break_duration_mins: config.intervals.break_duration_mins,
        is_snoozed: is_snoozed.clone(),
        is_in_break: is_in_break.clone(),
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
    let is_in_break_clone = is_in_break.clone();
    let final_warn_secs = config.notifications.final_warning_seconds;
    let ui_tx = ui_effect_tx.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let effect = fsm.transition(event);

            let current_work_mins = fsm.config.intervals.work_duration_mins;
            let current_break_mins = fsm.config.intervals.break_duration_mins;

            // Sync Tray UI Status
            match &fsm.state {
                State::Working { elapsed, total } => {
                    is_snoozed_clone.store(false, Ordering::Relaxed);
                    is_in_break_clone.store(false, Ordering::Relaxed);
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let formatted_rem = format!("{:02}:{:02}", remaining / 60, remaining % 60);

                    tray_handle.update(move |tray: &mut RestTimeTray| {
                        tray.display_text = formatted_rem.clone();
                        tray.tooltip_text = format!("Focus Time: {} remaining", formatted_rem);
                        tray.work_duration_mins = current_work_mins;
                        tray.break_duration_mins = current_break_mins;
                    });
                }
                State::InBreak { elapsed, total } => {
                    is_snoozed_clone.store(false, Ordering::Relaxed);
                    is_in_break_clone.store(true, Ordering::Relaxed);
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let formatted_rem = format!("{:02}:{:02}", remaining / 60, remaining % 60);

                    tray_handle.update(move |tray: &mut RestTimeTray| {
                        tray.display_text = formatted_rem.clone();
                        tray.tooltip_text = format!("Break Time: {} remaining", formatted_rem);
                        tray.work_duration_mins = current_work_mins;
                        tray.break_duration_mins = current_break_mins;
                    });
                }
                State::IdleMeasuring { idle_elapsed, target_break, .. } => {
                    let idle_s = idle_elapsed.as_secs();
                    let target_s = target_break.as_secs();
                    tray_handle.update(move |tray: &mut RestTimeTray| {
                        tray.tooltip_text = format!(
                            "Informal Break: {}s / {}s required to credit",
                            idle_s, target_s
                        );
                        tray.work_duration_mins = current_work_mins;
                        tray.break_duration_mins = current_break_mins;
                    });
                }
                State::PausedSnooze { resume_at } => {
                    is_snoozed_clone.store(true, Ordering::Relaxed);
                    is_in_break_clone.store(false, Ordering::Relaxed);
                    let remaining = resume_at.saturating_duration_since(std::time::Instant::now()).as_secs();
                    let hours = remaining / 3600;
                    let mins = (remaining % 3600) / 60;
                    let secs = remaining % 60;
                    let time_str = if hours > 0 {
                        format!("{}h {}m", hours, mins)
                    } else if mins > 0 {
                        format!("{}m {}s", mins, secs)
                    } else {
                        format!("{}s", secs)
                    };
                    tray_handle.update(move |tray: &mut RestTimeTray| {
                        tray.display_text = "PAUSED".into();
                        tray.tooltip_text = format!("Paused: {} remaining", time_str);
                        tray.work_duration_mins = current_work_mins;
                        tray.break_duration_mins = current_break_mins;
                    });
                }
                State::PausedManual => {
                    is_snoozed_clone.store(true, Ordering::Relaxed);
                    is_in_break_clone.store(false, Ordering::Relaxed);
                    tray_handle.update(move |tray: &mut RestTimeTray| {
                        tray.display_text = "PAUSED".into();
                        tray.tooltip_text = "Paused indefinitely (Click Resume Timer to start)".into();
                        tray.work_duration_mins = current_work_mins;
                        tray.break_duration_mins = current_break_mins;
                    });
                }
                _ => {}
            }

            // Execute Side Effects
            if let Some(eff) = effect {
                match &eff {
                    UiEffect::NotifyPreBreak { minutes_left } => {
                        NotificationEngine::send_warning(*minutes_left);
                    }
                    UiEffect::TriggerFinalWarning => {
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
                UiEffect::MountOverlay { total_duration } => {
                    let tx = unlock_tx.clone();
                    overlay_manager.spawn_overlays(total_duration, move || {
                        let _ = tx.try_send(Event::SkipBreak);
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
