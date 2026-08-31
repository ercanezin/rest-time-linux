use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::config::Config;
use crate::engine::types::{Event, State, UiEffect};

#[derive(Debug, Clone)]
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
        let break_duration = self.target_break_duration();

        match (&mut self.state, event) {
            // 1. Standard Working Progress
            (State::Working { elapsed, total }, Event::Tick(delta)) => {
                *elapsed += delta;

                let remaining_secs = total.saturating_sub(*elapsed).as_secs();

                // Progressive Pre-Break Notifications (e.g. at 10m, 5m, 3m)
                if self.config.notifications.enable_progressive_warnings {
                    for &threshold in &self.config.notifications.warning_minutes {
                        let threshold_secs = (threshold * 60) as u64;
                        if remaining_secs <= threshold_secs && !self.sent_warnings.contains(&threshold) {
                            self.sent_warnings.push(threshold);
                            effect = Some(UiEffect::NotifyPreBreak {
                                minutes_left: threshold,
                            });
                            break;
                        }
                    }
                }

                // Transition to Break Warning or Direct Break
                if *elapsed >= *total {
                    let final_warn = self.config.notifications.final_warning_seconds;
                    if *total > Duration::from_secs(600) && final_warn > 0 {
                        info!("Work session complete. Triggering final warning of {}s.", final_warn);
                        self.sent_warnings.clear();
                        self.state = State::BreakWarning {
                            seconds_remaining: final_warn,
                        };
                        effect = Some(UiEffect::TriggerFinalWarning);
                    } else {
                        info!("Session complete ({:?} of {:?}). Transitioning to break for {:?}", elapsed, total, break_duration);
                        self.sent_warnings.clear();
                        self.state = State::InBreak {
                            elapsed: Duration::ZERO,
                            total: break_duration,
                        };
                        effect = Some(UiEffect::MountOverlay {
                            total_duration: break_duration,
                        });
                    }
                }
            }

            // 1b. Break Warning State
            (State::BreakWarning { seconds_remaining }, Event::Tick(delta)) => {
                let delta_secs = delta.as_secs() as u32;
                *seconds_remaining = seconds_remaining.saturating_sub(delta_secs);

                if *seconds_remaining == 0 {
                    info!("Warning elapsed. Transitioning to break for {:?}", break_duration);
                    self.state = State::InBreak {
                        elapsed: Duration::ZERO,
                        total: break_duration,
                    };
                    effect = Some(UiEffect::MountOverlay {
                        total_duration: break_duration,
                    });
                }
            }

            // 2. Active Break Progress
            (State::InBreak { elapsed, total }, Event::Tick(delta)) => {
                *elapsed += delta;
                let remaining = total.saturating_sub(*elapsed).as_secs() as u32;

                if *elapsed >= *total {
                    info!("Break completed successfully. Resetting work timer.");
                    let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.sent_warnings.clear();
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total: work_total,
                    };
                    effect = Some(UiEffect::BreakComplete);
                } else {
                    effect = Some(UiEffect::UpdateOverlayProgress {
                        remaining_secs: remaining,
                    });
                }
            }

            // 3. User Idle Discovery & Informal Break Auto-Crediting
            (State::Working { elapsed, .. }, Event::IdleThresholdTriggered) => {
                info!("User idle threshold passed. Measuring potential natural break.");
                let current_elapsed = *elapsed;
                let target_break = break_duration;

                self.state = State::IdleMeasuring {
                    work_elapsed: current_elapsed,
                    idle_elapsed: Duration::from_secs(self.config.intervals.idle_threshold_seconds as u64),
                    target_break,
                };
            }

            (State::IdleMeasuring { idle_elapsed, target_break, .. }, Event::Tick(delta)) => {
                *idle_elapsed += delta;
                if *idle_elapsed >= *target_break {
                    info!("User was idle for {:?}. Auto-crediting break and resetting work session.", idle_elapsed);
                    let reset_work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.sent_warnings.clear();
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total: reset_work_total,
                    };
                    effect = Some(UiEffect::AutoCreditResolved);
                }
            }

            (
                State::IdleMeasuring {
                    work_elapsed,
                    idle_elapsed,
                    target_break,
                },
                Event::ActivityDetected,
            ) => {
                if *idle_elapsed >= *target_break {
                    info!("User resumed after {:?}. Auto-crediting break and resetting work session.", idle_elapsed);
                    let reset_work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.sent_warnings.clear();
                    self.state = State::Working {
                        elapsed: Duration::ZERO,
                        total: reset_work_total,
                    };
                    effect = Some(UiEffect::AutoCreditResolved);
                } else {
                    info!("User resumed activity before reaching full break credit ({:?} < {:?}). Restoring work timer.", idle_elapsed, target_break);
                    let work_total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                    self.state = State::Working {
                        elapsed: *work_elapsed,
                        total: work_total,
                    };
                }
            }

            // 4. User Interaction Overrides (Skip / Force Break / Postpone)
            (State::InBreak { .. }, Event::SkipBreak) => {
                info!("User skipped/unlocked break manually.");
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.sent_warnings.clear();
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::InBreak { .. } | State::BreakWarning { .. }, Event::PostponeBreak(duration)) => {
                info!("User postponed break for mini-session of {:?}", duration);
                self.sent_warnings.clear();
                let dur_mins = (duration.as_secs() / 60) as u32;
                for &threshold in &self.config.notifications.warning_minutes {
                    if threshold >= dur_mins {
                        self.sent_warnings.push(threshold);
                    }
                }
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: duration,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (State::InBreak { .. }, Event::CompleteBreak) => {
                info!("Manual break completion acknowledged.");
                let total = Duration::from_secs((self.config.intervals.work_duration_mins * 60) as u64);
                self.sent_warnings.clear();
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total,
                };
                effect = Some(UiEffect::BreakComplete);
            }

            (_, Event::TriggerForcedBreak) => {
                info!("Manual break triggered");
                self.sent_warnings.clear();
                self.state = State::InBreak {
                    elapsed: Duration::ZERO,
                    total: break_duration,
                };
                effect = Some(UiEffect::MountOverlay {
                    total_duration: break_duration,
                });
            }

            // 5. Dynamic Configuration Updates
            (_, Event::SetWorkDuration(mins)) => {
                info!("Updating work session duration to {} mins", mins);
                self.config.intervals.work_duration_mins = mins;
                let _ = self.config.save();
                let new_total = Duration::from_secs((mins * 60) as u64);
                self.sent_warnings.clear();
                self.state = State::Working {
                    elapsed: Duration::ZERO,
                    total: new_total,
                };
                effect = Some(UiEffect::DismissOverlay);
            }

            (_, Event::SetBreakDuration(mins)) => {
                info!("Updating break duration to {} mins", mins);
                self.config.intervals.break_duration_mins = mins;
                let _ = self.config.save();
                let new_break_total = Duration::from_secs((mins * 60) as u64);
                if let State::InBreak { elapsed, total } = &mut self.state {
                    *total = new_break_total;
                    *elapsed = Duration::ZERO;
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
