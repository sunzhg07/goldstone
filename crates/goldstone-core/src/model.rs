//! The on-disk document model (`.gold` files, TOML).
//!
//! Every field that can come from a style preset is an `Option` here; the
//! [`crate::resolve`] module turns a document into fully specified objects
//! (object field → class preset → built-in default).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Current file-format version written by this crate.
pub const FORMAT_VERSION: u32 = 1;

pub type Point = [f64; 2];

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub canvas: Canvas,
    #[serde(default, rename = "style", skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, Style>,
    #[serde(default, rename = "vertex", skip_serializing_if = "Vec::is_empty")]
    pub vertices: Vec<Vertex>,
    #[serde(default, rename = "edge", skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(default, rename = "text", skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<Text>,
    #[serde(default, rename = "axes", skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<Axes>,
    #[serde(default, rename = "shape", skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<Shape>,
    #[serde(default, rename = "group", skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            format: FORMAT_VERSION,
            canvas: Canvas::default(),
            styles: BTreeMap::new(),
            vertices: Vec::new(),
            edges: Vec::new(),
            texts: Vec::new(),
            axes: Vec::new(),
            shapes: Vec::new(),
            groups: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    /// Grid spacing in pt (editor only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<f64>,
    /// Margin added around the bounding box on export, in pt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    /// Background color for raster/SVG export; `None` means transparent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VertexKind {
    None,
    Dot,
    Cross,
    Ocross,
    Square,
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stroke {
    Plain,
    Dashed,
    Dotted,
    Wavy,
    Curly,
    Zigzag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArrowDir {
    None,
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    Center,
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Anchor {
    /// Offset of the anchor point relative to the box center, in units of the
    /// half-width/half-height (x right, y up).
    pub fn unit_offset(self) -> (f64, f64) {
        match self {
            Anchor::Center => (0.0, 0.0),
            Anchor::North => (0.0, 1.0),
            Anchor::South => (0.0, -1.0),
            Anchor::East => (1.0, 0.0),
            Anchor::West => (-1.0, 0.0),
            Anchor::NorthEast => (1.0, 1.0),
            Anchor::NorthWest => (-1.0, 1.0),
            Anchor::SouthEast => (1.0, -1.0),
            Anchor::SouthWest => (-1.0, -1.0),
        }
    }

    /// TikZ anchor name.
    pub fn tikz(self) -> &'static str {
        match self {
            Anchor::Center => "center",
            Anchor::North => "north",
            Anchor::South => "south",
            Anchor::East => "east",
            Anchor::West => "west",
            Anchor::NorthEast => "north east",
            Anchor::NorthWest => "north west",
            Anchor::SouthEast => "south east",
            Anchor::SouthWest => "south west",
        }
    }
}

/// A named preset of style properties (`[style.<name>]`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    // vertices
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<VertexKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    // edges
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_sep: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amplitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wavelength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow: Option<ArrowDir>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_pos: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_side: Option<Side>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_pos: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_offset: Option<f64>,
    // text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vertex {
    pub id: String,
    pub at: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<VertexKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_anchor: Option<Anchor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Direction (degrees, counter-clockwise from +x) from the vertex to the
    /// loop's center.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    // geometry
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<[Point; 2]>,
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_: Option<LoopSpec>,
    // style
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_sep: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amplitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wavelength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow: Option<ArrowDir>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_pos: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_width: Option<f64>,
    // label
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_side: Option<Side>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_pos: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_offset: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub at: Point,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_spacing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_length: Option<f64>,
    /// Labels for ticks 1, 2, 3, … (tick 0 sits at the origin and is not labelled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub origin: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Axis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Axis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Box,
    Ellipse,
    Polygon,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub kind: ShapeKind,
    /// Center (box, ellipse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// Width and height (box, ellipse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 2]>,
    /// Corners (polygon).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Point>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub members: Vec<String>,
}
