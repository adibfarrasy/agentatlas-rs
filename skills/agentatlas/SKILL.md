---
name: agentatlas
description: Map a codebase with agentatlas before you grep or read whole files — ranked symbols, callers, blast radius, tests-to-reach, all priced in tokens. Use when a task asks where something is, what calls it, what a change would break, or what to read first.
---

# agentatlas — map before you read

`agentatlas` builds a deterministic, ranked map of a codebase (25 languages) offline and answers
retrieval questions in one call — the alternative to blind `rg` + whole-file reads.

## When to reach for it

Use it BEFORE grepping or opening files when you need to:
- **Orient** on a task you are about to do → `agentatlas <dir> --for="<task in words>"`
- **Find a symbol** → `--callers=SYM` (who calls it), `--callees=SYM` (what it calls),
  `--uses=SYM` (every use site)
- **Check a change's blast radius** → `--impact=SYM` (transitive reach) + `--uses=SYM`
- **Locate code at a line** → `--at=FILE:LINE`
- **Read a definition's body** → `--expand=SYM` (whole file if smaller)
- **Map a stack trace / build error** → `--from-trace=FILE` (paste the error verbatim)
- **Find text** → `--grep=TERM`

## The commands

```bash
# the flagship: ranked symbols relevant to your task
agentatlas . --for="add retry with backoff to the HTTP client"

# who touches this, and what would break
agentatlas . --impact=ResolveRequest --uses=ResolveRequest

# understand a symbol without opening its file
agentatlas . --expand=src/client.go:ResolveRequest --top-k=0
```

## Reading the output

- Ranked rows are ordered by importance (PageRank), priced in `est_tokens=`.
- **Honesty rules to trust**: `counts_floor="1"` means a count is a floor, not a total; a zero
  means "none found", never "none exists"; every truncation is disclosed.
- `--for` output carries a `route=` (which ranker answered) and `confidence=` (`high`/`low`).
- A symbol's `next=` names the one follow-up command to run.

## Notes

- First call is ~0.1s warm; the binary is offline, no index server, no daemon.
- 25 languages: Go, Java, C/C++, C#, Python, Rust, TypeScript/TSX, JavaScript, Ruby, PHP,
  Swift, Kotlin, Dart, Elixir, Lua, Bash, Objective-C, CUDA, JSON, TOML, YAML, HTML.
- Track your savings: `agentatlas gain` rolls up tokens/time saved across your runs.