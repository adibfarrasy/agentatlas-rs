use std::path::{Path, PathBuf};

pub struct FileEntry {
    pub path: String, // root-relative
    pub bytes: usize,
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
    let mut out: Vec<FileEntry> = Vec::new();
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
                let Ok(bytes) = std::fs::read(&p) else {
                    continue;
                };
                if looks_binary(&bytes) {
                    continue;
                }
                let rel = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned();
                out.push(FileEntry {
                    path: rel,
                    bytes: bytes.len(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    out
}
