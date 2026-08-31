use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotSurfaceError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Neuroformats(#[from] neuroformats::error::NeuroformatsError),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
}

pub type Result<T> = std::result::Result<T, PlotSurfaceError>;
