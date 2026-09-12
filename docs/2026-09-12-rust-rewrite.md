# ripwire Rust essence rewrite — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or
> superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Rebuild ripwire's essence — crawl → call graph → PageRank → minified XML over CLI + MCP,
18 verbs, plus new `gain` — in Rust, **byte-identical to the C++ oracle** on Go and Java corpora,
then delete the C++.

**Architecture:** A Rust crate in `rust/` compiled against the vendored `tree-sitter-go` +
`tree-sitter-java` C sources (via `build.rs` + `cc`), driving the `tree-sitter` crate's C-API FFI.
The surviving C++ tree at HEAD (binary already at `./build/ripwire`) is the diff oracle throughout.
Milestones: skeleton+harness → ingest → graph → rank → serialize+CLI → MCP → gain → promotion.

**Tech Stack:** Rust (stable), cargo, `tree-sitter` crate (bundled C core), `cc` (build dep),
vendored grammar C sources, bash gate suite (`test/pargates.py`), Python generators.

## Global Constraints

- **Byte-identity:** Rust stdout must equal C++ stdout byte-for-byte on the surviving surface.
  Oracle: `./build/ripwire` (plain dev build, no NDEBUG). Determinism contract unchanged.
- **Honesty contract:** floors `counts_floor="1"`; a zero is "none found"; every truncation
  disclosed; every ambiguity labelled. Byte-parity with the oracle enforces this.
- **FP discipline:** strict IEEE in the rank kernel; block-1024 canonical reductions; no FMA
  contraction; match C++ float formatting specifiers exactly. Diff harness is the arbiter.
- **Language scope:** Go + Java only. Grammars from `third_party/deps/{go,java}/src/parser.c` +
  `src/tree_sitter/`. Queries from `queries/{go,java}/tags.scm` verbatim.
- **Gates:** `test/regression.sh` is the authoritative list; new gates listed there same-commit
  (manifestcheck). Black-box gates run any binary via `test/pargates.py <root> <binary>`.
- **Verification after every milestone:**
  - Oracle diff: `diff <(./build/ripwire <fixture> <flags>) <(./rust/target/release/ripwire <fixture> <flags>)`
  - `python3 test/pargates.py . ./target/release/ripwire -j 6` (from milestone 5 on)
  - `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`

---

## Milestone 0 — Skeleton + oracle harness

### Task 0.1: Crate skeleton

**Files:**
- Create: `rust/Cargo.toml`, `rust/build.rs`, `rust/src/main.rs`, `rust/src/lib.rs`,
  `rust/src/infra/mod.rs`
- Create: `scripts/diff-oracle.sh` (the harness)

- [ ] **Step 1:** `rust/Cargo.toml`:
```toml
[package]
name = "ripwire"
version = "0.7.0"
edition = "2021"

[dependencies]
tree-sitter = "0.23"

[build-dependencies]
cc = "1"
```
  (Pin the tree-sitter crate to the version whose bundled C core is ABI-compatible with the vendored
  grammar ABI 14 — resolve during build: if `ts_language_abi_version` mismatches, bump/downgrade.)
- [ ] **Step 2:** `rust/build.rs` — compile the two vendored grammars:
```rust
fn main() {
    for (name, dir) in [("go", "third_party/deps/go"), ("java", "third_party/deps/java")] {
        cc::Build::new()
            .file(format!("{dir}/src/parser.c"))
            .include(format!("{dir}/src"))
            .warnings(false)
            .compile(&format!("tree_sitter_{name}"));
    }
    println!("cargo:rerun-if-changed=third_party/deps/go");
    println!("cargo:rerun-if-changed=third_party/deps/java");
}
```
- [ ] **Step 3:** `scripts/diff-oracle.sh` — run both binaries on a fixture with the same flags,
  diff stdout, exit non-zero on mismatch, print a `cmp`-style first-difference line.
  Usage: `scripts/diff-oracle.sh <fixture-dir> [flags...]`.
- [ ] **Step 4:** `cargo build` — confirm tree-sitter core + both grammars link.
- [ ] **Step 5:** Fixtures: create `test/fixtures/golang/` (a small Go package: a function, a method,
  a struct, a caller) and `test/fixtures/java/` (a class with methods + one cross-class call).

### Task 0.2: Oracle baseline on fixtures

- [ ] **Step 1:** Run the C++ oracle on both fixtures, record outputs as the target:
  `./build/ripwire test/fixtures/golang > /tmp/ref-go.xml`
  `./build/ripwire test/fixtures/java > /tmp/ref-java.xml`
- [ ] **Step 2:** Record which gates currently pass on the C++ binary for the surviving surface
  (the background `pargates` run's final green list). This is the Rust target's must-pass set.
- [ ] **Step 3:** Commit milestone 0.
  `git add -A && git commit -m "chore(rust): crate skeleton + oracle diff harness + go/java fixtures"`

---

## Milestone 1 — Ingest (Go + Java)

Reimplement the ingest stage to produce the identical symbol table as the C++ ingest: same node IDs
(sorted crawl order), same names, kinds, paths, line spans, canonical ids, per-symbol metrics.

### Task 1.1: Crawl

**Files:**
- Create: `rust/src/ingest/crawl.rs`

Replicate `src/ingest_crawl.h` + `src/ingest.h`: sorted candidate path list (lexicographic by byte
BEFORE id assignment), denylist (`kCrawlSkipDirs[]`: `.git`, `.claude`, `.hg`, `.svn`,
`node_modules`, `vendor`, `third_party`, `.cache`, `build`, `dist`, `out`, `target`, `.venv`,
`venv`, `__pycache__`, `.idea`, `.vscode`, `asan`, `build_prof`, `CMakeFiles`, `captures`,
`cmake-build-*`, any dir containing `CMakeCache.txt`), gitignore consult via `git ls-files --others
--ignored --exclude-standard --directory`, size ceiling (`--max-file-size`, default 4 MB), binary
sniff (NUL in first 4 KB), `.go`/`.java` extension filter, no symlink following.

- [ ] **Step 1:** Implement crawl producing a sorted `Vec<PathBuf>` + per-file metadata
  (size, mtime) matching the oracle's `files=` header attribute list for the fixtures.
- [ ] **Step 2:** Oracle-check the file list: add a debug flag `--dump-files` (Rust-only, not a
  shipping surface) printing `<f p="…"/>` rows; diff against the oracle map's file rows for the
  two fixtures.

### Task 1.2: Parse + extract

**Files:**
- Create: `rust/src/ingest/parse.rs`, `rust/src/ingest/extract.rs`

Parse each file with the correct grammar; run `queries/go/tags.scm` / `queries/java/tags.scm` via the
`tree-sitter` crate `QueryCursor`; collect `@definition.*` captures as definitions, `@reference.*`
as references, name from `@name`. Map capture names → `NodeRole` per `src/model.h` / the language
tables. Build the per-language `extension → grammar` table (only go/java).

- [ ] **Step 1:** Implement parse+extract. Debug `--dump-symbols` emitting
  `name | kind | path | line | canonical-id | definition` rows in the oracle's canonical-id order.
- [ ] **Step 2:** Oracle-check symbol+reference tables on the fixtures; iterate until identical
  (names, kinds, spans, canonical ids, capture roles). Watch for: how the C++ mangles canonical ids,
  struct/class-member scope rules, Go package-vs-file scoping, Java nested-class canonical ids.

### Task 1.3: Ingest metrics + cache-adjacent plumbing

- [ ] **Step 1:** Wire the ingest output into the shared model struct (per `src/ingest_model.h`):
  dedup, symbol assignment, span attribution. `Symbol` fields: kind, name, canonical id, path, line
  span, language, complexity, fan-in, tested, churn.
- [ ] **Step 2:** Commit milestone 1.
  `git add -A && git commit -m "feat(rust): ingest — crawl, go/java parse+extract, symbol table at oracle parity"`

---

## Milestone 2 — Graph

Reimplement `src/graph.h` + the resolution ladder exactly: same-file → same-dir → unique global →
drop; k>1 candidates split into k edges weighted 1/k; edge weight = mean-per-reference-confidence ×
√(ref count) capped 8; confidence deboost for ≥16-definition names and leading-underscore; in-edge
CSR (`rowOffsets`, `colIndices`, `values`) + `wOutDeg` + dangling mask; self-loops dropped; dedup
structural (one entry per (src,dst) pair, nref counted).

**Files:**
- Create: `rust/src/graph/mod.rs`, `rust/src/graph/resolve.rs`, `rust/src/graph/csr.rs`

- [ ] **Step 1:** Implement the CSR builder and resolution ladder.
- [ ] **Step 2:** Debug `--dump-graph` emitting `src dst weight` triples; oracle-check against the
  fixture graphs until identical. Ambiguity counts (`amb="K"`) must match.
- [ ] **Step 3:** Commit milestone 2.
  `git add -A && git commit -m "feat(rust): call graph — resolution ladder + CSR at oracle parity"`

---

## Milestone 3 — Rank

Reimplement `src/pagerank.cpp`: personalized PageRank, power iteration, in-edge CSR gather,
dangling-mass redistribution through the teleport vector, fixed contiguous 1024-block reductions in
canonical order, damping α=0.85, τ=1e-6, maxIter=100, β=0.7. Strict IEEE; no FMA contraction
(`-C target-feature=-fma` for the rank module or per-operation guards); `double` rank vector.
Convergence disclosure (`pr_iters`, `pr_converged` absence semantics) unchanged.

**Files:**
- Create: `rust/src/rank.rs`

- [ ] **Step 1:** Implement the kernel with exact reduction order.
- [ ] **Step 2:** Debug `--dump-ranks` emitting per-node rank with the oracle's exact float
  formatting; diff against the oracle until byte-identical (this is the hardest FP milestone —
  iterate on summation order and FMA guards until the diff is zero).
- [ ] **Step 3:** Commit milestone 3.
  `git add -A && git commit -m "feat(rust): PageRank at oracle parity"`

---

## Milestone 4 — Serialize + CLI

Reimplement `src/serialize.h` + the flagless map + `src/cli.h` arg parser. Minified XML, streamed
(64 KB stack-backed buffer → stdout), sort (rank DESC, nodeId ASC), `escapeXml` on every attribute
and text node (escape `&` first). Attribute names, order, and legend text transcribed verbatim from
the C++ emit sites. Hand-rolled table-driven arg parser; flagless run = core map; every flag
additive. First target: `ripwire <dir>` byte-identical on both fixtures.

**Files:**
- Create: `rust/src/cli.rs`, `rust/src/serialize.rs`, `rust/src/emit.rs` (stdout writer)

- [ ] **Step 1:** Implement the serializer + flagless map; oracle-diff until byte-identical.
- [ ] **Step 2:** Implement the flag table + parse loop; wire `--legend`, `--top-k`, `--max-tokens`,
  `--rank-by`, `--no-cache` (cache itself is a later milestone; `--no-cache` default-ok first).
- [ ] **Step 3:** Run the surviving black-box gates against the Rust binary for the first time:
  `python3 test/pargates.py . ./target/release/ripwire -j 6` — triage failures (expect many; fix the
  ones covering the map).
- [ ] **Step 4:** Commit milestone 4.
  `git add -A && git commit -m "feat(rust): minified XML map + CLI at oracle parity"`

---

## Milestone 5 — The 18 verbs

For each verb, transcribe the C++ emitter (format strings, legends, caps, disclosures) and oracle-
diff its output on a fixture:
`analyze`, `find_symbol`/`find_referencing_symbols` (`--callees`/`--callers`/`--at`), `grep`
(`--grep` + `--pattern`/`--regex`/`--path`), `for`/`explore` (`--for`, `--pack-task`, routing,
`--no-route`, `--token-budget`), `lego`, `fetch_body` (`--expand`), `batch`, `path_between`,
`connect`, `from_trace` (`--from-trace`), `impact`, `uses`, `whereis`, `owners`, `exemplar`,
`quality_delta` (`--quality-delta`), `edit_check` (`--edit-check`).

**Files:**
- Create: `rust/src/verbs/` (one module per verb family)
- Create: `rust/src/retrieval.rs` (BM25 name-exact / subtoken+body lanes, query-shape router,
  mention anchoring, path-tier multiplier — the `--for` retrieval stack)

- [ ] **Step 1:** Shared infrastructure: paging caps, token estimation (`est_tokens`), disclosure
  vocabulary (`counts_floor`, `capped=`, `amb=`, `route=`), legend rendering.
- [ ] **Step 2:** Implement verbs one at a time; oracle-diff each on a fixture until byte-identical.
- [ ] **Step 3:** Retrieval stack (`--for`/`--explore`) — the subtoken splitter, BM25 with per-query
  IDF, and the confidence-gated router must match the oracle's `route=`/`confidence=`/`est_tokens=`.
- [ ] **Step 4:** Run the full surviving gate set against the Rust binary; drive to green.
- [ ] **Step 5:** Commit milestone 5.
  `git add -A && git commit -m "feat(rust): 18 verbs at oracle parity, surviving gates green"`

---

## Milestone 6 — MCP server

Reimplement `src/mcp*.h`: JSON-RPC over stdio, `initialize`, `tools/list` (18 verbs + `pack_task`
alias + `gain`), `tools/call`, field tables (`kMcpVerbFields` equivalents), refusal schemas,
text/JSON twin builders. MCP responses must match the C++ server byte-for-byte for the surviving
verbs (the `mcpclidiffcheck` CLI/MCP twin parity applies).

**Files:**
- Create: `rust/src/mcp/mod.rs`, `rust/src/mcp/tools.rs`, `rust/src/mcp/verbs.rs`

- [ ] **Step 1:** Implement the server + tools/list; oracle-check tools/list against the C++ MCP
  output (strip session-varying fields like ids; diff schemas byte-for-byte).
- [ ] **Step 2:** Implement the 18 verb twins; diff against the oracle's MCP responses.
- [ ] **Step 3:** Run surviving MCP gates (`mcpverbscheck`, `mcpcontractcheck`, `mcpclidiffcheck`
  equivalents) green.
- [ ] **Step 4:** Commit milestone 6.
  `git add -A && git commit -m "feat(rust): MCP server at oracle parity"`

---

## Milestone 7 — `gain`

Implement per the spec: ledger (XDG path, `--gain-log`/`--no-gain-log`, env `RIPWIRE_GAIN_LOG`),
auto-log on the 18 verbs, naive-tokens from the answer's distinct-file byte sum ÷ 4, `naive_ms` =
naive_tokens ÷ rate × 1000 (default 1000 tok/s, `RIPWIRE_GAIN_RATE`), `gain` report (totals +
per-day/week/repo/verb, disclosure line, "No runs recorded yet"), `--since/--until/--repo/--verb/
--json`. New `test/gaincheck.sh` (round-trip, determinism, disclosure, absent-ledger, `model="none"`
marking) listed in `test/regression.sh` same-commit.

**Files:**
- Create: `rust/src/gain.rs`, `test/gaincheck.sh`

- [ ] **Step 1:** Implement ledger + logging + report; `cargo test` for the report logic.
- [ ] **Step 2:** Write `test/gaincheck.sh`; add to `test/regression.sh`; run green.
- [ ] **Step 3:** Commit milestone 7.
  `git add -A && git commit -m "feat(rust): gain — tokens+time savings ledger and report"`

---

## Milestone 8 — Promotion

- [ ] **Step 1:** Delete the C++ tree: `git rm -r src cmake CMakeLists.txt lsan_suppressions.txt`
  and the vendored grammars not needed (keep `third_party/deps/{go,java}`), C++-specific gates
  (`portablebuildcheck`, formatcheck, cpp*check, non-go/java language gates), `docs/ARCHITECTURE.md`
  rewritten for the Rust pipeline, `docs/COMMANDS.md` regenerated from the Rust binary.
- [ ] **Step 2:** Move `rust/` to the root: `Cargo.toml` + `src/` at top level; delete `rust/` dir.
- [ ] **Step 3:** Rewrite `README.md` lean (approved copy), trim `docs/`, sync `skills/`,
  `prompts/`, `hooks/`, `CLAUDE.md`, `AGENTS.md` to the surviving surface (18 verbs, Go+Java).
- [ ] **Step 4:** Full gate run: `python3 test/pargates.py . ./target/release/ripwire -j 6` green;
  determinism; `cargo test`; clippy -D warnings; fmt --check. Add a cargo-based CI gate equivalent.
- [ ] **Step 5:** Squash the milestone history into the fork's final history if desired.

---

## Follow-on (optional)
Add more grammars (same pattern: vendor C source + queries row), re-add removed lenses as opt-in Rust
crates, or publish. Not part of this plan.