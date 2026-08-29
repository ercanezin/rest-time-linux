use rest_time_linux::config::Config;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_default_config_values() {
    let cfg = Config::default();
    assert_eq!(cfg.intervals.work_duration_mins, 25);
    assert_eq!(cfg.intervals.break_duration_mins, 5);
    assert_eq!(cfg.intervals.idle_threshold_seconds, 180);
    assert!(cfg.notifications.enable_progressive_warnings);
    assert_eq!(cfg.notifications.warning_minutes, vec![10, 5, 3]);
    assert_eq!(cfg.notifications.final_warning_seconds, 30);
    assert!(cfg.behavior.auto_credit_informal_breaks);
    assert!(cfg.behavior.strict_hold_to_unlock);
    assert_eq!(cfg.behavior.hold_unlock_duration_ms, 2500);
    assert!(cfg.audio.sound_enabled);
}

#[test]
fn test_config_load_or_create_file() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sub").join("config.toml");

    let cfg1 = Config::load_from_path_or_create(&config_path).unwrap();
    assert_eq!(cfg1.intervals.work_duration_mins, 25);
    assert!(config_path.exists());

    let mut custom_cfg = cfg1.clone();
    custom_cfg.intervals.work_duration_mins = 45;
    custom_cfg.intervals.break_duration_mins = 10;
    let toml_str = toml::to_string_pretty(&custom_cfg).unwrap();
    fs::write(&config_path, toml_str).unwrap();

    let cfg2 = Config::load_from_path_or_create(&config_path).unwrap();
    assert_eq!(cfg2.intervals.work_duration_mins, 45);
    assert_eq!(cfg2.intervals.break_duration_mins, 10);
}
