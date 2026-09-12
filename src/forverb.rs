use crate::graph::Graph;
use crate::ingest::Ingest;

// ── subtoken splitter (scalar port of ripwire's forEachLexTokenSpan) ────────────────────────────────
pub fn subtokens(id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = id.as_bytes();
    let n = bytes.len();
    let mut pending: Option<usize> = None;
    let mut prev_alnum = false;
    let mut prev_upper = false;
    for i in 0..n {
        let b = bytes[i];
        let alnum = b.is_ascii_alphanumeric();
        let upper = b.is_ascii_uppercase();
        let next_lower = i + 1 < n && bytes[i + 1].is_ascii_lowercase();
        // an uppercase starts a new token when the previous byte was alnum and (prev not upper OR a
        // lowercase follows) — the acronym rule: only the LAST uppercase of a run, and only when a
        // lowercase follows it
        let split = upper && prev_alnum && (!prev_upper || next_lower);
        let starts = (alnum && !prev_alnum) || split;
        if !alnum {
            if let Some(s) = pending {
                push_token(&mut out, &id[s..i]);
                pending = None;
            }
        } else if starts {
            if let Some(s) = pending {
                push_token(&mut out, &id[s..i]);
            }
            pending = Some(i);
        }
        prev_alnum = alnum;
        prev_upper = upper;
    }
    if let Some(s) = pending {
        push_token(&mut out, &id[s..n]);
    }
    out
}

fn push_token(out: &mut Vec<String>, tok: &str) {
    if tok.len() < 2 {
        return; // the ≥2-byte drop
    }
    out.push(tok.to_ascii_lowercase());
}

// ── router ────────────────────────────────────────────────────────────────────────────────────────
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "to", "of", "in", "for", "how", "does", "do", "where", "what",
    "which", "on", "with",
];

fn split_words(query: &str) -> Vec<&str> {
    query
        .split(|c: char| c == ' ' || c == '\t' || c == '\n' || c == '\r')
        .filter(|w| !w.is_empty())
        .collect()
}

pub struct Route {
    pub name_exact: bool,
    pub reason: String,
}

pub fn choose_ranker(ing: &Ingest, query: &str) -> Route {
    let words = split_words(query);
    let mut n_words = 0usize;
    let mut whole_name_hits = 0usize;
    let mut has_camel_snake = false;
    let mut identifier_hit = String::new();

    let lower_names: Vec<&str> = ing.symbols.iter().map(|s| s.name.as_str()).collect();
    for w in &words {
        let lw = w.to_ascii_lowercase();
        if STOPWORDS.contains(&lw.as_str()) {
            continue;
        }
        n_words += 1;
        let mut camel = false;
        let mut snake = false;
        let wb = w.as_bytes();
        for k in 1..w.len() {
            let c = wb[k];
            if c.is_ascii_uppercase() && wb[k - 1].is_ascii_lowercase() {
                camel = true;
            }
            if c == b'_' && k + 1 < w.len() {
                snake = true;
            }
        }
        if camel || snake {
            has_camel_snake = true;
            if identifier_hit.is_empty() {
                identifier_hit = w.to_string();
            }
            whole_name_hits += 1;
            continue;
        }
        if lower_names.iter().any(|n| n.to_ascii_lowercase() == lw) {
            whole_name_hits += 1;
            if identifier_hit.is_empty() {
                identifier_hit = w.to_string();
            }
        }
    }

    let camel_short = has_camel_snake && n_words <= 2;
    let all_names = n_words >= 1 && whole_name_hits == n_words;
    let name_exact = camel_short || all_names;

    if name_exact {
        Route {
            name_exact: true,
            reason: format!(
                "name-exact BM25 — query names a symbol ({})",
                identifier_hit
            ),
        }
    } else if n_words >= 3 {
        Route {
            name_exact: false,
            reason: "subtoken+body BM25 (--for's default) — no strong name hit, multi-word conceptual query".to_string(),
        }
    } else {
        Route {
            name_exact: false,
            reason: "subtoken+body BM25 (--for's default) — no strong name hit; broad query, plain rg may also win".to_string(),
        }
    }
}

// ── BM25 scoring ──────────────────────────────────────────────────────────────────────────────────
const K1: f64 = 1.5;
const B: f64 = 0.75;
const KW_NAME: i32 = 3;
const KW_CALLEE: i32 = 1;
const KW_BODY: i32 = 1;

struct Scorer<'a> {
    ing: &'a Ingest,
    g: &'a Graph,
    root: &'a str,
}

impl<'a> Scorer<'a> {
    /// tokenized evidence text for one symbol, with its weight: (text, weight)
    fn fields(&self, id: usize) -> Vec<(String, i32)> {
        let s = &self.ing.symbols[id];
        let mut f = vec![(s.name.clone(), KW_NAME)];
        // callee names (kwCallee)
        let mut callees = String::new();
        for (from, to, _) in &self.g.out_edges {
            if *from == id {
                callees.push_str(&self.ing.symbols[*to].name);
                callees.push(' ');
            }
        }
        if !callees.is_empty() {
            f.push((callees, KW_CALLEE));
        }
        // body
        if s.body_end > s.body_start {
            if let Ok(bytes) = std::fs::read(format!("{}/{}", self.root, self.ing.files[s.file_id]))
            {
                if s.body_end <= bytes.len() && s.body_start <= s.body_end {
                    f.push((
                        String::from_utf8_lossy(&bytes[s.body_start..s.body_end]).into_owned(),
                        KW_BODY,
                    ));
                }
            }
        }
        f
    }

    /// weighted subtoken list for a symbol (for dl) and the weighted tf per query unique token.
    fn doc_stats(&self, id: usize, uniq: &[String]) -> (i32, Vec<i32>) {
        let mut dl = 0i32;
        let mut tf = vec![0i32; uniq.len()];
        for (text, w) in self.fields(id) {
            for tok in subtokens(&text) {
                dl += w;
                for (u, _q) in uniq.iter().enumerate() {
                    if uniq[u] == tok {
                        tf[u] += w;
                    }
                }
            }
        }
        (dl, tf)
    }
}

pub struct ForRanking {
    pub scores: Vec<f32>,  // per-symbol
    pub order: Vec<usize>, // symbol ids, score desc (positive only, then by id)
    pub kept: usize,
    pub positive_hits: usize,
    pub hit_ceiling: bool,
    pub margin_pct: i32,
    pub route: Route,
    pub total_window: usize,
}

pub fn rank(ing: &Ingest, g: &Graph, root: &str, query: &str) -> ForRanking {
    let route = choose_ranker(ing, query);
    let words = split_words(query);
    let mut uniq: Vec<String> = Vec::new();
    for w in &words {
        let lw = w.to_ascii_lowercase();
        for t in subtokens(&lw) {
            if !uniq.contains(&t) {
                uniq.push(t);
            }
        }
    }

    let scorer = Scorer { ing, g, root };
    let s_count = ing.symbols.len();

    // document frequency: how many symbols contain each unique query token
    let mut df = vec![0usize; uniq.len()];
    let mut all_dl = vec![0i32; s_count];
    let mut all_tf = vec![vec![0i32; uniq.len()]; s_count];
    for i in 0..s_count {
        let (dl, tf) = scorer.doc_stats(i, &uniq);
        all_dl[i] = dl;
        for (u, v) in tf.iter().enumerate() {
            all_tf[i][u] = *v;
            if *v > 0 {
                df[u] += 1;
            }
        }
    }
    let s = s_count as f64;
    let avgdl = all_dl.iter().map(|&d| d as f64).sum::<f64>()
        / if s_count > 0 { s_count as f64 } else { 1.0 };

    let mut scores = vec![0f32; s_count];
    for i in 0..s_count {
        let mut sc = 0.0f64;
        for (u, _q) in uniq.iter().enumerate() {
            let tf = all_tf[i][u];
            if tf == 0 || df[u] == 0 {
                continue;
            }
            let n = df[u] as f64;
            let idf = ((s - n + 0.5) / (n + 0.5) + 1.0).ln();
            let dl = all_dl[i] as f64;
            let denom = tf as f64 + K1 * (1.0 - B + B * dl / if avgdl > 0.0 { avgdl } else { 1.0 });
            sc += idf * (tf as f64 * (K1 + 1.0)) / denom;
        }
        scores[i] = sc as f32;
    }

    // adaptive cut (floor 5, ceiling 40, scan full distribution)
    let mut pos: Vec<f32> = scores.iter().copied().filter(|&s| s > 0.0).collect();
    pos.sort_by(|a, b| b.total_cmp(a));
    let avail = pos.len();
    let positive_hits = avail;
    let floor_k = 5usize;
    let ceiling_k = 40usize;
    let total_window = ceiling_k;
    let mut kept;
    let mut hit_ceiling = true;
    let mut margin_pct = 0i32;

    if avail > 0 {
        let hard_ceil = (ceiling_k.min(avail)).max(floor_k.min(avail));
        let f = floor_k.min(hard_ceil);
        let scan_end = avail;
        let mut best_drop = 0.0f64;
        let mut best_cap_cut = 0usize;
        let mut best_cap_drop = 0.0f64;
        for i in 1..scan_end {
            let prev = pos[i - 1] as f64;
            let here = pos[i] as f64;
            let drop = if prev > 0.0 {
                (prev - here) / prev
            } else {
                0.0
            };
            if drop > best_drop {
                best_drop = drop;
            }
            if i < hard_ceil && drop > best_cap_drop {
                best_cap_drop = drop;
                best_cap_cut = i;
            }
        }
        const MIN_CLIFF: f64 = 0.20;
        if best_cap_drop >= MIN_CLIFF {
            kept = best_cap_cut.max(f);
            margin_pct = (best_cap_drop * 100.0 + 0.5) as i32;
            hit_ceiling = false;
        } else {
            kept = hard_ceil;
            margin_pct = if best_drop >= MIN_CLIFF {
                (best_drop * 100.0 + 0.5) as i32
            } else {
                0
            };
        }
        kept = kept.max(f);
    } else {
        kept = floor_k.min(ceiling_k);
    }

    // order: symbol ids sorted by score desc (positive scores first, then any remaining by id)
    let mut order: Vec<usize> = (0..s_count).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));

    ForRanking {
        scores,
        order,
        kept,
        positive_hits,
        hit_ceiling,
        margin_pct,
        route,
        total_window,
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// first line of the def, trimmed of a trailing "{" — the signature shown in <d> rows
pub fn signature(ing: &Ingest, root: &str, id: usize) -> String {
    let s = &ing.symbols[id];
    let Ok(bytes) = std::fs::read(format!("{}/{}", root, ing.files[s.file_id])) else {
        return String::new();
    };
    let sb = s.start_byte.min(bytes.len());
    let rest = &bytes[sb..];
    let line_end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
    let line = String::from_utf8_lossy(&rest[..line_end]).into_owned();
    let trimmed = line.trim_end().trim_end_matches('{').trim_end();
    trimmed.to_string()
}

/// reverse-reach count (dependents) — the amp= change-amplification gauge
fn amp_of(g: &Graph, id: usize) -> u32 {
    // BFS over reverse edges from id, excluding id itself
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        for (from, to, _) in &g.out_edges {
            if *to == cur && !seen.contains(from) {
                seen.insert(*from);
                stack.push(*from);
            }
        }
    }
    seen.len() as u32
}

/// fan-in (in= reuse count)
fn fan_in_of(g: &Graph, id: usize) -> u32 {
    let mut seen = std::collections::HashSet::new();
    for (from, to, _) in &g.out_edges {
        if *to == id {
            seen.insert(*from);
        }
    }
    seen.len() as u32
}

const LEGEND_MID: &str = ": reusable building blocks + quality facts for what you're about to touch (cx=complexity ccx=cognitive in=reuse-count churn=recent-commits amp=change-amplification clone=1(duplicated) tested=1) — prefer composing/reusing these; watch the high-churn/high-amp/cloned ones; bundle=compact: conceptual query, so this map ships one-hop EDGE context, no bodies (bodies=0, reason=compact-route or no_candidates). hops rows are h l=line p=file n=name, and a row's calls child names its callees (c n= l=). hops and calls disclose total=requested shown=printed capped=1 when the BUDGET cut a listing; noedge=N counts ranked symbols with no RESOLVED callee found (never none exists). For a body: expand=p:n pasted off a row; the auto-bodies flag puts the bodies back; tail: file-grain tail, WEAKER evidence than the ranked rows (paths only): every positive-score file NOT among the shown sigs rows — the files of trimmed rows first, best-symbol rank order; rows are t p=file; total=such files, shown=printed, capped=1 when they differ. r= on a ranked row is its 1-based rank in this lens ranking, rows in r= order, p= the file (a gap = a budget-trimmed row) -->";
const CONFIDENCE_NOTE: &str = " [confidence= derives from the ranked head's largest relative score drop (margin_pct=, whole percent, 0 = none; the same gap the adaptive flag cuts at). low = flat ranking: treat the set as a starting point, not an answer]";

pub fn emit(ing: &Ingest, g: &Graph, root: &str, query: &str, r: &ForRanking) -> String {
    let level = if !r.hit_ceiling || (r.positive_hits > 0 && r.positive_hits <= r.kept) {
        "high"
    } else {
        "low"
    };
    let zero_count = r.total_window.saturating_sub(r.positive_hits);

    let mut legend = String::new();
    legend.push_str("<!-- ripwire lens for \"");
    legend.push_str(&escape_xml(query));
    legend.push('"');
    legend.push_str(" [relevance floor: kept ");
    legend.push_str(&r.kept.to_string());
    legend.push_str(" of ");
    legend.push_str(&r.total_window.to_string());
    legend.push_str(" - the other ");
    legend.push_str(&zero_count.to_string());
    legend.push_str(" scored zero on this query, so the bundle shrank instead of padding]");
    legend.push_str(CONFIDENCE_NOTE);
    legend.push_str(LEGEND_MID);
    legend.push_str("<!-- root= is the crawl root; p= below is RELATIVE to it (single-root only; absent => p= is ingest's own path, unchanged). weak=\"1\" est_tokens= prices this bundle in tokens -->");

    // sigs rows: the top `kept` symbols in rank order
    let mut sigs = String::new();
    sigs.push_str("<sigs>");
    let mut rank_no = 0usize;
    let mut shown_syms: Vec<usize> = Vec::new();
    for &id in &r.order {
        if r.scores[id] <= 0.0 {
            continue;
        }
        rank_no += 1;
        if rank_no > r.kept {
            break;
        }
        shown_syms.push(id);
        let s = &ing.symbols[id];
        let mut row = format!(
            "<d l=\"{}\" n=\"{}\" p=\"{}\" cx=\"{}\" ccx=\"0\" in=\"{}\"",
            s.line,
            escape_xml(&s.name),
            escape_xml(&ing.files[s.file_id]),
            s.cx,
            fan_in_of(g, id)
        );
        let amp = amp_of(g, id);
        if amp > 0 {
            row.push_str(&format!(" amp=\"{}\"", amp));
        }
        row.push_str(&format!(" r=\"{}\"", rank_no));
        if rank_no == 1 {
            row.push_str(&format!(
                " next=\"--expand={}:{}\"",
                escape_xml(&ing.files[s.file_id]),
                escape_xml(&s.name)
            ));
        }
        row.push_str(&format!(">{}</d>", escape_xml(&signature(ing, root, id))));
        sigs.push_str(&row);
    }
    sigs.push_str("</sigs>");

    // tail: files of trimmed positive rows — none here (kept == positiveHits)
    let tail = format!("<tail total=\"0\" shown=\"0\" capped=\"0\"></tail>");

    // hops: ranked symbols with a resolved callee
    let mut hops_rows = String::new();
    let mut noedge = 0usize;
    let mut hops_shown = 0usize;
    for &id in &shown_syms {
        let callees: Vec<(usize, u32)> = g
            .out_edges
            .iter()
            .filter(|(from, _, _)| *from == id)
            .map(|(_, to, _)| (*to, ing.symbols[*to].line))
            .collect();
        if callees.is_empty() {
            noedge += 1;
            continue;
        }
        hops_shown += 1;
        let s = &ing.symbols[id];
        let mut h = format!(
            "<h l=\"{}\" p=\"{}\" n=\"{}\"><calls total=\"{}\">",
            s.line,
            escape_xml(&ing.files[s.file_id]),
            escape_xml(&s.name),
            callees.len()
        );
        for (to, l) in &callees {
            h.push_str(&format!(
                "<c n=\"{}\" l=\"{}\"/>",
                escape_xml(&ing.symbols[*to].name),
                l
            ));
        }
        h.push_str("</calls></h>");
        hops_rows.push_str(&h);
    }
    let hops = format!(
        "<hops shown=\"{}\" total=\"{}\" capped=\"0\" noedge=\"{}\">{}</hops>",
        hops_shown,
        shown_syms.len(),
        noedge,
        hops_rows
    );

    // assemble the document, then the est_tokens fixpoint over measured bytes
    let child = format!("{}{}{}", sigs, tail, hops);
    let rate = 2.50;
    let ctx = |est: usize| -> String {
        format!(
            "<ctx task=\"{}\" route=\"{}\" root=\"{}\" confidence=\"{}\" margin_pct=\"{}\" bundle=\"compact\" bodies=\"0\" reason=\"compact-route\" est_tokens=\"{}\">{}",
            escape_xml(query),
            escape_xml(&format!("routed: {}", r.route.reason)),
            escape_xml(root),
            level,
            r.margin_pct,
            est,
            legend
        )
    };
    let mut est = 0usize;
    let mut head = ctx(est);
    for _ in 0..4 {
        let next = ((head.len() + child.len() + 6) as f64 / rate + 0.5) as usize;
        if next == est {
            break;
        }
        est = next;
        head = ctx(est);
    }
    format!("{}{}</ctx>", head, child)
}
