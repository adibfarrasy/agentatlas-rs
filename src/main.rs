use tree_sitter::{Language, Parser};

extern "C" {
    fn tree_sitter_go() -> *const ();
    fn tree_sitter_java() -> *const ();
}

fn probe(name: &str, lang_fn: unsafe extern "C" fn() -> *const (), src: &str) {
    let lang = Language::new(unsafe { tree_sitter_language::LanguageFn::from_raw(lang_fn) });
    println!("{name}: ABI version = {}", lang.version());
    let mut parser = Parser::new();
    match parser.set_language(&lang) {
        Ok(()) => {
            let tree = parser.parse(src, None).expect("parse");
            let root = tree.root_node();
            println!("{name}: ABI ok, root kind = {:?}", root.kind());
        }
        Err(e) => println!("{name}: set_language FAILED: {e}"),
    }
}

fn main() {
    probe("go", tree_sitter_go, "package main\nfunc main() {}\n");
    probe(
        "java",
        tree_sitter_java,
        "class Main { public static void main(String[] a) {} }\n",
    );
}