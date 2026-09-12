use crate::ingest::{Ingest, Symbol};

pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub weight: f32,
}

pub struct Graph {
    pub row_offsets: Vec<u32>,
    pub col_indices: Vec<u32>,
    pub values: Vec<f32>,
    pub w_out_deg: Vec<f64>,
    pub out_edges: Vec<(usize, usize, f32)>, // (from, to, weight) sorted by (from, to)
}

const TIER_SAME_FILE: f32 = 1.0;

/// Enclosing definition of a reference, per file: innermost def whose byte span contains the ref.
fn enclosing(defs: &[Symbol], file_id: usize, sb: usize, eb: usize) -> Option<&Symbol> {
    defs.iter()
        .filter(|s| s.file_id == file_id && s.start_byte <= sb && s.end_byte >= eb)
        .max_by_key(|s| s.start_byte)
}

/// Resolve a reference's callee candidates by the precedence ladder: same file → same dir → unique
/// global (same language). Returns the candidate symbol ids.
fn candidates(ing: &Ingest, name: &str, file_id: usize) -> Vec<usize> {
    let same_file: Vec<usize> = ing
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == name && s.file_id == file_id)
        .map(|(i, _)| i)
        .collect();
    if !same_file.is_empty() {
        return same_file;
    }
    let dir = ing.files[file_id].rsplit('/').next().unwrap_or("");
    let same_dir: Vec<usize> = ing
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.name == name
                && ing.files[s.file_id].rsplit('/').next().unwrap_or("") == dir
                && s.file_id != file_id
        })
        .map(|(i, _)| i)
        .collect();
    if !same_dir.is_empty() {
        return same_dir;
    }
    let global: Vec<usize> = ing
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == name && s.file_id != file_id)
        .map(|(i, _)| i)
        .collect();
    if global.len() == 1 {
        return global;
    }
    Vec::new()
}

pub fn build(ing: &Ingest) -> Graph {
    // Per-file def spans for enclosing lookup.
    let mut defs = ing.symbols.clone();
    defs.sort_by_key(|s| (s.file_id, s.start_byte));

    // Accumulate edges: from (enclosing def) → resolved callee.
    let mut acc: std::collections::HashMap<(usize, usize), (f32, u32)> =
        std::collections::HashMap::new();
    for r in &ing.refs {
        let Some(encl) = enclosing(&defs, r.file_id, r.start_byte, r.end_byte) else {
            continue;
        };
        let from = defs.iter().position(|s| std::ptr::eq(s, encl)).unwrap();
        let cands = candidates(ing, &r.name, r.file_id);
        if cands.is_empty() {
            continue; // external / unresolved — no edge
        }
        // k>1 candidates split evenly: conf / k per target.
        let base = TIER_SAME_FILE / cands.len() as f32;
        for &to in &cands {
            if to == from {
                continue;
            }
            let e = acc.entry((from, to)).or_insert((0.0, 0));
            e.0 += base;
            e.1 += 1;
        }
    }

    // Flatten: weight = (confSum/nref) * sqrt(nref), cap 8.
    let mut out_edges: Vec<(usize, usize, f32)> = acc
        .iter()
        .map(|((from, to), (conf, nref))| {
            let w = (conf / *nref as f32) * (*nref as f32).sqrt();
            (*from, *to, w.min(8.0))
        })
        .collect();
    out_edges.sort_by_key(|e| (e.0, e.1));

    let n = ing.symbols.len();
    let mut w_out = vec![0.0f64; n];
    for (from, _, w) in &out_edges {
        w_out[*from] += *w as f64;
    }

    // In-edge CSR: row per target.
    let mut row_offsets = vec![0u32; n + 1];
    for (_, to, _) in &out_edges {
        row_offsets[*to + 1] += 1;
    }
    for i in 0..n {
        row_offsets[i + 1] += row_offsets[i];
    }
    let mut col_indices = vec![0u32; out_edges.len()];
    let mut values = vec![0.0f32; out_edges.len()];
    let mut fill = row_offsets.clone();
    for (from, to, w) in &out_edges {
        let slot = fill[*to] as usize;
        col_indices[slot] = *from as u32;
        values[slot] = *w;
        fill[*to] += 1;
    }

    Graph {
        row_offsets,
        col_indices,
        values,
        w_out_deg: w_out,
        out_edges,
    }
}
