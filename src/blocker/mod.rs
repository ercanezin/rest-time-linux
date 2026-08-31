use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::config::Config;

pub struct BlockerEngine {
    pub blocked_dir: PathBuf,
    pub is_enabled: Arc<AtomicBool>,
    pub active_lists: Arc<RwLock<HashSet<String>>>,
    pub active_domains: Arc<RwLock<Vec<String>>>,
    pub available_lists: Arc<RwLock<Vec<String>>>,
    config: Arc<RwLock<Config>>,
}

impl BlockerEngine {
    pub fn new(config: Config) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ee".into());
        let blocked_dir = PathBuf::from(home).join("blocked_sites");

        if !blocked_dir.exists() {
            let _ = fs::create_dir_all(&blocked_dir);
        }

        let is_enabled = Arc::new(AtomicBool::new(config.blocker.enabled));
        let active_lists = Arc::new(RwLock::new(config.blocker.active_lists.iter().cloned().collect()));
        let active_domains = Arc::new(RwLock::new(Vec::new()));
        let available_lists = Arc::new(RwLock::new(Vec::new()));
        let config_holder = Arc::new(RwLock::new(config));

        let engine = Self {
            blocked_dir,
            is_enabled,
            active_lists,
            active_domains,
            available_lists,
            config: config_holder,
        };

        engine.refresh_lists_and_patterns();
        engine
    }

    pub fn scan_available_lists(&self) -> Vec<String> {
        let mut lists = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.blocked_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "txt" {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                lists.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        lists.sort();
        lists
    }

    pub fn parse_line(line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') || trimmed.starts_with("//") {
            return None;
        }

        let mut clean = trimmed;

        // Handle hosts format: "0.0.0.0 example.com" or "127.0.0.1 example.com"
        if clean.starts_with("0.0.0.0 ") || clean.starts_with("127.0.0.1 ") {
            if let Some(second) = clean.split_whitespace().nth(1) {
                clean = second;
            }
        }

        // Handle dnsmasq: "address=/example.com/..." or "server=/example.com/..."
        if clean.starts_with("address=/") || clean.starts_with("server=/") {
            let after = clean.split('/').nth(1).unwrap_or("");
            if !after.is_empty() {
                clean = after;
            }
        }

        // Handle Adblock syntax: "||example.com^"
        if clean.starts_with("||") {
            clean = &clean[2..];
        }
        if let Some(idx) = clean.find('^') {
            clean = &clean[..idx];
        }

        // Handle Wildcard: "*.example.com" or "*example.com"
        if clean.starts_with("*.") {
            clean = &clean[2..];
        } else if clean.starts_with('*') {
            clean = &clean[1..];
        }

        let domain = clean.trim().trim_matches('.').to_lowercase();
        if domain.is_empty() || domain.contains(' ') || domain.starts_with('/') {
            return None;
        }

        Some(domain)
    }

    pub fn refresh_lists_and_patterns(&self) {
        let available = self.scan_available_lists();
        {
            let mut av = self.available_lists.write().unwrap();
            *av = available;
        }

        let active = self.active_lists.read().unwrap().clone();
        let mut domains = Vec::new();

        for list_name in &active {
            let list_path = self.blocked_dir.join(list_name);
            if let Ok(content) = fs::read_to_string(&list_path) {
                for line in content.lines() {
                    if let Some(dom) = Self::parse_line(line) {
                        domains.push(dom);
                    }
                }
            }
        }

        domains.sort();
        domains.dedup();

        info!("Loaded {} unique domains across {} active blocklists", domains.len(), active.len());

        {
            let mut dom = self.active_domains.write().unwrap();
            *dom = domains;
        }

        self.apply_system_proxy();
    }

    pub fn toggle_master(&self) -> bool {
        let current = self.is_enabled.load(Ordering::Relaxed);
        let new_val = !current;
        self.is_enabled.store(new_val, Ordering::Relaxed);

        {
            let mut cfg = self.config.write().unwrap();
            cfg.blocker.enabled = new_val;
            let _ = cfg.save();
        }

        self.apply_system_proxy();
        new_val
    }

    pub fn toggle_list(&self, list_name: &str) -> bool {
        let is_active = {
            let mut active = self.active_lists.write().unwrap();
            if active.contains(list_name) {
                active.remove(list_name);
                false
            } else {
                active.insert(list_name.to_string());
                true
            }
        };

        {
            let active = self.active_lists.read().unwrap();
            let mut cfg = self.config.write().unwrap();
            cfg.blocker.active_lists = active.iter().cloned().collect();
            let _ = cfg.save();
        }

        self.refresh_lists_and_patterns();
        is_active
    }

    pub fn generate_pac_script(&self) -> String {
        let domains = self.active_domains.read().unwrap();
        let port = self.config.read().unwrap().blocker.port;

        // Build high-performance JSON key-value dictionary for O(1) JavaScript PAC lookup
        let mut json_obj = String::with_capacity(domains.len() * 24 + 32);
        json_obj.push('{');
        for (i, d) in domains.iter().enumerate() {
            if i > 0 {
                json_obj.push(',');
            }
            json_obj.push('"');
            json_obj.push_str(d);
            json_obj.push_str("\":1");
        }
        json_obj.push('}');

        format!(
            r#"var BLOCKED_DOMAINS = {domains_map};

function FindProxyForURL(url, host) {{
    host = host.toLowerCase();
    var parts = host.split('.');
    for (var i = 0; i < parts.length - 1; i++) {{
        var sub = parts.slice(i).join('.');
        if (BLOCKED_DOMAINS[sub]) {{
            return "PROXY 127.0.0.1:{port}";
        }}
    }}
    return "DIRECT";
}}
"#,
            domains_map = json_obj,
            port = port
        )
    }

    pub fn apply_system_proxy(&self) {
        let enabled = self.is_enabled.load(Ordering::Relaxed);
        let has_patterns = !self.active_domains.read().unwrap().is_empty();
        let port = self.config.read().unwrap().blocker.port;

        if enabled && has_patterns {
            info!("Enabling GNOME system proxy for Focus Website Blocker (PAC port {})", port);
            let pac_url = format!("http://127.0.0.1:{}/proxy.pac", port);
            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "auto"])
                .status();
            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "autoconfig-url", &pac_url])
                .status();
        } else {
            info!("Disabling GNOME system proxy");
            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "none"])
                .status();
            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "autoconfig-url", ""])
                .status();
        }
    }

    pub fn disable_proxy_on_exit() {
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "none"])
            .status();
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "autoconfig-url", ""])
            .status();
    }

    pub fn spawn_server(self: Arc<Self>) {
        let port = self.config.read().unwrap().blocker.port;
        let addr = format!("127.0.0.1:{}", port);

        tokio::spawn(async move {
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => {
                    info!("Focus Website Blocker PAC service listening on http://{}", addr);
                    l
                }
                Err(e) => {
                    error!("Failed to bind Blocker PAC server to {}: {}", addr, e);
                    return;
                }
            };

            loop {
                match listener.accept().await {
                    Ok((socket, _)) => {
                        let engine = self.clone();
                        tokio::spawn(async move {
                            Self::handle_client(socket, engine).await;
                        });
                    }
                    Err(e) => {
                        warn!("Blocker server accept error: {}", e);
                    }
                }
            }
        });
    }

    async fn handle_client(mut socket: tokio::net::TcpStream, engine: Arc<Self>) {
        let mut buf = [0u8; 4096];
        let n = match socket.read(&mut buf).await {
            Ok(n) if n > 0 => n,
            _ => return,
        };

        let req = String::from_utf8_lossy(&buf[..n]);
        let first_line = req.lines().next().unwrap_or("");

        if first_line.starts_with("GET /proxy.pac") {
            let pac = engine.generate_pac_script();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                pac.len(),
                pac
            );
            let _ = socket.write_all(resp.as_bytes()).await;
        } else {
            // Clean silent rejection of all blocked traffic (HTTP / HTTPS CONNECT) with zero tab-opening
            let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(resp.as_bytes()).await;
        }
    }
}
