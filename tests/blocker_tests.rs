use std::fs;
use tempfile::tempdir;
use rest_time_linux::config::Config;
use rest_time_linux::blocker::BlockerEngine;

#[test]
fn test_blocker_scan_and_pac_generation() {
    let dir = tempdir().unwrap();
    let blocked_dir = dir.path().to_path_buf();

    // Create 2 test block lists with various formats (wildcards, hosts, adblock, dnsmasq)
    fs::write(
        blocked_dir.join("focus.txt"),
        "*.google.com\n*.youtube.com\n# Comment\nreddit.com\n||doubleclick.net^\n0.0.0.0 badtracker.com\n",
    ).unwrap();
    fs::write(blocked_dir.join("social.txt"), "*.x.com\n*.instagram.com\naddress=/malware.org/0.0.0.0\n").unwrap();

    let mut cfg = Config::default();
    cfg.blocker.active_lists = vec!["focus.txt".to_string(), "social.txt".to_string()];
    cfg.blocker.enabled = true;

    let mut engine = BlockerEngine::new(cfg);
    engine.blocked_dir = blocked_dir.clone();
    engine.refresh_lists_and_patterns();

    let available = engine.scan_available_lists();
    assert_eq!(available, vec!["focus.txt".to_string(), "social.txt".to_string()]);

    let pac = engine.generate_pac_script();
    assert!(pac.contains("\"google.com\":1"));
    assert!(pac.contains("\"youtube.com\":1"));
    assert!(pac.contains("\"reddit.com\":1"));
    assert!(pac.contains("\"doubleclick.net\":1"));
    assert!(pac.contains("\"badtracker.com\":1"));
    assert!(pac.contains("\"x.com\":1"));
    assert!(pac.contains("\"instagram.com\":1"));
    assert!(pac.contains("\"malware.org\":1"));
    assert!(!pac.contains("# Comment"));
}

#[test]
fn test_blocker_toggle_multiple_lists() {
    let dir = tempdir().unwrap();
    let blocked_dir = dir.path().to_path_buf();

    fs::write(blocked_dir.join("list_a.txt"), "*.sitea.com\n").unwrap();
    fs::write(blocked_dir.join("list_b.txt"), "siteb.com\n").unwrap();

    let mut cfg = Config::default();
    cfg.blocker.active_lists = vec!["list_a.txt".to_string()];

    let mut engine = BlockerEngine::new(cfg);
    engine.blocked_dir = blocked_dir.clone();
    engine.refresh_lists_and_patterns();

    // Initially only list_a is active
    let pac = engine.generate_pac_script();
    assert!(pac.contains("\"sitea.com\":1"));
    assert!(!pac.contains("\"siteb.com\":1"));

    // Activate list_b as well (multiple lists active!)
    let is_active_b = engine.toggle_list("list_b.txt");
    assert!(is_active_b);

    let pac_both = engine.generate_pac_script();
    assert!(pac_both.contains("\"sitea.com\":1"));
    assert!(pac_both.contains("\"siteb.com\":1"));

    // Deactivate list_a
    let is_active_a = engine.toggle_list("list_a.txt");
    assert!(!is_active_a);

    let pac_only_b = engine.generate_pac_script();
    assert!(!pac_only_b.contains("\"sitea.com\":1"));
    assert!(pac_only_b.contains("\"siteb.com\":1"));
}
