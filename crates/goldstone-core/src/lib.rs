//! Goldstone core: document model, validation and geometry.
//!
//! `.gold` text → [`io::parse`] → [`model::Document`] → [`scene::build`] →
//! [`scene::Scene`] (display list) → export backends.

pub mod color;
pub mod geom;
pub mod io;
pub mod model;
pub mod scene;

pub use kurbo;
