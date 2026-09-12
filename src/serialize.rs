use crate::graph::Graph;
use crate::ingest::{
    Ingest, KIND_CLASS, KIND_FUNCTION, KIND_INTERFACE, KIND_MACRO, KIND_METHOD, KIND_SECTION,
    KIND_STRUCT, KIND_VAR,
};
use crate::rank::RankRun;

const LEGEND_MAIN: &str = "<!-- ripwire v1 t=fn|method|cls|struct|iface|var|sec|macro(#define;degraded:body-is-replacement-text,edges-cross-expansion) p=path layer=arch-layer(opt) n=name id=canonical(path::scope::name,when-scoped) k=rank c=call amb=ambiguous-calls(read-source) lpin=calls-pinned-by-locality-prior-alone(a-disclosed-guess;read-source;absent-if-0) overloads=N-same-name-defs-merged-into-this-row(absent-if-1;shown=counts-them-individually,so-rows+sum(overloads-1)=shown) hdr:unresolved=call-name-defined-only-in-a-lang-incompatible-file (edges heuristic) hdr:locality_pinned=sum-of-lpin(absent-if-0) hdr:external=calls-refused-as-bound-outside-the-tree(builtin/stdlib-name-without-in-repo-evidence,external-import,super-past-the-tree;no-edge;absent-if-0) r:est_tokens=hdr-copy(none-if-stable) -->";
const LEGEND_ROOT: &str = "<!-- r:root=crawl-root-every-p=-is-relative-to(single-root-only;absent=>p=is-the-raw-ingest-path) -->";
const LEGEND_PR: &str = "<!-- pr_iters=pagerank-power-iterations(stop:L1-residual-below-tol,else-ceiling) pr_converged=0-only-when-ceiling-hit-first(absent=converged;no-such-attr=not-pagerank-ordered) -->";

const ENVELOPE_BYTES: usize = 320;
const FILE_MARKUP: usize = 12;
const SYM_MARKUP: usize = 19 + 11;
const EDGE_MARKUP: usize = 9;
const DEFAULT_BPT: f64 = 2.50;

fn lang_rate(lang: &str) -> f64 {
    match lang {
        "go" => 2.53,
        "java" => 2.55,
        _ => DEFAULT_BPT,
    }
}

/// The map token-estimate model (ripwire estimateTokens): markup at the default rate, content per
/// language at its own rate. Returns (tokens, modelBytes).
fn estimate_model(ing: &Ingest, g: &Graph, root: &str) -> (usize, usize) {
    let full = |rel: &str| -> usize { root.trim_end_matches('/').len() + 1 + rel.len() };
    let mut markup: f64 = ENVELOPE_BYTES as f64;
    let mut content: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut seen_files = vec![false; ing.files.len()];
    for (id, s) in ing.symbols.iter().enumerate() {
        let li = ing.files[s.file_id]
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_string();
        if !seen_files[s.file_id] {
            seen_files[s.file_id] = true;
            markup += FILE_MARKUP as f64;
            *content.entry(li.clone()).or_insert(0.0) += full(&ing.files[s.file_id]) as f64;
        }
        markup += SYM_MARKUP as f64;
        *content.entry(li.clone()).or_insert(0.0) += s.name.len() as f64;
        if !s.scope.is_empty() {
            markup += 6.0;
            *content.entry(li.clone()).or_insert(0.0) +=
                (full(&ing.files[s.file_id]) + s.scope.len() + s.name.len() + 4) as f64;
        }
        for (from, to, _) in &g.out_edges {
            if *from == id {
                markup += EDGE_MARKUP as f64;
                *content.entry(li.clone()).or_insert(0.0) += ing.symbols[*to].name.len() as f64;
            }
        }
    }
    let mut est: f64 = markup / DEFAULT_BPT;
    let mut model_bytes = markup;
    for (lang, cb) in &content {
        est += cb / lang_rate(lang);
        model_bytes += cb;
    }
    ((est + 0.5) as usize, (model_bytes + 0.5) as usize)
}

fn bytes_per_token(model: (usize, usize)) -> f64 {
    if model.1 > 0 {
        model.1 as f64 / model.0 as f64
    } else {
        DEFAULT_BPT
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// builtinLayer: first directory component (case-insensitive) matching the layer table.
pub fn builtin_layer(path: &str) -> &'static str {
    const LAYERS: &[(&str, &str)] = &[
        ("game", "game"),
        ("gameplay", "game"),
        ("infra", "infra"),
        ("infrastructure", "infra"),
        ("infrastucture", "infra"),
        ("metal", "render"),
        ("render", "render"),
        ("renderer", "render"),
        ("math", "math"),
        ("numerics", "math"),
        ("sound", "audio"),
        ("audio", "audio"),
        ("steer", "ai"),
        ("ai", "ai"),
        ("behavior", "ai"),
        ("test", "test"),
        ("tests", "test"),
        ("bench", "test"),
    ];
    let last_slash = path.rfind('/').unwrap_or(path.len());
    let dirs = &path[..last_slash];
    for comp in dirs.split('/') {
        for (dir, layer) in LAYERS {
            if comp.eq_ignore_ascii_case(dir) {
                return layer;
            }
        }
    }
    ""
}

pub fn sym_tag(kind: &str) -> &str {
    match kind {
        KIND_FUNCTION => "fn",
        KIND_METHOD => "method",
        KIND_CLASS => "cls",
        KIND_STRUCT => "struct",
        KIND_INTERFACE => "iface",
        KIND_VAR => "var",
        KIND_SECTION => "sec",
        KIND_MACRO => "macro",
        _ => "var",
    }
}

pub fn serialize(ing: &Ingest, g: &Graph, rank: &RankRun, root: &str) -> String {
    let full_path = |rel: &str| -> String { format!("{}/{}", root.trim_end_matches('/'), rel) };
    // Order symbols by (rank DESC, id ASC). Node ids are ingest order (file, line, name).
    let n = ing.symbols.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| rank.ranks[b].total_cmp(&rank.ranks[a]).then(a.cmp(&b)));

    let model = estimate_model(ing, g, root);
    let bpt = bytes_per_token(model);

    // Build children: <f> rows in rank order, symbols bucketed by file.
    let mut file_order: Vec<usize> = Vec::new();
    let mut seen = vec![false; ing.files.len()];
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); ing.files.len()];
    for &id in &order {
        let f = ing.symbols[id].file_id;
        if !seen[f] {
            seen[f] = true;
            file_order.push(f);
        }
        buckets[f].push(id);
    }

    let mut children = String::new();
    for &f in &file_order {
        children.push_str("<f p=\"");
        children.push_str(&escape_xml(&ing.files[f]));
        children.push('"');
        let layer = builtin_layer(&full_path(&ing.files[f]));
        if !layer.is_empty() {
            children.push_str(" layer=\"");
            children.push_str(layer);
            children.push('"');
        }
        children.push('>');
        for &id in &buckets[f] {
            let s = &ing.symbols[id];
            children.push_str("<s t=\"");
            children.push_str(sym_tag(s.kind));
            children.push_str("\" n=\"");
            children.push_str(&escape_xml(&s.name));
            children.push('"');
            // id= when scoped (canonical != name); overloads/amb/lpin absent for this surface.
            children.push_str(&format!(" k=\"{:.4}\"", rank.ranks[id]));
            children.push('>');
            for (from, to, _) in &g.out_edges {
                if *from == id {
                    children.push_str("<c n=\"");
                    children.push_str(&escape_xml(&ing.symbols[*to].name));
                    children.push_str("\"/>");
                }
            }
            children.push_str("</s>");
        }
        children.push_str("</f>");
    }

    // Build head at a given est_tokens value (fixpoint).
    let stats = |est: usize| -> String {
        format!(
            "<!-- files={} symbols={} edges={} shown={} est_tokens={} ambiguous=0 unresolved=0 order=important-first -->",
            ing.files.len(), n, g.out_edges.len(), n, est
        )
    };
    let head = |est: usize| -> String {
        let mut h = String::new();
        h.push_str(LEGEND_MAIN);
        h.push_str(LEGEND_ROOT);
        h.push_str(LEGEND_PR);
        h.push_str(&stats(est));
        h.push_str("<r root=\"");
        h.push_str(&escape_xml(root));
        h.push_str("\" est_tokens=\"");
        h.push_str(&est.to_string());
        h.push_str("\" pr_iters=\"");
        h.push_str(&rank.iterations.to_string());
        h.push_str("\">");
        h
    };

    children.push_str("</r>");

    // Fixpoint: est covers head + children; head prints est, so converge on the digit width.
    let mut est = model.0;
    let mut head_s = head(est);
    for _ in 0..4 {
        let next = ((head_s.len() + children.len()) as f64 / bpt + 0.5) as usize;
        if next == est {
            break;
        }
        est = next;
        head_s = head(est);
    }

    let mut doc = String::new();
    doc.push_str(&head_s);
    doc.push_str(&children);
    doc
}
