use crate::error::{PlotSurfaceError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColormapKind {
    Auto,
    Jet,
    Diverging,
    Blue,
    /// Discrete common/unique regions for Figure 6f (MATLAB createCustomColormap).
    CommonUnique,
}

impl ColormapKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "jet" => Ok(Self::Jet),
            "diverging" | "mycolormap" | "signed" => Ok(Self::Diverging),
            "blue" | "mycolormap_blue" | "positive" => Ok(Self::Blue),
            "common_unique" | "custom" | "createcustomcolormap" => Ok(Self::CommonUnique),
            other => Err(PlotSurfaceError::Message(format!(
                "Unknown colormap: {other}"
            ))),
        }
    }
}

pub fn build_colormap(kind: ColormapKind, cmin: f64, cmax: f64) -> Vec<[f32; 3]> {
    match kind {
        ColormapKind::Jet => jet_colormap(64),
        ColormapKind::Blue => mycolormap_blue(cmin, cmax),
        ColormapKind::Diverging => mycolormap(cmin, cmax),
        ColormapKind::Auto => {
            if cmin >= 0.0 || (-cmin < 0.1 * cmax) {
                mycolormap_blue(cmin, cmax)
            } else {
                mycolormap(cmin, cmax)
            }
        }
        ColormapKind::CommonUnique => common_unique_colormap(),
    }
}

/// Port of MATLAB `createCustomColormap`, with rows 1 and 2 swapped so value
/// assignments from `common_unique_regions` match the Figure 6f legend:
/// 0 = background, 1 = OASIS-unique (peach), 2 = ADNI-unique (green), 3 = shared (dark red).
fn common_unique_colormap() -> Vec<[f32; 3]> {
    vec![
        [1.0, 1.0, 1.0],
        color_rgb(251.0, 226.0, 209.0),
        color_rgb(205.0, 239.0, 195.0),
        color_rgb(164.0, 46.0, 42.0),
    ]
}

fn color_rgb(r: f64, g: f64, b: f64) -> [f32; 3] {
    [
        (r / 255.0) as f32,
        (g / 255.0) as f32,
        (b / 255.0) as f32,
    ]
}

pub fn sample_colormap(cmap: &[[f32; 3]], value: f64, cmin: f64, cmax: f64) -> [f32; 3] {
    if !value.is_finite() {
        return [0.85, 0.85, 0.85];
    }

    if (cmax - cmin).abs() < f64::EPSILON {
        return cmap[cmap.len() / 2];
    }

    let t = ((value - cmin) / (cmax - cmin)).clamp(0.0, 1.0);
    let idx = t * (cmap.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let frac = (idx - lo as f64) as f32;

    let a = cmap[lo];
    let b = cmap[hi.min(cmap.len() - 1)];
    [
        a[0] + (b[0] - a[0]) * frac,
        a[1] + (b[1] - a[1]) * frac,
        a[2] + (b[2] - a[2]) * frac,
    ]
}

fn jet_colormap(n: usize) -> Vec<[f32; 3]> {
    let mut cmap = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / (n.saturating_sub(1).max(1)) as f64;
        let r = (1.5 - (4.0 * t - 3.0).abs()).clamp(0.0, 1.0);
        let g = (1.5 - (4.0 * t - 2.0).abs()).clamp(0.0, 1.0);
        let b = (1.5 - (4.0 * t - 1.0).abs()).clamp(0.0, 1.0);
        cmap.push([r as f32, g as f32, b as f32]);
    }
    cmap.reverse();
    cmap
}

fn mycolormap(cmin: f64, cmax: f64) -> Vec<[f32; 3]> {
    let n = 256;
    let middle_value = 0.0;
    let range_thresh = 0.1;
    piecewise_colormap(
        n,
        cmin,
        cmax,
        middle_value,
        range_thresh,
        [
            [220.0, 0.0, 0.0],
            [247.0, 247.0, 247.0],
            [245.0, 245.0, 245.0],
            [60.0, 84.0, 136.0],
        ],
    )
}

fn mycolormap_blue(cmin: f64, cmax: f64) -> Vec<[f32; 3]> {
    let n = 256;
    let range_thresh = 0.01;
    let middle_value = cmin + (cmax - cmin) * 0.7;
    piecewise_colormap(
        n,
        cmin,
        cmax,
        middle_value,
        range_thresh,
        [
            [255.0, 255.0, 255.0],
            [247.0, 247.0, 247.0],
            [230.0, 230.0, 230.0],
            [60.0, 84.0, 136.0],
        ],
    )
}

fn piecewise_colormap(
    n: usize,
    cmin: f64,
    cmax: f64,
    middle_value: f64,
    range_thresh: f64,
    anchors: [[f64; 3]; 4],
) -> Vec<[f32; 3]> {
    let middle_index = nearest_index(cmin, cmax, n, middle_value);
    let delta = (n as f64 * range_thresh) as usize;
    let idx = [
        0,
        middle_index.saturating_sub(delta),
        (middle_index + delta).min(n.saturating_sub(1)),
        n.saturating_sub(1),
    ];

    let mut cmap = vec![[0.0, 0.0, 0.0]; n];
    for i in 0..n {
        let color = if i <= idx[1] {
            lerp_color(anchors[0], anchors[1], i, idx[0], idx[1])
        } else if i <= idx[2] {
            lerp_color(anchors[1], anchors[2], i, idx[1], idx[2])
        } else {
            lerp_color(anchors[2], anchors[3], i, idx[2], idx[3])
        };
        cmap[i] = [color[0] as f32, color[1] as f32, color[2] as f32];
    }
    cmap
}

fn nearest_index(cmin: f64, cmax: f64, n: usize, value: f64) -> usize {
    if (cmax - cmin).abs() < f64::EPSILON {
        return n / 2;
    }
    let t = ((value - cmin) / (cmax - cmin)).clamp(0.0, 1.0);
    (t * (n.saturating_sub(1).max(1)) as f64).round() as usize
}

fn lerp_color(a: [f64; 3], b: [f64; 3], i: usize, i0: usize, i1: usize) -> [f64; 3] {
    if i1 <= i0 {
        return a;
    }
    let t = (i - i0) as f64 / (i1 - i0) as f64;
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
    .map(|v| v / 255.0)
}
