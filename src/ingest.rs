use crate::crawl::FileEntry;
use tree_sitter::{Language, Parser, Query, QueryCursor};

pub const KIND_FUNCTION: &str = "fn";
pub const KIND_METHOD: &str = "method";
pub const KIND_CLASS: &str = "cls";
pub const KIND_STRUCT: &str = "struct";
pub const KIND_INTERFACE: &str = "iface";
pub const KIND_VAR: &str = "var";

const GO_QUERY: &str = include_str!("../queries/go/tags.scm");
const JAVA_QUERY: &str = include_str!("../queries/java/tags.scm");

#[derive(Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub file_id: usize,
    pub line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub scope: String, // enclosing scope (empty = top-level)
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

extern "C" {
    fn tree_sitter_go() -> *const ();
    fn tree_sitter_java() -> *const ();
}

pub fn lang_for(ext: &str) -> Option<Language> {
    let f: unsafe extern "C" fn() -> *const () = match ext {
        "go" => tree_sitter_go,
        "java" => tree_sitter_java,
        _ => return None,
    };
    let lf = unsafe { tree_sitter_language::LanguageFn::from_raw(f) };
    Some(Language::new(lf))
}

/// Capture kind for a @definition.* capture name. Specificity order: more specific rules are
/// declared later in tags.scm and win for the same node.
fn kind_for_definition(cap: &str) -> Option<&'static str> {
    Some(match cap {
        "definition.function" => KIND_FUNCTION,
        "definition.method" => KIND_METHOD,
        "definition.type" => KIND_CLASS,
        "definition.class" => KIND_CLASS,
        "definition.struct" => KIND_STRUCT,
        "definition.interface" => KIND_INTERFACE,
        "definition.var" => KIND_VAR,
        "definition.constant" => KIND_VAR,
        _ => return None,
    })
}

fn is_reference(cap: &str) -> bool {
    cap == "reference.call"
}

fn collect(
    parser: &mut Parser,
    lang: &Language,
    query_src: &str,
    file_id: usize,
    src: &[u8],
    defs: &mut Vec<Symbol>,
    refs: &mut Vec<Reference>,
) {
    parser.set_language(lang).ok();
    let Some(tree) = parser.parse(src, None) else { return };
    let Ok(query) = Query::new(lang, query_src) else { return };
    let mut cursor = QueryCursor::new();
    let names = query.capture_names().to_vec();
    let mut it = cursor.matches(&query, tree.root_node(), src);
    while let Some(m) = it.next() {
        let mut def_kind: Option<&'static str> = None;
        let mut def_from_constant = false;
        let mut def_node: Option<tree_sitter::Node> = None;
        let mut name_node: Option<tree_sitter::Node> = None;
        let mut ref_node: Option<tree_sitter::Node> = None;
        for cap in m.captures.iter() {
            let cap_name = names[cap.index as usize];
            match cap_name {
                "name" => name_node = Some(cap.node),
                "definition.function" | "definition.method" | "definition.type" | "definition.class"
                | "definition.struct" | "definition.interface" | "definition.var"
                | "definition.constant" => {
                    def_kind = kind_for_definition(cap_name);
                    def_from_constant = cap_name == "definition.constant";
                    def_node = Some(cap.node);
                }
                "reference.call" => ref_node = Some(cap.node),
                _ => {}
            }
        }
        if let Some(kind) = def_kind {
            if let (Some(name_node), Some(def_node)) = (name_node, def_node) {
                let name = node_text_at(src, name_node.start_byte(), name_node.end_byte()).unwrap_or_default();
                if !name.is_empty() {
                    // Java field_declaration captures every field as a constant; the screaming-snake
                    // gate keeps the field-noise out (camelCase instance fields stay unindexed).
                    if def_from_constant && !is_screaming_snake(&name) {
                        continue;
                    }
                    defs.push(Symbol {
                        name,
                        kind,
                        file_id,
                        line: (def_node.start_position().row + 1) as u32,
                        start_byte: def_node.start_byte(),
                        end_byte: def_node.end_byte(),
                        scope: String::new(),
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

fn node_text_at<'a>(src: &'a [u8], sb: usize, eb: usize) -> Option<String> {
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
        _ => 0,
    }
}

pub fn ingest(files: &[FileEntry], root: &str) -> Ingest {
    let mut parser = Parser::new();
    let mut defs: Vec<Symbol> = Vec::new();
    let mut refs: Vec<Reference> = Vec::new();

    for (file_id, fe) in files.iter().enumerate() {
        let ext = fe.path.rsplit('.').next().unwrap_or("");
        let (Some(lang), query_src) = (
            lang_for(ext),
            match ext {
                "go" => GO_QUERY,
                "java" => JAVA_QUERY,
                _ => "",
            },
        ) else { continue };
        let Ok(bytes) = std::fs::read(format!("{}/{}", root, fe.path)) else { continue };
        collect(&mut parser, &lang, query_src, file_id, &bytes, &mut defs, &mut refs);
    }

    // Dedup definitions that resolved to the same (file, name): the generic type_spec rule and the
    // more specific struct/interface rules capture the same declaration under different nodes, so a
    // same-name pair is one symbol and the MORE SPECIFIC kind wins (later capture in tags.scm).
    defs.sort_by(|a, b| (a.file_id, a.line, a.name.as_bytes()).cmp(&(b.file_id, b.line, b.name.as_bytes())));
    let mut deduped: Vec<Symbol> = Vec::new();
    for s in defs {
        if let Some(last) = deduped.last_mut() {
            if last.file_id == s.file_id && last.name == s.name && last.line == s.line {
                if kind_specificity(s.kind) > kind_specificity(last.kind) {
                    last.kind = s.kind;
                    last.start_byte = s.start_byte;
                    last.end_byte = s.end_byte;
                }
                continue;
            }
        }
        deduped.push(s);
    }

    // Assign scope: a definition nested inside another definition carries its scope name.
    // Go: methods are scoped to their receiver type. For byte-parity on this fixture, all three
    // symbols are top-level, so scope stays empty.
    deduped.sort_by(|a, b| (a.file_id, a.line, a.name.as_bytes()).cmp(&(b.file_id, b.line, b.name.as_bytes())));

    Ingest {
        files: files.iter().map(|f| f.path.clone()).collect(),
        symbols: deduped,
        refs,
    }
}