//! Prints the outlines used by the editor's boolean-operation icons
//! (`crates/goldstone-app/icons/{union,difference,intersection,exclusion}.gold`).
//! The icons show two overlapping circles and the result of each operation,
//! computed with Goldstone's own boolean code:
//!
//!     cargo run -p goldstone-core --example bool_icons

use goldstone_core::model::{Shape, ShapeKind};
use goldstone_core::shapes::{self, BoolOp};

fn circle(x: f64) -> Shape {
    Shape {
        id: None,
        kind: ShapeKind::Ellipse,
        at: Some([x, 12.0]),
        size: Some([13.0, 13.0]),
        points: None,
        d: None,
        rotation: None,
        class: None,
        color: None,
        fill: None,
        width: None,
        layer: None,
    }
}

fn main() {
    let a = shapes::outline(&circle(9.0)).unwrap();
    let b = shapes::outline(&circle(15.0)).unwrap();
    for op in [
        BoolOp::Union,
        BoolOp::Difference,
        BoolOp::Intersection,
        BoolOp::Exclusion,
    ] {
        let r = shapes::boolean(op, &[a.clone(), b.clone()]).unwrap();
        println!("{}: {}", op.name().to_lowercase(), shapes::format_d(&r));
    }
}
