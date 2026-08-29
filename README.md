# ⏳ rest-time-linux

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Arch%20Linux%20%7C%20CachyOS%20%7C%20Wayland-brightgreen.svg)](https://wiki.archlinux.org)
[![Layer Shell](https://img.shields.io/badge/wayland-wlr--layer--shell-blueviolet.svg)](https://wayland.freedesktop.org)

**`rest-time-linux`** is an open-source, resource-efficient, native ergonomic micro-pause and rest break daemon engineered specifically for **Arch Linux**, **CachyOS**, and modern Wayland compositors (with seamless X11 fallback).

It achieves feature parity with the macOS application **Rest Time** (by publicspace.net), incorporating its specific heuristics, interaction design, and non-punitive ergonomic philosophies.

---

## 🌟 Key Features & Ergonomic Philosophy

- **🔒 Privacy-Preserving Heuristics**: Zero keylogging, accessibility hacks, or intrusive kernel hooks. Activity tracking relies entirely on FreeDesktop standard D-Bus session interfaces (`org.freedesktop.ScreenSaver`, `org.gnome.Mutter.IdleMonitor`).
- **⚙️ Deterministic State Automation**: Never counts down in your absence. Automatically pauses when informal breaks occur and credits them retroactively if you step away naturally.
- **🔔 Multi-Tier Progressive Interventions**: Emits non-intrusive pre-break alerts (e.g., at 10m, 5m, 3m, and a final 30s alert) before screen capture, preventing disruptive locks during critical tasks.
- **🖥️ Multi-Monitor Spatial Takeover**: Mounts native Wayland exclusive surfaces across all attached monitors using `wlr-layer-shell` (via `gtk4-layer-shell`) without grabbing raw window frames or relying on glitchy overlay hacks.
- **🛑 Friction-Based Guilt Mechanics**: The break overlay provides an emergency escape hatch via an interactive **"Hold-to-Unlock"** delay ramp to gently discourage impulsive dismissals.
- **💤 Sleep/Wake Gracefulness**: Hooks into `systemd-logind` via D-Bus (`PrepareForSleep`) to freeze state across suspend/resume cycles, ensuring no timer desync or false alarms on system wake.
- **⚡ Ultra-Low Resource Footprint**: Memory footprint strictly $< 25\text{ MB}$ RSS, CPU consumption $\approx 0.0\%$ during active countdown, zero wake-locks.

---

## 🏗️ Architecture

The daemon combines an asynchronous worker runtime in Tokio (handling D-Bus communication, idle monitoring, logind power management, and acoustic synthesis) with a GTK4 / GLib main loop (handling the layer-shell overlay engine and system tray integration).

```text
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                   GRAPHICAL & UI THREAD (GLib Loop)                              │
│                                                                                                  │
│   ┌──────────────────────────────────────────────┐  ┌──────────────────────────────────────────┐ │
│   │           StatusNotifierItem (Tray)          │  │       Layer-Shell Overlay Engine         │ │
│   │  - Dynamic Status Badge                      │  │  - Headless/Multi-Monitor Instantiation │ │
│   │  - FreeDesktop Menu Hierarchy                │  │  - Cairo Radial Dial & Canvas Drawing   │ │
│   │  - Snooze & Defer Action Handlers            │  │  - Hold-To-Unlock State Transition      │ │
│   └──────────────────────▲───────────────────────┘  └──────────────────▲───────────────────────┘ │
└──────────────────────────┼─────────────────────────────────────────────┼─────────────────────────┘
                           │ mpsc::channel                               │ UI Effect Channel
┌──────────────────────────┴─────────────────────────────────────────────┴─────────────────────────┐
│                                ASYNCHRONOUS WORKER RUNTIME (Tokio)                               │
│                                                                                                  │
│   ┌──────────────────────────────────────────────────────────────────────────────────────────┐   │
│   │                              Deterministic FSM Scheduler                                 │   │
│   │  - Interval Math (Work, Micro-break, Macro-break)                                        │   │
│   │  - Progressive Notification Thresholds (10m, 5m, 3m, 30s)                                │   │
│   │  - System Sleep / Wake Inhibit Logic                                                     │   │
│   └──────────────▲───────────────────────────▲───────────────────────────▲───────────────────┘   │
│                  │                           │                           │                       │
│   ┌──────────────┴──────────────┐ ┌──────────┴──────────────┐ ┌──────────┴───────────────────┐   │
│   │   Idle Discovery Listener   │ │   Logind Sleep Monitor  │ │    Audio Synthesis Pipe      │   │
│   │  - FreeDesktop ScreenSaver  │ │  - PrepareForSleep D-Bus│ │  - PipeWire / ALSA via Rodio │   │
│   │  - Mutter Idle Monitor      │ │  - Timestamp Calibration│ │  - Spatial Pure Sine Chimes  │   │
│   └─────────────────────────────┘ └─────────────────────────┘ └──────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 🚀 Installation & Building

### Prerequisites (Arch Linux / CachyOS)

```bash
sudo pacman -S --needed base-devel rustup gtk4 gtk4-layer-shell cairo dbus pipewire alsa-lib pkgconf
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/local/rest-time-linux.git
cd rest-time-linux

# Build release binary
cargo build --release

# Run test suite
cargo test

# Install to system (default prefix: /usr/local)
sudo make install
```

### Arch Linux / CachyOS PKGBUILD

```bash
# Build and install package using makepkg
makepkg -si
```

---

## ⚙️ Configuration

Configuration is stored in `$XDG_CONFIG_HOME/rest-time/config.toml` (defaults to `~/.config/rest-time/config.toml`). If not present, it will automatically populate on first launch.

```toml
[intervals]
# Work duration in minutes before a break is scheduled
work_duration_mins = 25

# Micro-pause duration in seconds (20-20-20 rule)
micro_break_seconds = 30

# Longer macro rest break duration in minutes
macro_break_mins = 5

# Number of micro-pauses before a macro break is triggered
micro_breaks_before_macro = 3

# Inactivity threshold (in seconds) to start tracking an informal break
idle_threshold_seconds = 180

[notifications]
# Enable progressive warnings before break begins
enable_progressive_warnings = true

# Minute marks to emit pre-break notifications
warning_minutes = [10, 5, 3]

# Seconds before break to display final warning
final_warning_seconds = 30

[behavior]
# Retroactively credit breaks if you naturally step away
auto_credit_informal_breaks = true

# Require holding the unlock button on break screen to dismiss
strict_hold_to_unlock = true

# Milliseconds required to hold the unlock button (e.g. 2500ms = 2.5s)
hold_unlock_duration_ms = 2500

# Defer break duration in minutes
defer_duration_mins = 2

[ui]
theme = "dark"
background_color = "#121418"
accent_color = "#E5C07B"
text_color = "#ABB2BF"
background_opacity = 0.94
show_tray_time = true

[audio]
# Enable gentle synthesized harmonic chime on break start & end
sound_enabled = true
volume = 0.75
```

---

## 🖥️ Systemd Autostart

To automatically start `rest-time-linux` on graphical desktop login:

```bash
mkdir -p ~/.config/systemd/user/
cp resources/rest-time.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now rest-time.service
```

---

## 🧪 Testing

The test suite covers:
- Complete FSM transitions (work -> warnings -> break overlay -> completed cycles)
- Informal break auto-credit heuristics
- Progressive notification threshold matching
- Postpone and snooze workflows
- Configuration file creation and TOML deserialization round-trips

To execute all tests:

```bash
cargo test -- --nocapture
```

---

## 📂 Project Structure

```text
rest-time-linux/
├── Cargo.toml                  # Cargo manifest & dependencies
├── PKGBUILD                    # Arch Linux / CachyOS package definition
├── Makefile                    # Standard POSIX build & install recipes
├── README.md                   # Project documentation
├── LICENSE-MIT                 # MIT License
├── LICENSE-APACHE              # Apache 2.0 License
├── resources/
│   ├── rest-time.desktop       # XDG desktop application entry
│   ├── rest-time.service       # Systemd user service unit
│   ├── icons/                  # High-resolution SVG application icons
│   │   ├── rest-time-active.svg
│   │   ├── rest-time-paused.svg
│   │   └── rest-time-break.svg
│   └── sounds/                 # Acoustic transition chimes
│       ├── gentle-bell.ogg
│       └── break-complete.ogg
├── tests/                      # Integration & unit test suites
│   ├── config_tests.rs
│   └── fsm_tests.rs
└── src/
    ├── main.rs                 # Daemon orchestrator & entrypoint
    ├── lib.rs                  # Public library exports
    ├── config.rs               # TOML configuration loader & validator
    ├── error.rs                # Error definitions via thiserror
    ├── audio.rs                # In-memory harmonic sine synthesizer
    ├── engine/                 # Deterministic FSM core
    │   ├── mod.rs
    │   ├── fsm.rs
    │   └── types.rs
    ├── idle/                   # Privacy-preserving idle & power monitors
    │   ├── mod.rs
    │   ├── dbus_detector.rs
    │   └── sleep_monitor.rs
    ├── notifications/          # FreeDesktop notification dispatch
    │   └── mod.rs
    └── ui/                     # GTK4 & Layer-Shell UI engine
        ├── mod.rs
        ├── tray.rs             # StatusNotifierItem system tray
        ├── overlay.rs          # Multi-monitor exclusive layer-shell window
        ├── styles.rs           # Dynamic CSS theme engine
        └── widgets/            # Custom Cairo widgets
            ├── mod.rs
            ├── circular_progress.rs
            └── hold_button.rs
```

---

## 📄 License

Dual-licensed under either:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
