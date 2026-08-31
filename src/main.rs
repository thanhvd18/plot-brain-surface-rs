use std::path::PathBuf;

use clap::{Parser, Subcommand};
use plot_surface::{
    plot_brain_from_weights, AtlasKind, ColormapKind, PlotBrainOptions, Result,
};

#[derive(Parser, Debug)]
#[command(name = "plot-surface", about = "Rust brain surface plotting")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Plot parcel weights on cortical surface meshes.
    Plot {
        /// Directory containing GIFTI meshes and .annot files.
        #[arg(long, env = "PLOT_SURFACE_DATA_DIR")]
        data_dir: PathBuf,

        /// CSV file with parcel weights (column: cortical_thickness or weight).
        #[arg(long)]
        weights: PathBuf,

        /// Output image name prefix.
        #[arg(long)]
        name: String,

        /// Output directory for PNG files.
        #[arg(long)]
        out_dir: PathBuf,

        /// Atlas key (schaefer200, schaefer100, hcp_mmp, brodmann).
        #[arg(long, default_value = "schaefer200")]
        atlas: String,

        /// Optional atlas for ROI boundary lines (e.g. brodmann while coloring schaefer parcels).
        #[arg(long)]
        border_atlas: Option<String>,

        /// Colormap: auto, jet, diverging, blue, common_unique.
        #[arg(long, default_value = "auto")]
        colormap: String,

        /// Optional fixed minimum for color scale.
        #[arg(long)]
        vmin: Option<f64>,

        /// Optional fixed maximum for color scale.
        #[arg(long)]
        vmax: Option<f64>,

        /// Also write individual hemisphere view PNGs.
        #[arg(long, default_value_t = true)]
        plot_full: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Plot {
            data_dir,
            weights,
            name,
            out_dir,
            atlas,
            border_atlas,
            colormap,
            vmin,
            vmax,
            plot_full,
        } => plot_brain_from_weights(&PlotBrainOptions {
            data_dir,
            weights,
            name,
            out_dir,
            atlas: AtlasKind::parse(&atlas)?,
            border_atlas: border_atlas
                .as_deref()
                .map(AtlasKind::parse)
                .transpose()?,
            colormap: ColormapKind::parse(&colormap)?,
            vmin,
            vmax,
            plot_full,
        }),
    }
}
