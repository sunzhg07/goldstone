//! Reading and writing `.gold` files.
//!
//! [`save`] writes a document while keeping the comments, key order and
//! layout of the file it was loaded from: only values that actually changed
//! are rewritten, new objects are placed after the last object of the same
//! kind, and removed objects disappear together with their comments.

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, Value};

use crate::model::Document;
use crate::scene::Diagnostics;

/// Parses a `.gold` document. Syntax errors and unknown fields are reported
/// with line and column.
pub fn parse(src: &str) -> Result<Document, Diagnostics> {
    toml::from_str(src).map_err(|e| Diagnostics(vec![e.to_string().trim_end().to_string()]))
}

/// Serializes a document in canonical form: fixed section order, unset
/// fields omitted, whole numbers written as integers, nested tables inline.
/// Comments are not preserved; use [`save`] for that.
pub fn to_toml(doc: &Document) -> String {
    canonical(doc).to_string()
}

fn canonical(doc: &Document) -> DocumentMut {
    let text = toml::to_string(doc).expect("documents always serialize");
    let mut d: DocumentMut = text.parse().expect("serializer output is valid TOML");
    normalize_table(d.as_table_mut(), false);
    d
}

/// Writes `doc`, preserving comments and formatting of `previous` (the text
/// the document was loaded from) where possible.
pub fn save(doc: &Document, previous: Option<&str>) -> String {
    let new = canonical(doc);
    let Some(mut old) = previous.and_then(|p| p.parse::<DocumentMut>().ok()) else {
        return new.to_string();
    };
    merge_table(old.as_table_mut(), new.as_table());
    reposition(&mut old);
    let out = old.to_string();
    // Safety net: never write something that reads back differently.
    match parse(&out) {
        Ok(back) if back == *doc => out,
        _ => new.to_string(),
    }
}

// ---- normalisation ----------------------------------------------------------

fn normalize_value(v: &mut Value) {
    match v {
        Value::Float(f) => {
            let x = *f.value();
            if x.fract() == 0.0 && x.abs() < 1e15 {
                *v = Value::from(x as i64);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(normalize_value),
        Value::InlineTable(t) => t.iter_mut().for_each(|(_, v)| normalize_value(v)),
        _ => {}
    }
}

/// `inline`: sub-tables become inline tables (inside `[[…]]` entries).
fn normalize_table(t: &mut Table, inline: bool) {
    let keys: Vec<String> = t.iter().map(|(k, _)| k.to_string()).collect();
    for k in keys {
        let item = t.get_mut(&k).expect("key exists");
        match item {
            Item::Value(v) => normalize_value(v),
            Item::Table(sub) => {
                normalize_table(sub, inline);
                if inline {
                    let mut it = std::mem::take(sub).into_inline_table();
                    it.fmt();
                    it.decor_mut().set_prefix(" ");
                    *item = Item::Value(Value::InlineTable(it));
                    // `key = { … }`, not `key= { … }`.
                    if let Some(mut key) = t.key_mut(&k) {
                        key.leaf_decor_mut().set_suffix(" ");
                    }
                }
            }
            Item::ArrayOfTables(a) => a.iter_mut().for_each(|e| normalize_table(e, true)),
            Item::None => {}
        }
    }
}

// ---- semantic comparison ----------------------------------------------------

fn plain_value(v: &Value) -> Option<toml::Value> {
    let mut v = v.clone();
    v.decor_mut().clear();
    let t: toml::Table = toml::from_str(&format!("x = {v}")).ok()?;
    t.get("x").cloned()
}

fn plain(item: &Item) -> Option<toml::Value> {
    match item {
        Item::None => None,
        Item::Value(v) => plain_value(v),
        Item::Table(t) => {
            let mut out = toml::Table::new();
            for (k, i) in t.iter() {
                out.insert(k.to_string(), plain(i)?);
            }
            Some(toml::Value::Table(out))
        }
        Item::ArrayOfTables(a) => {
            let mut out = Vec::new();
            for t in a.iter() {
                out.push(plain(&Item::Table(t.clone()))?);
            }
            Some(toml::Value::Array(out))
        }
    }
}

fn same_value(a: &toml::Value, b: &toml::Value) -> bool {
    use toml::Value as V;
    match (a, b) {
        (V::Integer(i), V::Float(f)) | (V::Float(f), V::Integer(i)) => *i as f64 == *f,
        (V::Array(x), V::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_value(p, q))
        }
        (V::Table(x), V::Table(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| same_value(v, w)))
        }
        _ => a == b,
    }
}

fn same(a: &Item, b: &Item) -> bool {
    match (plain(a), plain(b)) {
        (Some(x), Some(y)) => same_value(&x, &y),
        _ => false,
    }
}

// ---- merging ----------------------------------------------------------------

fn merge_table(old: &mut Table, new: &Table) {
    let old_keys: Vec<String> = old.iter().map(|(k, _)| k.to_string()).collect();
    for k in old_keys {
        if !new.contains_key(&k) {
            old.remove(&k);
        }
    }
    for (k, n) in new.iter() {
        match old.get_mut(k) {
            None => {
                old.insert(k, n.clone());
            }
            Some(o) if same(o, n) => {}
            Some(o) => match (o, n) {
                (Item::Table(ot), Item::Table(nt)) => merge_table(ot, nt),
                (Item::ArrayOfTables(oa), Item::ArrayOfTables(na)) => merge_aot(oa, na),
                (Item::Value(ov), Item::Value(nv)) => {
                    let decor = ov.decor().clone();
                    let mut nv = nv.clone();
                    *nv.decor_mut() = decor;
                    *ov = nv;
                }
                (o, n) => *o = n.clone(),
            },
        }
    }
}

/// Identity of an array-of-tables entry used to match old and new entries.
fn entry_key(t: &Table) -> Option<String> {
    let s = |k: &str| t.get(k).and_then(plain).map(|v| v.to_string());
    if let Some(id) = s("id") {
        return Some(format!("id={id}"));
    }
    if let (Some(f), Some(to)) = (s("from"), s("to")) {
        return Some(format!("edge={f}->{to}"));
    }
    s("content").or_else(|| s("at")).or_else(|| s("origin"))
}

fn merge_aot(old: &mut ArrayOfTables, new: &ArrayOfTables) {
    let olds: Vec<Table> = old.iter().cloned().collect();
    let news: Vec<&Table> = new.iter().collect();
    let mut used = vec![false; olds.len()];
    let mut matched: Vec<Option<usize>> = vec![None; news.len()];
    // 1. Same identity, in order.
    for (i, n) in news.iter().enumerate() {
        if let Some(key) = entry_key(n)
            && let Some(j) = (0..olds.len())
                .find(|&j| !used[j] && entry_key(&olds[j]).as_deref() == Some(key.as_str()))
        {
            used[j] = true;
            matched[i] = Some(j);
        }
    }
    // 2. Entries without an `id` by position (an edited edge whose endpoint
    //    changed keeps its comments).
    for (i, m) in matched.iter_mut().enumerate() {
        if m.is_none()
            && i < olds.len()
            && !used[i]
            && olds[i].get("id").is_none()
            && news[i].get("id").is_none()
        {
            used[i] = true;
            *m = Some(i);
        }
    }
    let mut out = ArrayOfTables::new();
    for (i, n) in news.iter().enumerate() {
        match matched[i] {
            Some(j) => {
                let mut t = olds[j].clone();
                merge_table(&mut t, n);
                out.push(t);
            }
            None => {
                let mut t = (*n).clone();
                t.set_position(None);
                out.push(t);
            }
        }
    }
    *old = out;
}

// ---- table order ------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum Step {
    Key(String),
    Index(usize),
}

type Placement = (Vec<Step>, Option<isize>);

fn collect_tables(t: &Table, path: &mut Vec<Step>, out: &mut Vec<Placement>) {
    for (k, item) in t.iter() {
        path.push(Step::Key(k.to_string()));
        match item {
            Item::Table(sub) => {
                if !sub.is_implicit() {
                    out.push((path.clone(), sub.position()));
                }
                collect_tables(sub, path, out);
            }
            Item::ArrayOfTables(a) => {
                for (i, e) in a.iter().enumerate() {
                    path.push(Step::Index(i));
                    out.push((path.clone(), e.position()));
                    collect_tables(e, path, out);
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
}

fn table_at<'a>(t: &'a mut Table, path: &[Step]) -> Option<&'a mut Table> {
    let (Step::Key(k), rest) = path.split_first()? else {
        return None;
    };
    match t.get_mut(k)? {
        Item::Table(sub) => {
            if rest.is_empty() {
                Some(sub)
            } else {
                table_at(sub, rest)
            }
        }
        Item::ArrayOfTables(a) => {
            let (Step::Index(i), rest) = rest.split_first()? else {
                return None;
            };
            let e = a.get_mut(*i)?;
            if rest.is_empty() {
                Some(e)
            } else {
                table_at(e, rest)
            }
        }
        _ => None,
    }
}

/// Orders tables: existing ones keep their relative order; new ones go
/// right after the last existing entry of the same array (or at the end).
fn reposition(doc: &mut DocumentMut) {
    let mut all = Vec::new();
    collect_tables(doc.as_table(), &mut Vec::new(), &mut all);
    let (mut placed, new): (Vec<_>, Vec<_>) = all.into_iter().partition(|(_, p)| p.is_some());
    placed.sort_by_key(|(_, p)| *p);
    let mut order: Vec<Vec<Step>> = placed.into_iter().map(|(p, _)| p).collect();
    for (path, _) in new {
        let parent = &path[..path.len() - 1];
        let at = order
            .iter()
            .rposition(|p| p.len() == path.len() && &p[..p.len() - 1] == parent);
        match at {
            Some(i) => order.insert(i + 1, path),
            None => order.push(path),
        }
    }
    for (i, path) in order.iter().enumerate() {
        if let Some(t) = table_at(doc.as_table_mut(), path) {
            t.set_position(Some(i as isize + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latex_and_plot_sources_survive_saving() {
        let mut d = crate::expr::from_latex_editable(r"E2 = \frac{1}{4} V_{ijab} T_{abij}")
            .unwrap()
            .doc;
        let p = crate::plot::from_source(&crate::model::PlotSource::default()).unwrap();
        d.groups.extend(p.doc.groups.clone());
        let text = save(&d, None);
        let back = parse(&text).unwrap();
        assert_eq!(
            back.groups[0].latex.as_deref(),
            Some(r"E2 = \frac{1}{4} V_{ijab} T_{abij}")
        );
        assert_eq!(
            back.groups[1].plot,
            Some(crate::model::PlotSource::default())
        );
        // Saving over a hand-written file keeps them too.
        let again = save(&back, Some(&text));
        assert_eq!(parse(&again).unwrap(), back);
    }
    use crate::model::{ArrowDir, Vertex};

    const EXAMPLE: &str = include_str!("../../../examples/mbpt2.gold");

    fn vertex(id: &str, at: [f64; 2]) -> Vertex {
        Vertex {
            id: id.into(),
            at,
            class: None,
            kind: None,
            size: None,
            rotation: None,
            color: None,
            width: None,
            fill: None,
            layer: None,
            label: None,
            label_anchor: None,
        }
    }

    #[test]
    fn round_trip() {
        let doc = parse(EXAMPLE).unwrap();
        let out = to_toml(&doc);
        assert_eq!(parse(&out).unwrap(), doc);
        assert_eq!(
            to_toml(&parse(&out).unwrap()),
            out,
            "canonical form is stable"
        );
        assert!(
            out.contains("at = [70, 60]"),
            "whole numbers stay integers:\n{out}"
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

    #[test]
    fn save_unchanged_is_identical() {
        for src in [EXAMPLE, include_str!("../../../examples/gallery.gold")] {
            let doc = parse(src).unwrap();
            assert_eq!(save(&doc, Some(src)), src);
        }
    }

    #[test]
    fn save_keeps_comments_and_changes_only_what_changed() {
        let src = "# My diagram\nformat = 1\n\n# the bottom vertex\n[[vertex]]\nid = \"a\"\nat = [0, 0]   # origin\nkind = \"dot\"\n\n[[vertex]]\nid = \"b\"\nat = [0, 40]\n\n# the line\n[[edge]]\nfrom = \"a\"\nto = \"b\"\n";
        let mut doc = parse(src).unwrap();
        doc.vertices[1].at = [10.0, 40.5];
        doc.edges[0].arrow = Some(ArrowDir::Forward);
        let out = save(&doc, Some(src));
        assert!(
            out.starts_with("# My diagram\nformat = 1\n\n# the bottom vertex\n[[vertex]]\nid = \"a\"\nat = [0, 0]   # origin\n"),
            "{out}"
        );
        assert!(out.contains("at = [10, 40.5]"), "{out}");
        assert!(out.contains("# the line\n[[edge]]"), "{out}");
        assert!(out.contains("arrow = \"forward\""), "{out}");
        assert_eq!(parse(&out).unwrap(), doc);
    }

    #[test]
    fn save_places_new_objects_with_their_kind_and_drops_deleted_ones() {
        let src = "format = 1\n[[vertex]]\nid = \"a\"\nat = [0, 0]\n# keep b\n[[vertex]]\nid = \"b\"\nat = [0, 40]\n# remove c\n[[vertex]]\nid = \"c\"\nat = [5, 5]\n[[edge]]\nfrom = \"a\"\nto = \"b\"\n";
        let mut doc = parse(src).unwrap();
        doc.vertices.remove(2);
        doc.vertices.push(vertex("n", [20.0, 20.0]));
        let mut e = doc.edges[0].clone();
        e.to = "n".into();
        doc.edges.push(e);
        let out = save(&doc, Some(src));
        assert!(!out.contains("remove c"), "{out}");
        assert!(out.contains("# keep b"), "{out}");
        let n = out.find("id = \"n\"").unwrap();
        let first_edge = out.find("[[edge]]").unwrap();
        assert!(n < first_edge, "new vertex goes with the vertices:\n{out}");
        assert_eq!(parse(&out).unwrap(), doc);
    }

    #[test]
    fn save_without_previous_is_canonical() {
        let doc = parse(EXAMPLE).unwrap();
        assert_eq!(save(&doc, None), to_toml(&doc));
    }
}
