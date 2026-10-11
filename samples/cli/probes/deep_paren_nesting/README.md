# deep_paren_nesting

Deep parenthesis nesting (80) compiles clean.

## Gaps found while authoring (filed)
- The corpus pins 300-level parens reaching checked semantics via the corpus gate's large-stack compile thread — but `omega update`/`omega --check` on a user package aborts with a fatal stack overflow (no diagnostic) somewhere between depth 80 and 120. The large-stack compile thread is not wired to the CLI eval path.

Compile-checked only: run a check target, not native.
