//! Reading and writing `.gold` files.

use crate::model::Document;
use crate::scene::Diagnostics;

/// Parses a `.gold` document. Syntax errors and unknown fields are reported
/// with line and column.
pub fn parse(src: &str) -> Result<Document, Diagnostics> {
    toml::from_str(src).map_err(|e| Diagnostics(vec![e.to_string().trim_end().to_string()]))
}

/// Serializes a document in canonical form: fixed section order, fields
/// equal to "unset" omitted. Saving an unchanged document reproduces it.
pub fn to_toml(doc: &Document) -> String {
    toml::to_string(doc).expect("documents always serialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let src = include_str!("../../../examples/mbpt2.gold");
        let doc = parse(src).unwrap();
        let out = to_toml(&doc);
        assert_eq!(parse(&out).unwrap(), doc);
        assert_eq!(
            to_toml(&parse(&out).unwrap()),
            out,
            "canonical form is stable"
        );
    }

    #[test]
    fn unknown_field_is_an_error() {
        let err = parse("format = 1\n[[vertex]]\nid = \"a\"\nat = [0, 0]\ncolour = \"red\"\n")
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("colour"), "{msg}");
        assert!(msg.contains("line 5") || msg.contains("5 |"), "{msg}");
    }
}
