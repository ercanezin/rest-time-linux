use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakKind {
    Micro,
    Macro,
}

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
        kind: BreakKind,
        seconds_remaining: u32,
    },
    InBreak {
        kind: BreakKind,
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
    TriggerForcedBreak(BreakKind),
    PostponeBreak(Duration),
    Snooze(Duration),
    CancelSnooze,
    SkipBreak,
    CompleteBreak,
    ToggleManualPause,
    SetWorkDuration(u32),
    SetMicroBreakDuration(u32),
    SetMacroBreakDuration(u32),
    SystemSuspend,
    SystemResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEffect {
    NotifyPreBreak { minutes_left: u32, kind: BreakKind },
    TriggerFinalWarning(BreakKind),
    MountOverlay { kind: BreakKind, total_duration: Duration },
    UpdateOverlayProgress { remaining_secs: u32 },
    DismissOverlay,
    BreakComplete,
    AutoCreditResolved,
}
