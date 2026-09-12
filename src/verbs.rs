use crate::cli::Config;
use crate::graph::Graph;
use crate::ingest::{Ingest, Symbol};
use crate::legends::GREP_LEGEND;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Innermost symbol whose byte span contains `byte` in `file_id`.
fn enclosing<'a>(ing: &'a Ingest, file_id: usize, byte: usize) -> Option<&'a Symbol> {
    ing.symbols
        .iter()
        .filter(|s| s.file_id == file_id && s.start_byte <= byte && s.end_byte >= byte)
        .max_by_key(|s| s.start_byte)
}

/// Distinct in-edge sources per target (fan-in), for the enc rows' callers=.
fn fan_in(g: &Graph) -> Vec<u32> {
    let n = g.row_offsets.len() - 1;
    let mut out = vec![0u32; n];
    for t in 0..n {
        let mut seen = std::collections::BTreeSet::new();
        for e in g.row_offsets[t] as usize..g.row_offsets[t + 1] as usize {
            seen.insert(g.col_indices[e]);
        }
        out[t] = seen.len() as u32;
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
        let Ok(bytes) = std::fs::read(format!("{}/{}", root, rel)) else { continue };
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
            let Some(mpos) = line.find(pattern) else { continue };
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
                enc.map(|s| format!(" in=\"{}\"", escape_xml(&s.name))).unwrap_or_default(),
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
        'outer: for (fid, rel) in ing.files.iter().enumerate() {
            let Ok(bytes) = std::fs::read(format!("{}/{}", root, rel)) else { continue };
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