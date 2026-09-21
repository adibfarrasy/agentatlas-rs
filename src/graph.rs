use crate::ingest::{Ingest, Symbol};
use std::collections::HashMap;

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
/// `defs` is (original symbol index, symbol) pairs sorted by (file_id, start_byte), so the file's
/// defs are a contiguous slice found by binary search instead of a full scan.
fn enclosing(defs: &[(usize, &Symbol)], file_id: usize, sb: usize, eb: usize) -> Option<usize> {
    let start = defs.partition_point(|(_, s)| s.file_id < file_id);
    let end = start + defs[start..].partition_point(|(_, s)| s.file_id == file_id);
    defs[start..end]
        .iter()
        .filter(|(_, s)| s.start_byte <= sb && s.end_byte >= eb)
        .max_by_key(|(_, s)| s.start_byte)
        .map(|(i, _)| *i)
}

/// Resolve a reference's callee candidates by the precedence ladder: same file → same dir → unique
/// global (same language). Returns the candidate symbol ids. `by_name` narrows to same-named
/// symbols up front instead of scanning every symbol per reference.
fn candidates(ing: &Ingest, by_name: &HashMap<&str, Vec<usize>>, name: &str, file_id: usize) -> Vec<usize> {
    let Some(ids) = by_name.get(name) else {
        return Vec::new();
    };
    let same_file: Vec<usize> = ids
        .iter()
        .copied()
        .filter(|&i| ing.symbols[i].file_id == file_id)
        .collect();
    if !same_file.is_empty() {
        return same_file;
    }
    let dir = ing.files[file_id].rsplit('/').next().unwrap_or("");
    let same_dir: Vec<usize> = ids
        .iter()
        .copied()
        .filter(|&i| {
            let s = &ing.symbols[i];
            s.file_id != file_id && ing.files[s.file_id].rsplit('/').next().unwrap_or("") == dir
        })
        .collect();
    if !same_dir.is_empty() {
        return same_dir;
    }
    let global: Vec<usize> = ids
        .iter()
        .copied()
        .filter(|&i| ing.symbols[i].file_id != file_id)
        .collect();
    if global.len() == 1 {
        return global;
    }
    Vec::new()
}

pub fn build(ing: &Ingest) -> Graph {
    // Per-file def spans for enclosing lookup: (original index, symbol) sorted by (file_id, start_byte).
    let mut defs: Vec<(usize, &Symbol)> = ing.symbols.iter().enumerate().collect();
    defs.sort_by_key(|(_, s)| (s.file_id, s.start_byte));

    // Name -> symbol indices, so candidate resolution doesn't rescan every symbol per reference.
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, s) in ing.symbols.iter().enumerate() {
        by_name.entry(s.name.as_str()).or_default().push(i);
    }

    // Accumulate edges: from (enclosing def) → resolved callee.
    let mut acc: HashMap<(usize, usize), (f32, u32)> = HashMap::new();
    for r in &ing.refs {
        let Some(from) = enclosing(&defs, r.file_id, r.start_byte, r.end_byte) else {
            continue;
        };
        let cands = candidates(ing, &by_name, &r.name, r.file_id);
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
