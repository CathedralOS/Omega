# deep_paren_nesting

Deep parenthesis nesting (200) inside a package project compiles clean — below the corpus's 300-level `deep_nesting_within_limit` pin and the parser's 1024-level `MAX_NESTING_DEPTH` guard.

## Learnings
- Authored 200+ level nesting overflowed parallel parse workers on hosts without `RUST_MIN_STACK`: `parse_source_pages_with_count` fanned out onto the Rayon global pool (default ~2-8MB stacks), so whichever worker stole the deep unit recursed to death nondeterministically. The parser stage now owns a `PARSE_STACK_SIZE` worker pool (`omega-parse-N`), matching the compile thread's budget for every caller.

Compile-checked only: run a check target, not native.
