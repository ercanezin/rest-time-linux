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

const HEALTH_QUOTES: &[&str] = &[
    "“Frequent micro-movements reset posture, restore circulation, and protect long-term spine vitality.”\n— Dr. Kelly Starrett",
    "“Motion is lotion for your joints. Small daily stretches build strength and longevity.”\n— Dr. Stuart McGill",
    "“Movement is medicine. Brief breaks restore mental clarity, circulation, and spinal health.”\n— Dr. Joan Vernikos, NASA Life Sciences",
    "“Your best posture is your next posture. Move regularly to stay energized and pain-free.”\n— Prof. Alan Hedge, Cornell Ergonomics",
    "“A few minutes of movement reverses hours of stiffness and builds lifelong mobility.”\n— Dr. James Levine, Mayo Clinic",
];

pub struct BreakOverlayManager {
    windows: Rc<RefCell<Vec<Window>>>,
    timer_labels: Rc<RefCell<Vec<Label>>>,
    app: Application,
    config: Config,
    last_yoga_index: Cell<Option<usize>>,
}

impl BreakOverlayManager {
    pub fn new(app: &Application, config: Config) -> Self {
        Self::apply_theme(&config);
        Self {
            windows: Rc::new(RefCell::new(Vec::new())),
            timer_labels: Rc::new(RefCell::new(Vec::new())),
            app: app.clone(),
            config,
            last_yoga_index: Cell::new(None),
        }
    }

    fn apply_theme(cfg: &Config) {
        let provider = CssProvider::new();
        let css = format!(
            "
            .break-surface {{
                background-color: {};
            }}
            .time-display {{
                font-size: 64px;
                font-weight: 800;
                color: #FFFFFF;
                font-family: 'JetBrains Mono', 'Fira Code', monospace;
                margin-bottom: 6px;
            }}
            .quote-display {{
                font-size: 15px;
                font-weight: 500;
                font-style: italic;
                color: #94A3B8;
                margin-bottom: 22px;
            }}
            .yoga-picture {{
                border-radius: 18px;
                margin-bottom: 24px;
            }}
            ",
            cfg.ui.background_color,
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
        let mut labels = Vec::new();

        // 🎲 Randomly pick a new yoga movement and inspiring quote (guaranteeing different from previous)
        let total_poses = YOGA_IMAGES.len();
        let chosen_idx = match self.last_yoga_index.get() {
            Some(prev) if total_poses > 1 => {
                let offset = 1 + fastrand::usize(..(total_poses - 1));
                (prev + offset) % total_poses
            }
            _ => fastrand::usize(..total_poses),
        };
        self.last_yoga_index.set(Some(chosen_idx));

        let yoga_data = YOGA_IMAGES[chosen_idx];
        let quote = HEALTH_QUOTES[chosen_idx % HEALTH_QUOTES.len()];

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

            let total_secs = duration.as_secs();
            let timer_label = Label::builder()
                .label(&format!("{:02}:{:02}", total_secs / 60, total_secs % 60))
                .css_classes(["time-display"])
                .build();
            labels.push(timer_label.clone());
            root.append(&timer_label);

            let quote_label = Label::builder()
                .label(quote)
                .justify(gtk4::Justification::Center)
                .css_classes(["quote-display"])
                .build();
            root.append(&quote_label);

            // Embedded Chair Yoga Movement Image
            let bytes = glib::Bytes::from_static(yoga_data);
            let texture = gtk4::gdk::Texture::from_bytes(&bytes).expect("Failed to decode yoga image");
            let picture = Picture::for_paintable(&texture);
            picture.set_content_fit(gtk4::ContentFit::Contain);
            picture.set_can_shrink(true);
            picture.set_size_request(480, 400);
            picture.add_css_class("yoga-picture");
            root.append(&picture);

            // Hold-to-Unlock Gesture Area
            if self.config.behavior.strict_hold_to_unlock {
                let hold_secs = (self.config.behavior.hold_unlock_duration_ms as f64) / 1000.0;
                let drawing_area = DrawingArea::builder()
                    .content_width(320)
                    .content_height(48)
                    .build();

                let progress = Rc::new(Cell::new(0.0f64));
                let press_start = Rc::new(RefCell::new(None::<Instant>));
                let is_pressed = Rc::new(Cell::new(false));

                let p_draw = progress.clone();
                drawing_area.set_draw_func(move |_, cr, width, height| {
                    let w = width as f64;
                    let h = height as f64;
                    let prog = p_draw.get();

                    let radius = h / 2.0;
                    cr.new_sub_path();
                    cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                    cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
                    cr.close_path();

                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
                    let _ = cr.fill_preserve();
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.15);
                    cr.set_line_width(1.5);
                    let _ = cr.stroke();

                    if prog > 0.0 {
                        let fill_w = (w * prog).max(h);
                        cr.new_sub_path();
                        cr.arc(fill_w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                        cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
                        cr.close_path();
                        cr.set_source_rgba(0.2, 0.7, 0.9, 0.65);
                        let _ = cr.fill();
                    }

                    cr.set_source_rgb(1.0, 1.0, 1.0);
                    cr.set_font_size(14.0);
                    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                    let text = if prog >= 1.0 { "Unlocked!" } else { "Hold to Unlock" };
                    if let Ok(ext) = cr.text_extents(text) {
                        cr.move_to((w - ext.width()) / 2.0, (h + ext.height()) / 2.0 - 1.0);
                        let _ = cr.show_text(text);
                    }
                });

                let gesture = GestureClick::new();
                let p_press = press_start.clone();
                let is_p = is_pressed.clone();
                let da_clone = drawing_area.clone();
                let on_unlock_cb = on_unlock.clone();

                gesture.connect_pressed(move |_, _, _, _| {
                    *p_press.borrow_mut() = Some(Instant::now());
                    is_p.set(true);

                    let p_tick = p_press.clone();
                    let is_p_tick = is_p.clone();
                    let da_tick = da_clone.clone();
                    let prog_tick = progress.clone();
                    let unlock_cb = on_unlock_cb.clone();

                    glib::timeout_add_local(Duration::from_millis(16), move || {
                        if !is_p_tick.get() {
                            prog_tick.set(0.0);
                            da_tick.queue_draw();
                            return glib::ControlFlow::Break;
                        }

                        if let Some(start) = *p_tick.borrow() {
                            let elapsed = start.elapsed().as_secs_f64();
                            let frac = (elapsed / hold_secs).min(1.0);
                            prog_tick.set(frac);
                            da_tick.queue_draw();

                            if frac >= 1.0 {
                                unlock_cb();
                                return glib::ControlFlow::Break;
                            }
                        }

                        glib::ControlFlow::Continue
                    });
                });

                let p_rel = press_start.clone();
                let is_rel = is_pressed.clone();
                let da_rel = drawing_area.clone();
                gesture.connect_released(move |_, _, _, _| {
                    *p_rel.borrow_mut() = None;
                    is_rel.set(false);
                    da_rel.queue_draw();
                });

                drawing_area.add_controller(gesture);
                root.append(&drawing_area);
            }

            win.set_child(Some(&root));
            win.present();
            active_wins.push(win);
        }

        *self.timer_labels.borrow_mut() = labels;
        *self.windows.borrow_mut() = active_wins;
    }

    pub fn update_countdown(&self, remaining_secs: u32) {
        let mins = remaining_secs / 60;
        let secs = remaining_secs % 60;
        let text = format!("{:02}:{:02}", mins, secs);

        for label in self.timer_labels.borrow().iter() {
            label.set_text(&text);
        }
    }

    pub fn dismiss(&self) {
        self.timer_labels.borrow_mut().clear();
        for win in self.windows.borrow_mut().drain(..) {
            win.close();
        }
    }
}
