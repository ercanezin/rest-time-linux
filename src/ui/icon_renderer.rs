use cairo::{Context, Format, ImageSurface};
use ksni::Icon;

pub fn render_timer_icon(text: &str, is_break: bool, is_paused: bool) -> Icon {
    let width = 128;
    let height = 36;
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).unwrap();
    let cr = Context::new(&surface).unwrap();

    // Background badge pill
    let radius = 18.0;
    let w = width as f64;
    let h = height as f64;

    cr.new_sub_path();
    cr.arc(w - radius, radius, radius, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    cr.arc(radius, radius, radius, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();

    if is_paused {
        cr.set_source_rgba(0.95, 0.6, 0.2, 0.9); // Amber / Orange
    } else if is_break {
        cr.set_source_rgba(0.2, 0.7, 0.95, 0.9); // Cyan / Blue
    } else {
        cr.set_source_rgba(0.2, 0.75, 0.45, 0.9); // Emerald Green
    }
    let _ = cr.fill();

    // Text rendering
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_font_size(20.0);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);

    if let Ok(ext) = cr.text_extents(text) {
        cr.move_to((w - ext.width()) / 2.0, (h + ext.height()) / 2.0 - 2.0);
        let _ = cr.show_text(text);
    }

    drop(cr);
    let data_ref = surface.data().unwrap();
    let len = (width * height * 4) as usize;
    let mut argb_data = Vec::with_capacity(len);

    // Cairo ARgb32 is native endian (BGRA on Little Endian x86_64).
    // KSNI/D-Bus spec expects network byte order (ARGB: Byte 0=A, Byte 1=R, Byte 2=G, Byte 3=B).
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
