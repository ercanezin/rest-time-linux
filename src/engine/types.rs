use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Working {
        elapsed: Duration,
        total: Duration,
    },
    IdleMeasuring {
        work_elapsed: Duration,
        idle_elapsed: Duration,
        target_break: Duration,
    },
    BreakWarning {
        seconds_remaining: u32,
    },
    InBreak {
        elapsed: Duration,
        total: Duration,
    },
    PausedSnooze {
        resume_at: Instant,
    },
    PausedManual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Tick(Duration),
    ActivityDetected,
    IdleThresholdTriggered,
    TriggerForcedBreak,
    PostponeBreak(Duration),
    Snooze(Duration),
    CancelSnooze,
    SkipBreak,
    CompleteBreak,
    ToggleManualPause,
    SetWorkDuration(u32),
    SetBreakDuration(u32),
    SystemSuspend,
    SystemResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEffect {
    NotifyPreBreak { minutes_left: u32 },
    TriggerFinalWarning,
    MountOverlay { total_duration: Duration },
    UpdateOverlayProgress { remaining_secs: u32 },
    DismissOverlay,
    BreakComplete,
    AutoCreditResolved,
}
