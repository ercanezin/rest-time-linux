use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::engine::types::Event;
use crate::ui::icon_renderer::render_timer_icon;

pub struct RestTimeTray {
    pub display_text: String,
    pub tooltip_text: String,
    pub work_duration_mins: u32,
    pub break_duration_mins: u32,
    pub is_snoozed: Arc<AtomicBool>,
    pub is_in_break: Arc<AtomicBool>,
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
        // Return empty string so FreeDesktop/GNOME AppIndicator renders the dynamic IconPixmap
        "".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let is_break = self.is_in_break.load(Ordering::Relaxed);
        let is_paused = self.is_snoozed.load(Ordering::Relaxed);
        vec![render_timer_icon(&self.display_text, is_break, is_paused)]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "appointment-soon".into(),
            icon_pixmap: Vec::new(),
            title: "Rest Time".into(),
            description: self.tooltip_text.clone(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let is_paused = self.is_snoozed.load(Ordering::Relaxed);

        let make_work_item = |mins: u32, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let _ = tx.try_send(Event::SetWorkDuration(mins));
                }),
                ..Default::default()
            })
        };

        let make_break_item = |mins: u32, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let _ = tx.try_send(Event::SetBreakDuration(mins));
                }),
                ..Default::default()
            })
        };

        let make_snooze_item = |secs: u64, label: &str| -> MenuItem<Self> {
            let tx = self.tx.clone();
            MenuItem::Standard(ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |_| {
                    let _ = tx.try_send(Event::Snooze(Duration::from_secs(secs)));
                }),
                ..Default::default()
            })
        };

        let tx_break = self.tx.clone();
        let tx_toggle = self.tx.clone();

        vec![
            MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("Status: {}", self.tooltip_text),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("Work Duration: {} Min", self.work_duration_mins),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Standard(ksni::menu::StandardItem {
                label: format!("Break Duration: {} Min", self.break_duration_mins),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(ksni::menu::StandardItem {
                label: "▶ Take Break Now".into(),
                activate: Box::new(move |_| {
                    let _ = tx_break.try_send(Event::TriggerForcedBreak);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            // Work Duration Submenu
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "⏱ Set Work Duration".into(),
                submenu: vec![
                    make_work_item(10, "10 Minutes"),
                    make_work_item(15, "15 Minutes"),
                    make_work_item(20, "20 Minutes"),
                    make_work_item(25, "25 Minutes (Standard)"),
                    make_work_item(30, "30 Minutes"),
                    make_work_item(45, "45 Minutes"),
                    make_work_item(50, "50 Minutes"),
                    make_work_item(60, "60 Minutes (1 Hour)"),
                    make_work_item(90, "90 Minutes"),
                ],
                ..Default::default()
            }),
            // Break Duration Submenu
            MenuItem::SubMenu(ksni::menu::SubMenu {
                label: "☕ Set Break Duration".into(),
                submenu: vec![
                    make_break_item(1, "1 Minute"),
                    make_break_item(2, "2 Minutes"),
                    make_break_item(3, "3 Minutes"),
                    make_break_item(5, "5 Minutes (Default)"),
                    make_break_item(10, "10 Minutes"),
                    make_break_item(15, "15 Minutes"),
                    make_break_item(20, "20 Minutes"),
                    make_break_item(30, "30 Minutes"),
                ],
                ..Default::default()
            }),
            // Pause / Snooze Submenu
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
                                let _ = tx.try_send(Event::ToggleManualPause);
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
                    let _ = tx_toggle.try_send(Event::ToggleManualPause);
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
