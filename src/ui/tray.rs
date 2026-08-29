use ksni::{Category, MenuItem, Status, ToolTip, Tray};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::engine::types::{BreakKind, Event};

pub struct RestTimeTray {
    pub display_text: String,
    pub tooltip_text: String,
    pub is_snoozed: Arc<AtomicBool>,
    pub tx: mpsc::Sender<Event>,
}

impl Tray for RestTimeTray {
    fn id(&self) -> String {
        "rest-time-linux".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn title(&self) -> String {
        self.display_text.clone()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_name(&self) -> String {
        if self.is_snoozed.load(Ordering::Relaxed) {
            "rest-time-paused".into()
        } else {
            "rest-time-active".into()
        }
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "rest-time-active".into(),
            icon_pixmap: Vec::new(),
            title: "Rest Time".into(),
            description: self.tooltip_text.clone(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let is_paused = self.is_snoozed.load(Ordering::Relaxed);

        // Helper macro/closure to create duration sender
        let make_session_item = |mins: u32, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::SetWorkDuration(mins)).await;
                    });
                }),
                ..Default::default()
            })
        };

        let make_micro_item = |secs: u32, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::SetMicroBreakDuration(secs)).await;
                    });
                }),
                ..Default::default()
            })
        };

        let make_macro_item = |mins: u32, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::SetMacroBreakDuration(mins)).await;
                    });
                }),
                ..Default::default()
            })
        };

        let make_snooze_item = |secs: u64, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::Snooze(Duration::from_secs(secs))).await;
                    });
                }),
                ..Default::default()
            })
        };

        let tx_break = self.tx.clone();
        let tx_toggle = self.tx.clone();

        vec![
            MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("⏳ {}", self.tooltip_text),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(ksni::menu::StandardItem {
                label: "▶ Take Break Now".into(),
                activate: Box::new(move |_| {
                    let tx = tx_break.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::TriggerForcedBreak(BreakKind::Micro)).await;
                    });
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            // Session Length Submenu
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "⏱ Set Session Length".into(),
                submenu: vec![
                    make_session_item(10, "10 Minutes"),
                    make_session_item(15, "15 Minutes"),
                    make_session_item(20, "20 Minutes"),
                    make_session_item(25, "25 Minutes (Standard Pomodoro)"),
                    make_session_item(30, "30 Minutes"),
                    make_session_item(45, "45 Minutes"),
                    make_session_item(50, "50 Minutes (Ultradian Cycle)"),
                    make_session_item(60, "60 Minutes (1 Hour)"),
                    make_session_item(90, "90 Minutes (Deep Focus)"),
                ],
                ..Default::default()
            }),
            // Break Length Submenu
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "☕ Set Break Lengths".into(),
                submenu: vec![
                    MenuItem::SubMenu(ksni::menu::SubMenu {
                        label: "Micro-Pause Duration".into(),
                        submenu: vec![
                            make_micro_item(15, "15 Seconds"),
                            make_micro_item(20, "20 Seconds (20-20-20 Rule)"),
                            make_micro_item(30, "30 Seconds (Default)"),
                            make_micro_item(45, "45 Seconds"),
                            make_micro_item(60, "60 Seconds (1 Minute)"),
                        ],
                        ..Default::default()
                    }),
                    MenuItem::SubMenu(ksni::menu::SubMenu {
                        label: "Macro-Break Duration".into(),
                        submenu: vec![
                            make_macro_item(3, "3 Minutes"),
                            make_macro_item(5, "5 Minutes (Default)"),
                            make_macro_item(10, "10 Minutes"),
                            make_macro_item(15, "15 Minutes"),
                            make_macro_item(20, "20 Minutes"),
                            make_macro_item(30, "30 Minutes"),
                        ],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            // Granular Pause & Snooze Submenu
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "⏸ Pause / Snooze Timer".into(),
                submenu: vec![
                    make_snooze_item(5 * 60, "For 5 Minutes"),
                    make_snooze_item(10 * 60, "For 10 Minutes"),
                    make_snooze_item(15 * 60, "For 15 Minutes"),
                    make_snooze_item(20 * 60, "For 20 Minutes"),
                    make_snooze_item(30 * 60, "For 30 Minutes"),
                    make_snooze_item(45 * 60, "For 45 Minutes"),
                    MenuItem::Separator,
                    make_snooze_item(1 * 3600, "For 1 Hour"),
                    make_snooze_item(2 * 3600, "For 2 Hours"),
                    make_snooze_item(3 * 3600, "For 3 Hours"),
                    make_snooze_item(4 * 3600, "For 4 Hours"),
                    make_snooze_item(6 * 3600, "For 6 Hours"),
                    make_snooze_item(8 * 3600, "For 8 Hours"),
                    make_snooze_item(10 * 3600, "For 10 Hours"),
                    MenuItem::Separator,
                    make_snooze_item(24 * 3600, "For 1 Day (24 Hours)"),
                    make_snooze_item(48 * 3600, "For 2 Days (48 Hours)"),
                    MenuItem::Separator,
                    {
                        let tx = self.tx.clone();
                        MenuItem::Standard(ksni::menu::StandardItem {
                            label: "Indefinitely (Until I Resume)".into(),
                            activate: Box::new(move |_| {
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    let _ = tx.send(Event::ToggleManualPause).await;
                                });
                            }),
                            ..Default::default()
                        })
                    },
                ],
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(ksni::menu::StandardItem {
                label: if is_paused { "▶ Resume Timer".into() } else { "⏸ Pause Monitoring".into() },
                activate: Box::new(move |_| {
                    let tx = tx_toggle.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::ToggleManualPause).await;
                    });
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(ksni::menu::StandardItem {
                label: "Quit Rest Time".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }),
        ]
    }
}
