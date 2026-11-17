//! Colors: `"#rrggbb"`, `"#rgb"` or a small set of names.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub const NAMED: &[(&str, Color)] = &[
    ("black", Color::rgb(0, 0, 0)),
    ("white", Color::rgb(255, 255, 255)),
    ("gray", Color::rgb(128, 128, 128)),
    ("lightgray", Color::rgb(200, 200, 200)),
    ("darkgray", Color::rgb(64, 64, 64)),
    ("red", Color::rgb(220, 30, 30)),
    ("green", Color::rgb(20, 150, 50)),
    ("blue", Color::rgb(25, 70, 210)),
    ("cyan", Color::rgb(0, 160, 200)),
    ("magenta", Color::rgb(200, 30, 160)),
    ("yellow", Color::rgb(230, 190, 0)),
    ("orange", Color::rgb(240, 120, 0)),
    ("purple", Color::rgb(120, 40, 170)),
    ("brown", Color::rgb(130, 75, 30)),
];

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `"#rrggbb"`, `"#rgb"` or a named color (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, String> {
        let t = s.trim();
        if let Some(hex) = t.strip_prefix('#') {
            let digits: Vec<u8> = hex
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect::<Option<_>>()
                .ok_or_else(|| format!("invalid color {s:?}: not a hex number"))?;
            return match digits.len() {
                6 => Ok(Color::rgb(
                    digits[0] * 16 + digits[1],
                    digits[2] * 16 + digits[3],
                    digits[4] * 16 + digits[5],
                )),
                3 => Ok(Color::rgb(digits[0] * 17, digits[1] * 17, digits[2] * 17)),
                _ => Err(format!("invalid color {s:?}: use #rrggbb or #rgb")),
            };
        }
        let lower = t.to_ascii_lowercase();
        NAMED
            .iter()
            .find(|(n, _)| *n == lower)
            .map(|(_, c)| *c)
            .ok_or_else(|| {
                let names: Vec<&str> = NAMED.iter().map(|(n, _)| *n).collect();
                format!(
                    "unknown color {s:?}; use #rrggbb or one of: {}",
                    names.join(", ")
                )
            })
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        assert_eq!(Color::parse("#ff0000").unwrap(), Color::rgb(255, 0, 0));
        assert_eq!(Color::parse("#0f0").unwrap(), Color::rgb(0, 255, 0));
        assert_eq!(Color::parse("Black").unwrap(), Color::BLACK);
        assert!(Color::parse("#12345").is_err());
        assert!(Color::parse("chartreuse").is_err());
    }
}
