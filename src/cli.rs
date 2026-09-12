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
    pub top_k: Option<usize>,
    pub legend: Option<String>,
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
            top_k: None,
            legend: None,
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
            } else if a == "--no-cache" || a == "--mcp" {
                // accepted, no effect (no cache yet; MCP dropped)
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
        } else {
            Verb::Map
        }
    }
}