use neuroformats::FsAnnot;

use crate::error::{PlotSurfaceError, Result};

#[derive(Clone, Debug)]
pub struct HemisphereData {
    pub vertex_values: Vec<f64>,
    pub vertex_labels: Vec<i32>,
    /// Optional per-vertex labels used only for ROI boundary lines.
    pub border_labels: Option<Vec<i32>>,
}

impl HemisphereData {
    pub fn border_labels(&self) -> &[i32] {
        self.border_labels
            .as_deref()
            .unwrap_or(&self.vertex_labels)
    }
}

#[derive(Clone, Debug)]
pub struct BrainVertexData {
    pub left: HemisphereData,
    pub right: HemisphereData,
    pub both: Vec<f64>,
}

impl BrainVertexData {
    pub fn both_data_labels(&self) -> Vec<i32> {
        self.left
            .vertex_labels
            .iter()
            .chain(self.right.vertex_labels.iter())
            .copied()
            .collect()
    }

    pub fn both_border_labels(&self) -> Vec<i32> {
        self.left
            .border_labels()
            .iter()
            .chain(self.right.border_labels().iter())
            .copied()
            .collect()
    }
}

pub fn invalidate_non_surface_regions(mut weights: Vec<f64>) -> Vec<f64> {
    if weights.len() >= 1 {
        weights[0] = f64::NAN;
    }
    if weights.len() >= 2 {
        weights[1] = f64::NAN;
    }
    weights
}

pub fn normalize_weights(mut weights: Vec<f64>) -> Vec<f64> {
    for value in &mut weights {
        if (*value - (-999.0)).abs() < f64::EPSILON {
            *value = -0.5;
        }
    }
    weights
}

pub fn parcel_weights_to_vertex_data(
    lh_annot: &FsAnnot,
    rh_annot: &FsAnnot,
    weights: &[f64],
) -> Result<BrainVertexData> {
    let parcels_per_hemisphere = weights.len() / 2;
    if weights.len() % 2 != 0 {
        return Err(PlotSurfaceError::Message(format!(
            "Expected an even number of parcel weights, got {}",
            weights.len()
        )));
    }

    let lh_labels = atlas_region_labels(lh_annot);
    let rh_labels = atlas_region_labels(rh_annot);

    let mut left_values = vec![f64::NAN; lh_annot.vertex_labels.len()];
    let mut right_values = vec![f64::NAN; rh_annot.vertex_labels.len()];

    for parcel_idx in 0..parcels_per_hemisphere {
        let lh_value = weights[parcel_idx * 2];
        let rh_value = weights[parcel_idx * 2 + 1];

        if parcel_idx < lh_labels.len() {
            let lh_label = lh_labels[parcel_idx];
            for (vertex_idx, label) in lh_annot.vertex_labels.iter().enumerate() {
                if *label == lh_label {
                    left_values[vertex_idx] = lh_value;
                }
            }
        }

        if parcel_idx < rh_labels.len() {
            let rh_label = rh_labels[parcel_idx];
            for (vertex_idx, label) in rh_annot.vertex_labels.iter().enumerate() {
                if *label == rh_label {
                    right_values[vertex_idx] = rh_value;
                }
            }
        }
    }

    let both = left_values
        .iter()
        .chain(right_values.iter())
        .copied()
        .collect();

    Ok(BrainVertexData {
        left: HemisphereData {
            vertex_values: left_values,
            vertex_labels: lh_annot.vertex_labels.clone(),
            border_labels: None,
        },
        right: HemisphereData {
            vertex_values: right_values,
            vertex_labels: rh_annot.vertex_labels.clone(),
            border_labels: None,
        },
        both,
    })
}

pub fn apply_border_labels(
    mut vertex_data: BrainVertexData,
    lh_border_annot: &FsAnnot,
    rh_border_annot: &FsAnnot,
) -> Result<BrainVertexData> {
    if vertex_data.left.vertex_values.len() != lh_border_annot.vertex_labels.len() {
        return Err(PlotSurfaceError::Message(
            "Left border annotation vertex count does not match data atlas".into(),
        ));
    }
    if vertex_data.right.vertex_values.len() != rh_border_annot.vertex_labels.len() {
        return Err(PlotSurfaceError::Message(
            "Right border annotation vertex count does not match data atlas".into(),
        ));
    }

    vertex_data.left.border_labels = Some(lh_border_annot.vertex_labels.clone());
    vertex_data.right.border_labels = Some(rh_border_annot.vertex_labels.clone());
    Ok(vertex_data)
}

fn atlas_region_labels(annot: &FsAnnot) -> Vec<i32> {
    annot
        .colortable
        .regions
        .iter()
        .map(|region| region.label)
        .collect()
}

pub fn read_weights_csv(path: &std::path::Path) -> Result<Vec<f64>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)?;

    let headers = reader.headers()?.clone();
    let column = if headers.iter().any(|h| h == "cortical_thickness") {
        "cortical_thickness"
    } else if headers.iter().any(|h| h == "weight") {
        "weight"
    } else if headers.len() == 1 {
        headers[0].as_ref()
    } else {
        return Err(PlotSurfaceError::Message(format!(
            "CSV must contain cortical_thickness or weight column: {}",
            path.display()
        )));
    };

    let mut values = Vec::new();
    let column_idx = headers
        .iter()
        .position(|header| header == column)
        .ok_or_else(|| PlotSurfaceError::Message("Missing weight column".into()))?;
    for record in reader.records() {
        let record = record?;
        let raw = record
            .get(column_idx)
            .ok_or_else(|| PlotSurfaceError::Message("Missing weight column".into()))?;
        values.push(raw.parse::<f64>().unwrap_or(f64::NAN));
    }

    Ok(values)
}

pub fn data_limits(values: &[f64], vmin: Option<f64>, vmax: Option<f64>) -> (f64, f64) {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let auto_min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let auto_max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (vmin.unwrap_or(auto_min), vmax.unwrap_or(auto_max))
}
