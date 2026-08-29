use cairo::{Context, Format, ImageSurface};
use ksni::Icon;

pub fn render_timer_icon(text: &str, is_break: bool, is_paused: bool) -> Icon {
    // 4x Wide aspect ratio: 256px width x 64px height (4:1) for large, ultra-legible numbers
    let width = 256;
    let height = 64;
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).unwrap();
    let cr = Context::new(&surface).unwrap();

    let w = width as f64;
    let h = height as f64;
    let center_x = w / 2.0;
    let center_y = h / 2.0;
    let radius = h / 2.0;

    // 1. Pill-shaped badge path (4x wide)
    cr.new_sub_path();
    cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();

    // 2. High-contrast solid vibrant background
    if is_paused || text == "PAUSED" {
        cr.set_source_rgba(0.92, 0.52, 0.05, 0.98); // Vibrant Amber / Orange
    } else if is_break {
        cr.set_source_rgba(0.06, 0.58, 0.88, 0.98); // Vibrant Cyan / Blue
    } else {
        cr.set_source_rgba(0.08, 0.68, 0.38, 0.98); // Vibrant Emerald Green
    }
    let _ = cr.fill_preserve();

    // 3. Crisp contrasting outline
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    cr.set_line_width(3.0);
    let _ = cr.stroke();

    // 4. Large, Bold, Crystal-Clear Typography
    cr.set_source_rgb(1.0, 1.0, 1.0);
    if text == "PAUSED" {
        cr.set_font_size(36.0);
    } else {
        cr.set_font_size(44.0);
    }
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    if let Ok(ext) = cr.text_extents(text) {
        cr.move_to(center_x - (ext.width() / 2.0) - ext.x_bearing(), center_y - (ext.height() / 2.0) - ext.y_bearing());
        let _ = cr.show_text(text);
    }

    drop(cr);
    let data_ref = surface.data().unwrap();
    let len = (width * height * 4) as usize;
    let mut argb_data = Vec::with_capacity(len);

    for chunk in data_ref.chunks_exact(4) {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        argb_data.push(a);
        argb_data.push(r);
        argb_data.push(g);
        argb_data.push(b);
    }

    Icon {
        width,
        height,
        data: argb_data,
    }
}
