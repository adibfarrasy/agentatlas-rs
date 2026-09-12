# ripwire essence distillation — design (REVISED: Rust rewrite)

Date: 2026-09-12 · Status: approved-by-user (pivot to Rust, 2026-09-12) ·
Destination: personal fork (no upstream merge)

## Goal

`ripwire` is a ~168K-line C++23 codebase whose core — a deterministic codebase index for coding
agents (crawl → call graph → PageRank → minified XML, served over CLI and MCP) — is the thing worth
keeping. The surrounding repo is marketing, docs walls, and a long tail of lenses/verbs the user
does not want.

**Revised decision (2026-09-12):** rewrite the essence in **Rust**, **byte-identical to the C++
output** on the surviving surface. The C++ code is **deleted** once parity is proven. Test-suite
parity is preserved: the black-box gate suite transfers to the Rust binary mostly unchanged.

## Language scope (2026-09-12)
The user's use cases are **mostly Go and Java**. The rewrite's ingest/parse surface ships **Go +
Java only**: the vendored C sources of `tree-sitter-go` and `tree-sitter-java`, and
`queries/go/tags.scm` + `queries/java/tags.scm`. The grammar table stays extensible — adding a
language later is "vendor the grammar source + a queries row," exactly the C++ design — but no other
language is built or gated in this fork. Oracle parity is proven on Go and Java corpora; gates for
other languages are dropped.

## The parity contract

- The Rust binary must reproduce the C++ binary's stdout **byte-for-byte** on the surviving surface.
- During the rewrite the C++ tree (HEAD + `./build/ripwire`, already built) is the **diff oracle**:
  every Rust milestone is diffed against it on pinned fixtures.
- At the end the C++ source (`src/`, `CMakeLists.txt`, `cmake/`, vendored C grammar sources,
  C++-specific gates) is deleted; the Rust crate becomes the tree.
- Determinism contract carries over unchanged: same tree → same bytes, every run.

### What makes byte-parity hard, and how it is met
1. **Parse trees** — must be identical to the C++ build's. Met by compiling the **same vendored
   C grammar sources** from `third_party/` via `build.rs` (cc crate) and loading them through the
   `tree-sitter` crate's C-API FFI. The `queries/*/tags.scm` files are reused verbatim. Grammar
   version drift is impossible by construction.
2. **Floating point** — PageRank must emit identical doubles. The C++ pagerank TU is compiled
   strict-IEEE, block-1024 canonical reduction, no reassociation. Rust is strict-IEEE by default;
   the rank kernel must (a) mirror the exact reduction order, (b) disable FMA contraction
   (`-C target-feature=-fma` or per-op guards), (c) match the C++ formatting specifiers exactly.
3. **Float formatting** — every `printf`/format specifier in the C++ emit path is transcribed to an
   exact Rust equivalent (format string, precision, scientific-notation form, sign handling). Rust's
   `format!` differs from printf in edge cases; the diff harness is the arbiter.
4. **Iteration order** — sorted crawl order before IDs; sort by (rank DESC, nodeId ASC); fixed
   reductions. All re-implemented identically.

## Surviving surface (unchanged from the C++ plan)

- **18 verbs:** `analyze`, `find_symbol`, `find_referencing_symbols`, `grep`, `for`/`explore`
  (`pack_task` alias), `lego`, `fetch_body`, `batch`, `path_between`, `connect`, `from_trace`,
  `impact`, `uses`, `whereis`, `owners`, `exemplar`, `quality_delta`, `edit_check`.
- **CLI + MCP front doors**, one renderer shared, byte-identical twins.
- **Determinism + honesty contracts.**
- **Meta-agent layer** (`skills/`, `prompts/`, `hooks/`, `.mcp.json`, `.codex-plugin/`), re-synced to
  the surviving surface after the swap.
- **New:** `ripwire gain` — per-user cross-repo tokens+time savings ledger (spec below). Rust-only,
  no C++ oracle needed.

## Cut inventory (what the Rust rewrite does NOT build, and what gets deleted)
- **Lenses** (never built in Rust, C++ deleted): `clones/cloneidiom`, `commentcoherence`,
  `contextratio`, `dmm`, `ensemble`, `fieldaffinity`, `nonlocalstate`, `qualitypanel`, `readability`,
  `namingconsistency`, `naminglens`, `cachelint/lintrules/lintcatalog`, `--dead-code`, `--hotspots`,
  `--communities`.
- **11 cut verbs** (never built in Rust, C++ deleted): `slice`, `stray_content`, `flags`,
  `doc_drift`, `cochange`, `situational_awareness`, `memory_recall`, `quality_baseline`,
  `replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol`.
- **Marketing:** `present/`, `paper/`, `docs/assets/`; README rewritten lean (parodies, badge walls,
  eval-claims walls removed).
- **Docs:** cut `EVALS`, `LINEAGE`, `SUBSTITUTION_METER`, `CODEX_ORCHESTRATION`, `CACHELINT`,
  `FIELDAFFINITY`, `LOCALS_INDEXING`, `OPTREMARKS`, `TUNING`, `gatecount_build.py`,
  `lineage-paper-dates.tsv`. Keep `COMMANDS` (regenerated), `ARCHITECTURE` (rewritten for Rust),
  `METHODOLOGY`, `LIMITS`, `captures/`, generators (rewritten for Rust if needed).

## `ripwire gain` (new, Rust-only)
- Ledger: `$XDG_DATA_HOME/ripwire/gain.jsonl` (fallback `~/.local/share/ripwire/gain.jsonl`,
  `~/.ripwire/gain.jsonl`). Env `RIPWIRE_GAIN_LOG`; flags `--gain-log=FILE`, `--no-gain-log`.
- Auto-logged on the 18 retrieval verbs: `{ts, repo, verb, spent_tokens, spent_ms, naive_tokens,
  naive_ms, model}`. `naive_tokens` = Σ bytes of distinct files the answer names ÷ 4 (computed, not
  guessed). `naive_ms` = naive_tokens ÷ read-rate × 1000 (default 1000 tok/s, env
  `RIPWIRE_GAIN_RATE`). Non-file-set verbs (`analyze`, `batch`, `quality_delta`, `edit_check`) log
  spent-only, `model="none"`.
- `ripwire gain` reports totals + per-day/week + per-repo + per-verb, deterministic, disclosure line.
  `--since/--until/--repo/--verb/--json`. CLI-only, no MCP twin.
- Gate `test/gaincheck.sh` + `test/regression.sh` entry (manifestcheck rule).

## Milestones

1. **Skeleton + oracle harness** — Rust crate in `rust/`, Cargo build, a `diff-oracle` script that
   runs the C++ binary and the Rust binary on a fixture and diffs stdout. First fixture: a small
   two-file corpus.
2. **Ingest (Go + Java)** — crawl (sorted, denylist, gitignore, size/binary skips), tree-sitter
   parse via the two vendored grammars, symbol/reference extraction via `queries/{go,java}/tags.scm`.
   Oracle check: symbol table diff (IDs, names, kinds, spans).
3. **Graph** — resolution ladder, CSR, edge weights. Oracle check: identical edges + weights.
4. **Rank** — PageRank, strict IEEE, block-1024 reductions. Oracle check: identical rank vector.
5. **Serialize + CLI** — minified XML, flagless map, then the 18 verbs. Oracle check: byte-identical
   stdout per verb.
6. **MCP** — JSON-RPC server, 18 verbs, twin parity. Oracle check: identical responses.
7. **gain** — ledger + report + gate.
8. **Promotion** — delete C++ (src/, CMake, cmake/, vendored grammars), promote `rust/` to root,
   rewrite README/docs/skills for the surviving surface, run the full transferred gate suite green,
   ASan-equivalent (cargo test + Miri/valgrind where sensible), squash history.

## Sequencing / verification
- Oracle diff after every milestone on pinned fixtures; gate suite runs against the Rust binary from
  milestone 5 onward. `test/pargates.py` is language-agnostic (runs any binary) — the surviving
  gates run unchanged.
- C++-specific build gates (`portablebuildcheck`, formatcheck) are replaced by Rust equivalents
  (cargo fmt/clippy/test). Behavior gates transfer as-is.
- Commits per milestone; final squash for the fork.

## Non-goals
- No LSP integration, no new languages, no new retrieval features.
- No MCP twin for `gain`.
- The C++ cut plan (phases 1–4 of the previous design) is **cancelled** — the essence is built
  directly in Rust; the C++ long tail is deleted wholesale at promotion.