//! Runs every equation found in LaTeX files through the expression parser
//! and the diagram layout, and reports what works and why the rest fails.
//!
//!     cargo run -p goldstone-core --example corpus -- paper.tex more/*.tex
//!
//! Equations are taken from `equation`, `align`, `eqnarray`, `multline`,
//! `gather` (starred or not), `\[ … \]` and `$$ … $$`; one entry per
//! environment, or per `\\` line when the lines are separate equations
//! (`&+=&`, `&=` at the start of each line).

use std::collections::BTreeMap;

use goldstone_core::expr;

const ENVS: &[&str] = &[
    "equation",
    "equation*",
    "align",
    "align*",
    "eqnarray",
    "eqnarray*",
    "multline",
    "multline*",
    "gather",
    "gather*",
    "dmath",
    "dmath*",
];

fn equations(tex: &str) -> Vec<String> {
    // Drop comments.
    let tex: String = tex
        .lines()
        .map(|l| {
            let mut out = String::new();
            let mut prev = ' ';
            for c in l.chars() {
                if c == '%' && prev != '\\' {
                    break;
                }
                out.push(c);
                prev = c;
            }
            out
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = Vec::new();
    for env in ENVS {
        let (open, close) = (format!("\\begin{{{env}}}"), format!("\\end{{{env}}}"));
        let mut rest = tex.as_str();
        while let Some(a) = rest.find(&open) {
            let body = &rest[a + open.len()..];
            let Some(b) = body.find(&close) else { break };
            out.push(body[..b].to_string());
            rest = &body[b + close.len()..];
        }
    }
    for (open, close) in [("\\[", "\\]"), ("$$", "$$")] {
        let mut rest = tex.as_str();
        while let Some(a) = rest.find(open) {
            let body = &rest[a + open.len()..];
            let Some(b) = body.find(close) else { break };
            out.push(body[..b].to_string());
            rest = &body[b + close.len()..];
        }
    }
    out.into_iter()
        .map(|e| {
            // \label, \nonumber, … are not part of the equation.
            let mut e = e;
            while let Some(a) = e.find("\\label{") {
                let end = e[a..].find('}').map_or(e.len(), |b| a + b + 1);
                e.replace_range(a..end, "");
            }
            e.trim().to_string()
        })
        .filter(|e| !e.is_empty())
        .collect()
}

/// The reason with the specifics (indices, names) taken out, for grouping.
fn bucket(msg: &str) -> String {
    let msg = msg.split('\n').next().unwrap_or(msg);
    let msg = msg
        .split_once(": ")
        .filter(|(head, _)| head.starts_with("term "))
        .map_or(msg, |(_, rest)| rest);
    let mut out = String::new();
    let mut in_quote = false;
    for c in msg.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                out.push('…');
            }
            _ if in_quote => {}
            _ => out.push(c),
        }
    }
    let words: Vec<&str> = out.split_whitespace().collect();
    words
        .iter()
        .map(|w| {
            if w.len() <= 3 && w.chars().all(|c| c.is_ascii_lowercase() || c == '\'') {
                "x"
            } else {
                w
            }
        })
        .take(9)
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: corpus FILE.tex …");
        std::process::exit(2);
    }
    let mut reasons: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let (mut total, mut ok, mut partial, mut failed) = (0, 0, 0, 0);
    println!("| file | equations | drawn | partly | failed |\n|---|---|---|---|---|");
    for f in &files {
        let Ok(tex) = std::fs::read_to_string(f) else {
            eprintln!("{f}: cannot read");
            continue;
        };
        let (mut o, mut p, mut x) = (0, 0, 0);
        let eqs = equations(&tex);
        for e in &eqs {
            match expr::from_latex(e) {
                Ok(d) if d.warnings.is_empty() => o += 1,
                Ok(d) => {
                    p += 1;
                    for w in &d.warnings {
                        let r = reasons
                            .entry(format!("(term) {}", bucket(w)))
                            .or_insert((0, e.clone()));
                        r.0 += 1;
                    }
                }
                Err(msg) => {
                    x += 1;
                    let r = reasons.entry(bucket(&msg)).or_insert((0, e.clone()));
                    r.0 += 1;
                }
            }
        }
        let name = f.rsplit('/').next().unwrap_or(f);
        println!("| {name} | {} | {o} | {p} | {x} |", eqs.len());
        total += eqs.len();
        ok += o;
        partial += p;
        failed += x;
    }
    println!("| **total** | {total} | {ok} | {partial} | {failed} |\n");
    let mut rs: Vec<(String, (usize, String))> = reasons.into_iter().collect();
    rs.sort_by_key(|r| std::cmp::Reverse(r.1.0));
    println!("Failure reasons (count, example):\n");
    for (r, (n, ex)) in rs {
        let ex: String = ex.split_whitespace().collect::<Vec<_>>().join(" ");
        let ex: String = ex.chars().take(110).collect();
        println!("- {n:4} × {r}\n         e.g. `{ex}`");
    }
}
