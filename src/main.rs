use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use rest_time_linux::audio::AudioEngine;
use rest_time_linux::blocker::BlockerEngine;
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{Event, State, UiEffect};
use rest_time_linux::idle::dbus_detector::{ActivitySignal, IdleDetector};
use rest_time_linux::idle::sleep_monitor::{SleepMonitor, SleepSignal};
use rest_time_linux::notifications::NotificationEngine;
use rest_time_linux::ui::overlay::BreakOverlayManager;
use rest_time_linux::ui::zbus_tray::NativeTrayServer;

struct SingleInstanceLock {
    _file: std::fs::File,
}

impl SingleInstanceLock {
    fn acquire() -> Option<Self> {
        let lock_path = std::env::var("XDG_RUNTIME_DIR")
            .map(|r| std::path::PathBuf::from(r).join("rest-time-linux.lock"))
            .unwrap_or_else(|_| std::env::temp_dir().join("rest-time-linux.lock"));

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .ok()?;

        let fd = file.as_raw_fd();
        let res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if res == 0 {
            Some(Self { _file: file })
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Enforce strict single-instance execution
    let _instance_lock = match SingleInstanceLock::acquire() {
        Some(lock) => lock,
        None => {
            eprintln!("rest-time-linux is already running in your active session. Exiting second instance.");
            return Ok(());
        }
    };

    // Force rock-solid Cairo rendering backend to avoid hybrid GPU driver crashes (Nvidia/AMD)
    std::env::set_var("GSK_RENDERER", "cairo");
    std::env::set_var("GDK_BACKEND", "wayland,x11");

    // 1. Initialize Log Pipeline
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Initializing rest-time-linux enterprise ergonomic daemon");

    // 2. Load Configuration File
    let config = Config::load_or_create()?;

    // 3. Initialize Focus Website Blocker Engine
    let blocker_engine = Arc::new(BlockerEngine::new(config.clone()));
    blocker_engine.clone().spawn_server();

    // 4. Initialize GTK4 Core Context
    gtk4::init()?;
    let app = gtk4::Application::builder()
        .application_id("com.github.rest_time_linux")
        .flags(gtk4::gio::ApplicationFlags::FLAGS_NONE)
        .build();

    let overlay_manager = BreakOverlayManager::new(&app, config.clone());

    // 5. Initialize Core Async Channels
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(64);
    let (activity_tx, mut activity_rx) = mpsc::channel::<ActivitySignal>(16);
    let (sleep_tx, mut sleep_rx) = mpsc::channel::<SleepSignal>(16);
    let (ui_tx, ui_rx) = async_channel::unbounded::<UiEffect>();

    // 6. Spawn Idle Discovery Listener
    let idle_detector = IdleDetector::new(
        Duration::from_secs(config.intervals.idle_threshold_seconds as u64),
        activity_tx,
    );
    idle_detector.start().await;

    // 7. Spawn Sleep/Wake Monitor
    SleepMonitor::spawn(sleep_tx).await;

    // 8. Mount Native FreeDesktop + GNOME StatusNotifier Tray with XAyatanaLabel & Blocker Submenu
    let tray_handle = NativeTrayServer::spawn(
        config.intervals.work_duration_mins,
        config.intervals.break_duration_mins,
        blocker_engine.is_enabled.clone(),
        blocker_engine.available_lists.clone(),
        blocker_engine.active_lists.clone(),
        event_tx.clone(),
    ).await?;

    // 9. Spawn High-Precision 1Hz Clock Loop
    let ticker_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let _ = ticker_tx.send(Event::Tick(Duration::from_secs(1))).await;
        }
    });

    // 10. Dispatch Activity Signals into Engine Events
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

    // 11. Dispatch Sleep Signals into Engine Events
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

    // 12. Core Event Processing Loop in Tokio
    let mut fsm = FsmEngine::new(config.clone());
    let audio_cfg = config.audio.clone();
    let final_warn_secs = config.notifications.final_warning_seconds;
    let tray_updater = tray_handle.clone();
    let blocker_ctrl = blocker_engine.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            // Handle Blocker Events directly
            match &event {
                Event::ToggleBlockerMaster => {
                    let active = blocker_ctrl.toggle_master();
                    info!("Website Blocker master state toggled: {}", active);
                    tray_updater.trigger_menu_layout_update();
                }
                Event::ToggleBlockList(list_name) => {
                    let active = blocker_ctrl.toggle_list(list_name);
                    info!("Blocklist '{}' toggled: {}", list_name, active);
                    tray_updater.trigger_menu_layout_update();
                }
                Event::ReloadBlockerLists => {
                    blocker_ctrl.refresh_lists_and_patterns();
                    tray_updater.trigger_menu_layout_update();
                }
                _ => {}
            }

            let effect = fsm.transition(event);

            let current_break_mins = fsm.config.intervals.break_duration_mins;

            // Sync Tray UI Status & XAyatanaLabel (GNOME Top Bar Text)
            match &fsm.state {
                State::Working { elapsed, total } => {
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let formatted_rem = format!("{:02}:{:02}", remaining / 60, remaining % 60);
                    let active_work_mins = ((total.as_secs() + 59) / 60) as u32;
                    let tooltip = format!("Focus Time: {} remaining", formatted_rem);
                    tray_updater.update(&formatted_rem, &tooltip, active_work_mins, current_break_mins, false, false);
                }
                State::BreakWarning { seconds_remaining } => {
                    let formatted_rem = format!("00:{:02}", seconds_remaining);
                    let tooltip = format!("Break starting in {}s", seconds_remaining);
                    let work_total_mins = fsm.config.intervals.work_duration_mins;
                    tray_updater.update(&formatted_rem, &tooltip, work_total_mins, current_break_mins, false, false);
                }
                State::InBreak { elapsed, total } => {
                    let remaining = total.saturating_sub(*elapsed).as_secs();
                    let formatted_rem = format!("{:02}:{:02}", remaining / 60, remaining % 60);
                    let work_total_mins = fsm.config.intervals.work_duration_mins;
                    let tooltip = format!("Break Time: {} remaining", formatted_rem);
                    tray_updater.update(&formatted_rem, &tooltip, work_total_mins, current_break_mins, false, true);
                }
                State::IdleMeasuring { idle_elapsed, target_break, .. } => {
                    let idle_s = idle_elapsed.as_secs();
                    let target_s = target_break.as_secs();
                    let work_total_mins = fsm.config.intervals.work_duration_mins;
                    let tooltip = format!(
                        "Informal Break: {}s / {}s required to credit",
                        idle_s, target_s
                    );
                    tray_updater.update("IDLE", &tooltip, work_total_mins, current_break_mins, false, false);
                }
                State::PausedSnooze { resume_at } => {
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
                    let work_total_mins = fsm.config.intervals.work_duration_mins;
                    let tooltip = format!("Paused: {} remaining", time_str);
                    tray_updater.update("PAUSED", &tooltip, work_total_mins, current_break_mins, true, false);
                }
                State::PausedManual => {
                    let work_total_mins = fsm.config.intervals.work_duration_mins;
                    let tooltip = "Paused indefinitely (Click Resume Timer to start)".to_string();
                    tray_updater.update("PAUSED", &tooltip, work_total_mins, current_break_mins, true, false);
                }
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

    // 13. Main GLib Loop: Handle Overlay UI effects on the GTK main context
    let overlay_mgr = overlay_manager.clone();
    let unlock_tx = event_tx.clone();

    glib::MainContext::default().spawn_local(async move {
        while let Ok(effect) = ui_rx.recv().await {
            match effect {
                UiEffect::MountOverlay { total_duration } => {
                    let tx = unlock_tx.clone();
                    let tx_postpone = unlock_tx.clone();
                    overlay_mgr.spawn_overlays(
                        total_duration,
                        move || {
                            let _ = tx.try_send(Event::SkipBreak);
                        },
                        move |postpone_secs| {
                            let _ = tx_postpone.try_send(Event::PostponeBreak(Duration::from_secs(postpone_secs)));
                        },
                    );
                }
                UiEffect::UpdateOverlayProgress { remaining_secs } => {
                    overlay_mgr.update_countdown(remaining_secs);
                }
                UiEffect::DismissOverlay | UiEffect::BreakComplete => {
                    overlay_mgr.dismiss();
                }
                _ => {}
            }
        }
    });

    // 14. Run the Persistent GLib Main Loop
    let main_loop = glib::MainLoop::new(None, false);
    main_loop.run();

    // 15. Ensure proxy is disabled on shutdown
    BlockerEngine::disable_proxy_on_exit();
    Ok(())
}
