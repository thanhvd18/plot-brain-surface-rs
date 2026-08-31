pub mod atlas;
pub mod colorbar;
pub mod colormap;
pub mod error;
pub mod gifti;
pub mod mesh;
pub mod plot;
pub mod render;
pub mod svg;
pub mod weights;

pub use atlas::AtlasKind;
pub use colormap::ColormapKind;
pub use error::{PlotSurfaceError, Result};
pub use plot::{plot_brain_from_weights, PlotBrainOptions};
