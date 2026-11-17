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
const EXT: f64 = 34.0; // length of external lines
const TERM_GAP: f64 = 16.0; // between terms (besides room for labels)

const INTERACTIONS: &[&str] = &["V", "W", "v", "w", "g", "H", "F", "f", "U", "u"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Particle,
    Hole,
    /// Either (a general orbital): drawn without an arrow.
    General,
}

/// The kind of a line. With occupation factors (IMSRG notation) every
/// index is a general orbital unless `n_a` (hole) or `\bar n_a` (particle)
/// says otherwise; otherwise a–h are particles and i–n holes, and other
/// letters (p, q, …) or digits are general.
fn kind_in(term: &Term, index: &str) -> Kind {
    if let Some(k) = term.classes.get(index) {
        return *k;
    }
    if term.general {
        return Kind::General;
    }
    match index.chars().next() {
        Some('a'..='h') => Kind::Particle,
        Some('i'..='n') => Kind::Hole,
        _ => Kind::General,
    }
}

/// One operator of a product, e.g. `V_{ijab}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    /// Name as written (`V`, `t`, `\tau`, `\mathcal{W}`).
    pub name: String,
    /// Outgoing indices (bra).
    pub bra: Vec<String>,
    /// Incoming indices (ket).
    pub ket: Vec<String>,
    /// Written as `X_{..}^{..}`: which of the two index groups is the bra
    /// differs between notations (`t_{ij}^{ab}` vs `\mathcal{W}_{mn}^{ij}`),
    /// so it may be swapped to make the contractions consistent.
    pub flexible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// `+1` or `-1`.
    pub sign: f64,
    /// Prefactor as LaTeX (`\frac{1}{4}`), if any.
    pub factor: Option<String>,
    pub tensors: Vec<Tensor>,
    /// Line of the equation (`\\` starts a new row of diagrams).
    pub row: usize,
    /// Kinds fixed by occupation factors (`n_a`: hole, `\bar n_a`: particle).
    pub classes: BTreeMap<String, Kind>,
    /// IMSRG notation: indices without an occupation factor are general.
    pub general: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    /// Left-hand side as LaTeX (`E2`, `Z_{abic}`, `0`), if given.
    pub lhs: Option<String>,
    /// Indices of the left-hand side (the external lines). Empty: the
    /// external lines are the indices used once on the right.
    pub external: Vec<String>,
    pub terms: Vec<Term>,
}

// ---- parsing ------------------------------------------------------------------

struct P<'a> {
    s: &'a [u8],
    i: usize,
    /// Counts top-level products, to keep the pieces of one term together
    /// when occupation factors are multiplied out.
    origin: usize,
}

impl P<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn rest(&self) -> &[u8] {
        &self.s[self.i.min(self.s.len())..]
    }
    /// Skips spacing and layout markup: blanks, `&`, `*`, `~`, `\quad`,
    /// `\,`, `\left`, `\right`, `\cdot`, `\times`, `\begin{…}`, `\end{…}`.
    fn skip_ws(&mut self) {
        loop {
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_whitespace() || matches!(c, b'*' | b'~' | b'&'))
            {
                self.i += 1;
            }
            let before = self.i;
            for cmd in [
                "\\quad",
                "\\qquad",
                "\\left",
                "\\right",
                "\\cdot",
                "\\times",
                "\\displaystyle",
                "\\nonumber",
                "\\notag",
                "\\Biggl",
                "\\Biggr",
                "\\biggl",
                "\\biggr",
                "\\Bigg",
                "\\bigg",
                "\\Bigl",
                "\\Bigr",
                "\\bigl",
                "\\bigr",
                "\\Big",
                "\\big",
            ] {
                if self.eat(cmd) {
                    break;
                }
            }
            for punct in ["\\,", "\\;", "\\!", "\\:", "\\ "] {
                if self.eat(punct) {
                    break;
                }
            }
            if self.eat("\\begin") || self.eat("\\end") {
                let _ = self.group();
            }
            if self.i == before {
                return;
            }
        }
    }
    fn eat(&mut self, t: &str) -> bool {
        if self.rest().starts_with(t.as_bytes()) {
            // A command must not continue with letters (`\sum` vs `\summary`).
            let next = self.s.get(self.i + t.len()).copied();
            let is_cmd = t.starts_with('\\') && t[1..].bytes().all(|c| c.is_ascii_alphabetic());
            if is_cmd && next.is_some_and(|c| c.is_ascii_alphabetic()) {
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
        while self.peek() == Some(b' ') {
            self.i += 1;
        }
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
            // Without braces LaTeX takes one character (`t_i^a`); a run such
            // as `V_ijab` is accepted as the common shorthand for `V_{ijab}`.
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
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    // Orbitals written as digits (`f_{12}`, `\eta_{1a}` in IMSRG papers):
    // every digit that does not follow a letter is an index of its own.
    let digit_orbitals = compact.len() >= 2 && compact.chars().all(|c| c.is_ascii_alphanumeric());
    // `{a2}`, `{1a}`, `{12}`: every character is an orbital.
    if digit_orbitals
        && compact.chars().any(|c| c.is_ascii_digit())
        && (compact.len() == 2 || compact.starts_with(|c: char| c.is_ascii_digit()))
    {
        return Ok(compact.chars().map(|c| c.to_string()).collect());
    }
    for c in s.chars() {
        if c.is_ascii_digit()
            && digit_orbitals
            && out
                .last()
                .is_none_or(|l| l.chars().all(|c| c.is_ascii_digit()))
        {
            out.push(c.to_string());
            continue;
        }
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

/// Indices in a script group, or `None` if the group is a label
/// (`J`, `0`, `{\rm od}`, `(1)`, `\alpha`, `{j}_{a}`).
fn index_group(g: &str) -> Option<Vec<String>> {
    let ix = indices(g).ok()?;
    // A lone digit (`^{0}`, `z_{0}`) is a label, `{12}` two orbitals.
    (!ix.is_empty()
        && ix
            .iter()
            .all(|i| i.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()))
        && !(ix.len() == 1 && ix[0].starts_with(|c: char| c.is_ascii_digit())))
    .then_some(ix)
}

/// Keeps a script label that names the operator (`\eta_{\rm 2b}`,
/// `W^{(1)}`); drops angular-momentum labels.
fn label_of(g: &str, script: char, mut acc: String) -> String {
    if g.contains("\\rm") || g.contains("\\mathrm") || g.contains("\\text") || g.contains('(') {
        acc.push(script);
        acc.push('{');
        acc.push_str(g.trim());
        acc.push('}');
    }
    acc
}

/// Font and accent commands that take the operator letter as argument.
const NAME_WRAPPERS: &[&str] = &[
    "mathcal",
    "mathbf",
    "mathrm",
    "mathit",
    "mathsf",
    "boldsymbol",
    "bar",
    "tilde",
    "hat",
    "overline",
    "widetilde",
    "widehat",
    "check",
    "breve",
];

/// The letter an operator name is built on (`\mathcal{W}` → `W`).
fn base_name(name: &str) -> &str {
    let mut n = name;
    while let Some(rest) = n.strip_prefix('\\') {
        match rest.find('{') {
            Some(b) if NAME_WRAPPERS.contains(&&rest[..b]) => {
                n = rest[b + 1..].trim_end_matches('}');
            }
            _ => return n,
        }
    }
    n.trim_end_matches('\'')
}

fn parse_name(p: &mut P) -> Result<String, String> {
    let start = p.i;
    if p.peek() == Some(b'\\') {
        p.i += 1;
        let cmd_start = p.i;
        while p.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            p.i += 1;
        }
        let cmd = String::from_utf8_lossy(&p.s[cmd_start..p.i]).to_string();
        if NAME_WRAPPERS.contains(&cmd.as_str()) {
            let arg = p.group()?;
            let inner = if arg.starts_with('\\') {
                arg
            } else {
                arg.trim().to_string()
            };
            return Ok(format!("\\{cmd}{{{inner}}}"));
        }
    } else {
        p.i += 1; // a single letter
    }
    while p.peek() == Some(b'\'') {
        p.i += 1;
    }
    Ok(String::from_utf8_lossy(&p.s[start..p.i]).to_string())
}

fn parse_tensor(p: &mut P) -> Result<Tensor, String> {
    let name = parse_name(p)?;
    let (mut sub, mut sup) = (None, None);
    let mut label = String::new();
    loop {
        // `V ^{ij}_{ab}`: spaces before the scripts are fine in LaTeX.
        while p.peek() == Some(b' ') {
            p.i += 1;
        }
        if p.eat("_") {
            let g = p.group()?;
            match index_group(&g) {
                Some(ix) => sub = Some(ix),
                None => label = label_of(&g, '_', label),
            }
        } else if p.eat("^") {
            let g = p.group()?;
            match index_group(&g) {
                Some(ix) => sup = Some(ix),
                None => label = label_of(&g, '^', label),
            }
        } else if p.peek() == Some(b'{') && sub.is_none() && sup.is_none() {
            let g = p.group()?;
            return Err(format!(
                "{name}{{{g}}} is not valid LaTeX for indices: write {name}_{{{g}}}"
            ));
        } else {
            break;
        }
    }
    // Labels such as `{\rm 2b}` or `(1)` belong to the name; coupling
    // labels (`J`, `0`, `J_0`) are not part of the diagram.
    let name = format!("{name}{label}");
    let (bra, ket, flexible) = match (sup, sub) {
        (Some(up), Some(down)) => (up, down, true),
        (None, Some(all)) | (Some(all), None) => {
            if all.len() % 2 != 0 {
                return Err(format!(
                    "{name}: an odd number of indices ({}); write X_{{pq rs}} = <pq|X|rs> or X^{{pq}}_{{rs}}",
                    all.len()
                ));
            }
            let h = all.len() / 2;
            (all[..h].to_vec(), all[h..].to_vec(), false)
        }
        (None, None) => {
            return Err(format!(
                "{name}: an operator needs indices, e.g. {name}_{{ai}}"
            ));
        }
    };
    if bra.len() != ket.len() {
        return Err(format!(
            "{name}: {} upper but {} lower indices",
            bra.len(),
            ket.len()
        ));
    }
    Ok(Tensor {
        name,
        bra,
        ket,
        flexible,
    })
}

/// `\langle pq || rs \rangle` (antisymmetrized) or `\langle pq | rs \rangle`.
fn parse_braket(p: &mut P) -> Result<Tensor, String> {
    let start = p.i;
    let rest = String::from_utf8_lossy(p.rest()).to_string();
    let end = rest.find("\\rangle").ok_or("\\langle without \\rangle")?;
    let inside = &rest[..end];
    p.i = start + end + "\\rangle".len();
    let (bra, ket) = inside
        .split_once("||")
        .or_else(|| inside.split_once("\\|"))
        .or_else(|| inside.split_once('|'))
        .ok_or_else(|| format!("\\langle{inside}\\rangle: expected ⟨pq||rs⟩"))?;
    let (bra, ket) = (indices(bra)?, indices(ket)?);
    if bra.len() != ket.len() || bra.is_empty() {
        return Err(format!("\\langle{inside}\\rangle: expected ⟨pq||rs⟩"));
    }
    // ⟨pq||rs⟩ = ⟨rs||pq⟩ for real orbitals: either reading may be used.
    Ok(Tensor {
        name: "v".into(),
        bra,
        ket,
        flexible: true,
    })
}

#[derive(Clone)]
struct Prod {
    sign: f64,
    factors: Vec<String>,
    tensors: Vec<Tensor>,
    /// Occupation factors: (index, true = `n` hole, false = `\bar n` particle).
    occ: Vec<(String, bool)>,
    origin: usize,
    /// A number was read (so a following `-` starts a new product).
    number: bool,
}

impl Prod {
    fn one(sign: f64, origin: usize) -> Self {
        Prod {
            sign,
            factors: Vec::new(),
            tensors: Vec::new(),
            occ: Vec::new(),
            origin,
            number: false,
        }
    }
}

/// `n_a` (hole) or `\bar n_a`, `\bar{n}_a`, `\bar{n_a}`, `\overline{n}_a`
/// (particle): returns the occupation factors read.
fn parse_occupation(p: &mut P) -> Result<Option<Vec<(String, bool)>>, String> {
    let start = p.i;
    let hole = if p.peek() == Some(b'n') && p.s.get(p.i + 1) == Some(&b'_') {
        p.i += 1;
        true
    } else {
        let mut bar = false;
        for pre in [
            "\\bar{n}",
            "\\bar n",
            "\\bar{n",
            "\\overline{n}",
            "\\overline{n",
        ] {
            if p.rest().starts_with(pre.as_bytes()) {
                p.i += pre.len();
                bar = true;
                break;
            }
        }
        if !bar {
            return Ok(None);
        }
        false
    };
    while p.peek() == Some(b' ') {
        p.i += 1;
    }
    if !p.eat("_") {
        p.i = start;
        return Ok(None);
    }
    let g = p.group()?;
    // \bar{n_a}: the closing brace of \bar comes after the index.
    if p.peek() == Some(b'}') && !p.s[start..p.i].ends_with(b"}") {
        p.i += 1;
    }
    let ix = indices(&g)?;
    Ok(Some(ix.into_iter().map(|i| (i, hole)).collect()))
}

/// Multiplies out occupation factors: within one term, every product is
/// written over all indices that carry a factor (`n_a` = `n_a (n_b + \bar n_b)`),
/// so that each index is a definite hole or particle; equal products are
/// added and those that cancel dropped. `(n_a - n_b)` gives
/// `n_a \bar n_b - \bar n_a n_b`, `(1 - n_a - n_b)` gives `\bar n_a \bar n_b - n_a n_b`.
fn resolve_occupations(out: Vec<(Prod, usize)>) -> Vec<Term> {
    // IMSRG notation: occupation factors, or the Greek operators of IMSRG
    // papers (η, Γ, Ω, χ); then indices are general orbitals.
    let imsrg_name = |t: &Tensor| {
        let b = base_name(&t.name);
        ["\\eta", "\\Gamma", "\\Omega", "\\chi", "\\Xi", "\\Theta"]
            .iter()
            .any(|g| b.starts_with(g))
    };
    let general = out
        .iter()
        .any(|(pr, _)| !pr.occ.is_empty() || pr.tensors.iter().any(imsrg_name));
    // Group the products of one written term with the same operators.
    let mut groups: Vec<(String, Vec<(Prod, usize)>)> = Vec::new();
    for (pr, row) in out {
        let key = format!("{}|{:?}|{:?}", pr.origin, pr.tensors, pr.factors);
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1.push((pr, row)),
            None => groups.push((key, vec![(pr, row)])),
        }
    }
    let mut terms = Vec::new();
    for (_, group) in groups {
        let first = &group[0];
        let base = |sign: f64, classes: BTreeMap<String, Kind>, extra: Option<String>| Term {
            sign,
            factor: {
                let mut f: Vec<String> = extra.into_iter().collect();
                f.extend(first.0.factors.iter().cloned());
                (!f.is_empty()).then(|| f.join(" "))
            },
            tensors: first.0.tensors.clone(),
            row: first.1,
            classes,
            general,
        };
        let universe: std::collections::BTreeSet<String> = group
            .iter()
            .flat_map(|(pr, _)| pr.occ.iter().map(|(i, _)| i.clone()))
            .collect();
        if universe.is_empty() {
            for (pr, _) in &group {
                let mut t = base(pr.sign, BTreeMap::new(), None);
                t.factor = (!pr.factors.is_empty()).then(|| pr.factors.join(" "));
                terms.push(t);
            }
            continue;
        }
        let universe: Vec<String> = universe.into_iter().collect();
        let mut acc: BTreeMap<Vec<bool>, f64> = BTreeMap::new();
        'products: for (pr, _) in &group {
            let mut fixed: BTreeMap<&str, bool> = BTreeMap::new();
            for (i, h) in &pr.occ {
                if fixed.insert(i.as_str(), *h).is_some_and(|old| old != *h) {
                    continue 'products; // n_a \bar n_a = 0
                }
            }
            let free: Vec<usize> = (0..universe.len())
                .filter(|&k| !fixed.contains_key(universe[k].as_str()))
                .collect();
            for mask in 0..1u32 << free.len() {
                let assign: Vec<bool> = (0..universe.len())
                    .map(|k| match fixed.get(universe[k].as_str()) {
                        Some(h) => *h,
                        None => {
                            let pos = free.iter().position(|f| *f == k).expect("free");
                            mask >> pos & 1 == 1
                        }
                    })
                    .collect();
                *acc.entry(assign).or_insert(0.0) += pr.sign;
            }
        }
        // Positive terms first: (n_a - n_b) reads n_a \bar n_b - \bar n_a n_b.
        let mut acc: Vec<(Vec<bool>, f64)> = acc.into_iter().collect();
        acc.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (assign, coef) in acc {
            if coef.abs() < 1e-9 {
                continue;
            }
            let classes = universe
                .iter()
                .zip(&assign)
                .map(|(i, h)| (i.clone(), if *h { Kind::Hole } else { Kind::Particle }))
                .collect();
            let extra = ((coef.abs() - 1.0).abs() > 1e-9).then(|| crate::inspect::num(coef.abs()));
            terms.push(base(coef.signum(), classes, extra));
        }
    }
    terms
}

/// Skips a balanced `open … close` group starting at `p.i`.
fn skip_balanced(p: &mut P, open: u8, close: u8) {
    let mut depth = 0;
    while let Some(c) = p.peek() {
        p.i += 1;
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return;
            }
        }
    }
}

/// Angular-momentum names: factors built from them are scalars.
fn coupling_name(arg: &str) -> bool {
    let a = arg.trim().trim_matches(|c| c == '{' || c == '}');
    matches!(a.chars().next(), Some('J' | 'j' | 'L' | 'S'))
        || a.starts_with("\\lambda")
        || a.starts_with("\\jmath")
        || a.starts_with("\\ell")
}

/// Skips a scalar factor that is not part of a diagram: Kronecker deltas,
/// `\hat J`, `\sqrt{…}`, 6j/9j symbols, matrices, `\*`. Returns whether
/// something was skipped.
fn skip_scalar(p: &mut P) -> Result<bool, String> {
    let start = p.i;
    let scripts = |p: &mut P| -> Result<(), String> {
        loop {
            while p.peek() == Some(b' ') {
                p.i += 1;
            }
            if p.eat("_") || p.eat("^") {
                p.group()?;
            } else {
                return Ok(());
            }
        }
    };
    if p.eat("\\*") {
        return Ok(true);
    }
    for cmd in ["\\delta", "\\hatfact", "\\sqrt"] {
        if p.eat(cmd) {
            scripts(p)?;
            while p.peek() == Some(b'{') {
                p.group()?;
                scripts(p)?;
            }
            return Ok(true);
        }
    }
    for cmd in ["\\sixj", "\\ninej", "\\threej", "\\sixJ"] {
        if p.eat(cmd) {
            while p.peek() == Some(b'{') {
                p.group()?;
            }
            return Ok(true);
        }
    }
    for env in [
        "Bmatrix", "bmatrix", "pmatrix", "matrix", "Vmatrix", "vmatrix",
    ] {
        let open = format!("\\begin{{{env}}}");
        let close = format!("\\end{{{env}}}");
        if p.rest().starts_with(open.as_bytes()) {
            let rest = String::from_utf8_lossy(p.rest()).to_string();
            p.i += rest.find(&close).map_or(rest.len(), |k| k + close.len());
            return Ok(true);
        }
    }
    if p.eat("\\hat") {
        // \hat J, \hat{J}_1, \hat\lambda: scalars; \hat P: a permutation.
        while p.peek() == Some(b' ') {
            p.i += 1;
        }
        let arg = if p.peek() == Some(b'\\') {
            let a = p.i;
            p.i += 1;
            while p.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                p.i += 1;
            }
            String::from_utf8_lossy(&p.s[a..p.i]).to_string()
        } else if p.peek() == Some(b'{') {
            p.group()?
        } else {
            let a = p.i;
            p.i += 1;
            String::from_utf8_lossy(&p.s[a..p.i.min(p.s.len())]).to_string()
        };
        if coupling_name(&arg) || arg.trim() == "P" {
            while p.peek() == Some(b'\'') {
                p.i += 1;
            }
            scripts(p)?;
            if arg.trim() == "P" && p.peek() == Some(b'(') {
                skip_balanced(p, b'(', b')');
            }
            return Ok(true);
        }
        p.i = start;
        return Ok(false);
    }
    // A lone angular momentum (J, J_0, \lambda) used as a factor.
    if p.peek() == Some(b'J') || p.eat("\\lambda") {
        if p.peek() == Some(b'J') {
            p.i += 1;
        }
        while p.peek() == Some(b'\'') {
            p.i += 1;
        }
        scripts(p)?;
        return Ok(true);
    }
    Ok(false)
}

/// A sum of products, up to `stop` (`)` for a parenthesis) or the end.
/// Products with parentheses are multiplied out. `\\` ends a row.
fn parse_sum(
    p: &mut P,
    stop: Option<u8>,
    row: &mut usize,
    out: &mut Vec<(Prod, usize)>,
) -> Result<(), String> {
    let mut cur: Vec<Prod> = vec![Prod::one(1.0, p.origin)];
    let mut cur_row = *row;
    // Inside parentheses constants and occupation factors are kept, for
    // `(1 - n_a)` and `(n_a - n_b)`; at the top level a term needs operators.
    let inside = stop.is_some();
    let flush =
        |p: &mut P, cur: &mut Vec<Prod>, out: &mut Vec<(Prod, usize)>, r: usize, sign: f64| {
            for pr in cur.drain(..) {
                if !pr.tensors.is_empty()
                    || (inside && !(pr.factors.is_empty() && pr.occ.is_empty() && pr.sign == 0.0))
                {
                    out.push((pr, r));
                }
            }
            p.origin += 1;
            cur.push(Prod::one(sign, p.origin));
        };
    loop {
        p.skip_ws();
        let Some(c) = p.peek() else {
            if stop.is_some() {
                return Err("unbalanced ( … )".into());
            }
            break;
        };
        if Some(c) == stop {
            p.i += 1;
            break;
        }
        if p.rest().starts_with(b"\\\\") {
            p.i += 2;
            if stop.is_none() {
                flush(p, &mut cur, out, cur_row, 1.0);
                *row += 1;
                cur_row = *row;
            }
            continue;
        }
        if c == b'+' || c == b'-' {
            p.i += 1;
            // A sign right after a line break or at the start continues
            // the product being built (which is still empty).
            let empty = cur.iter().all(|pr| {
                pr.tensors.is_empty() && pr.factors.is_empty() && pr.occ.is_empty() && !pr.number
            });
            let sign = if c == b'-' { -1.0 } else { 1.0 };
            if empty {
                cur.iter_mut().for_each(|pr| pr.sign *= sign);
            } else {
                flush(p, &mut cur, out, cur_row, sign);
            }
            continue;
        }
        if c == b'(' || c == b'[' {
            p.i += 1;
            let close = if c == b'(' { b')' } else { b']' };
            let mut inner = Vec::new();
            let mut r = *row;
            let start = p.i;
            let parsed = parse_sum(p, Some(close), &mut r, &mut inner);
            // A group without operators is a scalar factor: a phase
            // (-1)^{…}, (2J+1), … — not part of the diagram.
            let text = String::from_utf8_lossy(&p.s[start..p.i.min(p.s.len())]).to_string();
            let coupling = text.contains('J') || text.contains("j_") || text.contains("\\lambda");
            // Also scalar: a group of constants only ((1 + P_{12}), (2J+1)),
            // or one raised to a power ((-1)^{j_a}).
            let constants = inner
                .iter()
                .all(|(b, _)| b.tensors.is_empty() && b.occ.is_empty());
            let power = parsed.is_ok() && matches!(p.peek(), Some(b'^'));
            let dropped = match &parsed {
                Ok(()) => inner.is_empty() || constants || power,
                Err(_) => coupling,
            };
            if dropped {
                if parsed.is_err() {
                    p.i = start - 1;
                    skip_balanced(p, c, close);
                }
                while p.eat("^") || p.eat("_") {
                    p.group()?;
                }
                continue;
            }
            parsed?;
            // Multiply out: every product so far times every inner term.
            let mut next = Vec::new();
            for a in &cur {
                for (b, _) in &inner {
                    let mut m = a.clone();
                    m.sign *= b.sign;
                    m.factors.extend(b.factors.iter().cloned());
                    m.tensors.extend(b.tensors.iter().cloned());
                    m.occ.extend(b.occ.iter().cloned());
                    next.push(m);
                }
            }
            cur = next;
            continue;
        }
        if p.eat("\\sum") {
            if p.eat("_") {
                p.group()?;
            }
            if p.eat("^") {
                p.group()?;
            }
            continue;
        }
        if let Some(occ) = parse_occupation(p)? {
            cur.iter_mut()
                .for_each(|pr| pr.occ.extend(occ.iter().cloned()));
            continue;
        }
        if skip_scalar(p)? {
            continue;
        }
        if p.eat("\\frac") || p.eat("\\tfrac") || p.eat("\\dfrac") {
            let (a, b) = (p.group()?, p.group()?);
            let numeric = |x: &str| {
                x.chars()
                    .all(|c| c.is_ascii_digit() || " ()!^.+-{}".contains(c))
            };
            // Numeric prefactors are shown; others (1/(2j+1), …) are
            // coupling factors and not part of the diagram.
            if numeric(&a) && numeric(&b) {
                let f = format!("\\frac{{{a}}}{{{b}}}");
                cur.iter_mut().for_each(|pr| pr.factors.push(f.clone()));
            }
            continue;
        }
        if c == b'.' || c == b',' || c == b';' {
            // Punctuation at the end of a displayed equation.
            if !p.s.get(p.i + 1).is_some_and(|d| d.is_ascii_digit()) {
                p.i += 1;
                continue;
            }
        }
        if c.is_ascii_digit() || c == b'.' {
            let start = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
                p.i += 1;
            }
            let f = String::from_utf8_lossy(&p.s[start..p.i]).to_string();
            // A factor 1 is no factor (the 1 in `(1 - n_a)`).
            if !f.parse::<f64>().is_ok_and(|x| x == 1.0) {
                cur.iter_mut().for_each(|pr| pr.factors.push(f.clone()));
            }
            cur.iter_mut().for_each(|pr| pr.number = true);
            continue;
        }
        // Permutation operators P(ab), \hat{P}_{ij}: not drawn.
        if p.eat("\\hat{P}")
            || p.eat("\\mathcal{P}")
            || (c == b'P' && matches!(p.s.get(p.i + 1), Some(b'(' | b'_')))
        {
            if c == b'P' {
                p.i += 1;
            }
            if p.peek() == Some(b'(') {
                while p.peek().is_some_and(|c| c != b')') {
                    p.i += 1;
                }
                p.i += 1;
            } else {
                while p.eat("_") || p.eat("^") {
                    p.group()?;
                }
            }
            continue;
        }
        if p.eat("\\langle") {
            let t = parse_braket(p)?;
            cur.iter_mut().for_each(|pr| pr.tensors.push(t.clone()));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'\\' {
            let t = parse_tensor(p)?;
            cur.iter_mut().for_each(|pr| pr.tensors.push(t.clone()));
            continue;
        }
        return Err(format!(
            "unexpected {:?}",
            String::from_utf8_lossy(&p.s[p.i..(p.i + 8).min(p.s.len())])
        ));
    }
    flush(p, &mut cur, out, cur_row, 1.0);
    Ok(())
}

/// `{n}_a` → `n_a` (but not the argument of `\bar{n}_a`).
fn unbrace_n(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find("{n}_") {
        let before = &rest[..k];
        out.push_str(before);
        if before.ends_with(|c: char| c.is_ascii_alphabetic()) {
            out.push_str("{n}_");
        } else {
            out.push_str("n_");
        }
        rest = &rest[k + 4..];
    }
    out.push_str(rest);
    out
}

/// Splits at the first `=` outside braces.
fn split_equals(src: &str) -> (Option<&str>, &str) {
    let mut depth = 0i32;
    for (k, c) in src.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '=' if depth == 0 => return (Some(&src[..k]), &src[k + 1..]),
            _ => {}
        }
    }
    (None, src)
}

/// Parses `lhs = rhs` (or just `rhs`).
pub fn parse(src: &str) -> Result<Equation, String> {
    let src = src.trim().trim_matches('$').trim();
    let (lhs, rhs) = split_equals(src);
    // `z &+=& …` (accumulating a term) reads as `z = …`.
    let lhs = lhs.map(|l| {
        l.trim_end()
            .trim_end_matches('&')
            .trim_end()
            .trim_end_matches('+')
    });
    let rhs = rhs.trim_start().trim_start_matches('&');
    let lhs = lhs.map(|l| {
        l.replace('&', "")
            .replace("\\begin{aligned}", "")
            .trim()
            .to_string()
    });
    let external = match &lhs {
        Some(l) => {
            let mut p = P {
                s: l.as_bytes(),
                i: 0,
                origin: 0,
            };
            // Indices after _ and ^ (E2, E^{(2)} and 0 have none).
            let mut ext = Vec::new();
            while let Some(c) = p.peek() {
                p.i += 1;
                if c == b'_' || c == b'^' {
                    let g = p.group()?;
                    if let Some(ix) = index_group(&g) {
                        ext.extend(ix);
                    }
                }
            }
            ext
        }
        None => Vec::new(),
    };
    // `\{ … \}` groups like parentheses; `{n}_a` is `n_a`.
    let rhs = rhs
        .replace("\\{", "(")
        .replace("\\}", ")")
        .replace("{\\bar{n}}_", "\\bar{n}_")
        .replace("{\\bar n}_", "\\bar n_");
    let rhs = unbrace_n(&rhs);
    let mut p = P {
        s: rhs.as_bytes(),
        i: 0,
        origin: 0,
    };
    let mut out = Vec::new();
    let mut row = 0;
    parse_sum(&mut p, None, &mut row, &mut out)?;
    let terms = resolve_occupations(out);
    if terms.is_empty() {
        return Err("no operators on the right-hand side".into());
    }
    Ok(Equation {
        lhs,
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

/// Whether an external line leaves its vertex upward. Particles go up when
/// outgoing, holes when incoming; general lines (no arrow, either kind) go
/// to the nearest edge of the diagram: down from the bottom level, up from
/// the top level.
fn external_up(kind: Kind, outgoing: bool, level: usize, max_level: usize) -> bool {
    match kind {
        Kind::Particle => outgoing,
        Kind::Hole => !outgoing,
        Kind::General if max_level > 0 && level == 0 => false,
        Kind::General if max_level > 0 && level == max_level => true,
        Kind::General => outgoing,
    }
}

/// Style class of a line: general lines have no arrow.
fn class_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Particle => "particle",
        Kind::Hole => "hole",
        Kind::General => "line",
    }
}

fn is_interaction(name: &str) -> bool {
    INTERACTIONS.contains(&base_name(name))
}

fn id_name(name: &str) -> String {
    base_name(name)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
}

fn latex_name(name: &str) -> String {
    format!("${name}$")
}

/// Chooses, for every operator written `X_{..}^{..}`, which index group
/// is the bra, so that each summed index is outgoing on one operator and
/// incoming on the other. Amplitudes are tried as `t^{ab}_{ij}` (upper =
/// outgoing) first; other operators either way.
fn orient(
    term: &Term,
    external: &[String],
    has_lhs: bool,
) -> Result<Vec<(Term, Vec<Line>)>, String> {
    // First as written; if no reading fits, operators written with
    // subscripts only may be read the other way round too (IMSRG papers
    // write Hermitian operators in either order: \chi_{ai} \Gamma_{ajkl}).
    match orient_with(term, external, has_lhs, false) {
        Ok(r) => Ok(r),
        Err(e) => orient_with(term, external, has_lhs, true).map_err(|_| e),
    }
}

fn orient_with(
    term: &Term,
    external: &[String],
    has_lhs: bool,
    all: bool,
) -> Result<Vec<(Term, Vec<Line>)>, String> {
    let flex: Vec<usize> = (0..term.tensors.len())
        .filter(|&t| all || term.tensors[t].flexible)
        .collect();
    if flex.len() > 12 {
        return lines(term, external, has_lhs).map(|l| vec![(term.clone(), l)]);
    }
    let mut masks: Vec<u32> = (0..1u32 << flex.len()).collect();
    // Fewest swaps of amplitudes first, then fewest swaps overall.
    let cost = |m: u32| {
        let mut amp = 0;
        let mut all = 0;
        for (k, &t) in flex.iter().enumerate() {
            if m >> k & 1 == 1 {
                all += 1;
                if !is_interaction(&term.tensors[t].name) {
                    amp += 1;
                }
            }
        }
        (amp, all)
    };
    masks.sort_by_key(|m| cost(*m));
    let mut first_err = None;
    let mut ok = Vec::new();
    for m in masks {
        let mut t = term.clone();
        for (k, &ti) in flex.iter().enumerate() {
            if m >> k & 1 == 1 {
                let ten = &mut t.tensors[ti];
                std::mem::swap(&mut ten.bra, &mut ten.ket);
            }
        }
        match lines(&t, external, has_lhs) {
            Ok(l) => ok.push((t, l)),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if ok.is_empty() {
        Err(first_err.unwrap_or_default())
    } else {
        Ok(ok)
    }
}

/// A term with its operators oriented, and its lines.
type Reading = (Term, Vec<Line>);

/// Which external indices are outgoing: terms of one equation should agree.
fn signature(lines: &[Line]) -> Vec<(String, bool)> {
    let mut s: Vec<(String, bool)> = lines
        .iter()
        .filter(|l| l.out.is_none() || l.inn.is_none())
        .map(|l| (l.index.clone(), l.out.is_some()))
        .collect();
    s.sort();
    s
}

/// Lines of one term, checked.
fn lines(term: &Term, external: &[String], has_lhs: bool) -> Result<Vec<Line>, String> {
    let mut map: BTreeMap<String, Line> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for (t, ten) in term.tensors.iter().enumerate() {
        for (j, (o, i)) in ten.bra.iter().zip(&ten.ket).enumerate() {
            for (idx, outgoing) in [(o, true), (i, false)] {
                let kind = kind_in(term, idx);
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
                Kind::Particle | Kind::General => (o.t, i.t),
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

/// Calls `f` with every permutation of `0..n` (Heap's algorithm).
fn each_permutation(n: usize, mut f: impl FnMut(&[usize])) {
    let mut a: Vec<usize> = (0..n).collect();
    let mut c = vec![0usize; n];
    f(&a);
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                a.swap(0, i);
            } else {
                a.swap(c[i], i);
            }
            f(&a);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
}

/// Do segments a–b and c–d cross (not counting shared end points)?
fn crosses(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let same = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9;
    if same(a, c) || same(a, d) || same(b, c) || same(b, d) {
        return false;
    }
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Positions of all vertices of one term.
///
/// Vertices joined by an internal line share a column where possible, so
/// internal lines run straight up and down, parallel and evenly spaced
/// (a ladder keeps its width). The order of the columns is chosen by
/// scoring every arrangement: operators on one level must not overlap, a
/// vertical line must not run through another vertex, and crossings,
/// slanted lines and stretched operators cost extra.
/// Positions of the vertices of one term.
type Placement = HashMap<V, (f64, f64)>;

/// How many of the best-scoring column arrangements are drawn and measured.
const CANDIDATES: usize = 8;

/// Candidate positions of all vertices of one term, best first.
fn place(term: &Term, lines: &[Line], level: &[usize]) -> Vec<HashMap<V, (f64, f64)>> {
    let verts: Vec<V> = term
        .tensors
        .iter()
        .enumerate()
        .flat_map(|(t, ten)| (0..ten.bra.len()).map(move |j| V { t, j }))
        .collect();
    // Columns: union of vertices along internal lines between operators,
    // never two vertices of one level in a column. Lines that pair the same
    // two vertices more than once (bubbles) are joined first.
    let mut pairs: Vec<(V, V, usize)> = Vec::new();
    for l in lines {
        if let (Some(a), Some(b)) = (l.out, l.inn)
            && a.t != b.t
        {
            let key = if a < b { (a, b) } else { (b, a) };
            match pairs.iter_mut().find(|p| (p.0, p.1) == key) {
                Some(p) => p.2 += 1,
                None => pairs.push((key.0, key.1, 1)),
            }
        }
    }
    pairs.sort_by_key(|p| std::cmp::Reverse(p.2));
    // Which vertices share a column is decided greedily along `pairs`;
    // different orders give different groupings, so try several (bubbles
    // stay first) and keep the best layout.
    let bubbles = pairs.iter().take_while(|p| p.2 > 1).count();
    let singles = pairs.len() - bubbles;
    let mut all: Vec<(f64, Placement)> = Vec::new();
    for rot in 0..singles.clamp(1, 6) {
        let mut order = pairs[..bubbles].to_vec();
        order.extend((0..singles).map(|k| pairs[bubbles + (k + rot) % singles]));
        all.extend(place_with(term, lines, level, &verts, &order));
    }
    all.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<HashMap<V, (f64, f64)>> = Vec::new();
    for (_, pos) in all {
        if !out.contains(&pos) {
            out.push(pos);
        }
        if out.len() == CANDIDATES {
            break;
        }
    }
    out
}

/// Layout for one grouping order: (score, positions).
fn place_with(
    term: &Term,
    lines: &[Line],
    level: &[usize],
    verts: &[V],
    pairs: &[(V, V, usize)],
) -> Vec<(f64, Placement)> {
    let lv = |v: V| level[v.t];
    let mut col_of: HashMap<V, usize> = verts.iter().enumerate().map(|(n, v)| (*v, n)).collect();
    let mut members: Vec<Vec<V>> = verts.iter().map(|v| vec![*v]).collect();
    for (a, b, _) in pairs {
        let (ca, cb) = (col_of[a], col_of[b]);
        if ca == cb {
            continue;
        }
        let clash = members[ca]
            .iter()
            .any(|x| members[cb].iter().any(|y| lv(*x) == lv(*y)));
        if !clash {
            let moved = std::mem::take(&mut members[cb]);
            for v in &moved {
                col_of.insert(*v, ca);
            }
            members[ca].extend(moved);
        }
    }
    let groups: Vec<Vec<V>> = members.into_iter().filter(|m| !m.is_empty()).collect();
    let group_of: HashMap<V, usize> = groups
        .iter()
        .enumerate()
        .flat_map(|(g, m)| m.iter().map(move |v| (*v, g)))
        .collect();
    let internal: Vec<(V, V)> = lines
        .iter()
        .filter_map(|l| match (l.out, l.inn) {
            (Some(a), Some(b)) if a.t != b.t => Some((a, b)),
            _ => None,
        })
        .collect();
    let n_ops = term.tensors.len();
    // External lines: (vertex, leaves upward). They run to the top or bottom
    // of the diagram, so operators in their way should not span their column.
    let max_level = level.iter().copied().max().unwrap_or(0);
    let externals: Vec<(V, bool)> = lines
        .iter()
        .filter_map(|l| match (l.out, l.inn) {
            (Some(o), None) => Some((o, external_up(l.kind, true, level[o.t], max_level))),
            (None, Some(i)) => Some((i, external_up(l.kind, false, level[i.t], max_level))),
            _ => None,
        })
        .collect();

    let score = |order: &[usize]| -> f64 {
        // order[g] = column of group g
        let x = |v: V| order[group_of[&v]] as f64;
        let y = |v: V| lv(v) as f64;
        let mut cost = 0.0;
        // Operator spans.
        let span = |t: usize| {
            let xs = (0..term.tensors[t].bra.len()).map(|j| x(V { t, j }));
            let lo = xs.clone().fold(f64::INFINITY, f64::min);
            let hi = xs.fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let spans: Vec<(f64, f64)> = (0..n_ops).map(span).collect();
        for t in 0..n_ops {
            let k = term.tensors[t].bra.len() as f64;
            cost += 5.0 * (spans[t].1 - spans[t].0 - (k - 1.0));
            for u in t + 1..n_ops {
                if level[t] == level[u]
                    && spans[t].0 <= spans[u].1 + 0.5
                    && spans[u].0 <= spans[t].1 + 0.5
                {
                    cost += 1000.0;
                }
            }
        }
        // Internal lines.
        for (n, &(a, b)) in internal.iter().enumerate() {
            let (pa, pb) = ((x(a), y(a)), (x(b), y(b)));
            cost += 10.0 * (pa.0 - pb.0).abs();
            let (y0, y1) = (pa.1.min(pb.1), pa.1.max(pb.1));
            for t in 0..n_ops {
                let ly = level[t] as f64;
                if ly > y0 && ly < y1 {
                    let lx = pa.0 + (pb.0 - pa.0) * (ly - pa.1) / (pb.1 - pa.1);
                    let on_vertex =
                        (0..term.tensors[t].bra.len()).any(|j| (x(V { t, j }) - lx).abs() < 0.3);
                    if on_vertex {
                        cost += 1000.0;
                    } else if lx > spans[t].0 - 0.3 && lx < spans[t].1 + 0.3 {
                        cost += 30.0;
                    }
                }
            }
            for &(c, d) in &internal[n + 1..] {
                if crosses(pa, pb, (x(c), y(c)), (x(d), y(d))) {
                    cost += 15.0;
                }
            }
        }
        // External lines blocked by operators above (or below) them, and
        // crossing internal lines (an external line runs about straight up
        // or down to the edge of the diagram).
        let max_y = level.iter().copied().max().unwrap_or(0) as f64;
        for &(v, up) in &externals {
            let (cx, cy) = (x(v), y(v));
            let end = (cx, if up { max_y + 0.6 } else { -0.6 });
            for &(a, b) in &internal {
                if crosses((cx, cy), end, (x(a), y(a)), (x(b), y(b))) {
                    cost += 15.0;
                }
            }
            for t in 0..n_ops {
                let ly = level[t] as f64;
                if (up && ly > cy) || (!up && ly < cy) {
                    let on_vertex =
                        (0..term.tensors[t].bra.len()).any(|j| (x(V { t, j }) - cx).abs() < 0.3);
                    if on_vertex {
                        cost += 8.0;
                    } else if cx > spans[t].0 && cx < spans[t].1 {
                        cost += 4.0;
                    }
                }
            }
        }
        cost
    };

    let g = groups.len();
    let mut best: (f64, Vec<usize>) = (f64::INFINITY, (0..g).collect());
    // The few best arrangements, to be drawn and measured by the caller.
    let mut top: Vec<(f64, Vec<usize>)> = Vec::new();
    if g <= 8 {
        each_permutation(g, |perm| {
            let c = score(perm);
            if top.len() < CANDIDATES || c < top[top.len() - 1].0 - 1e-9 {
                top.push((c, perm.to_vec()));
                top.sort_by(|a, b| a.0.total_cmp(&b.0));
                top.truncate(CANDIDATES);
            }
        });
    } else {
        // Too many for all arrangements: improve by swapping columns until
        // no swap helps.
        best.0 = score(&best.1);
        loop {
            let mut improved = false;
            for i in 0..g {
                for j in i + 1..g {
                    let mut o = best.1.clone();
                    o.swap(i, j);
                    let c = score(&o);
                    if c < best.0 - 1e-9 {
                        best = (c, o);
                        improved = true;
                    }
                }
            }
            if !improved {
                break;
            }
        }
        top.push(best);
    }
    top.into_iter()
        .map(|(c, order)| {
            (
                c,
                verts
                    .iter()
                    .map(|v| (*v, (order[group_of[v]] as f64 * DX, lv(*v) as f64 * DY)))
                    .collect(),
            )
        })
        .collect()
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
            ticks: None,
            tick_spacing: None,
            tick_length: None,
            marks: None,
            mark_pos: None,
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
    chosen: (Term, Vec<Line>),
    prefix: &str,
    x0: f64,
) -> Result<(f64, f64, Vec<String>), String> {
    let (oriented, lines) = chosen;
    let term = &oriented;
    let level = levels(term.tensors.len(), &lines)?;
    let candidates = place(term, &lines, &level);
    // Draw the candidates on their own and keep the one that measures best
    // (crossings, lines near vertices, overlapping labels).
    let mut pick = 0;
    if candidates.len() > 1 {
        let mut best = f64::INFINITY;
        for (k, pos) in candidates.iter().enumerate() {
            let mut trial = Builder {
                doc: Document {
                    styles: b.doc.styles.clone(),
                    ..Document::default()
                },
            };
            draw_term(&mut trial, term, &lines, &level, pos, prefix, x0)?;
            let q = crate::quality::measure(&trial.doc).score();
            if q < best - 1e-9 {
                best = q;
                pick = k;
            }
        }
    }
    draw_term(b, term, &lines, &level, &candidates[pick], prefix, x0)
}

/// Draws one term with the given vertex positions.
fn draw_term(
    b: &mut Builder,
    term: &Term,
    lines: &[Line],
    level: &[usize],
    pos: &HashMap<V, (f64, f64)>,
    prefix: &str,
    x0: f64,
) -> Result<(f64, f64, Vec<String>), String> {
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
            let class = class_of(l.kind);
            // Bends spread symmetrically; a line within one operator
            // bulges away from its interaction line.
            let spread = 40.0f64.min(80.0 / m as f64 + 20.0);
            let mut bend = if m == 1 {
                if same_op {
                    60.0
                } else {
                    // A straight line that would run through another vertex
                    // (sharing its column) goes around it as an arc, bulging
                    // away from the middle of the diagram.
                    let (pa, pb) = (
                        at(*u),
                        at(if *u == lines[n].out.expect("internal") {
                            lines[n].inn.expect("internal")
                        } else {
                            lines[n].out.expect("internal")
                        }),
                    );
                    let blocked = pos.keys().any(|w| {
                        let q = at(*w);
                        let inside = (q.1 - pa.1) * (q.1 - pb.1) < -1e-9;
                        let dx = pa.0 + (pb.0 - pa.0) * (q.1 - pa.1) / (pb.1 - pa.1) - q.0;
                        inside && dx.abs() < 8.0
                    });
                    if blocked {
                        let center = pos.values().map(|p| p.0 + x0).sum::<f64>() / pos.len() as f64;
                        // Left of the travel direction u → other is +bend.
                        let dir = (pb.0 - pa.0, pb.1 - pa.1);
                        let left = (-dir.1, dir.0);
                        let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                        let outward = (mid.0 - center) * left.0 >= 0.0;
                        if outward { 40.0 } else { -40.0 }
                    } else {
                        0.0
                    }
                }
            } else {
                spread * (1.0 - 2.0 * k as f64 / (m - 1) as f64)
            };
            if o != *u {
                bend = -bend;
            }
            let e = b.edge(id.clone(), vid(o), vid(i), class);
            e.arrow = (l.kind != Kind::General).then_some(ArrowDir::Forward);
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
    // the others downward. They run to the top / bottom of the whole
    // diagram, leaning away from their operator just enough to clear every
    // vertex and operator they pass and the other lines at their vertex.
    let top = max_level as f64 * DY + EXT;
    let bottom = -EXT;
    let all: Vec<(f64, f64)> = pos.keys().map(|v| at(*v)).collect();
    let op_spans: Vec<(f64, f64, f64, f64)> = (0..term.tensors.len())
        .map(|t| {
            let xs: Vec<f64> = (0..term.tensors[t].bra.len())
                .map(|j| at(V { t, j }).0)
                .collect();
            let inter = is_interaction(&term.tensors[t].name);
            let (mut lo, mut hi) = (
                xs.iter().copied().fold(f64::INFINITY, f64::min),
                xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            );
            // One-body operators: the ---X line to the right, or a short bar.
            if xs.len() == 1 {
                if inter {
                    hi += 28.0;
                } else {
                    lo -= 10.0;
                    hi += 10.0;
                }
            }
            (
                at(V { t, j: 0 }).1,
                lo,
                hi,
                // Crossing an interaction line is acceptable, a bar is not.
                if is_interaction(&term.tensors[t].name) {
                    30.0
                } else {
                    80.0
                },
            )
        })
        .collect();
    // Fermion lines drawn so far (chords), which external lines should not cross.
    let mut segments: Vec<((f64, f64), (f64, f64))> = lines
        .iter()
        .filter_map(|l| match (l.out, l.inn) {
            (Some(a), Some(b)) if a != b => Some((at(a), at(b))),
            _ => None,
        })
        .collect();
    let mut used: HashMap<(V, bool), Vec<f64>> = HashMap::new();
    for l in lines.iter().filter(|l| l.out.is_none() || l.inn.is_none()) {
        let (v, outgoing) = match (l.out, l.inn) {
            (Some(o), None) => (o, true),
            (None, Some(i)) => (i, false),
            _ => unreachable!(),
        };
        let up = external_up(l.kind, outgoing, level[v.t], max_level);
        let p = at(v);
        let end_y = if up { top } else { bottom };
        let len = (end_y - p.1).abs();
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
        // Cost of a candidate: hard limits for passing close to a vertex
        // or crowding another line at this vertex; crossing a fermion line
        // costs much more than crossing an interaction line or a bar.
        let cost = |dx: f64| -> f64 {
            let end = (p.0 + dx, end_y);
            let x_at = |y: f64| p.0 + dx * (y - p.1).abs() / len;
            let between = |y: f64| {
                if up {
                    y > p.1 + 1e-9 && y <= end_y
                } else {
                    y < p.1 - 1e-9 && y >= end_y
                }
            };
            let mut c = dx.abs() * 0.2;
            if all
                .iter()
                .any(|q| between(q.1) && (x_at(q.1) - q.0).abs() < 14.0)
            {
                c += 1000.0;
            }
            if slopes.iter().any(|s| (dx / len - s).abs() * len < 18.0) {
                c += 1000.0;
            }
            for s in op_spans.iter().filter(|s| between(s.0)) {
                if x_at(s.0) > s.1 - 8.0 && x_at(s.0) < s.2 + 8.0 {
                    c += s.3;
                }
            }
            for (a, q) in &segments {
                if crosses(p, end, *a, *q) {
                    c += 50.0;
                }
            }
            c
        };
        let dir = if lean < 0.0 { -1.0 } else { 1.0 };
        let dx = (0..30)
            .flat_map(|k| {
                let k = k as f64;
                [dir * (14.0 + 7.0 * k), -dir * (14.0 + 7.0 * k)]
            })
            .map(|dx| (cost(dx) + if dx.signum() == dir { 0.0 } else { 3.0 }, dx))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(dir * 14.0, |(_, dx)| dx);
        segments.push((p, (p.0 + dx, end_y)));
        slopes.push(dx / len);
        let end_at = (p.0 + dx, end_y);
        let end = b.vertex(format!("{prefix}{}_end", l.index), end_at, None, None);
        ids.push(end.clone());
        let id = format!("{prefix}{}", l.index);
        let class = class_of(l.kind);
        let (from, to) = if outgoing {
            (vid(v), end)
        } else {
            (end, vid(v))
        };
        let e = b.edge(id.clone(), from, to, class);
        e.arrow = (l.kind != Kind::General).then_some(ArrowDir::Forward);
        e.label = Some(format!("${}$", l.index));
        e.label_pos = Some(if outgoing { 0.85 } else { 0.15 });
        // Label on the outer side of the line.
        let left_of_travel = (up == outgoing) == (dx < 0.0);
        e.label_side = Some(if left_of_travel {
            Side::Left
        } else {
            Side::Right
        });
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
    // Characters in sub/superscripts are set smaller.
    let mut w = 0.0;
    let mut depth = 0usize; // brace depth
    let mut script_until: Option<usize> = None; // depth at which a script group ends
    let mut script_next = false;
    let mut chars = latex.chars().peekable();
    while let Some(c) = chars.next() {
        let small = script_until.is_some() || script_next;
        match c {
            '\\' => {
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    chars.next();
                }
            }
            '_' | '^' => script_next = true,
            '{' => {
                depth += 1;
                if script_next && script_until.is_none() {
                    script_until = Some(depth);
                }
                script_next = false;
            }
            '}' => {
                if script_until == Some(depth) {
                    script_until = None;
                }
                depth = depth.saturating_sub(1);
            }
            '$' | ' ' => {}
            _ => {
                w += if small { 3.6 } else { 5.6 };
                script_next = false;
            }
        }
    }
    w + 2.0
}

/// The left-hand side as written, with the index shorthand braced
/// (`Z_abic` → `Z_{abic}`, `Z_ij^ab` → `Z_{ij}^{ab}`).
fn lhs_latex(lhs: &str, _external: &[String]) -> String {
    let mut out = String::new();
    let mut chars = lhs.trim().chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if (c == '_' || c == '^') && chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) {
            let mut run = String::new();
            while let Some(n) = chars
                .peek()
                .copied()
                .filter(|n| n.is_ascii_alphanumeric() || *n == '\'')
            {
                run.push(n);
                chars.next();
            }
            out.push('{');
            out.push_str(&run);
            out.push('}');
        }
    }
    out
}

/// Builds the diagram(s) of an equation as a document.
pub fn diagram(eq: &Equation) -> Result<Diagram, String> {
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
    if eq.terms.iter().any(|t| {
        t.general
            || t.tensors.iter().any(|x| {
                x.bra
                    .iter()
                    .chain(&x.ket)
                    .any(|i| kind_in(t, i) == Kind::General)
            })
    }) {
        st.insert("line".into(), Style::default());
    }

    let many = eq.terms.len() > 1;
    // External lines must match the left-hand side, except for `0 = …`
    // (an amplitude equation), where they are whatever is left open.
    let check = eq.lhs.as_deref().is_some_and(|l| l.trim() != "0");
    // Every valid reading of each term; then pick, per term, the reading
    // whose external lines point the same way as in most terms.
    // A term that is not a valid contraction is left out with a warning.
    let mut warnings = Vec::new();
    let options: Vec<Option<Vec<Reading>>> = eq
        .terms
        .iter()
        .enumerate()
        .map(|(n, t)| match orient(t, &eq.external, check) {
            Ok(o) => Some(o),
            Err(e) => {
                warnings.push(format!("term {} ({}) left out: {e}", n + 1, term_latex(t)));
                None
            }
        })
        .collect();
    if options.iter().all(Option::is_none) {
        return Err(warnings.join("\n"));
    }
    let mut votes: Vec<(Vec<(String, bool)>, usize)> = Vec::new();
    for o in options.iter().flatten() {
        let sig = signature(&o[0].1);
        match votes.iter_mut().find(|v| v.0 == sig) {
            Some(v) => v.1 += 1,
            None => votes.push((sig, 1)),
        }
    }
    let reference = votes
        .iter()
        .max_by_key(|v| v.1)
        .map(|v| v.0.clone())
        .unwrap_or_default();
    let mut chosen: Vec<Option<(Term, Vec<Line>)>> = options
        .into_iter()
        .map(|o| {
            o.map(|o| {
                let k = o
                    .iter()
                    .position(|c| signature(&c.1) == reference)
                    .unwrap_or(0);
                o.into_iter().nth(k).expect("non-empty")
            })
        })
        .collect();
    let mut first = true;
    let mut cursor = 0.0;
    // Rows (`\\` in the equation): each row is built at y = 0 and then
    // moved below the previous ones.
    let mut row = 0;
    let (mut row_v, mut row_t) = (0, 0);
    let place_row = |b: &mut Builder, row_v: usize, row_t: usize| {
        if row_v == 0 {
            return;
        }
        let above = b.doc.vertices[..row_v]
            .iter()
            .map(|v| v.at[1])
            .fold(f64::INFINITY, f64::min);
        let top = b.doc.vertices[row_v..]
            .iter()
            .map(|v| v.at[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let dy = above - 50.0 - top;
        for v in &mut b.doc.vertices[row_v..] {
            v.at[1] = r2(v.at[1] + dy);
        }
        for t in &mut b.doc.texts[row_t..] {
            t.at[1] = r2(t.at[1] + dy);
        }
    };
    for (n, term) in eq.terms.iter().enumerate() {
        let Some(this) = chosen[n].take() else {
            continue;
        };
        if term.row != row {
            place_row(&mut b, row_v, row_t);
            row = term.row;
            row_v = b.doc.vertices.len();
            row_t = b.doc.texts.len();
            cursor = 0.0;
        }
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
        let e0 = b.doc.edges.len();
        let (_, mid, mut ids) =
            build_term(&mut b, this, &prefix, 0.0).map_err(|e| format!("term {}: {e}", n + 1))?;
        first = false;
        // Horizontal extent of the drawn term: lines, arcs and arrowheads,
        // plus room for the labels beside them.
        let (min_x, max_x) = {
            let part = Document {
                styles: b.doc.styles.clone(),
                vertices: b.doc.vertices[v0..].to_vec(),
                edges: b.doc.edges[e0..].to_vec(),
                ..Document::default()
            };
            match crate::scene::build(&part)
                .ok()
                .and_then(|sc| sc.path_bounds())
            {
                Some(r) => (r.x0 - 10.0, r.x1 + 10.0),
                None => {
                    let xs: Vec<f64> = b.doc.vertices[v0..].iter().map(|v| v.at[0]).collect();
                    (
                        xs.iter().copied().fold(f64::INFINITY, f64::min) - 20.0,
                        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 20.0,
                    )
                }
            }
        };
        let text_w = if pre.is_empty() {
            0.0
        } else {
            text_width(&pre) + 8.0
        };
        // Room for arcs bulging out and their labels on the left.
        let shift = cursor + text_w + 6.0 - min_x;
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
                latex: None,
                plot: None,
            });
        }
        cursor = max_x + shift + TERM_GAP / 2.0;
    }
    place_row(&mut b, row_v, row_t);
    crate::scene::build(&b.doc).map_err(|e| format!("internal layout error: {e}"))?;
    Ok(Diagram {
        doc: b.doc,
        warnings,
    })
}

/// The drawn equation, and the terms that were left out.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub doc: Document,
    /// One message per term that is not a valid contraction.
    pub warnings: Vec<String>,
}

/// A term written back as LaTeX, for messages.
fn term_latex(t: &Term) -> String {
    let mut parts: Vec<String> = Vec::new();
    if t.sign < 0.0 {
        parts.push("-".into());
    }
    parts.extend(t.factor.clone());
    for x in &t.tensors {
        parts.push(if x.name == "v" {
            format!("\\langle {} || {} \\rangle", x.bra.concat(), x.ket.concat())
        } else if x.flexible {
            format!("{}^{{{}}}_{{{}}}", x.name, x.bra.concat(), x.ket.concat())
        } else {
            format!("{}_{{{}{}}}", x.name, x.bra.concat(), x.ket.concat())
        });
    }
    parts.join(" ")
}

/// Parses and builds in one step.
pub fn from_latex(src: &str) -> Result<Diagram, String> {
    diagram(&parse(src)?)
}

/// Like [`from_latex`], with everything in one group that remembers the
/// equation, so the drawing can be redrawn after the equation is edited.
pub fn from_latex_editable(src: &str) -> Result<Diagram, String> {
    let mut d = from_latex(src)?;
    crate::scene::group_everything(&mut d.doc, "eq").latex = Some(src.to_string());
    Ok(d)
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
        assert!(t.tensors[1].flexible);
        assert_eq!(t.tensors[0].ket, vec!["a", "b"]);
        assert_eq!(t.tensors[1].bra, vec!["a", "b"]);
        assert_eq!(t.tensors[1].ket, vec!["i", "j"]);
    }

    #[test]
    fn mbpt2_energy() {
        let d = from_latex(r"E2=\sum_abij V_ijab * T_abij").unwrap().doc;
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
        let d = from_latex(r"Z_abic=V_abef* S_efic").unwrap().doc;
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
        let e = from_latex("E=V_ijab T_abij T_abij").unwrap_err();
        assert!(e.contains("twice"), "{e}");
    }

    #[test]
    fn one_body_amplitudes_and_strict_latex() {
        let e = from_latex(r"Z_{ai} = \sum_{ekjb} V_{kjeb} T_{ek} T{aj} T{bi}").unwrap_err();
        assert!(e.contains("write T_{aj}"), "{e}");
        let d = from_latex(r"Z_{ai} = \sum_{ekjb} V_{kjeb} T_{ek} T_{aj} T_{bi}")
            .unwrap()
            .doc;
        assert_eq!(
            d.edges
                .iter()
                .filter(|e| e.class.as_deref() == Some("amplitude"))
                .count(),
            3
        );
    }

    #[test]
    fn ladders_keep_their_width() {
        // Every internal line between operators is vertical.
        let d = from_latex(r"Z_abcd = V_baji V_jilk T_lkdc").unwrap().doc;
        let at = |id: &str| d.vertices.iter().find(|v| v.id == id).unwrap().at;
        for e in d
            .edges
            .iter()
            .filter(|e| matches!(e.class.as_deref(), Some("particle" | "hole")))
        {
            if !e.from.ends_with("_end") && !e.to.ends_with("_end") {
                assert_eq!(
                    at(&e.from)[0],
                    at(&e.to)[0],
                    "{} is vertical",
                    e.id.as_deref().unwrap()
                );
            }
        }
        // External lines span the whole height.
        let ys: Vec<f64> = d.vertices.iter().map(|v| v.at[1]).collect();
        let (lo, hi) = (
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        for v in d.vertices.iter().filter(|v| v.id.ends_with("_end")) {
            assert!(v.at[1] == lo || v.at[1] == hi, "{} ends at the edge", v.id);
        }
    }

    #[test]
    fn ccsd_doubles_equation() {
        let src = r"\begin{aligned}
0 &= \langle ij || ab \rangle + P(ab) \sum_e t_{ij}^{ae} \left( \mathcal{F}_b^e - \frac{1}{2} \sum_m t_m^b \mathcal{F}_m^e \right) - P(ij) \sum_m t_{im}^{ab} \left( \mathcal{F}_m^j + \frac{1}{2} \sum_e t_j^e \mathcal{F}_m^e \right) \\
&\quad + \frac{1}{2} \sum_{mn} \tau_{mn}^{ab} \mathcal{W}_{mn}^{ij} + \frac{1}{2} \sum_{ef} \tau_{ij}^{ef} \mathcal{W}_{ab}^{ef} + P(ij) P(ab) \sum_{me} \left( t_{im}^{ae} \mathcal{W}_{mb}^{ej} - t_i^e t_m^a \langle mb || ej \rangle \right) \\
&\quad + P(ij) \sum_e t_i^e \langle ab || ej \rangle - P(ab) \sum_m t_m^a \langle mb || ij \rangle
\end{aligned}";
        let eq = parse(src).unwrap();
        // 1 + 2 + 2 | 1 + 1 + 2 | 1 + 1
        assert_eq!(
            eq.terms.len(),
            11,
            "{:#?}",
            eq.terms
                .iter()
                .map(|t| t.tensors.iter().map(|x| x.name.clone()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        assert_eq!(eq.terms.iter().map(|t| t.row).max(), Some(2));
        assert_eq!(eq.terms[2].sign, -1.0);
        assert_eq!(eq.terms[2].factor.as_deref(), Some(r"\frac{1}{2}"));
        let d = diagram(&eq).unwrap().doc;
        assert_eq!(d.groups.len(), 11);
    }

    #[test]
    fn invalid_terms_are_left_out_with_a_warning() {
        let d =
            from_latex(r"Z_{ijab} = V_{ijef} T_{efab} + \frac{1}{2} V_{mnef} T_{ijab}").unwrap();
        assert_eq!(d.warnings.len(), 1, "{:?}", d.warnings);
        assert!(d.warnings[0].starts_with("term 2"), "{}", d.warnings[0]);
        assert!(
            from_latex("E = V_{mnef} T_{ijab}").is_err(),
            "nothing valid is an error"
        );
    }

    #[test]
    fn imsrg_occupation_factors() {
        // (n_a - n_b) = n_a \bar n_b - \bar n_a n_b: two diagrams; the
        // two-body term is one.
        let d = from_latex(
            r"\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba} + \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar{n}_c \bar{n}_d",
        )
        .unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        assert_eq!(d.doc.groups.len(), 3);
        // (1 - n_a - n_b) = \bar n_a \bar n_b - n_a n_b.
        let eq = parse(r"Z_{pq} = \sum_{ab} (1 - n_a - n_b) A_{pqab} B_{abpq}").unwrap();
        let signs: Vec<f64> = eq.terms.iter().map(|t| t.sign).collect();
        assert_eq!(
            eq.terms.len(),
            2,
            "{:?}",
            eq.terms.iter().map(|t| &t.classes).collect::<Vec<_>>()
        );
        assert!(signs.contains(&1.0) && signs.contains(&-1.0));
        // External lines and indices without a factor are general: no arrows.
        let d = from_latex(r"\frac{d f_{12}}{ds} = \sum_a (1 + P_{12}) \eta_{1a} f_{a2}")
            .unwrap()
            .doc;
        assert_eq!(
            d.edges
                .iter()
                .filter(|e| e.class.as_deref() == Some("line"))
                .count(),
            3,
            "IMSRG notation (\\eta): 1, 2 and a are general orbitals"
        );
        assert!(
            d.edges
                .iter()
                .all(|e| e.class.as_deref() != Some("line") || e.arrow.is_none())
        );
    }

    #[test]
    fn several_terms_and_one_body() {
        let d = from_latex(r"Z_ai = f_ac T_ci - f_ki T_ak + V_kaci T_ck")
            .unwrap()
            .doc;
        assert_eq!(d.groups.len(), 3);
        assert!(d.vertices.iter().any(|v| v.kind == Some(VertexKind::Cross)));
    }
}
