use base64::Engine;
use byteorder::{LittleEndian, ReadBytesExt};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use crate::error::{PlotSurfaceError, Result};
use crate::mesh::SurfaceMesh;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GiftiIntent {
    PointSet,
    Triangle,
    Unknown,
}

impl GiftiIntent {
    fn from_attr(value: &str) -> Self {
        match value {
            "NIFTI_INTENT_POINTSET" => Self::PointSet,
            "NIFTI_INTENT_TRIANGLE" => Self::Triangle,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug)]
struct GiftiArrayMeta {
    intent: GiftiIntent,
    data_type: String,
}

pub fn load_surface_mesh(path: &Path) -> Result<SurfaceMesh> {
    let xml = fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut arrays: Vec<(GiftiArrayMeta, Vec<u8>)> = Vec::new();

    let mut in_data_array = false;
    let mut current_meta: Option<GiftiArrayMeta> = None;
    let mut collecting_data = false;
    let mut data_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"DataArray" {
                    in_data_array = true;
                    let mut intent = GiftiIntent::Unknown;
                    let mut data_type = String::new();

                    for attr in e.attributes().flatten() {
                        let key = attr.key.as_ref();
                        let val = attr
                            .unescape_value()
                            .map_err(|err| PlotSurfaceError::Message(err.to_string()))?
                            .into_owned();
                        match key {
                            b"Intent" => intent = GiftiIntent::from_attr(&val),
                            b"DataType" => data_type = val,
                            _ => {}
                        }
                    }

                    current_meta = Some(GiftiArrayMeta { intent, data_type });
                } else if in_data_array && e.name().as_ref() == b"Data" {
                    collecting_data = true;
                    data_buf.clear();
                }
            }
            Ok(Event::Text(text)) => {
                if collecting_data {
                    data_buf.push_str(text.unescape().unwrap_or_default().as_ref());
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"Data" {
                    collecting_data = false;
                } else if e.name().as_ref() == b"DataArray" {
                    if let Some(meta) = current_meta.take() {
                        let payload = base64::engine::general_purpose::STANDARD
                            .decode(data_buf.trim())
                            .map_err(|err| PlotSurfaceError::Message(err.to_string()))?;
                        arrays.push((meta, payload));
                    }
                    in_data_array = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.into()),
            _ => {}
        }
        buf.clear();
    }

    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    for (meta, bytes) in arrays {
        match meta.intent {
            GiftiIntent::PointSet => {
                vertices = decode_vertices(&meta, &bytes)?;
            }
            GiftiIntent::Triangle => {
                faces = decode_faces(&meta, &bytes)?;
            }
            GiftiIntent::Unknown => {}
        }
    }

    if vertices.is_empty() || faces.is_empty() {
        return Err(PlotSurfaceError::Message(format!(
            "GIFTI file missing vertices or faces: {}",
            path.display()
        )));
    }

    Ok(SurfaceMesh { vertices, faces })
}

fn decode_vertices(meta: &GiftiArrayMeta, bytes: &[u8]) -> Result<Vec<[f32; 3]>> {
    if meta.data_type != "NIFTI_TYPE_FLOAT32" {
        return Err(PlotSurfaceError::Message(format!(
            "Unsupported GIFTI vertex type: {}",
            meta.data_type
        )));
    }

    let mut cursor = Cursor::new(bytes);
    let mut out = Vec::new();
    while let Ok(value) = cursor.read_f32::<LittleEndian>() {
        out.push(value);
    }

    if out.len() % 3 != 0 {
        return Err(PlotSurfaceError::Message(
            "GIFTI pointset length is not divisible by 3".into(),
        ));
    }

    Ok(out
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect())
}

fn decode_faces(meta: &GiftiArrayMeta, bytes: &[u8]) -> Result<Vec<[u32; 3]>> {
    if meta.data_type != "NIFTI_TYPE_INT32" {
        return Err(PlotSurfaceError::Message(format!(
            "Unsupported GIFTI face type: {}",
            meta.data_type
        )));
    }

    let mut cursor = Cursor::new(bytes);
    let mut out = Vec::new();
    while let Ok(value) = cursor.read_i32::<LittleEndian>() {
        out.push(value as u32);
    }

    if out.len() % 3 != 0 {
        return Err(PlotSurfaceError::Message(
            "GIFTI triangle length is not divisible by 3".into(),
        ));
    }

    Ok(out
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect())
}
