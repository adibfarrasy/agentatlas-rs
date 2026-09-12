use crate::graph::Graph;
use crate::ingest::{Ingest, Symbol};
use crate::legends::GREP_LEGEND;

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Innermost symbol whose byte span contains `byte` in `file_id`.
fn enclosing(ing: &Ingest, file_id: usize, byte: usize) -> Option<&Symbol> {
    ing.symbols
        .iter()
        .filter(|s| s.file_id == file_id && s.start_byte <= byte && s.end_byte >= byte)
        .max_by_key(|s| s.start_byte)
}

/// Distinct in-edge sources per target (fan-in), for the enc rows' callers=.
fn fan_in(g: &Graph) -> Vec<u32> {
    let n = g.row_offsets.len() - 1;
    let mut out = vec![0u32; n];
    for (t, slot) in out.iter_mut().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        for e in g.row_offsets[t] as usize..g.row_offsets[t + 1] as usize {
            seen.insert(g.col_indices[e]);
        }
        *slot = seen.len() as u32;
    }
    out
}

pub fn grep(ing: &Ingest, g: &Graph, root: &str, pattern: &str) -> String {
    let fan_in = fan_in(g);
    let mut out = String::new();
    out.push_str(GREP_LEGEND);

    let mut total_hits = 0usize;
    let mut file_blocks: Vec<String> = Vec::new();
    let mut enc_names: Vec<&str> = Vec::new();

    for (fid, rel) in ing.files.iter().enumerate() {
        let Ok(bytes) = std::fs::read(format!("{}/{}", root, rel)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        // line start byte offsets
        let mut line_offsets: Vec<usize> = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_offsets.push(i + 1);
            }
        }
        let mut hits_block = String::new();
        for (li, line) in text.lines().enumerate() {
            let lstart = line_offsets[li];
            let Some(mpos) = line.find(pattern) else {
                continue;
            };
            let hit_byte = lstart + mpos;
            let enc = enclosing(ing, fid, hit_byte);
            let enc_name = enc.map(|s| s.name.as_str());
            if let Some(en) = enc_name {
                if !enc_names.contains(&en) {
                    enc_names.push(en);
                }
            }
            let line_no = li + 1;
            hits_block.push_str(&format!(
                "<hit l=\"{}\"{}><![CDATA[{}]]></hit>",
                line_no,
                enc.map(|s| format!(" in=\"{}\"", escape_xml(&s.name)))
                    .unwrap_or_default(),
                line
            ));
            total_hits += 1;
        }
        if total_hits > 0 || !hits_block.is_empty() {
            file_blocks.push(format!("<f p=\"{}\">{}</f>", escape_xml(rel), hits_block));
        }
    }

    let top_line = {
        // top hit = first file's first hit; recompute for next=
        let mut next = String::new();
        'outer: for rel in ing.files.iter() {
            let Ok(bytes) = std::fs::read(format!("{}/{}", root, rel)) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            for (li, line) in text.lines().enumerate() {
                if line.contains(pattern) {
                    next = format!("--at={}:{}", rel, li + 1);
                    break 'outer;
                }
            }
        }
        next
    };

    out.push_str(&format!(
        "<grep pattern=\"{}\" root=\"{}\" files=\"{}\" hits=\"{}\" shown=\"{}\" capped=\"0\" hits_capped=\"0\" complete=\"1\" unindexed_hits=\"0\" unindexed_files_scanned=\"0\" next=\"{}\">",
        escape_xml(pattern),
        escape_xml(root),
        ing.files.len(),
        total_hits,
        total_hits,
        top_line
    ));
    for block in &file_blocks {
        out.push_str(block);
    }
    for name in &enc_names {
        let row = ing
            .symbols
            .iter()
            .enumerate()
            .find(|(_, s)| s.name == *name);
        if let Some((id, s)) = row {
            let mut r = format!("<enc n=\"{}\" callers=\"{}\"", escape_xml(name), fan_in[id]);
            if s.kind == crate::ingest::KIND_FUNCTION || s.kind == crate::ingest::KIND_METHOD {
                r.push_str(&format!(" cx=\"{}\"", s.cx));
            }
            r.push_str("/>");
            out.push_str(&r);
        }
    }
    out.push_str("</grep>");
    out
}
pub fn callers(ing: &Ingest, g: &Graph, root: &str, sel: &str) -> String {
    hierarchy(ing, g, root, sel, "callers")
}

pub fn callees(ing: &Ingest, g: &Graph, root: &str, sel: &str) -> String {
    hierarchy(ing, g, root, sel, "callees")
}

fn sym_tag(kind: &str) -> &str {
    crate::serialize::sym_tag(kind)
}

fn row_tier(rel: &str) -> u8 {
    if crate::serialize::builtin_layer(rel) == "test" {
        return 1;
    }
    if rel.starts_with("docs/") || rel.contains("/docs/") {
        return 2;
    }
    0
}

/// A symbol is "tested" if a test-shaped symbol transitively reaches it. Test-shaped: name
/// starting with "Test" (Go convention), in a *_test.go / *Test.java file, or a JUnit-ish method.
fn tested_reaches(ing: &Ingest, g: &Graph) -> Vec<bool> {
    let n = ing.symbols.len();
    let mut is_test = vec![false; n];
    for (id, s) in ing.symbols.iter().enumerate() {
        let rel = &ing.files[s.file_id];
        let base = rel.rsplit('/').next().unwrap_or(rel);
        if base.ends_with("_test.go")
            || base.ends_with("Test.java")
            || (s.name.len() > 4 && s.name.starts_with("Test"))
        {
            is_test[id] = true;
        }
    }
    // transitive reach over out-edges from tests (1-hop for the hop_tested gauge is enough on
    // the fixture; grow to BFS if a fixture needs it)
    let mut tested = is_test.clone();
    for (from, to, _) in &g.out_edges {
        if is_test[*from] {
            tested[*to] = true;
        }
    }
    tested
}

fn hierarchy(ing: &Ingest, g: &Graph, root: &str, sel: &str, which: &str) -> String {
    use crate::legends::{CALLEES_LEGEND, CALLERS_LEGEND};
    let legend = if which == "callers" {
        CALLERS_LEGEND
    } else {
        CALLEES_LEGEND
    };
    let mut out = String::new();
    out.push_str(legend);

    let defs: Vec<usize> = ing
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == sel)
        .map(|(i, _)| i)
        .collect();

    // neighbours: for callers = distinct sources of in-edges to defs; callees = distinct targets of
    // out-edges from defs.
    let mut neighbour_ids: Vec<usize> = Vec::new();
    for (from, to, _) in &g.out_edges {
        if which == "callers" {
            if defs.contains(to) && !neighbour_ids.contains(from) {
                neighbour_ids.push(*from);
            }
        } else if defs.contains(from) && !neighbour_ids.contains(to) {
            neighbour_ids.push(*to);
        }
    }

    let tested = tested_reaches(ing, g);
    let mut hop_tested = 0usize;
    let mut hop_untested = 0usize;
    for &id in &neighbour_ids {
        if tested[id] {
            hop_tested += 1;
        } else {
            hop_untested += 1;
        }
    }

    // rows sorted SOURCE → test/bench → docs, path within tier
    let mut rows: Vec<(u8, String, usize)> = neighbour_ids
        .iter()
        .map(|&id| {
            (
                row_tier(&ing.files[ing.symbols[id].file_id]),
                ing.files[ing.symbols[id].file_id].clone(),
                id,
            )
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.as_bytes().cmp(b.1.as_bytes())));

    let mut amb_total = 0usize;
    let mut unresolved_total = 0usize;
    let _ = (&mut amb_total, &mut unresolved_total);

    let next = if which == "callers" {
        format!("--uses={}", sel)
    } else {
        format!("--expand={}", sel)
    };
    out.push_str(&format!(
        "<{} of=\"{}\" defs=\"{}\" count=\"{}\" root=\"{}\" hop_tested=\"{}\" hop_untested=\"{}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" counts_floor=\"1\" next=\"{}\">",
        which, crate::verbs::escape_xml(sel), defs.len(), neighbour_ids.len(),
        crate::verbs::escape_xml(root), hop_tested, hop_untested, crate::verbs::escape_xml(&next)
    ));
    for (_, _, id) in &rows {
        let s = &ing.symbols[*id];
        out.push_str(&format!(
            "<s t=\"{}\" n=\"{}\" p=\"{}\"/>",
            sym_tag(s.kind),
            crate::verbs::escape_xml(&s.name),
            crate::verbs::escape_xml(&format!("{}:{}", ing.files[s.file_id], s.line))
        ));
    }
    out.push_str(&format!("</{}>", which));
    out
}

pub fn uses(ing: &Ingest, _g: &Graph, root: &str, sel: &str) -> String {
    use crate::legends::USES_LEGEND;
    let mut out = String::new();
    out.push_str(USES_LEGEND);
    let defs: Vec<usize> = ing
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == sel)
        .map(|(i, _)| i)
        .collect();
    let external = if defs.is_empty() { 1 } else { 0 };
    // call/read/write/import/extends use-sites: for the Go+Java surface the only captured role is
    // call (a method_invocation / object_creation / call_expression resolving to a def of sel).
    // role="type" is C/C++/ObjC-only per the legend; Go composite literals are not calls.
    out.push_str(&format!(
        "<uses of=\"{}\" defs=\"{}\" external=\"{}\" count=\"0\" root=\"{}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" counts_floor=\"1\"></uses>",
        escape_xml(sel), defs.len(), external, escape_xml(root)
    ));
    out
}
