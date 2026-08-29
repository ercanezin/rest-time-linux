use gtk4::prelude::*;
use gtk4::{Application, Box, CssProvider, DrawingArea, GestureClick, Label, Orientation, Picture, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};
use crate::config::Config;

const CHAIR_YOGA_1: &[u8] = include_bytes!("../../resources/chair_yoga/chair_yoga1.jpeg");
const CHAIR_YOGA_2: &[u8] = include_bytes!("../../resources/chair_yoga/chair_yoga2.jpeg");
const CHAIR_YOGA_3: &[u8] = include_bytes!("../../resources/chair_yoga/chair_yoga3.jpeg");
const CHAIR_YOGA_4: &[u8] = include_bytes!("../../resources/chair_yoga/chair_yoga4.jpeg");
const CHAIR_YOGA_5: &[u8] = include_bytes!("../../resources/chair_yoga/chair_yoga5.jpeg");

const YOGA_IMAGES: &[&[u8]] = &[
    CHAIR_YOGA_1,
    CHAIR_YOGA_2,
    CHAIR_YOGA_3,
    CHAIR_YOGA_4,
    CHAIR_YOGA_5,
];

pub struct BreakOverlayManager {
    windows: Rc<RefCell<Vec<Window>>>,
    app: Application,
    config: Config,
    yoga_index: Cell<usize>,
}

impl BreakOverlayManager {
    pub fn new(app: &Application, config: Config) -> Self {
        Self::apply_theme(&config);
        Self {
            windows: Rc::new(RefCell::new(Vec::new())),
            app: app.clone(),
            config,
            yoga_index: Cell::new(0),
        }
    }

    fn apply_theme(cfg: &Config) {
        let provider = CssProvider::new();
        let css = format!(
            "
            .break-surface {{
                background-color: {};
            }}
            .break-header {{
                font-size: 28px;
                font-weight: 800;
                color: {};
                margin-bottom: 2px;
            }}
            .time-display {{
                font-size: 64px;
                font-weight: 800;
                color: #FFFFFF;
                font-family: 'JetBrains Mono', 'Fira Code', monospace;
                margin-bottom: 4px;
            }}
            .yoga-instructions {{
                font-size: 17px;
                font-weight: 500;
                color: {};
                margin-bottom: 16px;
            }}
            .yoga-picture {{
                border-radius: 18px;
                margin-bottom: 24px;
            }}
            ",
            cfg.ui.background_color,
            cfg.ui.accent_color,
            cfg.ui.text_color
        );
        provider.load_from_string(&css);

        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    pub fn spawn_overlays(&self, duration: Duration, on_unlock: impl Fn() + 'static + Clone) {
        self.dismiss();

        let display = match gtk4::gdk::Display::default() {
            Some(d) => d,
            None => return,
        };

        let monitors = display.monitors();
        let mut active_wins = Vec::new();

        let idx = self.yoga_index.get();
        self.yoga_index.set((idx + 1) % YOGA_IMAGES.len());
        let yoga_data = YOGA_IMAGES[idx];
        let pose_num = idx + 1;

        let layer_shell_supported = gtk4_layer_shell::is_supported();
        let n_monitors = monitors.n_items().max(1);

        for i in 0..n_monitors {
            let monitor_opt = monitors.item(i).and_downcast::<gtk4::gdk::Monitor>();

            let win = Window::builder()
                .application(&self.app)
                .title("Rest Time Break")
                .css_classes(["break-surface"])
                .build();

            if layer_shell_supported {
                win.init_layer_shell();
                win.set_layer(Layer::Overlay);
                win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);
                if let Some(ref monitor) = monitor_opt {
                    win.set_monitor(monitor);
                }

                win.set_anchor(Edge::Top, true);
                win.set_anchor(Edge::Bottom, true);
                win.set_anchor(Edge::Left, true);
                win.set_anchor(Edge::Right, true);
                win.set_exclusive_zone(-1);
            } else {
                win.set_decorated(false);
                win.fullscreen();
            }

            let root = Box::new(Orientation::Vertical, 0);
            root.set_valign(gtk4::Align::Center);
            root.set_halign(gtk4::Align::Center);

            let header_label = Label::builder()
                .label(&format!("🧘 Chair Yoga — Movement {} of {}", pose_num, YOGA_IMAGES.len()))
                .css_classes(["break-header"])
                .build();

            let secs = duration.as_secs();
            let time_label = Label::builder()
                .label(&format!("{:02}:{:02}", secs / 60, secs % 60))
                .css_classes(["time-display"])
                .build();

            let prompt_label = Label::builder()
                .label("Follow this gentle movement to relieve tension, stretch, and refresh.")
                .css_classes(["yoga-instructions"])
                .build();

            root.append(&header_label);
            root.append(&time_label);
            root.append(&prompt_label);

            // Yoga Exercise Illustration
            let bytes = glib::Bytes::from_static(yoga_data);
            if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&bytes) {
                let picture = Picture::for_paintable(&texture);
                picture.set_can_shrink(true);
                picture.set_content_fit(gtk4::ContentFit::Contain);
                picture.set_size_request(460, 460);
                picture.add_css_class("yoga-picture");
                root.append(&picture);
            }

            // Hold to Unlock Guilt Barrier
            let hold_area = self.create_hold_button(on_unlock.clone());
            root.append(&hold_area);

            win.set_child(Some(&root));
            win.present();
            active_wins.push(win);

            if monitor_opt.is_none() {
                break;
            }
        }

        *self.windows.borrow_mut() = active_wins;
    }

    fn create_hold_button(&self, on_unlocked: impl Fn() + 'static) -> DrawingArea {
        let area = DrawingArea::builder()
            .content_width(260)
            .content_height(50)
            .build();

        let hold_start = Rc::new(Cell::new(None));
        let progress = Rc::new(Cell::new(0.0f64));
        let required_hold = Duration::from_millis(self.config.behavior.hold_unlock_duration_ms);
        let on_unlocked = Rc::new(on_unlocked);

        let p_draw = progress.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let width = w as f64;
            let height = h as f64;
            let radius = height / 2.0;
            let p = p_draw.get();

            // Background
            cr.set_source_rgba(0.9, 0.4, 0.4, 0.15);
            cr.arc(width - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
            cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
            cr.close_path();
            let _ = cr.fill();

            // Progress Fill
            if p > 0.0 {
                cr.set_source_rgba(0.9, 0.4, 0.4, 0.45);
                let fill_w = (width * p).max(radius * 2.0);
                cr.arc((fill_w - radius).min(width - radius), radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
                cr.close_path();
                let _ = cr.fill();
            }

            // Text
            cr.set_source_rgb(0.95, 0.95, 0.95);
            cr.set_font_size(14.0);
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let text = if p > 0.0 { "Keep Holding..." } else { "Hold to Unlock" };
            if let Ok(ext) = cr.text_extents(text) {
                cr.move_to((width - ext.width()) / 2.0, (height + ext.height()) / 2.0);
                let _ = cr.show_text(text);
            }
        });

        let gesture = GestureClick::new();
        let hs_press = hold_start.clone();
        let area_tick = area.clone();
        let p_tick = progress.clone();
        let unl = on_unlocked.clone();

        gesture.connect_pressed(move |_, _, _, _| {
            hs_press.set(Some(Instant::now()));
            let area_t = area_tick.clone();
            let hs_t = hs_press.clone();
            let p_t = p_tick.clone();
            let unl_cb = unl.clone();

            glib::timeout_add_local(Duration::from_millis(16), move || {
                if let Some(start) = hs_t.get() {
                    let elapsed = start.elapsed();
                    let ratio = (elapsed.as_secs_f64() / required_hold.as_secs_f64()).min(1.0);
                    p_t.set(ratio);
                    area_t.queue_draw();

                    if ratio >= 1.0 {
                        hs_t.set(None);
                        p_t.set(0.0);
                        area_t.queue_draw();
                        unl_cb();
                        return glib::ControlFlow::Break;
                    }
                    glib::ControlFlow::Continue
                } else {
                    p_t.set(0.0);
                    area_t.queue_draw();
                    glib::ControlFlow::Break
                }
            });
        });

        let hs_release = hold_start.clone();
        let area_rel = area.clone();
        let p_rel = progress.clone();

        gesture.connect_released(move |_, _, _, _| {
            hs_release.set(None);
            p_rel.set(0.0);
            area_rel.queue_draw();
        });

        area.add_controller(gesture);
        area
    }

    pub fn update_countdown(&self, remaining_secs: u32) {
        let text = format!("{:02}:{:02}", remaining_secs / 60, remaining_secs % 60);
        for win in self.windows.borrow().iter() {
            if let Some(root) = win.child().and_downcast::<Box>() {
                let child = root.first_child();
                // 1st child is header, 2nd is time_label
                if let Some(first) = child {
                    if let Some(time_label) = first.next_sibling().and_downcast::<Label>() {
                        time_label.set_label(&text);
                    }
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
