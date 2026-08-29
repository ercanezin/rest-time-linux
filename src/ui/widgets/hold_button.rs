use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureClick};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub struct HoldToUnlockButton {
    area: DrawingArea,
    hold_start: Rc<Cell<Option<Instant>>>,
    required_hold: Duration,
    progress: Rc<Cell<f64>>,
}

impl HoldToUnlockButton {
    pub fn new(required_hold: Duration, on_unlocked: impl Fn() + 'static) -> Self {
        let area = DrawingArea::builder()
            .content_width(280)
            .content_height(56)
            .css_classes(["hold-unlock-canvas"])
            .build();

        let hold_start = Rc::new(Cell::new(None));
        let progress = Rc::new(Cell::new(0.0));
        let on_unlocked = Rc::new(on_unlocked);

        let p_clone = progress.clone();
        area.set_draw_func(move |_, cr, width, height| {
            let p = p_clone.get();
            let w = width as f64;
            let h = height as f64;
            let radius = h / 2.0;

            // Draw Background Capsule
            cr.set_source_rgba(0.88, 0.42, 0.46, 0.15);
            cr.new_sub_path();
            cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
            cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
            cr.close_path();
            let _ = cr.fill();

            // Draw Fill Progress Capsule
            if p > 0.0 {
                cr.set_source_rgba(0.88, 0.42, 0.46, 0.55);
                cr.new_sub_path();
                let fill_w = (w * p).max(radius * 2.0);
                cr.arc((fill_w - radius).min(w - radius), radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
                cr.close_path();
                let _ = cr.fill();
            }

            // Draw Centered Text
            cr.set_source_rgb(0.95, 0.95, 0.95);
            cr.set_font_size(15.0);
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let label = if p > 0.0 { "Keep Holding..." } else { "Hold to Unlock" };
            if let Ok(ext) = cr.text_extents(label) {
                cr.move_to((w - ext.width()) / 2.0, (h + ext.height()) / 2.0);
                let _ = cr.show_text(label);
            }
        });

        // Click Gesture Handling
        let gesture = GestureClick::new();
        let hs_press = hold_start.clone();
        let area_ref = area.clone();
        let p_ref = progress.clone();
        let unl = on_unlocked.clone();

        gesture.connect_pressed(move |_, _, _, _| {
            hs_press.set(Some(Instant::now()));
            let area_tick = area_ref.clone();
            let hs_tick = hs_press.clone();
            let p_tick = p_ref.clone();
            let unl_cb = unl.clone();

            glib::timeout_add_local(Duration::from_millis(16), move || {
                if let Some(start) = hs_tick.get() {
                    let elapsed = start.elapsed();
                    let ratio = (elapsed.as_secs_f64() / required_hold.as_secs_f64()).min(1.0);
                    p_tick.set(ratio);
                    area_tick.queue_draw();

                    if ratio >= 1.0 {
                        hs_tick.set(None);
                        p_tick.set(0.0);
                        area_tick.queue_draw();
                        unl_cb();
                        return glib::ControlFlow::Break;
                    }
                    glib::ControlFlow::Continue
                } else {
                    p_tick.set(0.0);
                    area_tick.queue_draw();
                    glib::ControlFlow::Break
                }
            });
        });

        let hs_release = hold_start.clone();
        let area_release = area.clone();
        let p_release = progress.clone();

        gesture.connect_released(move |_, _, _, _| {
            hs_release.set(None);
            p_release.set(0.0);
            area_release.queue_draw();
        });

        area.add_controller(gesture);

        Self {
            area,
            hold_start,
            required_hold,
            progress,
        }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }
}
