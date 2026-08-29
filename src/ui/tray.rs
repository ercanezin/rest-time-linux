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
        let tx_break = self.tx.clone();
        let tx_snooze_1 = self.tx.clone();
        let tx_snooze_2 = self.tx.clone();
        let tx_snooze_12 = self.tx.clone();
        let tx_toggle = self.tx.clone();
        let is_paused = self.is_snoozed.load(Ordering::Relaxed);

        vec![
            MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("Status: {}", self.tooltip_text),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(ksni::menu::StandardItem {
                label: "Take Break Now".into(),
                activate: Box::new(move |_| {
                    let tx = tx_break.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Event::TriggerForcedBreak(BreakKind::Micro)).await;
                    });
                }),
                ..Default::default()
            }),
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "Snooze Reminders".into(),
                submenu: vec![
                    MenuItem::Standard(ksni::menu::StandardItem {
                        label: "For 1 Hour".into(),
                        activate: Box::new(move |_| {
                            let tx = tx_snooze_1.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Event::Snooze(Duration::from_secs(3600))).await;
                            });
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(ksni::menu::StandardItem {
                        label: "For 2 Hours".into(),
                        activate: Box::new(move |_| {
                            let tx = tx_snooze_2.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Event::Snooze(Duration::from_secs(7200))).await;
                            });
                        }),
                        ..Default::default()
                    }),
                    MenuItem::Standard(ksni::menu::StandardItem {
                        label: "For 12 Hours (Presentations)".into(),
                        activate: Box::new(move |_| {
                            let tx = tx_snooze_12.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Event::Snooze(Duration::from_secs(43200))).await;
                            });
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            MenuItem::Standard(ksni::menu::StandardItem {
                label: if is_paused { "Resume Timer".into() } else { "Pause Monitoring".into() },
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
