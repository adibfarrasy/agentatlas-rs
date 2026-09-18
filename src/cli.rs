// Hand-rolled argument parser (ripwire-style, additive flags). First positional arg = crawl root.
pub struct Config {
    pub root: String,
    pub grep: Option<String>,
    pub callers: Option<String>,
    pub callees: Option<String>,
    pub for_query: Option<String>,
    pub uses: Option<String>,
    pub at: Option<String>,
    pub expand: Option<String>,
    pub impact: Option<String>,
    pub from_trace: Option<String>,
    pub gain: bool,
    pub no_gain_log: bool,
    pub gain_log: Option<String>,
    pub no_cache: bool,
    pub top_k: Option<usize>,
    pub legend: Option<String>,
    pub depth: Option<u32>,
}

impl Config {
    pub fn parse(args: &[String]) -> Config {
        let mut c = Config {
            root: ".".to_string(),
            grep: None,
            callers: None,
            callees: None,
            for_query: None,
            uses: None,
            at: None,
            expand: None,
            impact: None,
            from_trace: None,
            gain: false,
            no_gain_log: false,
            gain_log: None,
            no_cache: false,
            top_k: None,
            legend: None,
            depth: None,
        };
        for a in args {
            if let Some(v) = a.strip_prefix("--grep=") {
                c.grep = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--callers=") {
                c.callers = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--callees=") {
                c.callees = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--for=") {
                c.for_query = Some(v.to_string());
            } else if a == "--gain" || a == "gain" {
                c.gain = true;
            } else if a == "--no-gain-log" {
                c.no_gain_log = true;
            } else if let Some(v) = a.strip_prefix("--gain-log=") {
                c.gain_log = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--from-trace=") {
                c.from_trace = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--at=") {
                c.at = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--expand=") {
                c.expand = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--impact=") {
                c.impact = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--uses=") {
                c.uses = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--top-k=") {
                c.top_k = v.parse().ok();
            } else if let Some(v) = a.strip_prefix("--legend=") {
                c.legend = Some(v.to_string());
            } else if let Some(v) = a.strip_prefix("--depth=") {
                c.depth = v.parse().ok();
            } else if a == "--no-cache" {
                c.no_cache = true;
            } else if a == "--mcp" {
                // accepted, no effect (MCP dropped)
            } else if !a.starts_with("--") {
                c.root = a.clone();
            }
        }
        c
    }
}

pub enum Verb {
    Map,
    Grep(String),
    Callers(String),
    Callees(String),
    For(String),
    Uses(String),
    At(String),
    Expand(String),
    Impact(String),
    FromTrace(String),
    Gain,
}

impl Config {
    pub fn verb(&self) -> Verb {
        if let Some(p) = &self.grep {
            Verb::Grep(p.clone())
        } else if let Some(s) = &self.callers {
            Verb::Callers(s.clone())
        } else if let Some(s) = &self.callees {
            Verb::Callees(s.clone())
        } else if let Some(q) = &self.for_query {
            Verb::For(q.clone())
        } else if let Some(s) = &self.uses {
            Verb::Uses(s.clone())
        } else if let Some(s) = &self.at {
            Verb::At(s.clone())
        } else if let Some(s) = &self.expand {
            Verb::Expand(s.clone())
        } else if let Some(s) = &self.impact {
            Verb::Impact(s.clone())
        } else if let Some(s) = &self.from_trace {
            Verb::FromTrace(s.clone())
        } else if self.gain {
            Verb::Gain
        } else {
            Verb::Map
        }
    }
}

pub const HELP: &str = r#"agentatlas — a deterministic codebase map for coding agents.
25 languages (Go, Java, C/C++, Python, Rust, TS/JS, Ruby, PHP, and more).
Offline. One binary. Same output bytes, every run.

USAGE
  agentatlas <dir> [VERB] [options]

  No verb → the ranked map: every symbol in the tree, ranked by
  importance (PageRank), with call edges. Minified XML.

VERBS
  --grep=TERM        find TERM in the code, grouped by file
  --callers=SYM      who calls SYM
  --callees=SYM      what SYM calls
  --uses=SYM         where SYM is used
  --impact=SYM       what a change to SYM would reach
  --at=FILE:LINE     the definition chain enclosing a line
  --expand=SYM       a symbol's full body (whole file if smaller)
  --for="TASK"       ranked symbols relevant to a task — the main one
  --from-trace=FILE  map a stack trace onto the code
  gain               report the token/time savings from your runs
  --help             this text

  SYM is a bare name, or FILE:NAME.

OPTIONS
  --top-k=N          cap the ranked rows
  --depth=N          cap --impact traversal to N hops (default: unbounded)
  --gain-log=FILE    where the savings ledger lives (env RIPWIRE_GAIN_LOG)
  --no-gain-log      don't log this run
  --no-cache         don't reuse the per-repo index cache
  --legend=compact   shorter legend on the map

OUTPUT
  Deterministic minified XML. Every uncertainty is labelled: floors say
  counts_floor="1", a zero means "none found" never "none exists", and every
  truncation is disclosed. est_tokens= prices the answer in tokens.
"#;
