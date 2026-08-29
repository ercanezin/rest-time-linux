use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub intervals: IntervalSettings,
    pub notifications: NotificationSettings,
    pub behavior: BehaviorSettings,
    pub ui: UiSettings,
    pub audio: AudioSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntervalSettings {
    pub work_duration_mins: u32,
    pub micro_break_seconds: u32,
    pub macro_break_mins: u32,
    pub micro_breaks_before_macro: u32,
    pub idle_threshold_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationSettings {
    pub enable_progressive_warnings: bool,
    pub warning_minutes: Vec<u32>,
    pub final_warning_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BehaviorSettings {
    pub auto_credit_informal_breaks: bool,
    pub strict_hold_to_unlock: bool,
    pub hold_unlock_duration_ms: u64,
    pub defer_duration_mins: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiSettings {
    pub theme: String,
    pub background_color: String,
    pub accent_color: String,
    pub text_color: String,
    pub background_opacity: f64,
    pub show_tray_time: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSettings {
    pub sound_enabled: bool,
    pub volume: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            intervals: IntervalSettings {
                work_duration_mins: 25,
                micro_break_seconds: 30,
                macro_break_mins: 5,
                micro_breaks_before_macro: 3,
                idle_threshold_seconds: 180,
            },
            notifications: NotificationSettings {
                enable_progressive_warnings: true,
                warning_minutes: vec![10, 5, 3],
                final_warning_seconds: 30,
            },
            behavior: BehaviorSettings {
                auto_credit_informal_breaks: true,
                strict_hold_to_unlock: true,
                hold_unlock_duration_ms: 2500,
                defer_duration_mins: 2,
            },
            ui: UiSettings {
                theme: "dark".into(),
                background_color: "#121418".into(),
                accent_color: "#E5C07B".into(),
                text_color: "#ABB2BF".into(),
                background_opacity: 0.94,
                show_tray_time: true,
            },
            audio: AudioSettings {
                sound_enabled: true,
                volume: 0.75,
            },
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self> {
        let path = Self::get_config_path();
        Self::load_from_path_or_create(&path)
    }

    pub fn load_from_path_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = fs::read_to_string(path)?;
            let cfg: Config = toml::from_str(&data)?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let serialized = toml::to_string_pretty(&cfg)?;
            fs::write(path, serialized)?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn get_config_path() -> PathBuf {
        ProjectDirs::from("com", "github", "rest-time-linux")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"))
    }
}
