// gain — the tokens+time savings ledger. Every retrieval run appends one JSONL row; `gain`
// reports cumulative savings. Naive-equivalent is COMPUTED, not guessed: the byte size of the
// distinct files the answer named, ÷4 (the repo's own bytes/4 token convention).
use std::io::Write;

pub struct GainRow {
    pub ts: u64, // unix seconds
    pub repo: String,
    pub verb: String,
    pub spent_tokens: u64,
    pub spent_ms: u64,
    pub naive_tokens: Option<u64>,
    pub naive_ms: Option<u64>,
    pub model: String, // "file-set" | "none"
}

fn xdg_data_home() -> String {
    if let Ok(d) = std::env::var("XDG_DATA_HOME") {
        if !d.is_empty() {
            return format!("{}/agentatlas", d.trim_end_matches('/'));
        }
    }
    if let Ok(h) = std::env::var("HOME") {
        return format!("{}/.local/share/agentatlas", h);
    }
    ".agentatlas".to_string()
}

pub fn ledger_path() -> String {
    if let Ok(p) = std::env::var("RIPWIRE_GAIN_LOG") {
        if !p.is_empty() {
            return p;
        }
    }
    format!("{}/gain.jsonl", xdg_data_home())
}

pub fn read_rate() -> f64 {
    std::env::var("RIPWIRE_GAIN_RATE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|r| *r > 0.0)
        .unwrap_or(1000.0)
}

/// naive_tokens for a document: sum of the byte sizes of the distinct files its `p=` attributes
/// name, ÷4. Computed from the doc + the crawl's known file sizes.
pub fn naive_for_doc(doc: &str, files: &[String], root: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut seen: Vec<&str> = Vec::new();
    let mut offset = 0usize;
    while let Some(rel) = doc[offset..].find(" p=\"") {
        let start = offset + rel + 4;
        let Some(end) = doc[start..].find('"') else {
            break;
        };
        let path = &doc[start..start + end];
        if !seen.contains(&path) && files.iter().any(|f| f == path) {
            seen.push(path);
            if let Ok(meta) = std::fs::metadata(format!("{root}/{path}")) {
                total += meta.len();
            }
        }
        offset = start + end + 1;
    }
    if seen.is_empty() {
        return None;
    }
    Some((total as f64 / 4.0 + 0.5) as u64)
}

/// Append one row to the ledger (atomic single-line append).
pub fn log_run(ledger: &str, row: &GainRow) {
    let path = std::path::Path::new(ledger);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let naive_tokens = row
        .naive_tokens
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string());
    let naive_ms = row
        .naive_ms
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string());
    let line = format!(
        "{{\"ts\":{},\"repo\":\"{}\",\"verb\":\"{}\",\"spent_tokens\":{},\"spent_ms\":{},\"naive_tokens\":{},\"naive_ms\":{},\"model\":\"{}\"}}\n",
        row.ts,
        json_escape(&row.repo),
        json_escape(&row.verb),
        row.spent_tokens,
        row.spent_ms,
        naive_tokens,
        naive_ms,
        row.model
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Shorten a large number to a readable form: 43.3K, 1.2M, or the raw number.
fn short(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    let abs = n.unsigned_abs();
    if abs >= 1_000_000 {
        format!("{sign}{:.1}M", abs as f64 / 1_000_000.0)
    } else if abs >= 1_000 {
        format!("{sign}{:.1}K", abs as f64 / 1_000.0)
    } else {
        format!("{sign}{abs}")
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Hand-parse one JSONL row (no JSON dependency — the schema is fixed and simple).
fn parse_row(line: &str) -> Option<GainRow> {
    let mut r = GainRow {
        ts: 0,
        repo: String::new(),
        verb: String::new(),
        spent_tokens: 0,
        spent_ms: 0,
        naive_tokens: None,
        naive_ms: None,
        model: String::new(),
    };
    for field in line
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
    {
        let Some((k, v)) = field.split_once(':') else {
            continue;
        };
        match k.trim() {
            "\"ts\"" => r.ts = v.trim().parse().ok()?,
            "\"repo\"" => r.repo = v.trim().trim_matches('"').to_string(),
            "\"verb\"" => r.verb = v.trim().trim_matches('"').to_string(),
            "\"spent_tokens\"" => r.spent_tokens = v.trim().parse().ok()?,
            "\"spent_ms\"" => r.spent_ms = v.trim().parse().ok()?,
            "\"naive_tokens\"" => {
                r.naive_tokens = if v.trim() == "null" {
                    None
                } else {
                    Some(v.trim().parse().ok()?)
                }
            }
            "\"naive_ms\"" => {
                r.naive_ms = if v.trim() == "null" {
                    None
                } else {
                    Some(v.trim().parse().ok()?)
                }
            }
            "\"model\"" => r.model = v.trim().trim_matches('"').to_string(),
            _ => {}
        }
    }
    Some(r)
}

pub struct GainReport {
    pub runs: usize,
    pub spent_tokens: u64,
    pub naive_tokens: u64,
    pub saved_tokens: i64,
    pub saved_pct: i64,
    pub saved_ms_pct: i64,
    pub spent_ms: u64,
    pub naive_ms: u64,
    pub saved_ms: i64,
    pub unmodeled: usize,
    pub per_repo: Vec<(String, u64, u64)>,
    pub per_verb: Vec<(String, u64, u64)>,
}

/// Read the ledger and roll up.
pub fn report(ledger: &str) -> Option<GainReport> {
    let text = std::fs::read_to_string(ledger).ok()?;
    let mut runs = 0usize;
    let mut spent_tokens = 0u64;
    let mut naive_tokens = 0u64;
    let mut spent_ms = 0u64;
    let mut naive_ms = 0u64;
    let mut unmodeled = 0usize;
    let mut repo_totals: std::collections::BTreeMap<String, (u64, u64)> =
        std::collections::BTreeMap::new();
    let mut verb_totals: std::collections::BTreeMap<String, (u64, u64)> =
        std::collections::BTreeMap::new();
    let mut bad_lines = 0usize;
    for line in text.lines() {
        let Some(r) = parse_row(line) else {
            bad_lines += 1;
            continue;
        };
        let st = r.spent_tokens;
        let sm = r.spent_ms;
        let nt = r.naive_tokens;
        let nm = r.naive_ms;
        runs += 1;
        spent_tokens += st;
        spent_ms += sm;
        if let Some(n) = nt {
            naive_tokens += n;
        }
        if let Some(n) = nm {
            naive_ms += n;
        }
        if r.model != "file-set" {
            unmodeled += 1;
        }
        let e = repo_totals.entry(r.repo).or_insert((0, 0));
        e.0 += 1;
        e.1 += st;
        let e = verb_totals.entry(r.verb).or_insert((0, 0));
        e.0 += 1;
        e.1 += st;
    }
    let _ = bad_lines;
    let saved = naive_tokens as i64 - spent_tokens as i64;
    let saved_pct = if naive_tokens > 0 && saved > 0 {
        (saved * 100 / naive_tokens as i64).min(100)
    } else {
        0
    };
    let saved_ms = naive_ms as i64 - spent_ms as i64;
    let saved_ms_pct = if naive_ms > 0 && saved_ms > 0 {
        (saved_ms * 100 / naive_ms as i64).min(100)
    } else {
        0
    };
    Some(GainReport {
        runs,
        spent_tokens,
        naive_tokens,
        saved_tokens: saved,
        saved_pct,
        saved_ms_pct,
        spent_ms,
        naive_ms,
        saved_ms,
        unmodeled,
        per_repo: repo_totals
            .into_iter()
            .map(|(k, v)| (k, v.0, v.1))
            .collect(),
        per_verb: verb_totals
            .into_iter()
            .map(|(k, v)| (k, v.0, v.1))
            .collect(),
    })
}

pub fn render(r: &GainReport, _rate: f64, ledger: &str) -> String {
    let mut out = String::new();
    if r.runs == 0 {
        out.push_str(
            "gain: no runs recorded yet — run a retrieval verb (e.g. --for) and it logs here.\n",
        );
        out.push_str(&format!("ledger: {ledger}\n"));
        return out;
    }
    out.push_str(&format!(
        "runs           {}\n\
         spent_tokens   {}\n\
         naive_tokens   {}\n\
         saved_tokens   {} ({}%)\n\
         spent_ms       {}\n\
         naive_ms       {}\n\
         saved_ms       {} ({}%)\n",
        r.runs,
        short(r.spent_tokens as i64),
        short(r.naive_tokens as i64),
        short(r.saved_tokens),
        r.saved_pct,
        short(r.spent_ms as i64),
        short(r.naive_ms as i64),
        short(r.saved_ms),
        r.saved_ms_pct
    ));
    out.push_str("\nper-repo (runs, spent_tokens):\n");
    for (repo, n, st) in &r.per_repo {
        out.push_str(&format!("  {:<32} {:>4}  {}\n", repo, n, short(*st as i64)));
    }
    out.push_str("\nper-verb (runs, spent_tokens):\n");
    for (verb, n, st) in &r.per_verb {
        out.push_str(&format!("  {:<24} {:>4}  {}\n", verb, n, short(*st as i64)));
    }
    if r.saved_tokens < 0 {
        out.push_str("\nnote: savings are negative — the answer cost more than reading those files directly.\n");
        out.push_str("That is expected on small repos/single files (the bundle carries a legend + context).\n");
    }
    out.push_str(&format!("\nledger: {ledger}\n"));
    out
}
