pub mod cli;
pub mod extraverbs;
pub mod forverb;
pub mod crawl;
pub mod graph;
pub mod ingest;
pub mod legends;
pub mod metrics;
pub mod rank;
pub mod serialize;
pub mod verbs;

use std::path::Path;


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cfg = cli::Config::parse(&args[1..]);
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
        cli::Verb::Impact(s) => {
            let run = rank::pagerank(&g);
            extraverbs::impact(&ing, &g, &root, &s, run.iterations)
        }
    };
    print!("{}", doc);
}