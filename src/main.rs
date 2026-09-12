pub mod cli;
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
        cli::Verb::Callers(_) | cli::Verb::Callees(_) => String::new(),
        cli::Verb::For(_) => String::new(),
        cli::Verb::Uses(_) => String::new(),
    };
    print!("{}", doc);
}