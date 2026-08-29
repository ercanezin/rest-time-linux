use gtk4::prelude::*;
use gtk4::DrawingArea;
use std::cell::Cell;
use std::rc::Rc;

pub struct CircularProgress {
    area: DrawingArea,
    progress: Rc<Cell<f64>>,
}

impl CircularProgress {
    pub fn new(size: i32, stroke_width: f64) -> Self {
        let area = DrawingArea::builder()
            .content_width(size)
            .content_height(size)
            .build();

        let progress = Rc::new(Cell::new(0.0f64));
        let p_clone = progress.clone();

        area.set_draw_func(move |_, cr, width, height| {
            let p = p_clone.get().clamp(0.0f64, 1.0f64);
            let w = width as f64;
            let h = height as f64;
            let center_x = w / 2.0;
            let center_y = h / 2.0;
            let radius = (w.min(h) - stroke_width) / 2.0;

            // Background Track
            cr.set_line_width(stroke_width);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.1);
            cr.arc(center_x, center_y, radius, 0.0, 2.0 * std::f64::consts::PI);
            let _ = cr.stroke();

            // Progress Arc
            if p > 0.0 {
                cr.set_line_width(stroke_width);
                cr.set_source_rgba(0.9, 0.75, 0.48, 0.9);
                let start_angle = -std::f64::consts::FRAC_PI_2;
                let end_angle = start_angle + (2.0 * std::f64::consts::PI * p);
                cr.arc(center_x, center_y, radius, start_angle, end_angle);
                let _ = cr.stroke();
            }
        });

        Self { area, progress }
    }

    pub fn set_progress(&self, val: f64) {
        self.progress.set(val);
        self.area.queue_draw();
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }
}
