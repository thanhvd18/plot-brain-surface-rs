use image::{ImageBuffer, Rgba, RgbaImage};

pub use crate::colorbar::render_colorbar;
use crate::colormap::sample_colormap;
use crate::mesh::SurfaceMesh;

#[derive(Clone, Copy, Debug)]
pub struct ViewAngles {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub mirror_x: bool,
}

impl ViewAngles {
    pub fn new(azimuth_deg: f32, elevation_deg: f32, mirror_x: bool) -> Self {
        Self {
            azimuth_deg,
            elevation_deg,
            mirror_x,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub background: [f32; 3],
    /// When true, scale x/y independently to fill the panel (MATLAB `axis image`).
    pub stretch_to_axes: bool,
    pub padding: f32,
    pub padding_y: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: MATLAB_CANVAS_WIDTH,
            height: MATLAB_CANVAS_HEIGHT,
            background: [1.0, 1.0, 1.0],
            stretch_to_axes: true,
            padding: 0.12,
            padding_y: 0.08,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatlabAxesPlacement {
    pub left: f32,
    pub bottom: f32,
}

impl MatlabAxesPlacement {
    pub const LH_LATERAL: Self = Self {
        left: -0.3097,
        bottom: 0.28,
    };
    pub const LH_MEDIAL: Self = Self {
        left: -0.3097,
        bottom: -0.1462,
    };
    pub const RH_LATERAL: Self = Self {
        left: 0.23,
        bottom: -0.1462,
    };
    pub const RH_MEDIAL: Self = Self {
        left: 0.23,
        bottom: 0.28,
    };
    pub const BOTH_CENTER: Self = Self {
        left: -0.04,
        bottom: 0.025,
    };
}

pub const MATLAB_CANVAS_WIDTH: u32 = 1852;
pub const MATLAB_CANVAS_HEIGHT: u32 = 1165;

/// Where the projected object should land in the canvas: a uniform scale so the
/// projected y-span fills `height_px`, centered at `center` (MATLAB `axis image`).
#[derive(Clone, Copy, Debug)]
pub struct RenderTarget {
    pub height_px: f32,
    pub center: [f32; 2],
}

impl RenderTarget {
    /// Fit the projected object to MATLAB default axes Position `[0.13 0.11 0.775 0.815]`.
    pub fn fitted(config: &RenderConfig) -> Self {
        Self {
            height_px: config.height as f32 * 0.815,
            center: [
                config.width as f32 * 0.5175,
                config.height as f32 * 0.4825,
            ],
        }
    }
}

const UNKNOWN_COLOR: [f32; 3] = [0.85, 0.85, 0.85];
const BOUNDARY_COLOR: [f32; 3] = [0.0, 0.0, 0.0];

pub fn render_surface_view(
    mesh: &SurfaceMesh,
    vertex_values: &[f64],
    vertex_labels: &[i32],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    view: ViewAngles,
    config: &RenderConfig,
) -> RgbaImage {
    render_surface_view_target(
        mesh,
        vertex_values,
        vertex_labels,
        vertex_labels,
        cmap,
        cmin,
        cmax,
        view,
        config,
        RenderTarget::fitted(config),
    )
}

pub fn render_surface_view_target(
    mesh: &SurfaceMesh,
    vertex_values: &[f64],
    data_labels: &[i32],
    border_labels: &[i32],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    view: ViewAngles,
    config: &RenderConfig,
    target: RenderTarget,
) -> RgbaImage {
    let mut image = ImageBuffer::from_pixel(
        config.width,
        config.height,
        rgba_from_rgb(config.background, 255),
    );
    let mut depth = vec![f32::INFINITY; (config.width * config.height) as usize];

    let projected = normalize_projection_pts(mesh, view, target);
    let vertex_normals = vertex_normals_world(mesh);

    let mut faces: Vec<(f32, usize)> = (0..mesh.faces.len())
        .map(|idx| {
            let face = mesh.faces[idx];
            let avg_depth = (projected[face[0] as usize][2]
                + projected[face[1] as usize][2]
                + projected[face[2] as usize][2])
                / 3.0;
            (avg_depth, idx)
        })
        .collect();
    faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (_, face_idx) in faces {
        let face = mesh.faces[face_idx];

        let screen = [
            projected[face[0] as usize],
            projected[face[1] as usize],
            projected[face[2] as usize],
        ];
        if is_degenerate_screen_triangle(screen) {
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
        let base_color = face_color(
            border_face_labels,
            data_face_labels,
            vertex_values,
            face,
            cmap,
            cmin,
            cmax,
        );

        // Gouraud: per-vertex shade (MATLAB material dull + camlights), interpolated.
        let colors = [
            scale_color(base_color, dull_shade(vertex_normals[face[0] as usize], view)),
            scale_color(base_color, dull_shade(vertex_normals[face[1] as usize], view)),
            scale_color(base_color, dull_shade(vertex_normals[face[2] as usize], view)),
        ];

        rasterize_triangle(&mut image, &mut depth, screen, colors);
    }

    image
}

pub fn compose_matlab_figure(
    panel_renders: &[RgbaImage],
    _placements: &[MatlabAxesPlacement],
    colorbar: Option<&RgbaImage>,
) -> RgbaImage {
    let mut canvas = ImageBuffer::from_pixel(
        MATLAB_CANVAS_WIDTH,
        MATLAB_CANVAS_HEIGHT,
        Rgba([255, 255, 255, 255]),
    );

    // MATLAB panels are pre-rendered onto full canvas coordinates and overlaid directly.
    for panel in panel_renders {
        overlay_layer_offset(&mut canvas, panel, 0, 0);
    }

    if let Some(colorbar) = colorbar {
        // MATLAB colorbar strip starts at x=0.914; labels extend to the right.
        let bar_x = (MATLAB_CANVAS_WIDTH as f32 * 0.914411586051746) as u32;
        let bar_top = (MATLAB_CANVAS_HEIGHT as f32 * (1.0 - 0.389458012533572)) as u32;
        let bar_y = bar_top.saturating_sub(colorbar.height());
        blit_stretch(
            &mut canvas,
            colorbar,
            bar_x,
            bar_y,
            colorbar.width(),
            colorbar.height(),
        );
    }

    canvas
}

pub fn compose_panels(
    panels: &[RgbaImage],
    colorbar: &RgbaImage,
    canvas_width: u32,
    canvas_height: u32,
) -> RgbaImage {
    let mut canvas = ImageBuffer::from_pixel(canvas_width, canvas_height, Rgba([255, 255, 255, 255]));

    let panel_w = canvas_width / 6;
    let panel_h = canvas_height / 2;
    let positions = [
        (0_u32, 0_u32),
        (0, panel_h),
        (panel_w * 2, panel_h),
        (panel_w * 2, 0),
        (panel_w, panel_h / 4),
    ];

    for (panel, (x0, y0)) in panels.iter().zip(positions.iter()) {
        blit_fit(&mut canvas, panel, *x0, *y0, panel_w, panel_h);
    }

    let bar_x = panel_w * 5 + panel_w / 4;
    let bar_y = canvas_height / 8;
    blit_stretch(
        &mut canvas,
        colorbar,
        bar_x,
        bar_y,
        panel_w / 2,
        canvas_height * 3 / 4,
    );

    canvas
}

pub fn normalize_projection_pts(
    mesh: &SurfaceMesh,
    view: ViewAngles,
    target: RenderTarget,
) -> Vec<[f32; 3]> {
    let projected: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|vertex| matlab_camera_project(*vertex, view))
        .collect();

    let min_x = projected.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = projected
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = projected.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = projected
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_z = projected.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
    let max_z = projected
        .iter()
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max);

    // MATLAB `axis image`: uniform scale preserving aspect, fitting the projected
    // object into the available area (padded canvas).
    let span_x = (max_x - min_x).max(1e-3);
    let span_y = (max_y - min_y).max(1e-3);
    let avail_w = target.height_px * (MATLAB_CANVAS_WIDTH as f32 / MATLAB_CANVAS_HEIGHT as f32);
    let avail_h = target.height_px;
    let scale = (avail_w / span_x).min(avail_h / span_y);
    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;
    let z_scale = (max_z - min_z).max(1e-3);

    let mirror = if view.mirror_x { -1.0 } else { 1.0 };
    projected
        .into_iter()
        .map(|point| {
            [
                target.center[0] + mirror * (point[0] - cx) * scale,
                target.center[1] - (point[1] - cy) * scale,
                (point[2] - min_z) / z_scale,
            ]
        })
        .collect()
}

fn matlab_camera_project(vertex: [f32; 3], view: ViewAngles) -> [f32; 3] {
    let az = view.azimuth_deg.to_radians();
    let el = view.elevation_deg.to_radians();
    let (sin_az, cos_az) = az.sin_cos();
    let (sin_el, cos_el) = el.sin_cos();

    // MATLAB view(az,el) orthographic basis, verified against reference PNGs:
    // screen X is the horizontal axis, screen Y the vertical, screen Z is depth
    // (larger = farther, so the z-buffer keeps smaller z = closer).
    let xaxis = [cos_az, sin_az, 0.0];
    let yaxis = [-sin_az * sin_el, cos_az * sin_el, cos_el];
    let zaxis = [sin_az * cos_el, -cos_az * cos_el, sin_el];

    [
        dot3(vertex, xaxis),
        dot3(vertex, yaxis),
        -dot3(vertex, zaxis),
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rasterize_triangle(
    image: &mut RgbaImage,
    depth: &mut [f32],
    screen: [[f32; 3]; 3],
    colors: [[f32; 3]; 3],
) {
    let min_x = screen
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_x = screen
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min((image.width() - 1) as f32) as i32;
    let min_y = screen
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_y = screen
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min((image.height() - 1) as f32) as i32;

    let area = edge(screen[0], screen[1], screen[2]);
    if area.abs() < 1e-6 {
        return;
    }

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5, 0.0];
            let w0 = edge(screen[1], screen[2], p) / area;
            let w1 = edge(screen[2], screen[0], p) / area;
            let w2 = edge(screen[0], screen[1], p) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }

            let z = w0 * screen[0][2] + w1 * screen[1][2] + w2 * screen[2][2];
            let idx = (y as u32 * image.width() + x as u32) as usize;
            if z >= depth[idx] {
                continue;
            }
            depth[idx] = z;

            let color = [
                (w0 * colors[0][0] + w1 * colors[1][0] + w2 * colors[2][0]) as f64,
                (w0 * colors[0][1] + w1 * colors[1][1] + w2 * colors[2][1]) as f64,
                (w0 * colors[0][2] + w1 * colors[1][2] + w2 * colors[2][2]) as f64,
            ];
            image.put_pixel(
                x as u32,
                y as u32,
                rgba_from_rgb(
                    [color[0] as f32, color[1] as f32, color[2] as f32],
                    255,
                ),
            );
        }
    }
}

pub fn face_color_rgb(
    border_labels: [i32; 3],
    data_labels: [i32; 3],
    vertex_values: &[f64],
    face: [u32; 3],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
) -> [f32; 3] {
    face_color(
        border_labels,
        data_labels,
        vertex_values,
        face,
        cmap,
        cmin,
        cmax,
    )
}

fn face_color(
    border_labels: [i32; 3],
    data_labels: [i32; 3],
    vertex_values: &[f64],
    face: [u32; 3],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
) -> [f32; 3] {
    if is_boundary_face(border_labels) {
        return BOUNDARY_COLOR;
    }

    let face_label = data_labels[0];
    if face_label == 0 {
        return UNKNOWN_COLOR;
    }

    let value = vertex_values[face[0] as usize];
    sample_colormap(cmap, value, cmin, cmax)
}

fn dull_shade(normal: [f32; 3], view: ViewAngles) -> f32 {
    // MATLAB material dull + camlight(80,-10) + camlight(-80,-10), gouraud lighting.
    // Light azimuth is relative to the camera azimuth; elevation is absolute.
    let ambient = 0.78;
    let diffuse = 0.22;
    let az = view.azimuth_deg.to_radians();
    let el_light = (-10.0f32).to_radians();
    let l1 = light_dir(az + 80.0f32.to_radians(), el_light);
    let l2 = light_dir(az - 80.0f32.to_radians(), el_light);
    let d = diffuse * (dot3(normal, l1).max(0.0) + dot3(normal, l2).max(0.0));
    (ambient + d).clamp(0.0, 1.0)
}

fn vertex_normals_world(mesh: &SurfaceMesh) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0; 3]; mesh.vertices.len()];
    for face in &mesh.faces {
        let v0 = mesh.vertices[face[0] as usize];
        let v1 = mesh.vertices[face[1] as usize];
        let v2 = mesh.vertices[face[2] as usize];
        // Area-weighted cross product (unnormalized).
        let ux = v1[0] - v0[0];
        let uy = v1[1] - v0[1];
        let uz = v1[2] - v0[2];
        let vx = v2[0] - v0[0];
        let vy = v2[1] - v0[1];
        let vz = v2[2] - v0[2];
        let n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx];
        for idx in face {
            for k in 0..3 {
                normals[*idx as usize][k] += n[k];
            }
        }
    }
    for n in &mut normals {
        *n = normalize(*n);
    }
    normals
}

fn light_dir(az: f32, el: f32) -> [f32; 3] {
    [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()]
}

fn is_boundary_face(labels: [i32; 3]) -> bool {
    labels[0] != labels[1] || labels[1] != labels[2]
}

fn is_degenerate_screen_triangle(screen: [[f32; 3]; 3]) -> bool {
    let max_edge = [
        screen_edge_length(screen[0], screen[1]),
        screen_edge_length(screen[1], screen[2]),
        screen_edge_length(screen[2], screen[0]),
    ]
    .into_iter()
    .fold(0.0_f32, f32::max);
    max_edge < 0.5
}

fn screen_edge_length(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

fn edge(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

fn overlay_layer_offset(canvas: &mut RgbaImage, layer: &RgbaImage, offset_x: i32, offset_y: i32) {
    for y in 0..canvas.height() as i32 {
        for x in 0..canvas.width() as i32 {
            let src_x = x - offset_x;
            let src_y = y - offset_y;
            if src_x < 0
                || src_y < 0
                || src_x >= layer.width() as i32
                || src_y >= layer.height() as i32
            {
                continue;
            }
            let pixel = *layer.get_pixel(src_x as u32, src_y as u32);
            if pixel[0] > 250 && pixel[1] > 250 && pixel[2] > 250 {
                continue;
            }
            canvas.put_pixel(x as u32, y as u32, pixel);
        }
    }
}

fn blit_stretch_clipped(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) {
    if w == 0 || h == 0 {
        return;
    }

    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let tx = x + dx;
            let ty = y + dy;
            if tx < 0
                || ty < 0
                || tx >= canvas.width() as i32
                || ty >= canvas.height() as i32
            {
                continue;
            }

            let sx = (dx as u32 * source.width()) / w.max(1);
            let sy = (dy as u32 * source.height()) / h.max(1);
            let pixel = *source.get_pixel(
                sx.min(source.width().saturating_sub(1)),
                sy.min(source.height().saturating_sub(1)),
            );
            if pixel[0] > 250 && pixel[1] > 250 && pixel[2] > 250 {
                continue;
            }
            canvas.put_pixel(tx as u32, ty as u32, pixel);
        }
    }
}

fn blit_stretch(canvas: &mut RgbaImage, source: &RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    for dy in 0..h {
        for dx in 0..w {
            let sx = dx * source.width() / w.max(1);
            let sy = dy * source.height() / h.max(1);
            let pixel = *source.get_pixel(sx.min(source.width() - 1), sy.min(source.height() - 1));
            let tx = x + dx;
            let ty = y + dy;
            if tx < canvas.width() && ty < canvas.height() {
                canvas.put_pixel(tx, ty, pixel);
            }
        }
    }
}

fn blit_fit(canvas: &mut RgbaImage, source: &RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }

    let src_aspect = source.width() as f32 / source.height().max(1) as f32;
    let dst_aspect = w as f32 / h as f32;

    let (fit_w, fit_h) = if src_aspect > dst_aspect {
        let fit_w = w;
        let fit_h = (w as f32 / src_aspect).round().max(1.0) as u32;
        (fit_w, fit_h)
    } else {
        let fit_h = h;
        let fit_w = (h as f32 * src_aspect).round().max(1.0) as u32;
        (fit_w, fit_h)
    };

    let offset_x = x + (w.saturating_sub(fit_w)) / 2;
    let offset_y = y + (h.saturating_sub(fit_h)) / 2;
    blit_stretch_transparent(canvas, source, offset_x, offset_y, fit_w, fit_h);
}

fn blit_stretch_transparent(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    for dy in 0..h {
        for dx in 0..w {
            let sx = dx * source.width() / w.max(1);
            let sy = dy * source.height() / h.max(1);
            let pixel = *source.get_pixel(
                sx.min(source.width().saturating_sub(1)),
                sy.min(source.height().saturating_sub(1)),
            );
            if pixel[0] > 250 && pixel[1] > 250 && pixel[2] > 250 {
                continue;
            }
            let tx = x + dx;
            let ty = y + dy;
            if tx < canvas.width() && ty < canvas.height() {
                canvas.put_pixel(tx, ty, pixel);
            }
        }
    }
}

fn blit(canvas: &mut RgbaImage, source: &RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }

    for dy in 0..h {
        for dx in 0..w {
            let sx = dx * source.width() / w.max(1);
            let sy = dy * source.height() / h.max(1);
            let pixel = *source.get_pixel(
                sx.min(source.width().saturating_sub(1)),
                sy.min(source.height().saturating_sub(1)),
            );
            let tx = x + dx;
            let ty = y + dy;
            if tx < canvas.width() && ty < canvas.height() {
                canvas.put_pixel(tx, ty, pixel);
            }
        }
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

fn scale_color(color: [f32; 3], shade: f32) -> [f32; 3] {
    [color[0] * shade, color[1] * shade, color[2] * shade]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-6 {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}
