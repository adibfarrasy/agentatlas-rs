# CREDITS

## agentatlas is a Rust rewrite of ripwire

The core design, the algorithm choices, the honesty contracts, and the output format of agentatlas are
the work of **redhat-et/ripwire** (Apache-2.0), a C++23 codebase-indexing tool. Specifically:

- The **pipeline**: crawl (sorted, deterministic) → tree-sitter extract → call-graph resolution →
  Personalized PageRank → minified XML.
- The **determinism contract**: sorted crawl order before ID assignment; fixed-block canonical FP
  reductions; byte-identical output between runs.
- The **honesty contract**: floors are labelled `counts_floor="1"`; a zero means "none found" never
  "none exists"; every truncation and ambiguity is disclosed (`amb=`, `pr_converged=`,
  `est_tokens=`, `layer=`, …).
- The **output schema**: the minified XML map, the `est_tokens` pricing model, the legend.
- The **`queries/{go,java}/tags.scm`** extraction queries, which ripwire derived from the
  tree-sitter grammars' own tags queries (MIT) and simplified for definition/call capture.

agentatlas re-implements these in Rust from the published behavior and source. It is a rewrite, not a
port: no ripwire code is copied verbatim except the vendored grammar sources (below) and the query
files (MIT). The committed golden outputs in `test/golden/` are ripwire's actual output, recorded
from the reference build, so the byte-parity claim is checkable.

### Why the rewrite

1. **Readability.** ~168K lines of C++23 with a hand-rolled argument parser, a 400-gate bash test
   suite, and a CMake/FetchContent/vendored-grammar build is not a codebase a solo maintainer who
   does not write C++ can keep. Rust's module system, `cargo`, `cargo test`, and memory safety make
   the same design hold-in-your-head-sized.
2. **The long tail.** Most of ripwire's surface (quality lenses, naming lenses, cache-lint rules,
   eleven long-tail verbs) is not the essence. agentatlas rebuilds only the retrieval/change-safety core.
3. **Language scope.** The use cases that first motivated this are Go and Java. agentatlas now
   ships all 25 grammars ripwire supports (plus HTML), one vendored grammar + query each.
4. **A fresh history.** A fork would carry 2,485 commits of an AI-authored development history.
   agentatlas starts clean and owns its own.

### Third-party vendored sources

| Source | License | Where |
| --- | --- | --- |
| [tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) (v0.23.4) | MIT | `third_party/deps/go/` |
| [tree-sitter-java](https://github.com/tree-sitter/tree-sitter-java) (v0.23.5) | MIT | `third_party/deps/java/` |
| [tree-sitter](https://github.com/tree-sitter/tree-sitter) core (via the `tree-sitter` crate) | MIT | cargo dependency |

### Versions pinned

- ripwire reference build: `redhat-et/ripwire` main, 2026-09-12 (the oracle for `test/golden/`).
- tree-sitter crate: 0.23.2 (bundled core ABI 14, matching the vendored grammar ABI).