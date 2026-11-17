//! Goldstone core: document model, validation and geometry.
//!
//! `.gold` text → [`io::parse`] → [`model::Document`] → [`scene::build`] →
//! [`scene::Scene`] (display list) → export backends.

pub mod color;
pub mod expr;
pub mod geom;
pub mod inspect;
pub mod io;
pub mod model;
pub mod plot;
pub mod quality;
pub mod scene;
pub mod shapes;
pub mod templates;
pub mod tex;

pub use kurbo;
