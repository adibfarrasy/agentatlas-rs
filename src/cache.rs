// cache.rs — per-repo index cache. Caches the Ingest (files/symbols/refs) + a content manifest so
// repeat runs skip the read+parse when the code is provably unchanged. A speed optimization only:
// every failure silently falls back to a fresh run.
use crate::ingest::{Ingest, Reference, Symbol};
use std::path::Path;

/// FNV-1a 64-bit. Cache invalidation only, not security. Deterministic and stable within a version.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub struct ManifestFile {
    pub path: String,
    pub len: u64,
    pub mtime_nanos: u64,
    pub hash: u64,
}

pub struct Manifest {
    pub files: Vec<ManifestFile>,
}

/// Escape a free-string field for the line format. Fields are space-separated on one line, so any
/// whitespace plus our control chars get backslash-escaped. Empty strings become `\e` so the field
/// keeps a visible token (empty is common: Symbol.scope is "" for top-level symbols).
fn esc(s: &str) -> String {
    if s.is_empty() {
        return "\\e".to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ' ' => out.push_str("\\s"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '|' => out.push_str("\\p"),
            c => out.push(c),
        }
    }
    out
}

fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('\\') => out.push('\\'),
                Some('s') => out.push(' '),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('p') => out.push('|'),
                Some('e') => out.push_str(""),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Line format, one record per line:
///   A1 <version> <esc(root)> <nfiles> <nsymbols> <nrefs>
///   F <esc(relpath)> <len> <mtime_nanos> <hash-hex>
///   S <esc(name)> <kind> <file_id> <line> <end_line> <start_byte> <end_byte> <esc(scope)> <cx> <body_start> <body_end>
///   R <esc(name)> <file_id> <start_byte> <end_byte>
/// F lines are in crawl (byte-sorted path) order; symbol/ref file_id index that order.
pub fn save(path: &str, manifest: &Manifest, ing: &Ingest, root: &str) {
    let p = Path::new(path);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut out = String::new();
    out.push_str(&format!(
        "A1 {} {} {} {} {}\n",
        env!("CARGO_PKG_VERSION"),
        esc(root),
        manifest.files.len(),
        ing.symbols.len(),
        ing.refs.len()
    ));
    for f in &manifest.files {
        out.push_str(&format!(
            "F {} {} {} {:016x}\n",
            esc(&f.path),
            f.len,
            f.mtime_nanos,
            f.hash
        ));
    }
    for s in &ing.symbols {
        out.push_str(&format!(
            "S {} {} {} {} {} {} {} {} {} {} {}\n",
            esc(&s.name),
            s.kind,
            s.file_id,
            s.line,
            s.end_line,
            s.start_byte,
            s.end_byte,
            esc(&s.scope),
            s.cx,
            s.body_start,
            s.body_end
        ));
    }
    for r in &ing.refs {
        out.push_str(&format!(
            "R {} {} {} {}\n",
            esc(&r.name),
            r.file_id,
            r.start_byte,
            r.end_byte
        ));
    }
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, out.as_bytes()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn load(path: &str) -> Option<(Manifest, Ingest)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head = lines.next()?;
    let mut h = head.split_whitespace();
    if h.next()? != "A1" {
        return None;
    }
    if h.next()? != env!("CARGO_PKG_VERSION") {
        return None;
    }
    let _root = unesc(h.next()?);
    let nfiles: usize = h.next()?.parse().ok()?;
    let nsymbols: usize = h.next()?.parse().ok()?;
    let nrefs: usize = h.next()?.parse().ok()?;

    let mut files: Vec<String> = Vec::with_capacity(nfiles);
    let mut manifest_files: Vec<ManifestFile> = Vec::with_capacity(nfiles);
    for _ in 0..nfiles {
        let f = lines.next()?;
        let mut t = f.split_whitespace();
        if t.next()? != "F" {
            return None;
        }
        let path = unesc(t.next()?);
        let len: u64 = t.next()?.parse().ok()?;
        let mtime_nanos: u64 = t.next()?.parse().ok()?;
        let hash = u64::from_str_radix(t.next()?, 16).ok()?;
        files.push(path.clone());
        manifest_files.push(ManifestFile {
            path,
            len,
            mtime_nanos,
            hash,
        });
    }

    let mut symbols: Vec<Symbol> = Vec::with_capacity(nsymbols);
    for _ in 0..nsymbols {
        let s = lines.next()?;
        let mut t = s.split_whitespace();
        if t.next()? != "S" {
            return None;
        }
        let name = unesc(t.next()?);
        // Symbol.kind is &'static str: map the stored word back to the KIND_* constant.
        let kind = match t.next()? {
            "fn" => crate::ingest::KIND_FUNCTION,
            "method" => crate::ingest::KIND_METHOD,
            "cls" => crate::ingest::KIND_CLASS,
            "struct" => crate::ingest::KIND_STRUCT,
            "iface" => crate::ingest::KIND_INTERFACE,
            "var" => crate::ingest::KIND_VAR,
            "sec" => crate::ingest::KIND_SECTION,
            "macro" => crate::ingest::KIND_MACRO,
            _ => return None,
        };
        let file_id: usize = t.next()?.parse().ok()?;
        let line: u32 = t.next()?.parse().ok()?;
        let end_line: u32 = t.next()?.parse().ok()?;
        let start_byte: usize = t.next()?.parse().ok()?;
        let end_byte: usize = t.next()?.parse().ok()?;
        let scope = unesc(t.next()?);
        let cx: u32 = t.next()?.parse().ok()?;
        let body_start: usize = t.next()?.parse().ok()?;
        let body_end: usize = t.next()?.parse().ok()?;
        symbols.push(Symbol {
            name,
            kind,
            file_id,
            line,
            end_line,
            start_byte,
            end_byte,
            scope,
            cx,
            body_start,
            body_end,
        });
    }

    let mut refs: Vec<Reference> = Vec::with_capacity(nrefs);
    for _ in 0..nrefs {
        let r = lines.next()?;
        let mut t = r.split_whitespace();
        if t.next()? != "R" {
            return None;
        }
        let name = unesc(t.next()?);
        let file_id: usize = t.next()?.parse().ok()?;
        let start_byte: usize = t.next()?.parse().ok()?;
        let end_byte: usize = t.next()?.parse().ok()?;
        refs.push(Reference {
            name,
            file_id,
            start_byte,
            end_byte,
        });
    }

    Some((
        Manifest {
            files: manifest_files,
        },
        Ingest {
            files,
            symbols,
            refs,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{Ingest, Reference, Symbol};
    use std::path::PathBuf;

    #[test]
    fn fnv1a_known_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn esc_unesc_round_trip() {
        let cases = [
            "",
            "plain_ident",
            "with space",
            "pipe|and\\backslash",
            "line\nbreak\tand space",
        ];
        for c in cases {
            assert_eq!(unesc(&esc(c)), c);
        }
    }

    #[test]
    fn save_load_round_trip() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("agentatlas-cache-test-{}.dat", std::process::id()));
        let mf = ManifestFile {
            path: "main.go".to_string(),
            len: 42,
            mtime_nanos: 7,
            hash: 0xdead_beef,
        };
        let manifest = Manifest { files: vec![mf] };
        let ing = Ingest {
            files: vec!["main.go".to_string()],
            symbols: vec![Symbol {
                name: "main".to_string(),
                kind: "fn",
                file_id: 0,
                line: 3,
                end_line: 5,
                start_byte: 10,
                end_byte: 100,
                scope: String::new(),
                cx: 2,
                body_start: 10,
                body_end: 100,
            }],
            refs: vec![Reference {
                name: "Point".to_string(),
                file_id: 0,
                start_byte: 20,
                end_byte: 25,
            }],
        };
        save(tmp.to_str().unwrap(), &manifest, &ing, "/repo");
        let (m2, i2) = load(tmp.to_str().unwrap()).expect("load");
        assert_eq!(m2.files.len(), 1);
        assert_eq!(m2.files[0].path, "main.go");
        assert_eq!(m2.files[0].hash, 0xdead_beef);
        assert_eq!(i2.files, ing.files);
        assert_eq!(i2.symbols.len(), 1);
        assert_eq!(i2.symbols[0].name, "main");
        assert_eq!(i2.symbols[0].kind, "fn");
        assert_eq!(i2.symbols[0].cx, 2);
        assert_eq!(i2.refs.len(), 1);
        assert_eq!(i2.refs[0].name, "Point");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_rejects_bad_version() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!("agentatlas-cache-test-bad-{}.dat", std::process::id()));
        std::fs::write(&tmp, "A1 99.99.99 /repo 0 0 0\n").unwrap();
        assert!(load(tmp.to_str().unwrap()).is_none());
        let _ = std::fs::remove_file(&tmp);
    }
}