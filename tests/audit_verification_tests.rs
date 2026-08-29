use std::time::{Duration, Instant};
use rest_time_linux::config::Config;
use rest_time_linux::engine::fsm::FsmEngine;
use rest_time_linux::engine::types::{Event, State, UiEffect};

fn setup_fsm() -> FsmEngine {
    let mut cfg = Config::default();
    cfg.intervals.work_duration_mins = 25;
    cfg.intervals.break_duration_mins = 5;
    cfg.intervals.idle_threshold_seconds = 180;
    cfg.notifications.enable_progressive_warnings = true;
    cfg.notifications.warning_minutes = vec![10, 5, 3];
    cfg.notifications.final_warning_seconds = 30;
    cfg.behavior.auto_credit_informal_breaks = true;
    FsmEngine::new(cfg)
}

#[test]
fn test_audit_1_progressive_warnings_sequence() {
    let mut fsm = setup_fsm();

    assert!(matches!(fsm.state, State::Working { .. }));

    // Tick to 10m remaining mark (15 mins elapsed = 900s)
    let e1 = fsm.transition(Event::Tick(Duration::from_secs(900)));
    assert_eq!(
        e1,
        Some(UiEffect::NotifyPreBreak {
            minutes_left: 10,
        })
    );
    assert_eq!(fsm.sent_warnings, vec![10]);

    // Tick to 5m remaining mark (+5 mins = 300s)
    let e2 = fsm.transition(Event::Tick(Duration::from_secs(300)));
    assert_eq!(
        e2,
        Some(UiEffect::NotifyPreBreak {
            minutes_left: 5,
        })
    );
    assert_eq!(fsm.sent_warnings, vec![10, 5]);

    // Tick to 3m remaining mark (+2 mins = 120s)
    let e3 = fsm.transition(Event::Tick(Duration::from_secs(120)));
    assert_eq!(
        e3,
        Some(UiEffect::NotifyPreBreak {
            minutes_left: 3,
        })
    );
    assert_eq!(fsm.sent_warnings, vec![10, 5, 3]);

    // Final warning
    let e4 = fsm.transition(Event::Tick(Duration::from_secs(180)));
    assert_eq!(e4, Some(UiEffect::TriggerFinalWarning));
    assert!(matches!(fsm.state, State::BreakWarning { seconds_remaining: 30 }));
}

#[test]
fn test_audit_2_auto_credit_informal_breaks() {
    let mut fsm = setup_fsm();

    // 15 mins of work elapsed
    fsm.transition(Event::Tick(Duration::from_secs(900)));

    // User steps away -> Idle threshold (180s)
    fsm.transition(Event::IdleThresholdTriggered);
    assert!(matches!(fsm.state, State::IdleMeasuring { .. }));

    // User is away for 5m break target (300s)
    let eff = fsm.transition(Event::Tick(Duration::from_secs(300)));
    assert_eq!(eff, Some(UiEffect::AutoCreditResolved));
    
    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::ZERO);
            assert_eq!(total, Duration::from_secs(25 * 60));
        }
        _ => panic!("Expected State::Working after auto-credit"),
    }
}

#[test]
fn test_audit_3_snooze_presets_and_expiry() {
    let mut fsm = setup_fsm();

    let e1 = fsm.transition(Event::Snooze(Duration::from_secs(3600)));
    assert_eq!(e1, Some(UiEffect::DismissOverlay));
    match fsm.state {
        State::PausedSnooze { resume_at } => {
            assert!(resume_at > Instant::now());
        }
        _ => panic!("Expected State::PausedSnooze"),
    }

    fsm.transition(Event::CancelSnooze);
    assert!(matches!(fsm.state, State::Working { elapsed, .. } if elapsed == Duration::ZERO));

    fsm.transition(Event::Snooze(Duration::from_secs(43200)));
    match fsm.state {
        State::PausedSnooze { resume_at } => {
            assert!(resume_at > Instant::now() + Duration::from_secs(43000));
        }
        _ => panic!("Expected State::PausedSnooze"),
    }
}

#[test]
fn test_audit_4_suspend_resume_invariants() {
    let mut fsm = setup_fsm();

    fsm.transition(Event::Tick(Duration::from_secs(600)));
    fsm.transition(Event::SystemSuspend);
    fsm.transition(Event::SystemResume);

    match fsm.state {
        State::Working { elapsed, total } => {
            assert_eq!(elapsed, Duration::from_secs(600));
            assert_eq!(total, Duration::from_secs(25 * 60));
        }
        _ => panic!("Expected State::Working"),
    }
}
