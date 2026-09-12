pub mod cli;
pub mod crawl;
pub mod extraverbs;
pub mod forverb;
pub mod gain;
pub mod graph;
pub mod ingest;
pub mod legends;
pub mod metrics;
pub mod rank;
pub mod serialize;
pub mod verbs;

use std::path::Path;

fn main() {
    let run_started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print!("{}", cli::HELP);
        return;
    }
    let cfg = cli::Config::parse(&args[1..]);

    // gain reads the ledger — it does not crawl or rank, so it runs before the pipeline.
    if matches!(cfg.verb(), cli::Verb::Gain) {
        let ledger = cfg.gain_log.clone().unwrap_or_else(gain::ledger_path);
        let rate = gain::read_rate();
        match gain::report(&ledger) {
            Some(r) => print!("{}", gain::render(&r, rate, &ledger)),
            None => print!(
                "gain: nothing recorded yet at {ledger}\nRun any retrieval verb first — e.g. `agentatlas <dir> --for=\"your task\"` — and it logs a row here.\n"
            ),
        }
        return;
    }

    let root = cfg.root.clone();
    let root_path = Path::new(&root);

    let files = crawl::crawl(root_path);
    let ing = ingest::ingest(&files, &root);
    let g = graph::build(&ing);

    let doc = match cfg.verb() {
        cli::Verb::Map => {
            let run = rank::pagerank(&g);
            serialize::serialize(&ing, &g, &run, &root)
        }
        cli::Verb::Grep(p) => verbs::grep(&ing, &g, &root, &p),
        cli::Verb::Callers(s) => verbs::callers(&ing, &g, &root, &s),
        cli::Verb::Callees(s) => verbs::callees(&ing, &g, &root, &s),
        cli::Verb::For(q) => {
            let r = forverb::rank(&ing, &g, &root, &q);
            forverb::emit(&ing, &g, &root, &q, &r)
        }
        cli::Verb::Uses(s) => verbs::uses(&ing, &g, &root, &s),
        cli::Verb::At(s) => extraverbs::at(&ing, &root, &s),
        cli::Verb::Expand(s) => extraverbs::expand(&ing, &g, &root, &s),
        cli::Verb::FromTrace(s) => extraverbs::from_trace(&ing, &g, &root, &s),
        cli::Verb::Impact(s) => {
            let run = rank::pagerank(&g);
            extraverbs::impact(&ing, &g, &root, &s, run.iterations)
        }
        cli::Verb::Gain => unreachable!(), // short-circuited above
    };
    if !cfg.no_gain_log {
        let ledger = cfg.gain_log.clone().unwrap_or_else(gain::ledger_path);
        let verb_name = match cfg.verb() {
            cli::Verb::Grep(_) => "grep",
            cli::Verb::Callers(_) => "callers",
            cli::Verb::Callees(_) => "callees",
            cli::Verb::For(_) => "for",
            cli::Verb::Uses(_) => "uses",
            cli::Verb::At(_) => "at",
            cli::Verb::Expand(_) => "expand",
            cli::Verb::Impact(_) => "impact",
            cli::Verb::FromTrace(_) => "from_trace",
            cli::Verb::Map => "analyze",
            cli::Verb::Gain => "gain",
        };
        let spent_tokens = doc
            .split("est_tokens=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let naive = gain::naive_for_doc(&doc, &ing.files, &root);
        let (nt, nm, model) = match naive {
            Some(n) => {
                let nm = (n as f64 / gain::read_rate() * 1000.0 + 0.5) as u64;
                (Some(n), Some(nm), "file-set")
            }
            None => (None, None, "none"),
        };
        let row = gain::GainRow {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            repo: root.clone(),
            verb: verb_name.to_string(),
            spent_tokens,
            spent_ms: run_started.elapsed().as_millis() as u64,
            naive_tokens: nt,
            naive_ms: nm,
            model: model.to_string(),
        };
        gain::log_run(&ledger, &row);
    }
    print!("{}", doc);
}
