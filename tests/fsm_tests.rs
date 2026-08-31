use std::time::Duration;
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{Event, State, UiEffect};

fn test_config() -> Config {
    let mut cfg = Config::default();
    cfg.intervals.work_duration_mins = 25;
    cfg.intervals.break_duration_mins = 5;
    cfg.intervals.idle_threshold_seconds = 180;
    cfg.notifications.warning_minutes = vec![10, 5, 3];
    cfg.notifications.final_warning_seconds = 30;
    cfg.notifications.enable_progressive_warnings = true;
    cfg
}

#[test]
fn test_fsm_initial_state() {
    let cfg = test_config();
    let fsm = FsmEngine::new(cfg);

    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(25 * 60));
        }
        _ => panic!("Expected State::Working"),
    }
}

#[test]
fn test_fsm_work_to_warning_and_break_transition() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Tick forward 25 mins
    let effect = fsm.transition(Event::Tick(Duration::from_secs(25 * 60)));
    assert_eq!(effect, Some(UiEffect::TriggerFinalWarning));
    assert!(matches!(
        fsm.state,
        State::BreakWarning {
            seconds_remaining: 30
        }
    ));

    // Tick through 30 seconds final warning
    let effect = fsm.transition(Event::Tick(Duration::from_secs(30)));
    assert_eq!(
        effect,
        Some(UiEffect::MountOverlay {
            total_duration: Duration::from_secs(5 * 60)
        })
    );
    assert!(matches!(
        fsm.state,
        State::InBreak {
            elapsed: Duration::ZERO,
            total: _
        }
    ));
}

#[test]
fn test_fsm_progressive_warnings() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // 25m total -> at 15m elapsed, 10m remaining
    let effect = fsm.transition(Event::Tick(Duration::from_secs(15 * 60)));
    assert_eq!(effect, Some(UiEffect::NotifyPreBreak { minutes_left: 10 }));

    // at 20m elapsed, 5m remaining
    let effect = fsm.transition(Event::Tick(Duration::from_secs(5 * 60)));
    assert_eq!(effect, Some(UiEffect::NotifyPreBreak { minutes_left: 5 }));

    // at 22m elapsed, 3m remaining
    let effect = fsm.transition(Event::Tick(Duration::from_secs(2 * 60)));
    assert_eq!(effect, Some(UiEffect::NotifyPreBreak { minutes_left: 3 }));
}

#[test]
fn test_fsm_idle_and_auto_credit() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // 10 mins into work
    fsm.transition(Event::Tick(Duration::from_secs(10 * 60)));

    // User becomes idle
    fsm.transition(Event::IdleThresholdTriggered);
    assert!(matches!(fsm.state, State::IdleMeasuring { .. }));

    // User is idle for 5 mins (>= break_duration_mins)
    let effect = fsm.transition(Event::Tick(Duration::from_secs(5 * 60)));
    assert_eq!(effect, Some(UiEffect::AutoCreditResolved));
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(25 * 60));
        }
        _ => panic!("Expected reset State::Working"),
    }
}

#[test]
fn test_fsm_snooze_and_pause() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Snooze 5 mins
    let effect = fsm.transition(Event::Snooze(Duration::from_secs(5 * 60)));
    assert_eq!(effect, Some(UiEffect::DismissOverlay));
    assert!(matches!(fsm.state, State::PausedSnooze { .. }));

    // Cancel snooze
    fsm.transition(Event::CancelSnooze);
    assert!(matches!(fsm.state, State::Working { .. }));

    // Manual pause toggle
    let effect = fsm.transition(Event::ToggleManualPause);
    assert_eq!(effect, Some(UiEffect::DismissOverlay));
    assert!(matches!(fsm.state, State::PausedManual));

    // Toggle back to active
    fsm.transition(Event::ToggleManualPause);
    assert!(matches!(fsm.state, State::Working { .. }));
}

#[test]
fn test_fsm_postpone_break() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Trigger warning
    fsm.transition(Event::Tick(Duration::from_secs(25 * 60)));
    assert!(matches!(fsm.state, State::BreakWarning { .. }));

    // Postpone by 2 mins (120 secs)
    let effect = fsm.transition(Event::PostponeBreak(Duration::from_secs(120)));
    assert_eq!(effect, Some(UiEffect::DismissOverlay));
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(120));
        }
        _ => panic!("Expected State::Working"),
    }
}

#[test]
fn test_fsm_dynamic_duration_changes() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Initial total is 25m
    match fsm.state {
        State::Working { total, .. } => assert_eq!(total, Duration::from_secs(25 * 60)),
        _ => panic!("Expected State::Working"),
    }

    // Change to 50m session
    fsm.transition(Event::SetWorkDuration(50));
    assert_eq!(fsm.config.intervals.work_duration_mins, 50);
    match fsm.state {
        State::Working { total, .. } => assert_eq!(total, Duration::from_secs(50 * 60)),
        _ => panic!("Expected State::Working"),
    }

    // Change break to 10m
    fsm.transition(Event::SetBreakDuration(10));
    assert_eq!(fsm.config.intervals.break_duration_mins, 10);
    assert_eq!(fsm.target_break_duration(), Duration::from_secs(10 * 60));
}

#[test]
fn test_fsm_postpone_options_1m_5m_10m() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Force break into active overlay
    let effect = fsm.transition(Event::TriggerForcedBreak);
    assert!(matches!(effect, Some(UiEffect::MountOverlay { .. })));
    assert!(matches!(fsm.state, State::InBreak { .. }));

    // Test Postpone +1m (60s)
    let effect_1m = fsm.transition(Event::PostponeBreak(Duration::from_secs(60)));
    assert_eq!(effect_1m, Some(UiEffect::DismissOverlay));
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(60));
        }
        _ => panic!("Expected State::Working"),
    }

    // Force break again and test +5m (300s)
    fsm.transition(Event::TriggerForcedBreak);
    let effect_5m = fsm.transition(Event::PostponeBreak(Duration::from_secs(300)));
    assert_eq!(effect_5m, Some(UiEffect::DismissOverlay));
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(300));
        }
        _ => panic!("Expected State::Working"),
    }

    // Force break again and test +10m (600s)
    fsm.transition(Event::TriggerForcedBreak);
    let effect_10m = fsm.transition(Event::PostponeBreak(Duration::from_secs(600)));
    assert_eq!(effect_10m, Some(UiEffect::DismissOverlay));
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(600));
        }
        _ => panic!("Expected State::Working"),
    }
}
