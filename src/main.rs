pub mod crawl;
pub mod graph;
pub mod ingest;
pub mod rank;
pub mod serialize;

use std::path::Path;


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = if args.len() > 1 { args[1].as_str() } else { "." };
    let root_path = Path::new(root);

    let files = crawl::crawl(root_path);
    let ing = ingest::ingest(&files, root);
    let g = graph::build(&ing);
    let run = rank::pagerank(&g);
    let doc = serialize::serialize(&ing, &g, &run, root);
    print!("{}", doc);
}