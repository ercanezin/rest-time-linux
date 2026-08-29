use gtk4::prelude::*;
use gtk4::{Application, Box, CssProvider, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tracing::info;

use crate::config::Config;
use crate::engine::types::BreakKind;
use crate::ui::styles::generate_css;
use crate::ui::widgets::hold_button::HoldToUnlockButton;

pub struct BreakOverlayManager {
    windows: Rc<RefCell<Vec<Window>>>,
    app: Application,
    config: Config,
}

impl BreakOverlayManager {
    pub fn new(app: &Application, config: Config) -> Self {
        Self::apply_css(&config);
        Self {
            windows: Rc::new(RefCell::new(Vec::new())),
            app: app.clone(),
            config,
        }
    }

    pub fn apply_css(cfg: &Config) {
        let provider = CssProvider::new();
        let css = generate_css(cfg);
        provider.load_from_string(&css);

        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    pub fn spawn_overlays(
        &self,
        kind: BreakKind,
        total_duration: Duration,
        on_unlock: impl Fn() + 'static + Clone,
    ) {
        self.dismiss();

        let display = match gtk4::gdk::Display::default() {
            Some(d) => d,
            None => {
                info!("No GDK display available for overlay rendering");
                return;
            }
        };

        let monitors = display.monitors();
        let mut created = Vec::new();

        let (title_text, prompt, sub_prompt) = match kind {
            BreakKind::Micro => (
                "Micro-Pause",
                "Rest your eyes.",
                "Look 20 feet away into the distance for 20 seconds.",
            ),
            BreakKind::Macro => (
                "Rest Break",
                "Step away from your desk.",
                "Stand up, stretch, grab some water, and relax your shoulders.",
            ),
        };

        let layer_shell_supported = gtk4_layer_shell::is_supported();
        info!("Layer shell supported: {}", layer_shell_supported);

        let n_monitors = monitors.n_items().max(1);

        for i in 0..n_monitors {
            let monitor_opt = monitors.item(i).and_downcast::<gtk4::gdk::Monitor>();

            let win = Window::builder()
                .application(&self.app)
                .title(format!("Rest Time - {}", title_text))
                .css_classes(["break-surface"])
                .build();

            if layer_shell_supported {
                win.init_layer_shell();
                win.set_layer(Layer::Overlay);
                win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);
                if let Some(ref monitor) = monitor_opt {
                    win.set_monitor(monitor);
                }

                // Lock window bounds to monitor physical dimensions
                win.set_anchor(Edge::Top, true);
                win.set_anchor(Edge::Bottom, true);
                win.set_anchor(Edge::Left, true);
                win.set_anchor(Edge::Right, true);
                win.set_exclusive_zone(-1);
            } else {
                win.set_decorated(false);
                win.fullscreen();
            }

            let root_box = Box::new(Orientation::Vertical, 0);
            root_box.set_valign(gtk4::Align::Center);
            root_box.set_halign(gtk4::Align::Center);

            let secs = total_duration.as_secs();
            let time_label = Label::builder()
                .label(&format!("{:02}:{:02}", secs / 60, secs % 60))
                .css_classes(["dial-label"])
                .build();

            let prompt_label = Label::builder()
                .label(prompt)
                .css_classes(["instruction-text"])
                .build();

            let sub_prompt_label = Label::builder()
                .label(sub_prompt)
                .css_classes(["sub-instruction-text"])
                .build();

            root_box.append(&time_label);
            root_box.append(&prompt_label);
            root_box.append(&sub_prompt_label);

            // Attach Hold to Unlock Friction Widget
            let unlock_cb = on_unlock.clone();
            let hold_duration = Duration::from_millis(self.config.behavior.hold_unlock_duration_ms);
            let hold_btn = HoldToUnlockButton::new(hold_duration, move || {
                unlock_cb();
            });
            root_box.append(hold_btn.widget());

            win.set_child(Some(&root_box));
            win.present();
            created.push(win);

            if monitor_opt.is_none() {
                break;
            }
        }

        *self.windows.borrow_mut() = created;
    }

    pub fn update_countdown(&self, remaining_secs: u32) {
        let text = format!("{:02}:{:02}", remaining_secs / 60, remaining_secs % 60);
        for win in self.windows.borrow().iter() {
            if let Some(root_box) = win.child().and_downcast::<Box>() {
                if let Some(label) = root_box.first_child().and_downcast::<Label>() {
                    label.set_label(&text);
                }
            }
        }
    }

    pub fn dismiss(&self) {
        for win in self.windows.borrow_mut().drain(..) {
            win.close();
        }
    }
}
