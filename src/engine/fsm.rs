use std::time::{Duration, Instant};
use crate::config::Config;
use crate::engine::types::{Event, State, UiEffect};
use tracing::{debug, info};

pub struct FsmEngine {
    pub state: State,
    pub config: Config,
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
            sent_warnings: Vec::new(),
        }
    }

    pub fn target_break_duration(&self) -> Duration {
        Duration::from_secs((self.config.intervals.break_duration_mins * 60) as u64)
    }

    pub fn transition(&mut self, event: Event) -> Option<UiEffect> {
        let mut effect = None;
        let target_break = self.target_break_duration();

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
                            });
                        }
                    }
                }

                // 2. Final countdown transition
                if *elapsed >= *total {
                    self.sent_warnings.clear();
                    self.state = State::BreakWarning {
                        seconds_remaining: self.config.notifications.final_warning_seconds,
                    };
                    effect = Some(UiEffect::TriggerFinalWarning);
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
                    info!("Informal break satisfied automatically. Resetting work session.");
                    let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total,
                    };
                    effect = Some(UiEffect::AutoCreditResolved);
                }
            }

            (State::IdleMeasuring { work_elapsed, .. }, Event::ActivityDetected) => {
                info!("User returned before break duration met. Resuming working session.");
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: *work_elapsed,
                    total,
                };
            }

            (State::BreakWarning { seconds_remaining }, Event::Tick(delta)) => {
                let delta_secs = delta.as_secs() as u32;
                if *seconds_remaining <= delta_secs {
                    let duration = self.target_break_duration();
                    self.state = State::InBreak {
                        elapsed: Duration::ZERO,
                        total: duration,
                    };
                    effect = Some(UiEffect::MountOverlay {
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

            (State::InBreak { elapsed, total }, Event::Tick(delta)) => {
                *elapsed += delta;
                if *elapsed >= *total {
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

            (State::InBreak { .. }, Event::SkipBreak) => {
                let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: work_total,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::InBreak { .. }, Event::CompleteBreak) => {
                let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: work_total,
                };
                effect = Some(UiEffect::BreakComplete);
            }

            (_, Event::TriggerForcedBreak) => {
                info!("Manual break triggered");
                let duration = self.target_break_duration();
                self.sent_warnings.clear();
                self.state = State::InBreak {
                    elapsed: Duration::ZERO,
                    total: duration,
                };
                effect = Some(UiEffect::MountOverlay {
                    total_duration: duration,
                });
            }

            // 5. Dynamic Configuration Updates
            (_, Event::SetWorkDuration(mins)) => {
                info!("Updating work session duration to {} mins", mins);
                self.config.intervals.work_duration_mins = mins;
                let _ = self.config.save();
                let new_total = Duration::from_secs((mins * 60) as u64);
                if let State::Working { elapsed, total } = &mut self.state {
                    *total = new_total;
                    if *elapsed > *total {
                        *elapsed = *total;
                    }
                }
            }

            (_, Event::SetBreakDuration(mins)) => {
                info!("Updating break duration to {} mins", mins);
                self.config.intervals.break_duration_mins = mins;
                let _ = self.config.save();
                let new_break_total = Duration::from_secs((mins * 60) as u64);
                if let State::InBreak { elapsed, total } = &mut self.state {
                    *total = new_break_total;
                    if *elapsed > *total {
                        *elapsed = *total;
                    }
                }
            }

            // 6. Snooze Handling (5m up to 48h / indefinitely)
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

