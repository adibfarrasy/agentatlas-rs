use std::path::{Path, PathBuf};

pub struct FileEntry {
    pub path: String, // root-relative
    pub bytes: usize,
}

pub struct StatEntry {
    pub path: String,
    pub len: u64,
    pub mtime_nanos: u64,
}

fn mtime_nanos(p: &Path) -> u64 {
    std::fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".claude",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "third_party",
    ".cache",
    "build",
    "dist",
    "out",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
    "asan",
    "build_prof",
    "CMakeFiles",
    "captures",
];
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const BINARY_SNIFF_CAP: usize = 4096;

pub fn is_source(ext: &str) -> bool {
    crate::ingest::grammar_for_ext(ext).is_some()
}

fn is_skipped_dir(name: &str) -> bool {
    if SKIP_DIRS.contains(&name) {
        return true;
    }
    if name.len() > 12 && name.starts_with("cmake-build-") {
        return true;
    }
    // plugin/tool build-output dirs (grule's .grule-plugins-tmp holds compiled .so + generated .go)
    if name.contains("plugins-tmp") {
        return true;
    }
    name.len() > 5 && name.ends_with(".dSYM")
}

fn looks_binary(bytes: &[u8]) -> bool {
    let n = bytes.len().min(BINARY_SNIFF_CAP);
    bytes[..n].contains(&0)
}

/// Deterministic crawl: collect all candidate source files under `root`, sorted by byte order,
/// root-relative paths.
pub fn crawl(root: &Path) -> Vec<FileEntry> {
    read_filter(root, &enumerate(root))
}

/// Stat-only walk: same skip/ext/size rules and byte-order sort as `crawl`, but reads nothing.
pub fn enumerate(root: &Path) -> Vec<StatEntry> {
    let mut out: Vec<StatEntry> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if p.is_dir() {
                if is_skipped_dir(&name) {
                    continue;
                }
                stack.push(p);
            } else if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !is_source(ext) {
                    continue;
                }
                let Ok(meta) = std::fs::metadata(&p) else {
                    continue;
                };
                if meta.len() > MAX_FILE_BYTES as u64 {
                    continue;
                }
                let rel = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned();
                out.push(StatEntry {
                    path: rel,
                    len: meta.len(),
                    mtime_nanos: mtime_nanos(&p),
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    out
}

/// Read + filter a stat list: full bytes, binary-sniff, byte lengths. Preserves input order.
pub fn read_filter(root: &Path, entries: &[StatEntry]) -> Vec<FileEntry> {
    let mut out: Vec<FileEntry> = Vec::new();
    for e in entries {
        let full = root.join(&e.path);
        let Ok(bytes) = std::fs::read(&full) else {
            continue;
        };
        if looks_binary(&bytes) {
            continue;
        }
        out.push(FileEntry {
            path: e.path.clone(),
            bytes: bytes.len(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/fixtures/golang")
    }

    #[test]
    fn enumerate_matches_previous_crawl_paths() {
        let root = fixture();
        let stats = enumerate(&root);
        assert!(!stats.is_empty());
        for s in &stats {
            assert!(s.len > 0);
        }
        for s in &stats {
            let full = root.join(&s.path);
            assert!(full.is_file(), "{} not a file", s.path);
        }
    }

    #[test]
    fn read_filter_drops_binary() {
        let dir = std::env::temp_dir().join("agentatlas-crawl-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.go"), b"package x\n").unwrap();
        std::fs::write(dir.join("bin.go"), b"package x\x00binary").unwrap();
        let stats = enumerate(&dir);
        assert_eq!(stats.len(), 2, "both files stat'd before sniff");
        let files = read_filter(&dir, &stats);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "ok.go");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crawl_is_read_filter_of_enumerate() {
        let root = fixture();
        let a = crawl(&root);
        let b = read_filter(&root, &enumerate(&root));
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.path, y.path);
            assert_eq!(x.bytes, y.bytes);
        }
    }
}
