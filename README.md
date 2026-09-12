# agentatlas.rs

A deterministic codebase index for coding agents. Crawl → call graph → PageRank → one minified XML
answer: ranked symbols, callers, blast radius, tests to run. Offline, one binary, zero runtime
dependencies, every uncertainty labelled.

**agentatlas.rs is a from-scratch Rust rewrite of [ripwire](https://github.com/redhat-et/ripwire).**
It keeps ripwire's core idea and its honesty contracts, re-implemented in a language a solo
maintainer can actually read. See [CREDITS.md](CREDITS.md).

## Why

ripwire's idea is good; its ~168K lines of C++23 and its gate bureaucracy are not something a single
maintainer who does not write C++ day-to-day can keep. This repo rebuilds the idea in Rust, small
enough to hold in your head — and verifies every byte it emits against the ripwire binary that
inspired it. The golden outputs in `test/golden/` are committed ripwire output, so the parity claim
is checkable, not atmospheric.

## Status

**Done.** The core surface is byte-identical to ripwire: the ranked map and the retrieval/change-
safety verbs (`--grep`, `--callers`/`--callees`, `--uses`, `--for`, `--at`, `--impact`, `--expand`,
`--from-trace`) — 12/12 parity checks green. Plus `gain`, the token/time savings ledger. Go + Java.

## Build

```bash
cargo build --release
./target/release/agentatlas test/fixtures/golang
```

## Test

```bash
cargo test
test/parity.sh            # byte-identical vs the committed ripwire golden outputs
test/gaincheck.sh         # the gain ledger gate
```

## Track your savings

Every retrieval run appends one row to a JSONL ledger at `$XDG_DATA_HOME/agentatlas/gain.jsonl`
(override `--gain-log=FILE`, disable `--no-gain-log`). `gain` rolls it up — tokens spent vs the
naive read those files would have cost, plus time:

```bash
agentatlas --for="find the point distance"   # logs a row
agentatlas gain                              # totals, per-repo, per-verb, disclosure
```

The naive side is computed, not guessed: the byte size of the distinct files your answer named, ÷4.
Verbs with no file-set (`analyze`) log spent-only, disclosed as unmodeled.

## License

Apache-2.0 — see [LICENSE](LICENSE). The design, algorithms, honesty contracts, and `queries/`
files are derived from ripwire (redhat-et, Apache-2.0); the vendored grammars are
[tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) and
[tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) (MIT). Full accounting in
[CREDITS.md](CREDITS.md).