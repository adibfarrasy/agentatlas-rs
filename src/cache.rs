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

use crate::crawl::StatEntry;
use std::collections::HashMap;

pub enum Source {
    Hit { ing: Ingest },
    Rebuilt { ing: Ingest },
}

fn cache_dir() -> String {
    if let Ok(d) = std::env::var("AGENTATLAS_CACHE_DIR") {
        if !d.is_empty() {
            return d;
        }
    }
    format!("{}/cache", crate::gain::xdg_data_home())
}

fn cache_path_in(cdir: &str, root: &str) -> Option<String> {
    let canon = Path::new(root).canonicalize().ok()?;
    let key = fnv1a(canon.to_string_lossy().as_bytes());
    Some(format!("{cdir}/{:016x}.dat", key))
}

/// Stat-only walk of the repo, as (relpath, len, mtime_nanos).
fn walk(root: &Path) -> Vec<(String, u64, u64)> {
    crate::crawl::enumerate(root)
        .into_iter()
        .map(|e| (e.path, e.len, e.mtime_nanos))
        .collect()
}

struct Diff {
    /// stat entries that are new or whose (len, mtime) differ from the manifest
    changed: Vec<StatEntry>,
    /// manifest paths absent from the current tree (deleted files)
    missing: Vec<String>,
}

fn diff(manifest: &Manifest, stats: &[(String, u64, u64)]) -> Diff {
    let mut present: HashMap<&str, ()> = HashMap::new();
    let mut changed: Vec<StatEntry> = Vec::new();
    for (path, len, mtime) in stats {
        present.insert(path.as_str(), ());
        let m = manifest.files.iter().find(|f| f.path == *path);
        match m {
            Some(mf) if mf.len == *len && mf.mtime_nanos == *mtime => {}
            _ => changed.push(StatEntry {
                path: path.clone(),
                len: *len,
                mtime_nanos: *mtime,
            }),
        }
    }
    let missing: Vec<String> = manifest
        .files
        .iter()
        .filter(|f| !present.contains_key(f.path.as_str()))
        .map(|f| f.path.clone())
        .collect();
    Diff { changed, missing }
}

pub fn get(root: &str, no_cache: bool) -> Source {
    get_with(root, no_cache, &cache_dir())
}

fn get_with(root: &str, no_cache: bool, cdir: &str) -> Source {
    if no_cache {
        return rebuild(root, None);
    }
    let Some(cpath) = cache_path_in(cdir, root) else {
        return rebuild(root, None);
    };
    let Some((mut manifest, ing)) = load(&cpath) else {
        return rebuild(root, Some(&cpath));
    };
    let stats = walk(Path::new(root));
    let d = diff(&manifest, &stats);
    if d.changed.is_empty() && d.missing.is_empty() {
        return Source::Hit { ing };
    }
    if !d.missing.is_empty() {
        return rebuild(root, Some(&cpath));
    }
    // Hash-verify: a stat change that leaves the content identical (touch / git checkout) is mtime
    // noise → stay a hit and refresh the manifest. Any content change, or a new file, → rebuild.
    let mut real_change = false;
    let mut refreshed: Vec<&StatEntry> = Vec::new();
    for e in &d.changed {
        let Ok(bytes) = std::fs::read(Path::new(root).join(&e.path)) else {
            real_change = true;
            break;
        };
        let h = fnv1a(&bytes);
        let m = manifest.files.iter().find(|f| f.path == e.path);
        match m {
            Some(mf) if mf.hash == h => refreshed.push(e),
            _ => {
                real_change = true;
                break;
            }
        }
    }
    if real_change {
        return rebuild(root, Some(&cpath));
    }
    for e in refreshed {
        if let Some(mf) = manifest.files.iter_mut().find(|f| f.path == e.path) {
            mf.mtime_nanos = e.mtime_nanos;
        }
    }
    save(&cpath, &manifest, &ing, root);
    Source::Hit { ing }
}

fn rebuild(root: &str, cpath: Option<&str>) -> Source {
    let files = crate::crawl::crawl(Path::new(root));
    let ing = crate::ingest::ingest(&files, root);
    if let Some(cp) = cpath {
        save(cp, &build_manifest(&files, root), &ing, root);
    }
    Source::Rebuilt { ing }
}

fn build_manifest(files: &[crate::crawl::FileEntry], root: &str) -> Manifest {
    let mut mf = Vec::with_capacity(files.len());
    for f in files {
        let full = format!("{root}/{}", f.path);
        let meta = std::fs::metadata(&full).ok();
        let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let bytes = std::fs::read(&full).unwrap_or_default();
        mf.push(ManifestFile {
            path: f.path.clone(),
            len,
            mtime_nanos: mtime,
            hash: fnv1a(&bytes),
        });
    }
    Manifest { files: mf }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{Ingest, Reference, Symbol};

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

    fn to_stats(entries: &[StatEntry]) -> Vec<(String, u64, u64)> {
        entries
            .iter()
            .map(|e| (e.path.clone(), e.len, e.mtime_nanos))
            .collect()
    }

    fn write_tree(root: &Path) -> Vec<StatEntry> {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(root.join("a.go"), b"package a\nfunc A(){}\n").unwrap();
        std::fs::write(root.join("b.go"), b"package a\nfunc B(){}\n").unwrap();
        crate::crawl::enumerate(root)
    }

    fn hash_path(root: &Path, rel: &str) -> u64 {
        fnv1a(&std::fs::read(root.join(rel)).unwrap())
    }

    fn manifest_of(root: &Path, stats: &[StatEntry]) -> Manifest {
        let files = stats
            .iter()
            .map(|e| ManifestFile {
                path: e.path.clone(),
                len: e.len,
                mtime_nanos: e.mtime_nanos,
                hash: hash_path(root, &e.path),
            })
            .collect();
        Manifest { files }
    }

    #[test]
    fn diff_detects_no_change() {
        let root = std::env::temp_dir().join("agentatlas-diff-none");
        let stats = write_tree(&root);
        let manifest = manifest_of(&root, &stats);
        let d = diff(&manifest, &to_stats(&stats));
        assert!(d.changed.is_empty());
        assert!(d.missing.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn diff_detects_mtime_and_new() {
        let root = std::env::temp_dir().join("agentatlas-diff-mt");
        let stats = write_tree(&root);
        let manifest = manifest_of(&root, &stats);
        std::fs::write(root.join("c.go"), b"package a\n").unwrap();
        let mut stats2 = crate::crawl::enumerate(&root);
        let a = stats2.iter_mut().find(|e| e.path == "a.go").unwrap();
        a.mtime_nanos += 1;
        let d = diff(&manifest, &to_stats(&stats2));
        assert!(d.changed.iter().any(|e| e.path == "a.go"));
        assert!(d.changed.iter().any(|e| e.path == "c.go"));
        assert!(d.missing.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn diff_detects_deleted() {
        let root = std::env::temp_dir().join("agentatlas-diff-del");
        let stats = write_tree(&root);
        let manifest = manifest_of(&root, &stats);
        std::fs::remove_file(root.join("b.go")).unwrap();
        let stats2 = crate::crawl::enumerate(&root);
        let d = diff(&manifest, &to_stats(&stats2));
        assert!(d.missing.iter().any(|p| p == "b.go"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn get_with_hit_then_rebuilt() {
        let root = std::env::temp_dir().join("agentatlas-get-root");
        let cdir = std::env::temp_dir().join("agentatlas-get-cache");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cdir);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.go"), b"package a\nfunc A(){}\n").unwrap();
        let r = root.to_str().unwrap();
        let c = cdir.to_str().unwrap();

        let first = get_with(r, false, c);
        assert!(matches!(first, Source::Rebuilt { .. }));

        let second = get_with(r, false, c);
        let third = get_with(r, false, c);
        match (&second, &third) {
            (Source::Hit { ing: a }, Source::Hit { ing: b }) => {
                assert_eq!(a.files, b.files);
                assert_eq!(a.symbols.len(), b.symbols.len());
            }
            _ => panic!("expected two hits"),
        }

        std::fs::write(root.join("a.go"), b"package a\nfunc A(){}\nfunc B(){}\n").unwrap();
        let changed = get_with(r, false, c);
        assert!(matches!(changed, Source::Rebuilt { .. }));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cdir);
    }

    #[test]
    fn no_cache_never_hits() {
        let root = std::env::temp_dir().join("agentatlas-nocache-root");
        let cdir = std::env::temp_dir().join("agentatlas-nocache-cache");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cdir);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.go"), b"package a\n").unwrap();
        let r = root.to_str().unwrap();
        let c = cdir.to_str().unwrap();
        let a = get_with(r, true, c);
        let b = get_with(r, true, c);
        assert!(matches!(a, Source::Rebuilt { .. }));
        assert!(matches!(b, Source::Rebuilt { .. }));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cdir);
    }
}