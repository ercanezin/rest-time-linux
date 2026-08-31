use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, Value};
use zbus::{interface, Connection};
use crate::engine::types::Event;

// ---------------------------------------------------------------------------
// 1. StatusNotifierItem D-Bus Interface (with XAyatanaLabel for GNOME Top Bar)
// ---------------------------------------------------------------------------

pub struct StatusNotifierItem {
    pub display_text: Arc<RwLock<String>>,
    pub tooltip_text: Arc<RwLock<String>>,
    pub is_in_break: Arc<AtomicBool>,
    pub is_snoozed: Arc<AtomicBool>,
    pub tx: mpsc::Sender<Event>,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "rest-time-linux"
    }

    #[zbus(property)]
    fn title(&self) -> String {
        self.display_text.read().unwrap().clone()
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::from_static_str("/MenuBar").unwrap()
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        if self.is_in_break.load(Ordering::Relaxed) {
            "rest-time-break".into()
        } else if self.is_snoozed.load(Ordering::Relaxed) {
            "rest-time-paused".into()
        } else {
            "rest-time-active".into()
        }
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> &str {
        ""
    }

    // AYATANA LABEL PROPERTY READ DIRECTLY BY GNOME APPINDICATOR TO SHOW TOP BAR TEXT
    #[zbus(property)]
    fn x_ayatana_label(&self) -> String {
        self.display_text.read().unwrap().clone()
    }

    #[zbus(property)]
    fn x_ayatana_label_guide(&self) -> &str {
        "00:00"
    }

    #[zbus(property)]
    fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
        let desc = self.tooltip_text.read().unwrap().clone();
        ("rest-time-active".into(), Vec::new(), "Rest Time".into(), desc)
    }

    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.try_send(Event::TriggerForcedBreak);
    }

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.tx.try_send(Event::ToggleManualPause);
    }

    fn x_ayatana_secondary_activate(&self, _timestamp: u32) {
        let _ = self.tx.try_send(Event::ToggleManualPause);
    }

    #[zbus(signal)]
    pub async fn x_ayatana_new_label(emitter: &SignalEmitter<'_>, label: &str, guide: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// 2. DBusMenu Implementation (com.canonical.dbusmenu)
// ---------------------------------------------------------------------------

pub struct DBusMenuService {
    pub tooltip_text: Arc<RwLock<String>>,
    pub work_duration_mins: Arc<AtomicU32>,
    pub break_duration_mins: Arc<AtomicU32>,
    pub is_snoozed: Arc<AtomicBool>,
    pub is_blocker_enabled: Arc<AtomicBool>,
    pub available_lists: Arc<RwLock<Vec<String>>>,
    pub active_lists: Arc<RwLock<HashSet<String>>>,
    pub tx: mpsc::Sender<Event>,
}

impl DBusMenuService {
    fn build_menu_definitions(&self) -> Vec<(i32, HashMap<String, Value<'static>>, Vec<Value<'static>>)> {
        let is_paused = self.is_snoozed.load(Ordering::Relaxed);
        let work_m = self.work_duration_mins.load(Ordering::Relaxed);
        let break_m = self.break_duration_mins.load(Ordering::Relaxed);
        let status_text = self.tooltip_text.read().unwrap().clone();

        let make_leaf = |id: i32, label: &'static str| -> (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>) {
            let mut props: HashMap<String, Value<'static>> = HashMap::new();
            props.insert("label".to_string(), Value::from(label));
            props.insert("enabled".to_string(), Value::from(true));
            props.insert("visible".to_string(), Value::from(true));
            (id, props, Vec::new())
        };

        let make_leaf_owned = |id: i32, label: String| -> (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>) {
            let mut props: HashMap<String, Value<'static>> = HashMap::new();
            props.insert("label".to_string(), Value::from(label));
            props.insert("enabled".to_string(), Value::from(true));
            props.insert("visible".to_string(), Value::from(true));
            (id, props, Vec::new())
        };

        let make_sep = |id: i32| -> (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>) {
            let mut props: HashMap<String, Value<'static>> = HashMap::new();
            props.insert("type".to_string(), Value::from("separator"));
            props.insert("visible".to_string(), Value::from(true));
            (id, props, Vec::new())
        };

        let work_submenu: Vec<Value<'static>> = vec![
            make_leaf(101, "10 Minutes"),
            make_leaf(102, "15 Minutes"),
            make_leaf(103, "20 Minutes"),
            make_leaf(104, "25 Minutes (Standard)"),
            make_leaf(105, "30 Minutes"),
            make_leaf(106, "45 Minutes"),
            make_leaf(107, "50 Minutes"),
            make_leaf(108, "60 Minutes (1 Hour)"),
            make_leaf(109, "90 Minutes"),
        ].into_iter().map(Value::from).collect();

        let break_submenu: Vec<Value<'static>> = vec![
            make_leaf(201, "1 Minute"),
            make_leaf(202, "2 Minutes"),
            make_leaf(203, "3 Minutes"),
            make_leaf(204, "5 Minutes (Default)"),
            make_leaf(205, "10 Minutes"),
            make_leaf(206, "15 Minutes"),
            make_leaf(207, "20 Minutes"),
            make_leaf(208, "30 Minutes"),
        ].into_iter().map(Value::from).collect();

        let pause_submenu: Vec<Value<'static>> = vec![
            make_leaf(301, "For 5 Minutes"),
            make_leaf(302, "For 10 Minutes"),
            make_leaf(303, "For 15 Minutes"),
            make_leaf(304, "For 20 Minutes"),
            make_leaf(305, "For 30 Minutes"),
            make_leaf(306, "For 45 Minutes"),
            make_leaf(307, "For 1 Hour"),
            make_leaf(308, "For 2 Hours"),
            make_leaf(309, "For 3 Hours"),
            make_leaf(310, "For 4 Hours"),
            make_leaf(311, "For 6 Hours"),
            make_leaf(312, "For 8 Hours"),
            make_leaf(313, "For 10 Hours"),
            make_leaf(314, "For 1 Day (24 Hours)"),
            make_leaf(315, "For 2 Days (48 Hours)"),
            make_leaf(316, "Indefinitely (Until I Resume)"),
        ].into_iter().map(Value::from).collect();

        // Focus Website Blocker Submenu
        let blocker_active = self.is_blocker_enabled.load(Ordering::Relaxed);
        let available_l = self.available_lists.read().unwrap().clone();
        let active_l = self.active_lists.read().unwrap().clone();

        let mut blocker_items: Vec<Value<'static>> = Vec::new();
        let master_label = if blocker_active {
            "🛑 Website Blocker: ACTIVE (Click to Disable)"
        } else {
            "🛡️ Website Blocker: OFF (Click to Enable)"
        };
        blocker_items.push(Value::from(make_leaf_owned(401, master_label.to_string())));
        blocker_items.push(Value::from(make_sep(402)));

        if available_l.is_empty() {
            blocker_items.push(Value::from(make_leaf(403, "(No .txt lists in ~/blocked_sites)")));
        } else {
            for (idx, list_file) in available_l.iter().enumerate() {
                let is_list_active = active_l.contains(list_file);
                let checkmark = if is_list_active { "☑" } else { "☐" };
                let label = format!("{} {}", checkmark, list_file);
                let item_id = 410 + (idx as i32);
                blocker_items.push(Value::from(make_leaf_owned(item_id, label)));
            }
        }

        blocker_items.push(Value::from(make_sep(450)));
        blocker_items.push(Value::from(make_leaf(490, "🌐 View Motivational Page")));
        blocker_items.push(Value::from(make_leaf(491, "📁 Open Block Lists Folder")));

        let make_item = |id: i32, label: String, enabled: bool, is_submenu: bool, children: Vec<Value<'static>>| -> (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>) {
            let mut props: HashMap<String, Value<'static>> = HashMap::new();
            props.insert("label".to_string(), Value::from(label));
            props.insert("enabled".to_string(), Value::from(enabled));
            props.insert("visible".to_string(), Value::from(true));
            if is_submenu {
                props.insert("children-display".to_string(), Value::from("submenu"));
            }
            (id, props, children)
        };

        let toggle_label = if is_paused {
            "▶ Continue Monitoring".to_string()
        } else {
            "⏸ Pause Monitoring".to_string()
        };

        vec![
            make_item(1, format!("Status: {}", status_text), false, false, Vec::new()),
            make_item(2, format!("Work Duration: {} Min", work_m), false, false, Vec::new()),
            make_item(3, format!("Break Duration: {} Min", break_m), false, false, Vec::new()),
            make_sep(4),
            make_item(10, "▶ Take Break Now".into(), true, false, Vec::new()),
            make_sep(11),
            make_item(100, "⏱ Set Work Duration".into(), true, true, work_submenu),
            make_item(200, "☕ Set Break Duration".into(), true, true, break_submenu),
            make_item(300, "⏸ Pause / Snooze Timer".into(), true, true, pause_submenu),
            make_item(400, "🛡️ Focus Website Blocker".into(), true, true, blocker_items),
            make_sep(12),
            make_item(20, toggle_label, true, false, Vec::new()),
            make_sep(21),
            make_item(99, "Quit Rest Time".into(), true, false, Vec::new()),
        ]
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl DBusMenuService {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_layout(
        &self,
        parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>)) {
        let root_children = self.build_menu_definitions();

        if parent_id == 0 {
            let mut root_props: HashMap<String, Value<'static>> = HashMap::new();
            root_props.insert("children-display".to_string(), Value::from("submenu"));
            let children_val = root_children.into_iter().map(Value::from).collect();
            (1, (0, root_props, children_val))
        } else {
            for (id, props, children) in root_children {
                if id == parent_id {
                    return (1, (id, props, children));
                }
            }
            (1, (parent_id, HashMap::new(), Vec::new()))
        }
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, Value<'static>>)> {
        let mut result = Vec::new();
        let root_children = self.build_menu_definitions();

        let mut all_items: HashMap<i32, HashMap<String, Value<'static>>> = HashMap::new();

        fn harvest(item: (i32, HashMap<String, Value<'static>>, Vec<Value<'static>>), map: &mut HashMap<i32, HashMap<String, Value<'static>>>) {
            let (id, props, children) = item;
            map.insert(id, props);
            for c in children {
                if let Ok(tuple) = <(i32, HashMap<String, Value<'static>>, Vec<Value<'static>>)>::try_from(c) {
                    harvest(tuple, map);
                }
            }
        }

        for item in root_children {
            harvest(item, &mut all_items);
        }

        for id in ids {
            if let Some(props) = all_items.get(&id) {
                result.push((id, props.clone()));
            }
        }

        result
    }

    fn get_property(&self, id: i32, name: &str) -> Value<'static> {
        let root_children = self.build_menu_definitions();
        for (item_id, props, _) in root_children {
            if item_id == id {
                if let Some(v) = props.get(name) {
                    return v.clone();
                }
            }
        }
        Value::from("")
    }

    fn event(&self, id: i32, event_id: &str, _data: Value<'_>, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }

        match id {
            10 => {
                let _ = self.tx.try_send(Event::TriggerForcedBreak);
            }
            20 => {
                let _ = self.tx.try_send(Event::ToggleManualPause);
            }
            99 => {
                crate::blocker::BlockerEngine::disable_proxy_on_exit();
                std::process::exit(0);
            }
            // Work duration options
            101 => { let _ = self.tx.try_send(Event::SetWorkDuration(10)); }
            102 => { let _ = self.tx.try_send(Event::SetWorkDuration(15)); }
            103 => { let _ = self.tx.try_send(Event::SetWorkDuration(20)); }
            104 => { let _ = self.tx.try_send(Event::SetWorkDuration(25)); }
            105 => { let _ = self.tx.try_send(Event::SetWorkDuration(30)); }
            106 => { let _ = self.tx.try_send(Event::SetWorkDuration(45)); }
            107 => { let _ = self.tx.try_send(Event::SetWorkDuration(50)); }
            108 => { let _ = self.tx.try_send(Event::SetWorkDuration(60)); }
            109 => { let _ = self.tx.try_send(Event::SetWorkDuration(90)); }
            // Break duration options
            201 => { let _ = self.tx.try_send(Event::SetBreakDuration(1)); }
            202 => { let _ = self.tx.try_send(Event::SetBreakDuration(2)); }
            203 => { let _ = self.tx.try_send(Event::SetBreakDuration(3)); }
            204 => { let _ = self.tx.try_send(Event::SetBreakDuration(5)); }
            205 => { let _ = self.tx.try_send(Event::SetBreakDuration(10)); }
            206 => { let _ = self.tx.try_send(Event::SetBreakDuration(15)); }
            207 => { let _ = self.tx.try_send(Event::SetBreakDuration(20)); }
            208 => { let _ = self.tx.try_send(Event::SetBreakDuration(30)); }
            // Snooze options
            301 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(5 * 60))); }
            302 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(10 * 60))); }
            303 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(15 * 60))); }
            304 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(20 * 60))); }
            305 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(30 * 60))); }
            306 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(45 * 60))); }
            307 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(3600))); }
            308 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(7200))); }
            309 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(10800))); }
            310 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(14400))); }
            311 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(21600))); }
            312 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(28800))); }
            313 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(36000))); }
            314 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(86400))); }
            315 => { let _ = self.tx.try_send(Event::Snooze(Duration::from_secs(172800))); }
            316 => { let _ = self.tx.try_send(Event::ToggleManualPause); }
            // Blocker options
            401 => {
                let _ = self.tx.try_send(Event::ToggleBlockerMaster);
            }
            490 => {
                let _ = std::process::Command::new("xdg-open").arg("http://127.0.0.1:8765").spawn();
            }
            491 => {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ee".into());
                let dir = std::path::PathBuf::from(home).join("blocked_sites");
                let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
            }
            id if (410..450).contains(&id) => {
                let idx = (id - 410) as usize;
                let available = self.available_lists.read().unwrap().clone();
                if let Some(list_name) = available.get(idx) {
                    let _ = self.tx.try_send(Event::ToggleBlockList(list_name.clone()));
                }
            }
            _ => {}
        }
    }

    fn event_group(&self, events: Vec<(i32, String, Value<'_>, u32)>) -> Vec<i32> {
        for (id, event_id, data, ts) in events {
            self.event(id, &event_id, data, ts);
        }
        Vec::new()
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(signal)]
    pub async fn layout_updated(emitter: &SignalEmitter<'_>, revision: u32, parent: i32) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// 3. Tray Service Orchestrator
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TrayHandle {
    display_text: Arc<RwLock<String>>,
    tooltip_text: Arc<RwLock<String>>,
    work_duration_mins: Arc<AtomicU32>,
    break_duration_mins: Arc<AtomicU32>,
    is_snoozed: Arc<AtomicBool>,
    is_in_break: Arc<AtomicBool>,
    pub is_blocker_enabled: Arc<AtomicBool>,
    pub available_lists: Arc<RwLock<Vec<String>>>,
    pub active_lists: Arc<RwLock<HashSet<String>>>,
    last_emitted_work_m: Arc<AtomicU32>,
    last_emitted_break_m: Arc<AtomicU32>,
    last_emitted_snoozed: Arc<AtomicBool>,
    last_emitted_blocker_enabled: Arc<AtomicBool>,
    last_emitted_active_lists_len: Arc<AtomicU32>,
    conn: Arc<RwLock<Option<Connection>>>,
}

impl TrayHandle {
    pub fn update(
        &self,
        display: &str,
        tooltip: &str,
        work_m: u32,
        break_m: u32,
        snoozed: bool,
        in_break: bool,
    ) {
        {
            let mut d = self.display_text.write().unwrap();
            *d = display.to_string();
        }
        {
            let mut t = self.tooltip_text.write().unwrap();
            *t = tooltip.to_string();
        }
        self.work_duration_mins.store(work_m, Ordering::Relaxed);
        self.break_duration_mins.store(break_m, Ordering::Relaxed);
        self.is_snoozed.store(snoozed, Ordering::Relaxed);
        self.is_in_break.store(in_break, Ordering::Relaxed);

        let blocker_en = self.is_blocker_enabled.load(Ordering::Relaxed);
        let active_count = self.active_lists.read().unwrap().len() as u32;

        let prev_work = self.last_emitted_work_m.swap(work_m, Ordering::Relaxed);
        let prev_break = self.last_emitted_break_m.swap(break_m, Ordering::Relaxed);
        let prev_snoozed = self.last_emitted_snoozed.swap(snoozed, Ordering::Relaxed);
        let prev_blocker = self.last_emitted_blocker_enabled.swap(blocker_en, Ordering::Relaxed);
        let prev_active_count = self.last_emitted_active_lists_len.swap(active_count, Ordering::Relaxed);

        let menu_structure_changed = prev_work != work_m
            || prev_break != break_m
            || prev_snoozed != snoozed
            || prev_blocker != blocker_en
            || prev_active_count != active_count;

        // Notify D-Bus of changed properties and emit XAyatanaNewLabel signal
        if let Some(conn) = self.conn.read().unwrap().as_ref() {
            let conn = conn.clone();
            let display_str = display.to_string();
            let icon_str = if in_break {
                "rest-time-break".to_string()
            } else if snoozed {
                "rest-time-paused".to_string()
            } else {
                "rest-time-active".to_string()
            };

            tokio::spawn(async move {
                if let Ok(emitter) = SignalEmitter::new(&conn, "/StatusNotifierItem") {
                    let _ = StatusNotifierItem::x_ayatana_new_label(&emitter, &display_str, "00:00").await;
                    let _ = StatusNotifierItem::new_title(&emitter).await;
                    let _ = StatusNotifierItem::new_icon(&emitter).await;
                }

                // ONLY emit LayoutUpdated if menu settings actually changed
                if menu_structure_changed {
                    if let Ok(menu_emitter) = SignalEmitter::new(&conn, "/MenuBar") {
                        let _ = DBusMenuService::layout_updated(&menu_emitter, 1, 0).await;
                    }
                }

                let mut changed = HashMap::new();
                changed.insert("XAyatanaLabel", Value::from(display_str));
                changed.insert("IconName", Value::from(icon_str));
                let _ = conn.emit_signal(
                    Option::<&str>::None,
                    "/StatusNotifierItem",
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &("org.kde.StatusNotifierItem", changed, Vec::<&str>::new()),
                ).await;
            });
        }
    }

    pub fn trigger_menu_layout_update(&self) {
        if let Some(conn) = self.conn.read().unwrap().as_ref() {
            let conn = conn.clone();
            tokio::spawn(async move {
                if let Ok(menu_emitter) = SignalEmitter::new(&conn, "/MenuBar") {
                    let _ = DBusMenuService::layout_updated(&menu_emitter, 1, 0).await;
                }
            });
        }
    }
}

pub struct NativeTrayServer;

impl NativeTrayServer {
    pub async fn spawn(
        initial_work_mins: u32,
        initial_break_mins: u32,
        is_blocker_enabled: Arc<AtomicBool>,
        available_lists: Arc<RwLock<Vec<String>>>,
        active_lists: Arc<RwLock<HashSet<String>>>,
        tx: mpsc::Sender<Event>,
    ) -> Result<TrayHandle, Box<dyn std::error::Error>> {
        let display_text = Arc::new(RwLock::new(format!("{:02}:00", initial_work_mins)));
        let tooltip_text = Arc::new(RwLock::new("Focus Time: Starting...".into()));
        let work_duration_mins = Arc::new(AtomicU32::new(initial_work_mins));
        let break_duration_mins = Arc::new(AtomicU32::new(initial_break_mins));
        let is_snoozed = Arc::new(AtomicBool::new(false));
        let is_in_break = Arc::new(AtomicBool::new(false));
        let last_emitted_work_m = Arc::new(AtomicU32::new(initial_work_mins));
        let last_emitted_break_m = Arc::new(AtomicU32::new(initial_break_mins));
        let last_emitted_snoozed = Arc::new(AtomicBool::new(false));
        let last_emitted_blocker_enabled = Arc::new(AtomicBool::new(is_blocker_enabled.load(Ordering::Relaxed)));
        let last_emitted_active_lists_len = Arc::new(AtomicU32::new(active_lists.read().unwrap().len() as u32));
        let conn_holder = Arc::new(RwLock::new(None));

        let handle = TrayHandle {
            display_text: display_text.clone(),
            tooltip_text: tooltip_text.clone(),
            work_duration_mins: work_duration_mins.clone(),
            break_duration_mins: break_duration_mins.clone(),
            is_snoozed: is_snoozed.clone(),
            is_in_break: is_in_break.clone(),
            is_blocker_enabled: is_blocker_enabled.clone(),
            available_lists: available_lists.clone(),
            active_lists: active_lists.clone(),
            last_emitted_work_m,
            last_emitted_break_m,
            last_emitted_snoozed,
            last_emitted_blocker_enabled,
            last_emitted_active_lists_len,
            conn: conn_holder.clone(),
        };

        let pid = std::process::id();
        let service_name = format!("org.kde.StatusNotifierItem-{}-1", pid);

        let sni = StatusNotifierItem {
            display_text: display_text.clone(),
            tooltip_text: tooltip_text.clone(),
            is_in_break: is_in_break.clone(),
            is_snoozed: is_snoozed.clone(),
            tx: tx.clone(),
        };

        let menu = DBusMenuService {
            tooltip_text: tooltip_text.clone(),
            work_duration_mins: work_duration_mins.clone(),
            break_duration_mins: break_duration_mins.clone(),
            is_snoozed: is_snoozed.clone(),
            is_blocker_enabled,
            available_lists,
            active_lists,
            tx,
        };

        tokio::spawn(async move {
            match zbus::connection::Builder::session() {
                Ok(builder) => {
                    match builder
                        .name(service_name.as_str())
                        .and_then(|b| b.serve_at("/StatusNotifierItem", sni))
                        .and_then(|b| b.serve_at("/MenuBar", menu))
                    {
                        Ok(configured_builder) => {
                            match configured_builder.build().await {
                                Ok(connection) => {
                                    info!("Registered native StatusNotifierItem on {}", service_name);
                                    *conn_holder.write().unwrap() = Some(connection.clone());

                                    // Register with StatusNotifierWatcher
                                    let _ = connection.call_method(
                                        Some("org.kde.StatusNotifierWatcher"),
                                        "/StatusNotifierWatcher",
                                        Some("org.kde.StatusNotifierWatcher"),
                                        "RegisterStatusNotifierItem",
                                        &("/StatusNotifierItem"),
                                    ).await;

                                    info!("Successfully registered with StatusNotifierWatcher (GNOME/KDE)");
                                }
                                Err(e) => error!("Failed to build D-Bus connection: {}", e),
                            }
                        }
                        Err(e) => error!("Failed to configure D-Bus builder: {}", e),
                    }
                }
                Err(e) => error!("Failed to connect to Session D-Bus: {}", e),
            }
        });

        Ok(handle)
    }
}
