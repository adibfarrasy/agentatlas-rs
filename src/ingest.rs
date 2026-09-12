use crate::crawl::FileEntry;
use std::collections::HashMap;
use tree_sitter::{Language, Parser, Query, QueryCursor};

pub const KIND_FUNCTION: &str = "fn";
pub const KIND_METHOD: &str = "method";
pub const KIND_CLASS: &str = "cls";
pub const KIND_STRUCT: &str = "struct";
pub const KIND_INTERFACE: &str = "iface";
pub const KIND_VAR: &str = "var";
pub const KIND_SECTION: &str = "sec";
pub const KIND_MACRO: &str = "macro";

// (grammar name, extensions, query file). Missing: markdown (no query — parses but yields no
// symbols), cuda (uses the c grammar + c query).
pub const LANG_TABLE: &[(&str, &[&str], &str)] = &[
    ("go", &["go"], "queries/go/tags.scm"),
    ("java", &["java"], "queries/java/tags.scm"),
    ("c", &["c", "h"], "queries/c/tags.scm"),
    (
        "cpp",
        &["cc", "cpp", "cxx", "hpp", "hh", "hxx", "c++", "h++"],
        "queries/cpp/tags.scm",
    ),
    ("csharp", &["cs"], "queries/csharp/tags.scm"),
    ("cuda", &["cu", "cuh"], "queries/c/tags.scm"),
    ("dart", &["dart"], "queries/dart/tags.scm"),
    ("elixir", &["ex", "exs"], "queries/elixir/tags.scm"),
    ("bash", &["sh", "bash"], "queries/bash/tags.scm"),
    (
        "javascript",
        &["js", "jsx", "mjs", "cjs"],
        "queries/javascript/tags.scm",
    ),
    ("json", &["json"], "queries/json/tags.scm"),
    ("kotlin", &["kt", "kts"], "queries/kotlin/tags.scm"),
    ("lua", &["lua"], "queries/lua/tags.scm"),
    ("objc", &["m", "mm"], "queries/objc/tags.scm"),
    ("php", &["php"], "queries/php/tags.scm"),
    ("python", &["py"], "queries/python/tags.scm"),
    ("ruby", &["rb"], "queries/ruby/tags.scm"),
    ("rust", &["rs"], "queries/rust/tags.scm"),
    ("swift", &["swift"], "queries/swift/tags.scm"),
    ("toml", &["toml"], "queries/toml/tags.scm"),
    ("yaml", &["yml", "yaml"], "queries/yaml/tags.scm"),
    ("typescript", &["ts"], "queries/typescript/tags.scm"),
    ("tsx", &["tsx"], "queries/typescript/tags.scm"),
    ("html", &["html", "htm"], "queries/html/tags.scm"),
];

const GO_QUERY: &str = include_str!("../queries/go/tags.scm");
const JAVA_QUERY: &str = include_str!("../queries/java/tags.scm");
const C_QUERY: &str = include_str!("../queries/c/tags.scm");
const CPP_QUERY: &str = include_str!("../queries/cpp/tags.scm");
const CSHARP_QUERY: &str = include_str!("../queries/csharp/tags.scm");
const DART_QUERY: &str = include_str!("../queries/dart/tags.scm");
const ELIXIR_QUERY: &str = include_str!("../queries/elixir/tags.scm");
const BASH_QUERY: &str = include_str!("../queries/bash/tags.scm");
const JAVASCRIPT_QUERY: &str = include_str!("../queries/javascript/tags.scm");
const JSON_QUERY: &str = include_str!("../queries/json/tags.scm");
const KOTLIN_QUERY: &str = include_str!("../queries/kotlin/tags.scm");
const LUA_QUERY: &str = include_str!("../queries/lua/tags.scm");
const OBJC_QUERY: &str = include_str!("../queries/objc/tags.scm");
const PHP_QUERY: &str = include_str!("../queries/php/tags.scm");
const PYTHON_QUERY: &str = include_str!("../queries/python/tags.scm");
const RUBY_QUERY: &str = include_str!("../queries/ruby/tags.scm");
const RUST_QUERY: &str = include_str!("../queries/rust/tags.scm");
const SWIFT_QUERY: &str = include_str!("../queries/swift/tags.scm");
const TOML_QUERY: &str = include_str!("../queries/toml/tags.scm");
const YAML_QUERY: &str = include_str!("../queries/yaml/tags.scm");
const TYPESCRIPT_QUERY: &str = include_str!("../queries/typescript/tags.scm");
const HTML_QUERY: &str = include_str!("../queries/html/tags.scm");

fn query_for(name: &str) -> Option<&'static str> {
    Some(match name {
        "go" => GO_QUERY,
        "java" => JAVA_QUERY,
        "c" | "cuda" => C_QUERY,
        "cpp" => CPP_QUERY,
        "csharp" => CSHARP_QUERY,
        "dart" => DART_QUERY,
        "elixir" => ELIXIR_QUERY,
        "bash" => BASH_QUERY,
        "javascript" => JAVASCRIPT_QUERY,
        "json" => JSON_QUERY,
        "kotlin" => KOTLIN_QUERY,
        "lua" => LUA_QUERY,
        "objc" => OBJC_QUERY,
        "php" => PHP_QUERY,
        "python" => PYTHON_QUERY,
        "ruby" => RUBY_QUERY,
        "rust" => RUST_QUERY,
        "swift" => SWIFT_QUERY,
        "toml" => TOML_QUERY,
        "yaml" => YAML_QUERY,
        "typescript" | "tsx" => TYPESCRIPT_QUERY,
        "html" => HTML_QUERY,
        _ => return None,
    })
}

/// extension → grammar name (lowercased extension, no dot).
pub fn grammar_for_ext(ext: &str) -> Option<&'static str> {
    LANG_TABLE
        .iter()
        .find(|(_, exts, _)| exts.contains(&ext))
        .map(|(name, _, _)| *name)
}

extern "C" {
    fn tree_sitter_go() -> *const ();
    fn tree_sitter_java() -> *const ();
    fn tree_sitter_c() -> *const ();
    fn tree_sitter_cpp() -> *const ();
    fn tree_sitter_c_sharp() -> *const ();
    fn tree_sitter_cuda() -> *const ();
    fn tree_sitter_dart() -> *const ();
    fn tree_sitter_elixir() -> *const ();
    fn tree_sitter_bash() -> *const ();
    fn tree_sitter_javascript() -> *const ();
    fn tree_sitter_json() -> *const ();
    fn tree_sitter_kotlin() -> *const ();
    fn tree_sitter_lua() -> *const ();
    fn tree_sitter_markdown() -> *const ();
    fn tree_sitter_objc() -> *const ();
    fn tree_sitter_php() -> *const ();
    fn tree_sitter_python() -> *const ();
    fn tree_sitter_ruby() -> *const ();
    fn tree_sitter_rust() -> *const ();
    fn tree_sitter_swift() -> *const ();
    fn tree_sitter_toml() -> *const ();
    fn tree_sitter_yaml() -> *const ();
    fn tree_sitter_typescript() -> *const ();
    fn tree_sitter_tsx() -> *const ();
    fn tree_sitter_html() -> *const ();
}

pub fn lang_for(g: &str) -> Option<Language> {
    let f: unsafe extern "C" fn() -> *const () = match g {
        "go" => tree_sitter_go,
        "java" => tree_sitter_java,
        "c" => tree_sitter_c,
        "cpp" => tree_sitter_cpp,
        "csharp" => tree_sitter_c_sharp,
        "cuda" => tree_sitter_cuda,
        "dart" => tree_sitter_dart,
        "elixir" => tree_sitter_elixir,
        "bash" => tree_sitter_bash,
        "javascript" => tree_sitter_javascript,
        "json" => tree_sitter_json,
        "kotlin" => tree_sitter_kotlin,
        "lua" => tree_sitter_lua,
        "markdown" => tree_sitter_markdown,
        "objc" => tree_sitter_objc,
        "php" => tree_sitter_php,
        "python" => tree_sitter_python,
        "ruby" => tree_sitter_ruby,
        "rust" => tree_sitter_rust,
        "swift" => tree_sitter_swift,
        "toml" => tree_sitter_toml,
        "yaml" => tree_sitter_yaml,
        "typescript" => tree_sitter_typescript,
        "tsx" => tree_sitter_tsx,
        "html" => tree_sitter_html,
        _ => return None,
    };
    let lf = unsafe { tree_sitter_language::LanguageFn::from_raw(f) };
    Some(Language::new(lf))
}

#[derive(Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub file_id: usize,
    pub line: u32,
    pub end_line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub scope: String,     // enclosing scope (empty = top-level)
    pub cx: u32,           // cyclomatic complexity (1 + decision points); fn/method only
    pub body_start: usize, // body span (fn/method only; 0 = none)
    pub body_end: usize,
}

pub struct Reference {
    pub name: String,
    pub file_id: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub struct Ingest {
    pub files: Vec<String>,
    pub symbols: Vec<Symbol>,
    pub refs: Vec<Reference>,
}

/// Capture kind for a @definition.* capture name.
fn kind_for_definition(cap: &str) -> Option<&'static str> {
    Some(match cap {
        "definition.function" => KIND_FUNCTION,
        "definition.method" | "definition.protomethod" => KIND_METHOD,
        "definition.type" | "definition.class" | "definition.module" => KIND_CLASS,
        "definition.struct" => KIND_STRUCT,
        "definition.interface" => KIND_INTERFACE,
        "definition.var"
        | "definition.field"
        | "definition.enummember"
        | "definition.cjsexport" => KIND_VAR,
        "definition.constant" => KIND_VAR,
        "definition.section" | "definition.yamlkey" => KIND_SECTION,
        "definition.macro" | "definition.testmacroblock" => KIND_MACRO,
        _ => return None,
    })
}

fn is_reference(cap: &str) -> bool {
    matches!(cap, "reference.call" | "reference.import")
}

#[allow(clippy::too_many_arguments)]
fn collect(
    parser: &mut Parser,
    lang: &Language,
    query: &Query,
    g: &str,
    file_id: usize,
    src: &[u8],
    defs: &mut Vec<Symbol>,
    refs: &mut Vec<Reference>,
) {
    parser.set_language(lang).ok();
    let Some(tree) = parser.parse(src, None) else {
        return;
    };
    let mut cursor = QueryCursor::new();
    let names = query.capture_names().to_vec();
    use streaming_iterator::StreamingIterator;
    let mut it = cursor.matches(query, tree.root_node(), src);
    while let Some(m) = it.next() {
        let mut def_kind: Option<&'static str> = None;
        let mut def_from_constant = false;
        let mut def_node: Option<tree_sitter::Node> = None;
        let mut name_node: Option<tree_sitter::Node> = None;
        let mut ref_node: Option<tree_sitter::Node> = None;
        for cap in m.captures.iter() {
            let cap_name = names[cap.index as usize];
            if cap_name == "name" {
                name_node = Some(cap.node);
            } else if let Some(k) = kind_for_definition(cap_name) {
                def_kind = Some(k);
                def_from_constant = cap_name == "definition.constant";
                def_node = Some(cap.node);
            } else if is_reference(cap_name) {
                ref_node = Some(cap.node);
            }
        }
        if let Some(kind) = def_kind {
            if let (Some(name_node), Some(def_node)) = (name_node, def_node) {
                let name = node_text_at(src, name_node.start_byte(), name_node.end_byte())
                    .unwrap_or_default();
                if !name.is_empty() {
                    // Java's field_declaration captures every field as a constant; the screaming-snake
                    // gate keeps field noise out (camelCase instance fields stay unindexed). Only
                    // Java: other languages' real constants must not be gated.
                    if def_from_constant && g == "java" && !is_screaming_snake(&name) {
                        continue;
                    }
                    let (bs, be) = (def_node.start_byte(), def_node.end_byte());
                    defs.push(Symbol {
                        name,
                        kind,
                        file_id,
                        line: (def_node.start_position().row + 1) as u32,
                        end_line: (def_node.end_position().row + 1) as u32,
                        start_byte: def_node.start_byte(),
                        end_byte: def_node.end_byte(),
                        scope: String::new(),
                        cx: if kind == KIND_FUNCTION || kind == KIND_METHOD {
                            crate::metrics::complexity_of(def_node)
                        } else {
                            0
                        },
                        body_start: bs,
                        body_end: be,
                    });
                }
            }
        } else if ref_node.is_some() {
            if let Some(name_node) = name_node {
                let (sb, eb) = (name_node.start_byte(), name_node.end_byte());
                refs.push(Reference {
                    name: node_text_at(src, sb, eb).unwrap_or_default(),
                    file_id,
                    start_byte: sb,
                    end_byte: eb,
                });
            }
        }
    }
}

fn node_text_at(src: &[u8], sb: usize, eb: usize) -> Option<String> {
    if sb < eb && eb <= src.len() {
        Some(String::from_utf8_lossy(&src[sb..eb]).into_owned())
    } else {
        None
    }
}

fn is_screaming_snake(name: &str) -> bool {
    let mut has_upper = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            has_upper = true;
        } else if !(c.is_ascii_digit() || c == '_') {
            return false;
        }
    }
    has_upper
}

fn kind_specificity(k: &str) -> u8 {
    match k {
        KIND_FUNCTION => 4,
        KIND_METHOD => 5,
        KIND_CLASS => 3,
        KIND_STRUCT => 4,
        KIND_INTERFACE => 4,
        KIND_VAR => 1,
        KIND_SECTION => 1,
        KIND_MACRO => 2,
        _ => 0,
    }
}

pub fn ingest(files: &[FileEntry], root: &str) -> Ingest {
    // Parallel parse pool: one Parser + compiled Query per worker (tree-sitter parsers are not
    // thread-safe). Files are partitioned into fixed contiguous slices so worker scheduling never
    // reaches the output — the merge is in slice order and the symbol table is re-sorted anyway.
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let chunk = files.len().div_ceil(worker_count).max(1);

    let mut results: Vec<(Vec<Symbol>, Vec<Reference>)> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for w in 0..worker_count {
            let start = w * chunk;
            if start >= files.len() {
                break;
            }
            let end = ((w + 1) * chunk).min(files.len());
            let files_ref = files;
            handles.push(scope.spawn(move || {
                let mut parser = Parser::new();
                let mut langs: HashMap<&'static str, (Language, Query)> = HashMap::new();
                let mut defs: Vec<Symbol> = Vec::new();
                let mut refs: Vec<Reference> = Vec::new();
                for (i, fe) in files_ref[start..end].iter().enumerate() {
                    let file_id = start + i;
                    let ext = fe.path.rsplit('.').next().unwrap_or("");
                    let Some(g) = grammar_for_ext(ext) else {
                        continue;
                    };
                    let Ok(bytes) = std::fs::read(format!("{root}/{}", fe.path)) else {
                        continue;
                    };
                    let Some(query_src) = query_for(g) else {
                        continue;
                    };
                    if !langs.contains_key(g) {
                        let Some(lang) = lang_for(g) else { continue };
                        let Some(q) = Query::new(&lang, query_src).ok() else {
                            continue;
                        };
                        langs.insert(g, (lang, q));
                    }
                    let (lang, query) = &langs[g];
                    collect(
                        &mut parser,
                        lang,
                        query,
                        g,
                        file_id,
                        &bytes,
                        &mut defs,
                        &mut refs,
                    );
                }
                (defs, refs)
            }));
        }
        for h in handles {
            results.push(h.join().expect("parse worker"));
        }
    });

    // Merge in slice order — deterministic by construction.
    let mut defs: Vec<Symbol> = Vec::new();
    let mut refs: Vec<Reference> = Vec::new();
    for (d, r) in results {
        defs.extend(d);
        refs.extend(r);
    }

    // Dedup definitions that resolved to the same (file, name): the generic type_spec rule and the
    // more specific struct/interface rules capture the same declaration under different nodes, so a
    // same-name pair is one symbol and the MORE SPECIFIC kind wins (later capture in tags.scm).
    defs.sort_by(|a, b| {
        (a.file_id, a.line, a.name.as_bytes()).cmp(&(b.file_id, b.line, b.name.as_bytes()))
    });
    let mut deduped: Vec<Symbol> = Vec::new();
    for s in defs {
        if let Some(last) = deduped.last_mut() {
            if last.file_id == s.file_id && last.name == s.name && last.line == s.line {
                if kind_specificity(s.kind) > kind_specificity(last.kind) {
                    last.kind = s.kind;
                    last.start_byte = s.start_byte;
                    last.end_byte = s.end_byte;
                    last.body_start = s.body_start;
                    last.body_end = s.body_end;
                }
                continue;
            }
        }
        deduped.push(s);
    }

    deduped.sort_by(|a, b| {
        (a.file_id, a.line, a.name.as_bytes()).cmp(&(b.file_id, b.line, b.name.as_bytes()))
    });

    Ingest {
        files: files.iter().map(|f| f.path.clone()).collect(),
        symbols: deduped,
        refs,
    }
}
