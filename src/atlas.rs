use std::path::{Path, PathBuf};

use crate::error::{PlotSurfaceError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasKind {
    Schaefer200,
    Schaefer100,
    HcpMmp,
    Brodmann,
}

impl AtlasKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "schaefer200" | "schaefer_200" => Ok(Self::Schaefer200),
            "schaefer100" | "schaefer_100" => Ok(Self::Schaefer100),
            "hcp_mmp" | "hcpmmp" | "mmp" => Ok(Self::HcpMmp),
            "brodmann" | "ba" => Ok(Self::Brodmann),
            other => Err(PlotSurfaceError::Message(format!(
                "Unknown atlas: {other}"
            ))),
        }
    }

    pub fn num_regions(&self) -> usize {
        match self {
            Self::Schaefer200 => 200,
            Self::Schaefer100 => 100,
            Self::HcpMmp => 360,
            Self::Brodmann => 74,
        }
    }

    pub fn lh_annot_name(&self) -> &'static str {
        match self {
            Self::Schaefer200 => "lh.Schaefer2018_200Parcels_7Networks_order.annot",
            Self::Schaefer100 => "lh.Schaefer2018_100Parcels_7Networks_order.annot",
            Self::HcpMmp => "lh.HCPMMP1.annot",
            Self::Brodmann => "lh.PALS_B12_Brodmann.annot",
        }
    }

    pub fn rh_annot_name(&self) -> &'static str {
        match self {
            Self::Schaefer200 => "rh.Schaefer2018_200Parcels_7Networks_order.annot",
            Self::Schaefer100 => "rh.Schaefer2018_100Parcels_7Networks_order.annot",
            Self::HcpMmp => "rh.HCPMMP1.annot",
            Self::Brodmann => "rh.PALS_B12_Brodmann.annot",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AtlasPaths {
    pub lh_annot: PathBuf,
    pub rh_annot: PathBuf,
    pub lh_surface: PathBuf,
    pub rh_surface: PathBuf,
    pub both_surface: PathBuf,
}

pub fn resolve_atlas_paths(data_dir: &Path, atlas: AtlasKind) -> Result<AtlasPaths> {
    let lh_annot = data_dir.join(atlas.lh_annot_name());
    let rh_annot = data_dir.join(atlas.rh_annot_name());
    let lh_surface = data_dir.join("lh.inflated.freesurfer.gii");
    let rh_surface = data_dir.join("rh.inflated.freesurfer.gii");
    let both_surface = data_dir.join("mesh.inflated.freesurfer.gii");

    for path in [&lh_annot, &rh_annot, &lh_surface, &rh_surface, &both_surface] {
        if !path.exists() {
            return Err(PlotSurfaceError::Message(format!(
                "Missing required data file: {}",
                path.display()
            )));
        }
    }

    Ok(AtlasPaths {
        lh_annot,
        rh_annot,
        lh_surface,
        rh_surface,
        both_surface,
    })
}
