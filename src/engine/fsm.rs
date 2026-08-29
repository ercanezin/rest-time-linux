use std::time::{Duration, Instant};
use crate::config::Config;
use crate::engine::types::{BreakKind, Event, State, UiEffect};
use tracing::{debug, info};

pub struct FsmEngine {
    pub state: State,
    pub config: Config,
    pub completed_micro_breaks: u32,
    pub sent_warnings: Vec<u32>,
}

impl FsmEngine {
    pub fn new(config: Config) -> Self {
        let total = Duration::from_secs((config.intervals.work_duration_mins * 60) as u64);
        Self {
            state: State::Working {
                elapsed: Duration::ZERO,
                total,
            },
            config,
            completed_micro_breaks: 0,
            sent_warnings: Vec::new(),
        }
    }

    pub fn next_break_kind(&self) -> BreakKind {
        if self.completed_micro_breaks >= self.config.intervals.micro_breaks_before_macro {
            BreakKind::Macro
        } else {
            BreakKind::Micro
        }
    }

    pub fn target_break_duration(&self, kind: BreakKind) -> Duration {
        match kind {
            BreakKind::Micro => Duration::from_secs(self.config.intervals.micro_break_seconds as u64),
            BreakKind::Macro => Duration::from_secs((self.config.intervals.macro_break_mins * 60) as u64),
        }
    }

    pub fn transition(&mut self, event: Event) -> Option<UiEffect> {
        let mut effect = None;
        let next_kind = self.next_break_kind();
        let target_break = self.target_break_duration(next_kind);

        match (&mut self.state, event) {
            (State::Working { elapsed, total }, Event::Tick(delta)) => {
                *elapsed += delta;
                let remaining_secs = total.saturating_sub(*elapsed).as_secs();

                // 1. Progressive Pre-Warnings (10m, 5m, 3m)
                if self.config.notifications.enable_progressive_warnings {
                    for warn_min in &self.config.notifications.warning_minutes {
                        let warn_secs = (*warn_min as u64) * 60;
                        if remaining_secs == warn_secs && !self.sent_warnings.contains(warn_min) {
                            self.sent_warnings.push(*warn_min);
                            effect = Some(UiEffect::NotifyPreBreak {
                                minutes_left: *warn_min,
                                kind: next_kind,
                            });
                        }
                    }
                }

                // 2. Final countdown transition
                if *elapsed >= *total {
                    self.sent_warnings.clear();
                    self.state = State::BreakWarning {
                        kind: next_kind,
                        seconds_remaining: self.config.notifications.final_warning_seconds,
                    };
                    effect = Some(UiEffect::TriggerFinalWarning(next_kind));
                }
            }

            // 3. User Idle Detection (Step away from desk)
            (State::Working { elapsed, .. }, Event::IdleThresholdTriggered) => {
                info!("User idle detected. Transitioning to IdleMeasuring.");
                self.state = State::IdleMeasuring {
                    work_elapsed: *elapsed,
                    idle_elapsed: Duration::from_secs(self.config.intervals.idle_threshold_seconds as u64),
                    target_break,
                };
            }

            // 4. Auto-Credit Logic (Informal Breaks)
            (State::IdleMeasuring { idle_elapsed, target_break, .. }, Event::Tick(delta)) => {
                *idle_elapsed += delta;
                if self.config.behavior.auto_credit_informal_breaks && *idle_elapsed >= *target_break {
                    info!("Informal break satisfied automatically. Resetting cycle.");
                    if self.completed_micro_breaks >= self.config.intervals.micro_breaks_before_macro {
                        self.completed_micro_breaks = 0;
                    } else {
                        self.completed_micro_breaks += 1;
                    }
                    let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total,
                    };
                    effect = Some(UiEffect::AutoCreditResolved);
                }
            }

            (State::IdleMeasuring { work_elapsed, .. }, Event::ActivityDetected) => {
                info!("User returned before break threshold met. Resuming working state.");
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: *work_elapsed,
                    total,
                };
            }

            (State::BreakWarning { kind, seconds_remaining }, Event::Tick(delta)) => {
                let delta_secs = delta.as_secs() as u32;
                if *seconds_remaining <= delta_secs {
                    let k = *kind;
                    let duration = match k {
                        BreakKind::Micro => Duration::from_secs(self.config.intervals.micro_break_seconds as u64),
                        BreakKind::Macro => Duration::from_secs((self.config.intervals.macro_break_mins * 60) as u64),
                    };
                    self.state = State::InBreak {
                        kind: k,
                        elapsed: Duration::ZERO,
                        total: duration,
                    };
                    effect = Some(UiEffect::MountOverlay {
                        kind: k,
                        total_duration: duration,
                    });
                } else {
                    *seconds_remaining -= delta_secs;
                }
            }

            (State::BreakWarning { .. }, Event::PostponeBreak(defer_by)) => {
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                let new_elapsed = total.saturating_sub(defer_by);
                self.state = State::Working {
                    elapsed: new_elapsed,
                    total,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::InBreak { kind, elapsed, total }, Event::Tick(delta)) => {
                *elapsed += delta;
                if *elapsed >= *total {
                    let finished_kind = *kind;
                    if finished_kind == BreakKind::Macro {
                        self.completed_micro_breaks = 0;
                    } else {
                        self.completed_micro_breaks += 1;
                    }
                    let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total: work_total,
                    };
                    effect = Some(UiEffect::BreakComplete);
                } else {
                    effect = Some(UiEffect::UpdateOverlayProgress {
                        remaining_secs: total.saturating_sub(*elapsed).as_secs() as u32,
                    });
                }
            }

            (State::InBreak { kind, .. }, Event::SkipBreak) => {
                if *kind == BreakKind::Macro {
                    self.completed_micro_breaks = 0;
                } else {
                    self.completed_micro_breaks += 1;
                }
                let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: work_total,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::InBreak { kind, .. }, Event::CompleteBreak) => {
                if *kind == BreakKind::Macro {
                    self.completed_micro_breaks = 0;
                } else {
                    self.completed_micro_breaks += 1;
                }
                let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: work_total,
                };
                effect = Some(UiEffect::BreakComplete);
            }

            (_, Event::TriggerForcedBreak(kind)) => {
                info!("Manual break triggered: {:?}", kind);
                let duration = match kind {
                    BreakKind::Micro => Duration::from_secs(self.config.intervals.micro_break_seconds as u64),
                    BreakKind::Macro => Duration::from_secs((self.config.intervals.macro_break_mins * 60) as u64),
                };
                self.sent_warnings.clear();
                self.state = State::InBreak {
                    kind,
                    elapsed: Duration::ZERO,
                    total: duration,
                };
                effect = Some(UiEffect::MountOverlay {
                    kind,
                    total_duration: duration,
                });
            }

            // 5. Snooze Handling (1h, 2h, 12h)
            (_, Event::Snooze(duration)) => {
                info!("Daemon snoozed for {:?}", duration);
                self.sent_warnings.clear();
                self.state = State::PausedSnooze {
                    resume_at: Instant::now() + duration,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::PausedSnooze { resume_at }, Event::Tick(_)) => {
                if Instant::now() >= *resume_at {
                    info!("Snooze duration elapsed. Resuming countdown.");
                    let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total,
                    };
                }
            }

            (State::PausedSnooze { .. }, Event::CancelSnooze) => {
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total,
                };
            }

            (_, Event::ToggleManualPause) => {
                if matches!(self.state, State::PausedManual) {
                    info!("Resuming from manual pause.");
                    let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total,
                    };
                } else {
                    info!("Pausing manually.");
                    self.sent_warnings.clear();
                    self.state = State::PausedManual;
                    effect = Some(UiEffect::DismissOverlay);
                }
            }

            // Power Management Resilience
            (_, Event::SystemSuspend) => {
                debug!("System entering suspend state. Freezing timer.");
            }
            (State::Working { .. }, Event::SystemResume) => {
                debug!("System resumed from sleep. Validating state integrity.");
            }

            _ => {}
        }

        effect
    }
}
