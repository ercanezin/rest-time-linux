use std::time::Duration;
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{BreakKind, Event, State, UiEffect};

fn test_config() -> Config {
    let mut cfg = Config::default();
    cfg.intervals.work_duration_mins = 25;
    cfg.intervals.micro_break_seconds = 20;
    cfg.intervals.macro_break_mins = 5;
    cfg.intervals.micro_breaks_before_macro = 2;
    cfg.intervals.idle_threshold_seconds = 180;
    cfg.notifications.enable_progressive_warnings = true;
    cfg.notifications.warning_minutes = vec![10, 5, 3];
    cfg.notifications.final_warning_seconds = 30;
    cfg.behavior.auto_credit_informal_breaks = true;
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
    assert_eq!(fsm.completed_micro_breaks, 0);
}

#[test]
fn test_fsm_progressive_warnings() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Fast-forward to 10 minutes remaining: 25 - 10 = 15 minutes = 900 seconds
    let effect = fsm.transition(Event::Tick(Duration::from_secs(900)));
    assert_eq!(
        effect,
        Some(UiEffect::NotifyPreBreak {
            minutes_left: 10,
            kind: BreakKind::Micro,
        })
    );

    // Next tick should not repeat warning
    let effect2 = fsm.transition(Event::Tick(Duration::from_secs(1)));
    assert_eq!(effect2, None);

    // Fast-forward to 5 minutes remaining: (25 - 5) * 60 = 1200 seconds -> +299 secs
    let effect3 = fsm.transition(Event::Tick(Duration::from_secs(299)));
    assert_eq!(
        effect3,
        Some(UiEffect::NotifyPreBreak {
            minutes_left: 5,
            kind: BreakKind::Micro,
        })
    );
}

#[test]
fn test_fsm_work_to_warning_and_break_transition() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Run full 25 mins
    let effect = fsm.transition(Event::Tick(Duration::from_secs(25 * 60)));
    assert_eq!(effect, Some(UiEffect::TriggerFinalWarning(BreakKind::Micro)));
    assert!(matches!(fsm.state, State::BreakWarning { kind: BreakKind::Micro, seconds_remaining: 30 }));

    // Advance 30 seconds final warning
    let effect2 = fsm.transition(Event::Tick(Duration::from_secs(30)));
    assert_eq!(
        effect2,
        Some(UiEffect::MountOverlay {
            kind: BreakKind::Micro,
            total_duration: Duration::from_secs(20),
        })
    );
    assert!(matches!(fsm.state, State::InBreak { kind: BreakKind::Micro, .. }));

    // In break ticks: progress update
    let effect3 = fsm.transition(Event::Tick(Duration::from_secs(5)));
    assert_eq!(effect3, Some(UiEffect::UpdateOverlayProgress { remaining_secs: 15 }));

    // Finish remaining 15 seconds
    let effect4 = fsm.transition(Event::Tick(Duration::from_secs(15)));
    assert_eq!(effect4, Some(UiEffect::BreakComplete));
    assert_eq!(fsm.completed_micro_breaks, 1);
    assert!(matches!(fsm.state, State::Working { elapsed, .. } if elapsed == Duration::ZERO));
}

#[test]
fn test_fsm_macro_break_cycle() {
    let cfg = test_config(); // micro_breaks_before_macro = 2
    let mut fsm = FsmEngine::new(cfg);

    // Complete 1st micro break
    fsm.transition(Event::TriggerForcedBreak(BreakKind::Micro));
    fsm.transition(Event::CompleteBreak);
    assert_eq!(fsm.completed_micro_breaks, 1);
    assert_eq!(fsm.next_break_kind(), BreakKind::Micro);

    // Complete 2nd micro break
    fsm.transition(Event::TriggerForcedBreak(BreakKind::Micro));
    fsm.transition(Event::CompleteBreak);
    assert_eq!(fsm.completed_micro_breaks, 2);
    assert_eq!(fsm.next_break_kind(), BreakKind::Macro);

    // Complete macro break -> resets counter to 0
    fsm.transition(Event::TriggerForcedBreak(BreakKind::Macro));
    fsm.transition(Event::CompleteBreak);
    assert_eq!(fsm.completed_micro_breaks, 0);
    assert_eq!(fsm.next_break_kind(), BreakKind::Micro);
}

#[test]
fn test_fsm_idle_and_auto_credit() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Work for 10 mins
    fsm.transition(Event::Tick(Duration::from_secs(600)));

    // User goes idle (threshold 180s triggered)
    fsm.transition(Event::IdleThresholdTriggered);
    assert!(matches!(fsm.state, State::IdleMeasuring { .. }));

    // User returns after 10s (before micro break duration 20s target is met)
    fsm.transition(Event::ActivityDetected);
    match fsm.state {
        State::Working { elapsed, .. } => {
            assert_eq!(elapsed, Duration::from_secs(600));
        }
        _ => panic!("Expected State::Working with preserved elapsed time"),
    }

    // User goes idle again
    fsm.transition(Event::IdleThresholdTriggered);
    // Idle time surpasses target break duration (target break is 20s, initial idle is 180s, so >= 20s)
    let effect = fsm.transition(Event::Tick(Duration::from_secs(1)));
    assert_eq!(effect, Some(UiEffect::AutoCreditResolved));
    assert_eq!(fsm.completed_micro_breaks, 1);
    assert!(matches!(fsm.state, State::Working { elapsed, .. } if elapsed == Duration::ZERO));
}

#[test]
fn test_fsm_snooze_and_pause() {
    let cfg = test_config();
    let mut fsm = FsmEngine::new(cfg);

    // Snooze 1 hour
    let effect = fsm.transition(Event::Snooze(Duration::from_secs(3600)));
    assert_eq!(effect, Some(UiEffect::DismissOverlay));
    assert!(matches!(fsm.state, State::PausedSnooze { .. }));

    // Cancel Snooze
    fsm.transition(Event::CancelSnooze);
    assert!(matches!(fsm.state, State::Working { .. }));

    // Toggle Manual Pause
    let effect2 = fsm.transition(Event::ToggleManualPause);
    assert_eq!(effect2, Some(UiEffect::DismissOverlay));
    assert!(matches!(fsm.state, State::PausedManual));

    // Toggle again to resume
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
            assert_eq!(elapsed, total - Duration::from_secs(120));
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

    // Change micro-break to 45s
    fsm.transition(Event::SetMicroBreakDuration(45));
    assert_eq!(fsm.config.intervals.micro_break_seconds, 45);
    assert_eq!(fsm.target_break_duration(BreakKind::Micro), Duration::from_secs(45));

    // Change macro-break to 15m
    fsm.transition(Event::SetMacroBreakDuration(15));
    assert_eq!(fsm.config.intervals.macro_break_mins, 15);
    assert_eq!(fsm.target_break_duration(BreakKind::Macro), Duration::from_secs(15 * 60));
}
