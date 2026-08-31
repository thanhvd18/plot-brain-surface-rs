use std::fmt::Write as FmtWrite;

use crate::colorbar::colorbar_ticks;
use crate::mesh::{BrainSurfaces, SurfaceMesh};
use crate::render::{
    face_color_rgb, normalize_projection_pts, RenderConfig, RenderTarget, ViewAngles,
    MATLAB_CANVAS_HEIGHT, MATLAB_CANVAS_WIDTH,
};

/// Render a single surface view to compact SVG `<path>` batches grouped by color.
pub fn render_surface_view_svg(
    mesh: &SurfaceMesh,
    vertex_values: &[f64],
    data_labels: &[i32],
    border_labels: &[i32],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    view: ViewAngles,
    _config: &RenderConfig,
    target: RenderTarget,
) -> String {
    let projected = normalize_projection_pts(mesh, view, target);

    // Sort faces from back to front (Painter's algorithm: largest depth = farthest = render first).
    let mut faces: Vec<(f32, usize)> = (0..mesh.faces.len())
        .map(|idx| {
            let f = mesh.faces[idx];
            let avg_z = (projected[f[0] as usize][2]
                + projected[f[1] as usize][2]
                + projected[f[2] as usize][2])
                / 3.0;
            (avg_z, idx)
        })
        .collect();
    faces.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = String::with_capacity(faces.len() * 28);
    let mut current_color: Option<[u8; 3]> = None;
    let mut current_path = String::new();

    let flush_path = |out: &mut String, color: [u8; 3], path: &mut String| {
        if path.is_empty() {
            return;
        }
        let _ = write!(
            out,
            "<path fill=\"#{:02x}{:02x}{:02x}\" d=\"{}\"/>",
            color[0],
            color[1],
            color[2],
            path
        );
        path.clear();
    };

    for (_, face_idx) in faces {
        let face = mesh.faces[face_idx];
        let p0 = projected[face[0] as usize];
        let p1 = projected[face[1] as usize];
        let p2 = projected[face[2] as usize];

        // 2D screen cross-product for backface culling.
        let cross = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p1[1] - p0[1]) * (p2[0] - p0[0]);
        if cross <= 0.0 {
            continue;
        }

        let border_face_labels = [
            border_labels[face[0] as usize],
            border_labels[face[1] as usize],
            border_labels[face[2] as usize],
        ];
        let data_face_labels = [
            data_labels[face[0] as usize],
            data_labels[face[1] as usize],
            data_labels[face[2] as usize],
        ];
        let color = face_color_rgb(
            border_face_labels,
            data_face_labels,
            vertex_values,
            face,
            cmap,
            cmin,
            cmax,
        );
        let color_bytes = rgb_to_bytes(color);

        if current_color != Some(color_bytes) {
            if let Some(prev_color) = current_color {
                flush_path(&mut out, prev_color, &mut current_path);
            }
            current_color = Some(color_bytes);
        }

        append_triangle_path(&mut current_path, p0, p1, p2);
    }

    if let Some(color) = current_color {
        flush_path(&mut out, color, &mut current_path);
    }

    out
}

/// Render full composite 5-panel brain map + colorbar as a complete, standalone SVG document.
pub fn render_composite_svg(
    surfaces: &BrainSurfaces,
    vertex_data: &crate::weights::BrainVertexData,
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    show_colorbar: bool,
) -> String {
    let config = RenderConfig::default();
    let both_data_labels: Vec<i32> = vertex_data
        .left
        .vertex_labels
        .iter()
        .chain(vertex_data.right.vertex_labels.iter())
        .copied()
        .collect();
    let both_border_labels: Vec<i32> = vertex_data
        .left
        .border_labels()
        .iter()
        .chain(vertex_data.right.border_labels().iter())
        .copied()
        .collect();

    let mut svg = String::with_capacity(8 * 1024 * 1024);
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" style=\"background:#ffffff\">",
        MATLAB_CANVAS_WIDTH, MATLAB_CANVAS_HEIGHT, MATLAB_CANVAS_WIDTH, MATLAB_CANVAS_HEIGHT
    );

    if show_colorbar {
        svg.push_str("<defs>");
        svg.push_str(&generate_colorbar_gradient_def("cb_grad", cmap));
        svg.push_str("</defs>");
    }

    // 1. LH Lateral
    svg.push_str(&render_surface_view_svg(
        &surfaces.left,
        &vertex_data.left.vertex_values,
        &vertex_data.left.vertex_labels,
        vertex_data.left.border_labels(),
        cmap,
        cmin,
        cmax,
        ViewAngles::new(-90.0, 0.0, false),
        &config,
        RenderTarget {
            height_px: 392.0,
            center: [352.4, 256.3],
        },
    ));

    // 2. LH Medial
    svg.push_str(&render_surface_view_svg(
        &surfaces.left,
        &vertex_data.left.vertex_values,
        &vertex_data.left.vertex_labels,
        vertex_data.left.border_labels(),
        cmap,
        cmin,
        cmax,
        ViewAngles::new(90.0, 0.0, false),
        &config,
        RenderTarget {
            height_px: 392.0,
            center: [352.4, 752.8],
        },
    ));

    // 3. RH Lateral (view -90 deg, placed bottom-right)
    svg.push_str(&render_surface_view_svg(
        &surfaces.right,
        &vertex_data.right.vertex_values,
        &vertex_data.right.vertex_labels,
        vertex_data.right.border_labels(),
        cmap,
        cmin,
        cmax,
        ViewAngles::new(-90.0, 0.0, false),
        &config,
        RenderTarget {
            height_px: 396.0,
            center: [1352.0, 752.8],
        },
    ));

    // 4. RH Medial (view +90 deg, placed top-right)
    svg.push_str(&render_surface_view_svg(
        &surfaces.right,
        &vertex_data.right.vertex_values,
        &vertex_data.right.vertex_labels,
        vertex_data.right.border_labels(),
        cmap,
        cmin,
        cmax,
        ViewAngles::new(90.0, 0.0, false),
        &config,
        RenderTarget {
            height_px: 396.0,
            center: [1352.0, 256.3],
        },
    ));

    // 5. Both dorsal center
    svg.push_str(&render_surface_view_svg(
        &surfaces.both,
        &vertex_data.both,
        &both_data_labels,
        &both_border_labels,
        cmap,
        cmin,
        cmax,
        ViewAngles::new(0.0, 90.0, false),
        &config,
        RenderTarget {
            height_px: 510.0,
            center: [851.9, 553.4],
        },
    ));

    if show_colorbar {
        svg.push_str(&render_colorbar_svg(
            cmin,
            cmax,
            (MATLAB_CANVAS_WIDTH as f32 * 0.914411586051746) as u32,
            (MATLAB_CANVAS_HEIGHT as f32 * (1.0 - 0.389458012533572 - 0.291629340901834)) as u32,
            (MATLAB_CANVAS_WIDTH as f32 * 0.020059992500933) as u32,
            (MATLAB_CANVAS_HEIGHT as f32 * 0.291629340901834) as u32,
        ));
    }

    svg.push_str("</svg>");
    svg
}

/// Render a single standalone panel SVG.
pub fn render_single_panel_svg(
    mesh: &SurfaceMesh,
    vertex_values: &[f64],
    data_labels: &[i32],
    border_labels: &[i32],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    view: ViewAngles,
) -> String {
    let config = RenderConfig::default();
    let target = RenderTarget::fitted(&config);

    let mut svg = String::with_capacity(4 * 1024 * 1024);
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" style=\"background:#ffffff\">",
        MATLAB_CANVAS_WIDTH, MATLAB_CANVAS_HEIGHT, MATLAB_CANVAS_WIDTH, MATLAB_CANVAS_HEIGHT
    );

    svg.push_str(&render_surface_view_svg(
        mesh,
        vertex_values,
        data_labels,
        border_labels,
        cmap,
        cmin,
        cmax,
        view,
        &config,
        target,
    ));

    svg.push_str("</svg>");
    svg
}

fn append_triangle_path(path: &mut String, p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) {
    let _ = write!(
        path,
        "M{},{}L{},{}L{},{}Z",
        p0[0].round() as i32,
        p0[1].round() as i32,
        p1[0].round() as i32,
        p1[1].round() as i32,
        p2[0].round() as i32,
        p2[1].round() as i32,
    );
}

fn rgb_to_bytes(rgb: [f32; 3]) -> [u8; 3] {
    [
        (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn generate_colorbar_gradient_def(id: &str, cmap: &[[f32; 3]]) -> String {
    let mut out = format!("<linearGradient id=\"{}\" x1=\"0%\" y1=\"0%\" x2=\"0%\" y2=\"100%\">", id);
    let n = cmap.len();
    for (i, c) in cmap.iter().rev().enumerate() {
        let offset = (i as f32 / (n.saturating_sub(1).max(1)) as f32) * 100.0;
        let rgb = rgb_to_bytes(*c);
        let _ = write!(
            out,
            "<stop offset=\"{:.1}%\" stop-color=\"#{:02x}{:02x}{:02x}\"/>",
            offset, rgb[0], rgb[1], rgb[2]
        );
    }
    out.push_str("</linearGradient>");
    out
}

fn render_colorbar_svg(
    cmin: f64,
    cmax: f64,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> String {
    let mut out = String::with_capacity(1024);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#cb_grad)\" stroke=\"#000\" stroke-width=\"1\"/>",
        x, y, w, h
    );

    let font_size = (h as f32 * 0.045).clamp(12.0, 22.0);
    let ticks = colorbar_ticks(cmin, cmax);

    for tick in ticks {
        let frac = if (cmax - cmin).abs() < f64::EPSILON {
            0.5
        } else {
            (tick - cmin) / (cmax - cmin)
        };
        let tick_y = y as f32 + ((1.0 - frac) * (h as f64)) as f32;
        let label = format_tick_val(tick);

        let _ = write!(
            out,
            "<line x1=\"{}\" y1=\"{:.1}\" x2=\"{}\" y2=\"{:.1}\" stroke=\"#000\" stroke-width=\"1\"/>",
            x + w - 1,
            tick_y,
            x + w + 3,
            tick_y
        );

        let text_x = x + w + 6;
        let text_y = tick_y + font_size * 0.35;
        let _ = write!(
            out,
            "<text x=\"{}\" y=\"{:.1}\" font-family=\"Arial,sans-serif\" font-size=\"{:.1}\" fill=\"#000\">{}</text>",
            text_x, text_y, font_size, label
        );
    }

    out
}

fn format_tick_val(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-6 {
        format!("{}", rounded as i64)
    } else if (value * 10.0 - (value * 10.0).round()).abs() < 1e-6 {
        format!("{:.1}", value)
    } else {
        format!("{:.2}", value)
    }
}
