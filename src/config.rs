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
    #[serde(default)]
    pub blocker: BlockerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntervalSettings {
    pub work_duration_mins: u32,
    pub break_duration_mins: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockerSettings {
    pub enabled: bool,
    pub active_lists: Vec<String>,
    pub port: u16,
}

impl Default for BlockerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            active_lists: vec!["focus.txt".to_string()],
            port: 8765,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            intervals: IntervalSettings {
                work_duration_mins: 25,
                break_duration_mins: 5,
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
            blocker: BlockerSettings::default(),
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
            let default_cfg = Config::default();
            default_cfg.save_to_path(path)?;
            Ok(default_cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_config_path();
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn get_config_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "github", "rest-time-linux") {
            proj_dirs.config_dir().join("config.toml")
        } else {
            PathBuf::from("config.toml")
        }
    }
}
