use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let deps = root.join("third_party/deps");

    // (grammar object name, source dir relative to deps/, scanner needs C++)
    let grammars: &[(&str, &str, bool)] = &[
        ("go", "go", false),
        ("java", "java", false),
        ("c", "c", false),
        ("cpp", "cpp", false),
        ("csharp", "csharp", false),
        ("cuda", "cuda", false),
        ("dart", "dart", false),
        ("elixir", "elixir", false),
        ("bash", "bash", false),
        ("javascript", "javascript", false),
        ("json", "json", false),
        ("kotlin", "kotlin", false),
        ("lua", "lua", false),
        ("markdown", "markdown", false),
        ("objc", "objc", false),
        ("php", "php/php", false),
        ("python", "python", false),
        ("ruby", "ruby", false),
        ("rust", "rust", false),
        ("swift", "swift", false),
        ("toml", "toml", false),
        ("yaml", "yaml", false),
        ("typescript", "ts_typescript/typescript", false),
        ("tsx", "ts_typescript/tsx", false),
        ("html", "html", false),
    ];

    for (name, dir, cpp_scanner) in grammars {
        let dir = deps.join(dir).join("src");
        let mut b = cc::Build::new();
        b.include(&dir).file(dir.join("parser.c"));
        let scanner = dir.join("scanner.c");
        if scanner.exists() {
            b.file(&scanner);
            if *cpp_scanner {
                b.cpp(true);
            }
        }
        b.warnings(false).compile(&format!("tree_sitter_{name}"));
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
