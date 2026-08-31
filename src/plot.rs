use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;

use neuroformats::read_annot;

use crate::atlas::{resolve_atlas_paths, AtlasKind};
use crate::colormap::{build_colormap, ColormapKind};
use crate::error::Result;
use crate::gifti::load_surface_mesh;
use crate::mesh::{BrainSurfaces, SurfaceMesh};
use crate::render::{
    compose_matlab_figure, render_colorbar, render_surface_view_target, MatlabAxesPlacement,
    RenderConfig, RenderTarget, ViewAngles, MATLAB_CANVAS_HEIGHT, MATLAB_CANVAS_WIDTH,
};
use crate::svg::{render_composite_svg, render_single_panel_svg};
use crate::weights::{
    apply_border_labels, data_limits, invalidate_non_surface_regions, normalize_weights,
    parcel_weights_to_vertex_data, read_weights_csv,
};

#[derive(Clone, Debug)]
pub struct PlotBrainOptions {
    pub data_dir: PathBuf,
    pub weights: PathBuf,
    pub name: String,
    pub out_dir: PathBuf,
    pub atlas: AtlasKind,
    pub border_atlas: Option<AtlasKind>,
    pub colormap: ColormapKind,
    pub vmin: Option<f64>,
    pub vmax: Option<f64>,
    pub plot_full: bool,
}

pub fn plot_brain_from_weights(options: &PlotBrainOptions) -> Result<()> {
    fs::create_dir_all(&options.out_dir)?;

    let atlas_paths = resolve_atlas_paths(&options.data_dir, options.atlas)?;
    let surfaces = load_brain_surfaces(&atlas_paths)?;

    let lh_annot = read_annot(&atlas_paths.lh_annot)?;
    let rh_annot = read_annot(&atlas_paths.rh_annot)?;

    let weights = read_weights_csv(&options.weights)?;
    // MATLAB fig1c.m: cmin/cmax = min/max over the raw (normalized) data BEFORE
    // invalidating non-surface parcels, then passed as explicit min/max args.
    let weights_normalized = normalize_weights(weights);
    let (cmin, cmax) = data_limits(&weights_normalized, options.vmin, options.vmax);
    let cmap = build_colormap(options.colormap, cmin, cmax);

    let weights = invalidate_non_surface_regions(weights_normalized);
    let mut vertex_data = parcel_weights_to_vertex_data(&lh_annot, &rh_annot, &weights)?;

    if let Some(border_atlas) = options.border_atlas {
        let border_paths = resolve_atlas_paths(&options.data_dir, border_atlas)?;
        let lh_border_annot = read_annot(&border_paths.lh_annot)?;
        let rh_border_annot = read_annot(&border_paths.rh_annot)?;
        vertex_data = apply_border_labels(vertex_data, &lh_border_annot, &rh_border_annot)?;
    }

    let both_data_labels = vertex_data.both_data_labels();
    let both_border_labels = vertex_data.both_border_labels();

    // MATLAB `_0.png`: the composite figure with the colorbar hidden.
    let main_panel = render_main_panel(&surfaces, &vertex_data, &cmap, cmin, cmax, false);
    save_png(
        &options.out_dir.join(format!("{}_0.png", options.name)),
        &main_panel,
    )?;
    save_svg(
        &options.out_dir.join(format!("{}_0.svg", options.name)),
        &render_composite_svg(&surfaces, &vertex_data, &cmap, cmin, cmax, false),
    )?;

    // MATLAB `_color_bar.png`: the same figure with the colorbar shown.
    let full_figure = render_main_panel(&surfaces, &vertex_data, &cmap, cmin, cmax, true);
    save_png(
        &options
            .out_dir
            .join(format!("{}_color_bar.png", options.name)),
        &full_figure,
    )?;
    save_svg(
        &options
            .out_dir
            .join(format!("{}_color_bar.svg", options.name)),
        &render_composite_svg(&surfaces, &vertex_data, &cmap, cmin, cmax, true),
    )?;

    if options.plot_full {
        write_single_view(
            &surfaces.left,
            &vertex_data.left.vertex_values,
            &vertex_data.left.vertex_labels,
            vertex_data.left.border_labels(),
            &cmap,
            cmin,
            cmax,
            ViewAngles::new(-90.0, 0.0, false),
            &options.out_dir.join(format!("lh_lateral_{}_0.png", options.name)),
            true,
        )?;
        save_svg(
            &options
                .out_dir
                .join(format!("lh_lateral_{}_0.svg", options.name)),
            &render_single_panel_svg(
                &surfaces.left,
                &vertex_data.left.vertex_values,
                &vertex_data.left.vertex_labels,
                vertex_data.left.border_labels(),
                &cmap,
                cmin,
                cmax,
                ViewAngles::new(-90.0, 0.0, false),
            ),
        )?;
        write_single_view(
            &surfaces.left,
            &vertex_data.left.vertex_values,
            &vertex_data.left.vertex_labels,
            vertex_data.left.border_labels(),
            &cmap,
            cmin,
            cmax,
            ViewAngles::new(90.0, 0.0, false),
            &options.out_dir.join(format!("lh_medial_{}_0.png", options.name)),
            true,
        )?;
        save_svg(
            &options
                .out_dir
                .join(format!("lh_medial_{}_0.svg", options.name)),
            &render_single_panel_svg(
                &surfaces.left,
                &vertex_data.left.vertex_values,
                &vertex_data.left.vertex_labels,
                vertex_data.left.border_labels(),
                &cmap,
                cmin,
                cmax,
                ViewAngles::new(90.0, 0.0, false),
            ),
        )?;
        write_single_view(
            &surfaces.right,
            &vertex_data.right.vertex_values,
            &vertex_data.right.vertex_labels,
            vertex_data.right.border_labels(),
            &cmap,
            cmin,
            cmax,
            ViewAngles::new(90.0, 0.0, false),
            &options.out_dir.join(format!("rh_lateral_{}_0.png", options.name)),
            true,
        )?;
        save_svg(
            &options
                .out_dir
                .join(format!("rh_lateral_{}_0.svg", options.name)),
            &render_single_panel_svg(
                &surfaces.right,
                &vertex_data.right.vertex_values,
                &vertex_data.right.vertex_labels,
                vertex_data.right.border_labels(),
                &cmap,
                cmin,
                cmax,
                ViewAngles::new(90.0, 0.0, false),
            ),
        )?;
        write_single_view(
            &surfaces.right,
            &vertex_data.right.vertex_values,
            &vertex_data.right.vertex_labels,
            vertex_data.right.border_labels(),
            &cmap,
            cmin,
            cmax,
            ViewAngles::new(-90.0, 0.0, false),
            &options.out_dir.join(format!("rh_medial_{}_0.png", options.name)),
            true,
        )?;
        save_svg(
            &options
                .out_dir
                .join(format!("rh_medial_{}_0.svg", options.name)),
            &render_single_panel_svg(
                &surfaces.right,
                &vertex_data.right.vertex_values,
                &vertex_data.right.vertex_labels,
                vertex_data.right.border_labels(),
                &cmap,
                cmin,
                cmax,
                ViewAngles::new(-90.0, 0.0, false),
            ),
        )?;
        write_single_view(
            &surfaces.both,
            &vertex_data.both,
            &both_data_labels,
            &both_border_labels,
            &cmap,
            cmin,
            cmax,
            ViewAngles::new(0.0, 90.0, false),
            &options
                .out_dir
                .join(format!("both_hemis_{}_0.png", options.name)),
            false,
        )?;
        save_svg(
            &options
                .out_dir
                .join(format!("both_hemis_{}_0.svg", options.name)),
            &render_single_panel_svg(
                &surfaces.both,
                &vertex_data.both,
                &both_data_labels,
                &both_border_labels,
                &cmap,
                cmin,
                cmax,
                ViewAngles::new(0.0, 90.0, false),
            ),
        )?;
    }

    println!("Saved brain maps to {}", options.out_dir.display());
    Ok(())
}

fn render_main_panel(
    surfaces: &BrainSurfaces,
    vertex_data: &crate::weights::BrainVertexData,
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    show_colorbar: bool,
) -> image::RgbaImage {
    let config = RenderConfig::default();
    let both_data_labels = vertex_data.both_data_labels();
    let both_border_labels = vertex_data.both_border_labels();

    let placements = [
        MatlabAxesPlacement::LH_LATERAL,
        MatlabAxesPlacement::LH_MEDIAL,
        MatlabAxesPlacement::RH_LATERAL,
        MatlabAxesPlacement::RH_MEDIAL,
        MatlabAxesPlacement::BOTH_CENTER,
    ];

    let panels = vec![
        render_surface_view_target(
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
        ),
        render_surface_view_target(
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
        ),
        render_surface_view_target(
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
        ),
        render_surface_view_target(
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
        ),
        render_surface_view_target(
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
        ),
    ];

    let colorbar_opt = if show_colorbar {
        let bar_w = (MATLAB_CANVAS_WIDTH as f32 * 0.020059992500933) as u32;
        let bar_h = (MATLAB_CANVAS_HEIGHT as f32 * 0.291629340901834) as u32;
        Some(render_colorbar(cmap, cmin, cmax, bar_w, bar_h))
    } else {
        None
    };
    compose_matlab_figure(&panels, &placements, colorbar_opt.as_ref())
}

fn write_single_view(
    mesh: &SurfaceMesh,
    values: &[f64],
    data_labels: &[i32],
    border_labels: &[i32],
    cmap: &[[f32; 3]],
    cmin: f64,
    cmax: f64,
    view: ViewAngles,
    path: &Path,
    _stretch_to_axes: bool,
) -> Result<()> {
    let config = RenderConfig::default();
    let target = RenderTarget::fitted(&config);
    let image = render_surface_view_target(
        mesh,
        values,
        data_labels,
        border_labels,
        cmap,
        cmin,
        cmax,
        view,
        &config,
        target,
    );
    save_png(path, &image)
}

fn load_brain_surfaces(paths: &crate::atlas::AtlasPaths) -> Result<BrainSurfaces> {
    let left = load_surface_mesh(&paths.lh_surface)?;
    let right = load_surface_mesh(&paths.rh_surface)?;
    let both = SurfaceMesh::merge_hemispheres(&left, &right);
    Ok(BrainSurfaces { left, right, both })
}

fn save_png(path: &Path, image: &image::RgbaImage) -> Result<()> {
    image.save(path)?;
    println!("Saved {}", path.display());
    Ok(())
}

fn save_svg(path: &Path, svg_content: &str) -> Result<()> {
    std::fs::write(path, svg_content)?;
    println!("Saved {} ({:.1} MB)", path.display(), svg_content.len() as f64 / 1_048_576.0);

    let svgz_path = path.with_extension("svgz");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(svg_content.as_bytes())?;
    let compressed = encoder.finish()?;
    std::fs::write(&svgz_path, compressed)?;
    println!(
        "Saved {} ({:.1} MB)",
        svgz_path.display(),
        svgz_path.metadata()?.len() as f64 / 1_048_576.0
    );
    Ok(())
}
