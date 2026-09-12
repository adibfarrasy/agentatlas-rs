# agentatlas.rs

A deterministic codebase index for coding agents. Crawl → call graph → PageRank → one minified XML
answer: ranked symbols, callers, blast radius, tests to run. Offline, one binary, zero runtime
dependencies, every uncertainty labelled.

**agentatlas.rs is a from-scratch Rust rewrite of [ripwire](https://github.com/redhat-et/ripwire).**
It keeps ripwire's core idea and its honesty contracts, re-implemented in a language a solo
maintainer can actually read. See [CREDITS.md](CREDITS.md).

## Why this exists

ripwire is a genuinely good tool with a real insight — but it is ~168K lines of C++23 wrapped in a
build system, a 400-gate test bureaucracy, and a documentation tower that mostly exists to justify
its own claims. For a single maintainer who does not write C++ day-to-day, that is unreadable. This
project exists to answer one question:

> Can the *idea* — an offline, deterministic, token-priced codebase map for coding agents — be
> rebuilt in Rust, small enough to hold in your head, and kept byte-identical to the original where
> it matters?

The answer this repo is working toward is **yes**. Everything agentatlas emits is verified
byte-for-byte against the ripwire binary that inspired it (the golden outputs in `test/golden/` are
committed ripwire output, attributed, so the parity claim is checkable rather than atmospheric).

## Status

Foundation proven: vendored `tree-sitter-go` and `tree-sitter-java` compile and parse (ABI 14),
oracle outputs captured, pipeline under construction. Go + Java only — the languages that matter
here. See `docs/2026-09-12-rust-rewrite.md` for the milestone plan.

## Build

```bash
cargo build --release
./target/release/agentatlas test/fixtures/golang
```

## Test

```bash
cargo test
test/parity.sh            # byte-identical vs the committed ripwire golden outputs
```

## License

Apache-2.0 — see [LICENSE](LICENSE). The design, algorithms, honesty contracts, and `queries/`
files are derived from ripwire (redhat-et, Apache-2.0); the vendored grammars are
[tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) and
[tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) (MIT). Full accounting in
[CREDITS.md](CREDITS.md).