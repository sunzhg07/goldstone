//! Diagrams from tensor expressions written in LaTeX, e.g.
//!
//! ```text
//! E2 = \sum_{abij} V_{ijab} T_{abij}
//! Z_{abic} = V_{abef} S_{efic}
//! ```
//!
//! Conventions (Shavitt–Bartlett):
//!
//! - `X_{pqrs}` is ⟨pq|X|rs⟩: the first half of the indices are *outgoing*
//!   (bra), the second half *incoming* (ket). `X^{pq}_{rs}` means the same.
//! - Indices `a`–`h` are particles (arrow up), `i`–`n` holes (arrow down);
//!   an index may carry digits or primes (`a1`, `i'`).
//! - Every vertex of an operator joins one outgoing and one incoming index
//!   (`p` with `r`, `q` with `s`). A one-body operator has one vertex, a
//!   two-body operator two, and so on.
//! - An index used twice on the right is an internal line from the vertex
//!   where it is outgoing to the vertex where it is incoming; an index used
//!   once is an external line and must appear on the left.
//! - Interactions (`V W v w g H F f U`) are drawn as a dashed line between
//!   their vertices (one-body: `---X`); every other operator (amplitudes
//!   such as `T`, `S`, `R`) as a thick bar.
//! - Terms separated by `+` / `-` become diagrams side by side. Numeric
//!   prefactors (`\frac{1}{4}`, `0.5`) are kept as text; `\sum_{…}`,
//!   `*`, `\cdot`, `\times` are ignored.

use std::collections::{BTreeMap, HashMap};

use crate::model::*;

/// Layout constants (pt).
const DX: f64 = 60.0; // between the vertices of one operator
const DY: f64 = 60.0; // between levels (operators above one another)
const GAP: f64 = 40.0; // between operators on one level
const EXT: f64 = 30.0; // length of external lines
const TERM_GAP: f64 = 16.0; // between terms (besides room for labels)

const INTERACTIONS: &[&str] = &["V", "W", "v", "w", "g", "H", "F", "f", "U", "u"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Particle,
    Hole,
}

fn kind_of(index: &str) -> Result<Kind, String> {
    match index.chars().next() {
        Some('a'..='h') => Ok(Kind::Particle),
        Some('i'..='n') => Ok(Kind::Hole),
        _ => Err(format!(
            "index {index:?}: use a–h for particles and i–n for holes"
        )),
    }
}

/// One operator of a product, e.g. `V_{ijab}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    /// Name as written (`V`, `T`, `\Gamma`).
    pub name: String,
    /// Outgoing indices (bra).
    pub bra: Vec<String>,
    /// Incoming indices (ket).
    pub ket: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// `+1` or `-1`.
    pub sign: f64,
    /// Prefactor as LaTeX (`\frac{1}{4}`), if any.
    pub factor: Option<String>,
    pub tensors: Vec<Tensor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    /// Left-hand side as LaTeX (`E2`, `Z_{abic}`), if given.
    pub lhs: Option<String>,
    /// Indices of the left-hand side (the external lines).
    pub external: Vec<String>,
    pub terms: Vec<Term>,
}

// ---- parsing ------------------------------------------------------------------

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_whitespace() || c == b'*' || c == b'~')
        {
            self.i += 1;
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        if self.s[self.i..].starts_with(t.as_bytes()) {
            // A command must not continue with letters (`\sum` vs `\summary`).
            let next = self.s.get(self.i + t.len()).copied();
            if t.starts_with('\\') && next.is_some_and(|c| c.is_ascii_alphabetic()) {
                return false;
            }
            self.i += t.len();
            true
        } else {
            false
        }
    }
    /// `{…}` (balanced) or a run of index characters.
    fn group(&mut self) -> Result<String, String> {
        self.skip_ws();
        if self.peek() == Some(b'{') {
            let start = self.i + 1;
            let mut depth = 0;
            while let Some(c) = self.peek() {
                self.i += 1;
                match c {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(String::from_utf8_lossy(&self.s[start..self.i - 1]).into());
                        }
                    }
                    _ => {}
                }
            }
            Err("unbalanced {".into())
        } else {
            let start = self.i;
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'\'')
            {
                self.i += 1;
            }
            Ok(String::from_utf8_lossy(&self.s[start..self.i]).into())
        }
    }
}

/// Splits `abij` / `a1 b' i j` into indices: a letter followed by digits
/// or primes.
fn indices(s: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for c in s.chars() {
        if c.is_whitespace() || c == ',' {
            continue;
        }
        if c.is_ascii_alphabetic() {
            out.push(c.to_string());
        } else if (c.is_ascii_digit() || c == '\'') && !out.is_empty() {
            out.last_mut().expect("non-empty").push(c);
        } else {
            return Err(format!("unexpected {c:?} in the indices {s:?}"));
        }
    }
    Ok(out)
}

fn parse_tensor(p: &mut P) -> Result<Tensor, String> {
    let start = p.i;
    if p.peek() == Some(b'\\') {
        p.i += 1;
    }
    while p.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
        p.i += 1;
        if p.s[start] != b'\\' {
            break; // names are single letters (V_ij…), commands are \Gamma
        }
    }
    while p.peek() == Some(b'\'') {
        p.i += 1;
    }
    let name = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
    let (mut sub, mut sup) = (None, None);
    loop {
        p.skip_ws();
        if p.eat("_") {
            sub = Some(indices(&p.group()?)?);
        } else if p.eat("^") {
            sup = Some(indices(&p.group()?)?);
        } else {
            break;
        }
    }
    let (bra, ket) = match (sup, sub) {
        (Some(up), Some(down)) => (up, down),
        (None, Some(all)) | (Some(all), None) => {
            if all.len() % 2 != 0 {
                return Err(format!(
                    "{name}: an odd number of indices ({}); write X_{{pq rs}} = <pq|X|rs> or X^{{pq}}_{{rs}}",
                    all.len()
                ));
            }
            let h = all.len() / 2;
            (all[..h].to_vec(), all[h..].to_vec())
        }
        (None, None) => (vec![], vec![]),
    };
    if bra.len() != ket.len() {
        return Err(format!(
            "{name}: {} outgoing but {} incoming indices",
            bra.len(),
            ket.len()
        ));
    }
    Ok(Tensor { name, bra, ket })
}

/// Parses `lhs = rhs` (or just `rhs`).
pub fn parse(src: &str) -> Result<Equation, String> {
    let src = src.trim().trim_matches('$').trim();
    let (lhs, rhs) = match src.split_once('=') {
        Some((l, r)) => (Some(l.trim()), r),
        None => (None, src),
    };
    let external = match lhs {
        Some(l) => {
            let mut p = P {
                s: l.as_bytes(),
                i: 0,
            };
            // Name: anything up to the first _ or ^ (E2, E^{(2)}, Z).
            let mut ext = Vec::new();
            while p.peek().is_some_and(|c| c != b'_') {
                p.i += 1;
            }
            if p.eat("_") {
                ext = indices(&p.group()?)?;
            }
            ext
        }
        None => Vec::new(),
    };
    let mut p = P {
        s: rhs.as_bytes(),
        i: 0,
    };
    let mut terms = Vec::new();
    let mut sign = 1.0;
    let mut cur: Option<Term> = None;
    loop {
        p.skip_ws();
        let Some(c) = p.peek() else { break };
        if c == b'+' || c == b'-' {
            p.i += 1;
            if let Some(t) = cur.take() {
                terms.push(t);
            }
            sign = if c == b'-' { -1.0 } else { 1.0 };
            continue;
        }
        let term = cur.get_or_insert_with(|| Term {
            sign,
            factor: None,
            tensors: Vec::new(),
        });
        if p.eat("\\sum") {
            p.skip_ws();
            if p.eat("_") {
                p.group()?;
            }
            continue;
        }
        if p.eat("\\cdot") || p.eat("\\times") || p.eat("\\,") || p.eat("\\;") {
            continue;
        }
        if p.eat("\\frac") || p.eat("\\tfrac") || p.eat("\\dfrac") {
            let (a, b) = (p.group()?, p.group()?);
            let f = format!("\\frac{{{a}}}{{{b}}}");
            term.factor = Some(match term.factor.take() {
                Some(g) => format!("{g}{f}"),
                None => f,
            });
            continue;
        }
        if c.is_ascii_digit() || c == b'.' {
            let start = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
                p.i += 1;
            }
            let f = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
            term.factor = Some(match term.factor.take() {
                Some(g) => format!("{g}{f}"),
                None => f,
            });
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'\\' {
            let t = parse_tensor(&mut p)?;
            if t.bra.is_empty() {
                return Err(format!("{}: an operator needs indices", t.name));
            }
            term.tensors.push(t);
            continue;
        }
        return Err(format!(
            "unexpected {:?} in {rhs:?}",
            String::from_utf8_lossy(&p.s[p.i..(p.i + 1).min(p.s.len())])
        ));
    }
    if let Some(t) = cur.take() {
        terms.push(t);
    }
    terms.retain(|t| !t.tensors.is_empty());
    if terms.is_empty() {
        return Err("no operators on the right-hand side".into());
    }
    Ok(Equation {
        lhs: lhs.map(str::to_string),
        external,
        terms,
    })
}

// ---- layout -------------------------------------------------------------------

/// A vertex: operator `t`, slot `j` (joins bra[j] and ket[j]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct V {
    t: usize,
    j: usize,
}

#[derive(Debug, Clone)]
struct Line {
    index: String,
    kind: Kind,
    /// Vertex where the index is outgoing / incoming (None: external).
    out: Option<V>,
    inn: Option<V>,
}

fn is_interaction(name: &str) -> bool {
    INTERACTIONS.contains(&name)
}

fn id_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
}

fn latex_name(name: &str) -> String {
    format!("${name}$")
}

/// Lines of one term, checked.
fn lines(term: &Term, external: &[String], has_lhs: bool) -> Result<Vec<Line>, String> {
    let mut map: BTreeMap<String, Line> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for (t, ten) in term.tensors.iter().enumerate() {
        for (j, (o, i)) in ten.bra.iter().zip(&ten.ket).enumerate() {
            for (idx, outgoing) in [(o, true), (i, false)] {
                let kind = kind_of(idx)?;
                let l = map.entry(idx.clone()).or_insert_with(|| {
                    order.push(idx.clone());
                    Line {
                        index: idx.clone(),
                        kind,
                        out: None,
                        inn: None,
                    }
                });
                let slot = if outgoing { &mut l.out } else { &mut l.inn };
                if slot.is_some() {
                    return Err(format!(
                        "index {idx} is {} twice; a contracted index must be outgoing (bra) on one operator and incoming (ket) on the other",
                        if outgoing { "outgoing" } else { "incoming" }
                    ));
                }
                *slot = Some(V { t, j });
            }
        }
    }
    for l in map.values() {
        let free = l.out.is_none() || l.inn.is_none();
        let listed = external.contains(&l.index);
        if free && has_lhs && !listed {
            return Err(format!(
                "index {} appears once on the right but not on the left",
                l.index
            ));
        }
        if !free && listed {
            return Err(format!(
                "index {} is summed over (it appears twice on the right) but also stands on the left",
                l.index
            ));
        }
    }
    for e in external {
        kind_of(e)?;
        if !map.contains_key(e) {
            return Err(format!("index {e} is on the left but not on the right"));
        }
    }
    Ok(order
        .into_iter()
        .map(|k| map.remove(&k).expect("listed"))
        .collect())
}

/// Vertical level of each operator: an arrow goes up for particles and
/// down for holes, so a particle line puts its incoming end above its
/// outgoing end, a hole line below.
fn levels(n: usize, lines: &[Line]) -> Result<Vec<usize>, String> {
    // below[a] contains b: operator a is below operator b.
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for l in lines {
        if let (Some(o), Some(i)) = (l.out, l.inn)
            && o.t != i.t
        {
            edges.push(match l.kind {
                Kind::Particle => (o.t, i.t),
                Kind::Hole => (i.t, o.t),
            });
        }
    }
    let mut level = vec![0usize; n];
    for _ in 0..=n {
        let mut changed = false;
        for &(a, b) in &edges {
            if level[b] < level[a] + 1 {
                level[b] = level[a] + 1;
                changed = true;
            }
        }
        if !changed {
            return Ok(level);
        }
    }
    Err("the operators cannot be ordered in time: the lines form a cycle".into())
}

fn permutations(k: usize) -> Vec<Vec<usize>> {
    if k == 0 {
        return vec![vec![]];
    }
    let mut out = Vec::new();
    for p in permutations(k - 1) {
        for pos in 0..=p.len() {
            let mut q = p.clone();
            q.insert(pos, k - 1);
            out.push(q);
        }
    }
    out
}

/// Positions of all vertices of one term, with the term's left edge at 0.
fn place(term: &Term, lines: &[Line], level: &[usize]) -> HashMap<V, (f64, f64)> {
    let n = term.tensors.len();
    let mut pos: HashMap<V, (f64, f64)> = HashMap::new();
    let max_level = level.iter().copied().max().unwrap_or(0);
    let neighbors = |v: V| -> Vec<V> {
        lines
            .iter()
            .filter_map(|l| match (l.out, l.inn) {
                (Some(a), Some(b)) if a == v && b.t != v.t => Some(b),
                (Some(a), Some(b)) if b == v && a.t != v.t => Some(a),
                _ => None,
            })
            .collect()
    };
    for lv in 0..=max_level {
        let mut placed: Vec<(usize, f64, Vec<usize>)> = Vec::new(); // (tensor, x0, order)
        let mut cursor = 0.0;
        for t in (0..n).filter(|t| level[*t] == lv) {
            let k = term.tensors[t].bra.len();
            // Where each vertex would like to be: over/under its partners.
            let targets: Vec<Option<f64>> = (0..k)
                .map(|j| {
                    let xs: Vec<f64> = neighbors(V { t, j })
                        .iter()
                        .filter_map(|v| pos.get(v).map(|p| p.0))
                        .collect();
                    (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64)
                })
                .collect();
            let mut best: Option<(f64, f64, Vec<usize>)> = None;
            for perm in permutations(k.min(4)) {
                let perm: Vec<usize> = perm.into_iter().chain(4..k).collect();
                // perm[slot position] = vertex j
                let pairs: Vec<(f64, f64)> = perm
                    .iter()
                    .enumerate()
                    .filter_map(|(at, &j)| targets[j].map(|x| (at as f64 * DX, x)))
                    .collect();
                let x0 = if pairs.is_empty() {
                    cursor
                } else {
                    pairs.iter().map(|(o, x)| x - o).sum::<f64>() / pairs.len() as f64
                };
                let cost: f64 = pairs.iter().map(|(o, x)| (x0 + o - x).powi(2)).sum();
                if best.as_ref().is_none_or(|b| cost < b.0 - 1e-9) {
                    best = Some((cost, x0, perm));
                }
            }
            let (_, x0, perm) = best.expect("at least one permutation");
            cursor = cursor.max(x0 + (k - 1) as f64 * DX + GAP);
            placed.push((t, x0, perm));
        }
        // Resolve overlaps from left to right.
        placed.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut right = f64::NEG_INFINITY;
        for (t, x0, perm) in &mut placed {
            let k = term.tensors[*t].bra.len();
            if *x0 < right + GAP {
                *x0 = right + GAP;
            }
            right = *x0 + (k - 1) as f64 * DX;
            for (at, &j) in perm.iter().enumerate() {
                pos.insert(V { t: *t, j }, (*x0 + at as f64 * DX, lv as f64 * DY));
            }
        }
    }
    // Shift so the leftmost vertex is at x = 0.
    let min_x = pos.values().map(|p| p.0).fold(f64::INFINITY, f64::min);
    for p in pos.values_mut() {
        p.0 -= min_x;
    }
    pos
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

struct Builder {
    doc: Document,
}

impl Builder {
    fn vertex(
        &mut self,
        id: String,
        at: (f64, f64),
        class: Option<&str>,
        kind: Option<VertexKind>,
    ) -> String {
        let mut v = Vertex {
            id: id.clone(),
            at: [r2(at.0), r2(at.1)],
            class: class.map(str::to_string),
            kind,
            size: None,
            rotation: None,
            color: None,
            width: None,
            fill: None,
            layer: None,
            label: None,
            label_anchor: None,
        };
        if kind.is_none() && class.is_none() {
            v.kind = None;
        }
        self.doc.vertices.push(v);
        id
    }

    fn edge(&mut self, id: String, from: String, to: String, class: &str) -> &mut Edge {
        self.doc.edges.push(Edge {
            id: Some(id),
            from,
            to,
            class: Some(class.to_string()),
            bend: None,
            controls: None,
            loop_: None,
            stroke: None,
            color: None,
            width: None,
            layer: None,
            double: None,
            double_sep: None,
            dash: None,
            amplitude: None,
            wavelength: None,
            arrow: None,
            arrow_pos: None,
            arrow_length: None,
            arrow_width: None,
            label: None,
            label_side: None,
            label_pos: None,
            label_offset: None,
        });
        self.doc.edges.last_mut().expect("just pushed")
    }

    fn text(&mut self, id: String, at: (f64, f64), content: String, anchor: Anchor) {
        self.doc.texts.push(Text {
            id: Some(id),
            at: [r2(at.0), r2(at.1)],
            content,
            class: None,
            anchor: Some(anchor),
            size: None,
            rotation: None,
            color: None,
            layer: None,
        });
    }
}

fn style(f: impl FnOnce(&mut Style)) -> Style {
    let mut s = Style::default();
    f(&mut s);
    s
}

/// Builds one term at horizontal offset `x0`; returns its right edge and
/// the ids of its objects.
fn build_term(
    b: &mut Builder,
    term: &Term,
    external: &[String],
    has_lhs: bool,
    prefix: &str,
    x0: f64,
) -> Result<(f64, f64, Vec<String>), String> {
    let lines = lines(term, external, has_lhs)?;
    let level = levels(term.tensors.len(), &lines)?;
    let pos = place(term, &lines, &level);
    let mut ids: Vec<String> = Vec::new();
    let vid = |v: V| {
        format!(
            "{prefix}{}{}_{}",
            id_name(&term.tensors[v.t].name),
            v.t + 1,
            v.j + 1
        )
    };
    let at = |v: V| {
        let p = pos[&v];
        (p.0 + x0, p.1)
    };
    let max_level = level.iter().copied().max().unwrap_or(0);

    // Operators: vertices and the interaction line / amplitude bar.
    for (t, ten) in term.tensors.iter().enumerate() {
        let k = ten.bra.len();
        let inter = is_interaction(&ten.name);
        let mut vs: Vec<V> = (0..k).map(|j| V { t, j }).collect();
        vs.sort_by(|a, b| at(*a).0.total_cmp(&at(*b).0));
        for &v in &vs {
            let class = if inter { Some("v") } else { None };
            ids.push(b.vertex(vid(v), at(v), class, None));
        }
        // Operator label below the bottom operator, otherwise above.
        let below = level[t] == 0 && max_level > 0;
        let side = if below { Side::Right } else { Side::Left };
        let oid = format!("{prefix}{}{}", id_name(&ten.name), t + 1);
        if k == 1 {
            let v = vs[0];
            let p = at(v);
            if inter {
                // ---X
                let x = b.vertex(
                    format!("{oid}_x"),
                    (p.0 + 28.0, p.1),
                    None,
                    Some(VertexKind::Cross),
                );
                ids.push(x.clone());
                let e = b.edge(oid.clone(), vid(v), x, "interaction");
                e.label = Some(latex_name(&ten.name));
                e.label_side = Some(side);
            } else {
                let l = b.vertex(format!("{oid}_l"), (p.0 - 10.0, p.1), None, None);
                let r = b.vertex(format!("{oid}_r"), (p.0 + 10.0, p.1), None, None);
                ids.push(l.clone());
                ids.push(r.clone());
                let e = b.edge(oid.clone(), l, r, "amplitude");
                e.label = Some(latex_name(&ten.name));
                e.label_side = Some(side);
            }
            ids.push(oid);
        } else if inter {
            for (n, w) in vs.windows(2).enumerate() {
                let id = if n == 0 {
                    oid.clone()
                } else {
                    format!("{oid}_line{}", n + 1)
                };
                let e = b.edge(id.clone(), vid(w[0]), vid(w[1]), "interaction");
                if n == 0 {
                    e.label = Some(latex_name(&ten.name));
                    e.label_side = Some(side);
                }
                ids.push(id);
            }
        } else {
            let e = b.edge(oid.clone(), vid(vs[0]), vid(vs[k - 1]), "amplitude");
            e.label = Some(latex_name(&ten.name));
            e.label_side = Some(side);
            ids.push(oid);
        }
    }

    // Internal lines: group the lines between the same two vertices and
    // spread them as arcs.
    let mut bundles: BTreeMap<(V, V), Vec<usize>> = BTreeMap::new();
    for (n, l) in lines.iter().enumerate() {
        if let (Some(o), Some(i)) = (l.out, l.inn) {
            let key = if o <= i { (o, i) } else { (i, o) };
            bundles.entry(key).or_default().push(n);
        }
    }
    for ((u, _), members) in &bundles {
        let m = members.len();
        let same_op = {
            let l = &lines[members[0]];
            l.out.map(|v| v.t) == l.inn.map(|v| v.t)
        };
        for (k, &n) in members.iter().enumerate() {
            let l = &lines[n];
            let (o, i) = (l.out.expect("internal"), l.inn.expect("internal"));
            let id = format!("{prefix}{}", l.index);
            let class = if l.kind == Kind::Particle {
                "particle"
            } else {
                "hole"
            };
            // Bends spread symmetrically; a line within one operator
            // bulges away from its interaction line.
            let spread = 40.0f64.min(80.0 / m as f64 + 20.0);
            let mut bend = if m == 1 {
                if same_op { 60.0 } else { 0.0 }
            } else {
                spread * (1.0 - 2.0 * k as f64 / (m - 1) as f64)
            };
            if o != *u {
                bend = -bend;
            }
            let e = b.edge(id.clone(), vid(o), vid(i), class);
            e.arrow = Some(ArrowDir::Forward);
            if o == i {
                e.loop_ = Some(LoopSpec {
                    radius: None,
                    angle: Some(if at(o).1 > 0.0 { 90.0 } else { 270.0 }),
                });
            } else if bend != 0.0 {
                e.bend = Some(r2(bend));
            }
            e.label = Some(format!("${}$", l.index));
            e.label_side = Some(if bend < 0.0 { Side::Right } else { Side::Left });
            if bend == 0.0 && o != i {
                // Off the middle, so labels of crossing lines do not meet.
                e.label_pos = Some(0.35);
            }
            ids.push(id);
        }
    }

    // External lines: an outgoing particle or incoming hole leaves upward,
    // the others downward; they lean away from the operator's center.
    let mut used: HashMap<(V, bool), Vec<f64>> = HashMap::new();
    for l in lines.iter().filter(|l| l.out.is_none() || l.inn.is_none()) {
        let (v, outgoing) = match (l.out, l.inn) {
            (Some(o), None) => (o, true),
            (None, Some(i)) => (i, false),
            _ => unreachable!(),
        };
        let up = matches!(
            (l.kind, outgoing),
            (Kind::Particle, true) | (Kind::Hole, false)
        );
        let p = at(v);
        let ten = &term.tensors[v.t];
        let xs: Vec<f64> = (0..ten.bra.len()).map(|j| at(V { t: v.t, j }).0).collect();
        let center = xs.iter().sum::<f64>() / xs.len() as f64;
        let lean = if (p.0 - center).abs() < 1e-9 {
            0.0
        } else {
            (p.0 - center).signum()
        };
        // Slopes (dx per unit height) already used on this side of the
        // vertex: internal lines, then earlier external lines.
        let slopes = used.entry((v, up)).or_insert_with(|| {
            lines
                .iter()
                .filter_map(|l| match (l.out, l.inn) {
                    (Some(a), Some(c)) if a == v && c != v => Some(c),
                    (Some(a), Some(c)) if c == v && a != v => Some(a),
                    _ => None,
                })
                .map(at)
                .filter(|q| (q.1 - p.1).abs() > 1e-9 && (q.1 > p.1) == up)
                .map(|q| (q.0 - p.0) / (q.1 - p.1).abs())
                .collect()
        });
        let dir = if lean < 0.0 { -1.0 } else { 1.0 };
        let dx = [0.0, -1.0, 1.0, -2.0, 2.0, 3.0, -3.0]
            .iter()
            .map(|k| lean * 12.0 + k * dir * 20.0)
            .find(|dx| slopes.iter().all(|s| (dx / EXT - s).abs() > 0.5))
            .unwrap_or(lean * 12.0);
        slopes.push(dx / EXT);
        let end_at = (p.0 + dx, p.1 + if up { EXT } else { -EXT });
        let end = b.vertex(format!("{prefix}{}_end", l.index), end_at, None, None);
        ids.push(end.clone());
        let id = format!("{prefix}{}", l.index);
        let class = if l.kind == Kind::Particle {
            "particle"
        } else {
            "hole"
        };
        let (from, to) = if outgoing {
            (vid(v), end)
        } else {
            (end, vid(v))
        };
        let e = b.edge(id.clone(), from, to, class);
        e.arrow = Some(ArrowDir::Forward);
        e.label = Some(format!("${}$", l.index));
        e.label_pos = Some(if outgoing { 0.8 } else { 0.2 });
        e.label_side = Some(if dx < 0.0 { Side::Left } else { Side::Right });
        if (up && outgoing) || (!up && !outgoing) {
            // label on the outer side of the bundle
            e.label_side = Some(if dx < 0.0 { Side::Left } else { Side::Right });
        } else {
            e.label_side = Some(if dx < 0.0 { Side::Right } else { Side::Left });
        }
        ids.push(id);
    }

    let xs = pos.values().map(|p| p.0 + x0);
    let right = xs.clone().fold(f64::NEG_INFINITY, f64::max);
    let ys: Vec<f64> = pos.values().map(|p| p.1).collect();
    let mid = (ys.iter().copied().fold(f64::INFINITY, f64::min)
        + ys.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        / 2.0;
    Ok((right, mid, ids))
}

/// Rough width of typeset LaTeX (pt), for placing the text before a term.
fn text_width(latex: &str) -> f64 {
    let mut n = 0.0;
    let mut chars = latex.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                // A command name is not printed (\frac, \sum).
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    chars.next();
                }
            }
            '{' | '}' | '_' | '^' | '$' | ' ' => {}
            _ => n += 1.0,
        }
    }
    n * 5.0 + 2.0
}

/// The left-hand side as LaTeX with braced indices (`Z_abic` → `Z_{abic}`).
fn lhs_latex(lhs: &str, external: &[String]) -> String {
    match lhs.split_once('_') {
        Some((name, _)) if !external.is_empty() => {
            format!("{}_{{{}}}", name.trim(), external.concat())
        }
        _ => lhs.trim().to_string(),
    }
}

/// Builds the diagram(s) of an equation as a document.
pub fn diagram(eq: &Equation) -> Result<Document, String> {
    let mut b = Builder {
        doc: Document::default(),
    };
    b.doc.canvas.grid = Some(10.0);
    let st = &mut b.doc.styles;
    st.insert(
        "particle".into(),
        style(|s| s.arrow = Some(ArrowDir::Forward)),
    );
    st.insert("hole".into(), style(|s| s.arrow = Some(ArrowDir::Forward)));
    st.insert(
        "interaction".into(),
        style(|s| s.stroke = Some(Stroke::Dashed)),
    );
    st.insert("amplitude".into(), style(|s| s.width = Some(3.0)));
    st.insert("v".into(), style(|s| s.kind = Some(VertexKind::Dot)));

    let many = eq.terms.len() > 1;
    let mut cursor = 0.0;
    for (n, term) in eq.terms.iter().enumerate() {
        let first = n == 0;
        let prefix = if many {
            format!("d{}_", n + 1)
        } else {
            String::new()
        };
        // Text before the term: "E2 =", "+", "- \frac{1}{2}".
        let mut pre: Vec<String> = Vec::new();
        if first && let Some(l) = &eq.lhs {
            pre.push(format!("{} =", lhs_latex(l, &eq.external)));
        }
        if term.sign < 0.0 {
            pre.push("-".into());
        } else if !first {
            pre.push("+".into());
        }
        pre.extend(term.factor.clone());
        let pre = pre.join(" ");
        // Build at the origin, then shift right of the text.
        let v0 = b.doc.vertices.len();
        let (_, mid, mut ids) =
            build_term(&mut b, term, &eq.external, eq.lhs.is_some(), &prefix, 0.0)?;
        let xs: Vec<f64> = b.doc.vertices[v0..].iter().map(|v| v.at[0]).collect();
        let (min_x, max_x) = (
            xs.iter().copied().fold(f64::INFINITY, f64::min),
            xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        );
        let text_w = if pre.is_empty() {
            0.0
        } else {
            text_width(&pre) + 8.0
        };
        // Room for arcs bulging out and their labels on the left.
        let shift = cursor + text_w + 32.0 - min_x;
        for v in &mut b.doc.vertices[v0..] {
            v.at[0] = r2(v.at[0] + shift);
        }
        if !pre.is_empty() {
            let id = format!("{prefix}eq");
            b.text(id.clone(), (cursor, mid), format!("${pre}$"), Anchor::West);
            ids.push(id);
        }
        if many {
            b.doc.groups.push(Group {
                id: format!("d{}", n + 1),
                members: ids,
            });
        }
        cursor = max_x + shift + 22.0 + TERM_GAP / 2.0;
    }
    crate::scene::build(&b.doc).map_err(|e| format!("internal layout error: {e}"))?;
    Ok(b.doc)
}

/// Parses and builds in one step.
pub fn from_latex(src: &str) -> Result<Document, String> {
    diagram(&parse(src)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_index_styles() {
        let eq = parse(r"E2=\sum_{abij} \frac{1}{4} V_ijab * T^{ab}_{ij}").unwrap();
        assert_eq!(eq.lhs.as_deref(), Some("E2"));
        assert!(eq.external.is_empty());
        let t = &eq.terms[0];
        assert_eq!(t.factor.as_deref(), Some(r"\frac{1}{4}"));
        assert_eq!(t.tensors[0].bra, vec!["i", "j"]);
        assert_eq!(t.tensors[0].ket, vec!["a", "b"]);
        assert_eq!(t.tensors[1].bra, vec!["a", "b"]);
        assert_eq!(t.tensors[1].ket, vec!["i", "j"]);
    }

    #[test]
    fn mbpt2_energy() {
        let d = from_latex(r"E2=\sum_abij V_ijab * T_abij").unwrap();
        // V above T, each V vertex over a T vertex, two arcs per pair.
        let y = |id: &str| d.vertices.iter().find(|v| v.id == id).unwrap().at;
        assert_eq!(y("V1_1")[1], 60.0);
        assert_eq!(y("T2_1")[1], 0.0);
        assert_eq!(y("V1_1")[0], y("T2_1")[0]);
        let arcs = d.edges.iter().filter(|e| e.bend.is_some()).count();
        assert_eq!(arcs, 4);
        // Particle arrows point up, hole arrows down.
        for e in &d.edges {
            let (f, t) = (y(&e.from), y(&e.to));
            match e.class.as_deref() {
                Some("particle") => assert!(t[1] > f[1]),
                Some("hole") => assert!(t[1] < f[1]),
                _ => {}
            }
        }
    }

    #[test]
    fn open_diagram() {
        let d = from_latex(r"Z_abic=V_abef* S_efic").unwrap();
        let ends: Vec<&str> = d
            .vertices
            .iter()
            .filter(|v| v.id.ends_with("_end"))
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(ends.len(), 4, "{ends:?}");
        let at = |id: &str| d.vertices.iter().find(|v| v.id == id).unwrap().at;
        assert!(
            at("a_end")[1] > at("V1_1")[1],
            "outgoing particle leaves upward"
        );
        assert!(
            at("c_end")[1] < at("S2_1")[1],
            "incoming particle comes from below"
        );
        assert!(
            at("i_end")[1] > at("S2_1")[1],
            "incoming hole comes from above"
        );
    }

    #[test]
    fn errors_are_explained() {
        let e = from_latex("E=V_ijab T_abik").unwrap_err();
        assert!(e.contains("index"), "{e}");
        let e = from_latex("E=V_ija").unwrap_err();
        assert!(e.contains("odd"), "{e}");
        let e = from_latex("E=V_ijpq T_pqij").unwrap_err();
        assert!(e.contains("a–h"), "{e}");
        let e = from_latex("E=V_ijab T_ijab").unwrap_err();
        assert!(e.contains("twice"), "{e}");
    }

    #[test]
    fn several_terms_and_one_body() {
        let d = from_latex(r"Z_ai = f_ac T_ci - f_ki T_ak + V_kaci T_ck").unwrap();
        assert_eq!(d.groups.len(), 3);
        assert!(d.vertices.iter().any(|v| v.kind == Some(VertexKind::Cross)));
    }
}
