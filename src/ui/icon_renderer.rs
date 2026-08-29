use cairo::{Context, Format, ImageSurface};
use ksni::Icon;

pub fn render_timer_icon(text: &str, is_break: bool, is_paused: bool) -> Icon {
    let width = 84;
    let height = 24;
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).unwrap();
    let cr = Context::new(&surface).unwrap();

    let radius = 12.0;
    let w = width as f64;
    let h = height as f64;

    cr.new_sub_path();
    cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();

    if is_paused {
        cr.set_source_rgba(0.9, 0.55, 0.1, 0.95);
    } else if is_break {
        cr.set_source_rgba(0.1, 0.65, 0.95, 0.95);
    } else {
        cr.set_source_rgba(0.15, 0.75, 0.4, 0.95);
    }
    let _ = cr.fill();

    // Subtle Outline
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
    cr.set_line_width(1.0);
    cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
    let _ = cr.stroke();

    // Crisp Text Centering
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_font_size(14.0);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    if let Ok(ext) = cr.text_extents(text) {
        cr.move_to((w - ext.width()) / 2.0, (h + ext.height()) / 2.0 - 1.0);
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
