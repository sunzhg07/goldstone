//! Layout quality of "Diagram from LaTeX" over every equation in LaTeX
//! files: problems per diagram type, and the worst equations.
//!
//!     cargo run --release -p goldstone-core --example layout_report -- FILE.tex … [--worst N] [--dump DIR]
//!
//! With `--dump DIR` the worst equations are written to DIR/worst.txt
//! (one per line) for rendering with `goldstone latex`.

use std::collections::BTreeMap;

use goldstone_core::{expr, quality, tex};

#[derive(Default)]
struct Stats {
    n: usize,
    score: f64,
    crossings: usize,
    through: usize,
    ext_over: usize,
    near: usize,
    label_overlaps: usize,
    label_lines: usize,
    slanted: usize,
    width: f64,
    with_problem: usize,
}

fn main() {
    let mut files = Vec::new();
    let mut worst_n = 15;
    let mut dump: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--worst" => worst_n = args.next().and_then(|v| v.parse().ok()).unwrap_or(15),
            "--dump" => dump = args.next(),
            _ => files.push(a),
        }
    }
    let mut by_type: BTreeMap<String, Stats> = BTreeMap::new();
    let mut all = Stats::default();
    let mut worst: Vec<(f64, String, String)> = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        for eq in tex::equations(&src) {
            let Ok(parsed) = expr::parse(&eq.source) else {
                continue;
            };
            let Ok(d) = expr::diagram(&parsed) else {
                continue;
            };
            // Diagram type: operators by name and rank, e.g. "V2 t1 t3".
            let mut ops: Vec<String> = parsed.terms[0]
                .tensors
                .iter()
                .map(|t| {
                    let base: String = t
                        .name
                        .chars()
                        .filter(|c| c.is_ascii_alphabetic())
                        .take(1)
                        .collect();
                    format!("{}{}", base.to_ascii_uppercase(), t.bra.len())
                })
                .collect();
            ops.sort();
            let kind = if parsed.terms.len() > 1 {
                format!("{} (+{} terms)", ops.join(" "), parsed.terms.len() - 1)
            } else {
                ops.join(" ")
            };
            let q = quality::measure(&d.doc);
            let s = q.score();
            for st in [by_type.entry(kind.clone()).or_default(), &mut all] {
                st.n += 1;
                st.score += s;
                st.crossings += q.crossings;
                st.through += q.through_operators;
                st.ext_over += q.external_over_interaction;
                st.near += q.near_vertices;
                st.label_overlaps += q.label_overlaps;
                st.label_lines += q.label_on_lines;
                st.slanted += q.slanted;
                st.width += q.width;
                st.with_problem += usize::from(s >= 2.0);
            }
            let one: String = eq.source.split_whitespace().collect::<Vec<_>>().join(" ");
            worst.push((s, format!("{f}:{} [{kind}] {q:?}", eq.line), one));
        }
    }
    let row = |name: &str, s: &Stats| {
        let n = s.n.max(1) as f64;
        println!(
            "| {name} | {} | {:.0}% | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.0} |",
            s.n,
            100.0 * s.with_problem as f64 / n,
            s.score / n,
            s.crossings as f64 / n,
            s.through as f64 / n,
            s.ext_over as f64 / n,
            s.near as f64 / n,
            s.label_overlaps as f64 / n,
            s.label_lines as f64 / n,
            s.slanted as f64 / n,
            s.width / n
        );
    };
    println!(
        "Per diagram (averages): score, crossings, lines through operators, lines near vertices, label overlaps, labels on lines, slanted lines, width (pt)\n"
    );
    println!(
        "| type | n | with problems | score | cross | through op | ext×V | near vertex | label×label | label×line | slanted | width |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut types: Vec<(&String, &Stats)> = by_type.iter().collect();
    types.sort_by(|a, b| (b.1.score / b.1.n as f64).total_cmp(&(a.1.score / a.1.n as f64)));
    for (k, s) in types.iter().take(25) {
        row(k, s);
    }
    row("**all**", &all);
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\nWorst {worst_n}:\n");
    for (s, what, src) in worst.iter().take(worst_n) {
        println!("- {s:.0}  {what}\n      {src}");
    }
    if let Some(dir) = dump {
        let lines: Vec<&str> = worst.iter().take(worst_n).map(|w| w.2.as_str()).collect();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(format!("{dir}/worst.txt"), lines.join("\n"));
    }
}
