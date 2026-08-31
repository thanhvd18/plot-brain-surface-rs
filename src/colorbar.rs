use ab_glyph::{Font, FontRef, Glyph, PxScale, ScaleFont, point};
use image::{ImageBuffer, Rgba, RgbaImage};

use crate::colormap::sample_colormap;

const FONT_DATA: &[u8] = include_bytes!("../assets/Arial.ttf");

pub fn render_colorbar(
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    bar_w: u32,
    bar_h: u32,
) -> RgbaImage {
    let label_w = (bar_h as f32 * 0.22).round().max(48.0) as u32;
    let total_w = bar_w + label_w;
    let mut image = ImageBuffer::from_pixel(total_w, bar_h, Rgba([255, 255, 255, 255]));

    for y in 0..bar_h {
        let t = 1.0 - (y as f64 / (bar_h.saturating_sub(1).max(1) as f64));
        let value = cmin + t * (cmax - cmin);
        let color = sample_colormap(cmap, value, cmin, cmax);
        let pixel = rgba_from_rgb(color, 255);
        for x in 0..bar_w {
            image.put_pixel(x, y, pixel);
        }
    }

    let font = FontRef::try_from_slice(FONT_DATA).expect("embedded Arial font");
    let font_size = (bar_h as f32 * 0.045).clamp(12.0, 22.0);
    let tick_color = Rgba([0, 0, 0, 255]);

    for tick in colorbar_ticks(cmin, cmax) {
        let frac = if (cmax - cmin).abs() < f64::EPSILON {
            0.5
        } else {
            (tick - cmin) / (cmax - cmin)
        };
        let y = ((1.0 - frac) * (bar_h.saturating_sub(1) as f64)).round() as i32;
        let y = y.clamp(0, bar_h as i32 - 1) as u32;

        for dx in 0..4 {
            let x = bar_w.saturating_sub(1) + dx;
            if x < total_w {
                image.put_pixel(x, y, tick_color);
            }
        }

        let label = format_tick(tick);
        let text_x = bar_w as f32 + 4.0;
        let text_y = y as f32 + font_size * 0.75;
        draw_text(&mut image, text_x, text_y, &label, &font, font_size, tick_color);
    }

    image
}

pub fn colorbar_ticks(cmin: f64, cmax: f64) -> Vec<f64> {
    if !cmin.is_finite() || !cmax.is_finite() {
        return vec![0.0];
    }
    if (cmax - cmin).abs() < f64::EPSILON {
        return vec![cmin];
    }

    let step = nice_tick_step((cmax - cmin) / 5.0);
    let mut ticks = Vec::new();
    let mut value = (cmin / step).ceil() * step;
    while value <= cmax + step * 0.25 {
        if value >= cmin - step * 0.25 {
            ticks.push(normalize_tick(value, step));
        }
        value += step;
    }
    if ticks.is_empty() {
        ticks.push(cmin);
        ticks.push(cmax);
    }
    ticks
}

fn nice_tick_step(raw: f64) -> f64 {
    if raw <= 0.0 || !raw.is_finite() {
        return 1.0;
    }
    let exponent = raw.log10().floor();
    let fraction = raw / 10f64.powf(exponent);
    let nice_fraction = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else if fraction <= 7.5 {
        5.0
    } else {
        10.0
    };
    nice_fraction * 10f64.powf(exponent)
}

fn normalize_tick(value: f64, step: f64) -> f64 {
    if step >= 1.0 {
        value.round()
    } else if step >= 0.1 {
        (value * 10.0).round() / 10.0
    } else {
        (value * 100.0).round() / 100.0
    }
}

fn format_tick(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-6 {
        format!("{}", rounded as i64)
    } else if (value * 10.0 - (value * 10.0).round()).abs() < 1e-6 {
        format!("{:.1}", value)
    } else {
        format!("{:.2}", value)
    }
}

fn draw_text(
    image: &mut RgbaImage,
    x: f32,
    y: f32,
    text: &str,
    font: &FontRef<'_>,
    size: f32,
    color: Rgba<u8>,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut cursor_x = x;
    let mut prev_id = None;

    for ch in text.chars() {
        let glyph_id = font.glyph_id(ch);
        if let Some(prev) = prev_id {
            cursor_x += scaled.kern(prev, glyph_id);
        }
        let glyph: Glyph = glyph_id.with_scale_and_position(size, point(cursor_x, y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 {
                    return;
                }
                let px = px as u32;
                let py = py as u32;
                if px >= image.width() || py >= image.height() {
                    return;
                }
                let alpha = (coverage * color[3] as f32) as u8;
                if alpha == 0 {
                    return;
                }
                let pixel = image.get_pixel_mut(px, py);
                blend_pixel(pixel, color, alpha);
            });
        }
        cursor_x += scaled.h_advance(glyph_id);
        prev_id = Some(glyph_id);
    }
}

fn blend_pixel(dst: &mut Rgba<u8>, color: Rgba<u8>, alpha: u8) {
    let a = alpha as f32 / 255.0;
    for i in 0..3 {
        dst[i] = (color[i] as f32 * a + dst[i] as f32 * (1.0 - a)).round() as u8;
    }
}

fn rgba_from_rgb(rgb: [f32; 3], alpha: u8) -> Rgba<u8> {
    Rgba([
        (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
        alpha,
    ])
}

#[cfg(test)]
mod tests {
    use super::colorbar_ticks;

    #[test]
    fn jet_ticks_match_matlab() {
        let ticks = colorbar_ticks(1.5, 3.5);
        assert_eq!(ticks, vec![1.5, 2.0, 2.5, 3.0, 3.5]);
    }

    #[test]
    fn diverging_ticks_are_reasonable() {
        let ticks = colorbar_ticks(-18.9, 15.67);
        assert!(ticks.len() >= 5);
        assert!(ticks.iter().any(|v| (*v - 0.0).abs() < 1e-6));
        assert!(ticks.contains(&-15.0) || ticks.contains(&-10.0));
    }
}
