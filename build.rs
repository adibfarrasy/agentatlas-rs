fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = root;
    for (name, dir) in [("go", "third_party/deps/go"), ("java", "third_party/deps/java")] {
        let dir = root.join(dir);
        cc::Build::new()
            .file(dir.join("src/parser.c"))
            .include(dir.join("src"))
            .warnings(false)
            .compile(&format!("tree_sitter_{name}"));
        println!("cargo:rerun-if-changed={}", dir.join("src").display());
    }
}