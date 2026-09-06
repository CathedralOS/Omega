# Delta-generated function census

This gate compiles two ordinary Delta programs through the selected compiler
and executes the complete generated Gamma receipts. It does not extract helpers
or substitute a host compiler. Every helper returns `65`; `main : Bytes -> Bytes`
calls the last authored helper and returns one byte, `A`.

The 4,090-function source previously compiled successfully but produced a
receipt that exceeded Gamma's 4,096-function census after runtime helpers were
included. That exact 107,735-byte receipt returned raw status `3`, empty stdout,
and empty stderr under the former census; the enlarged evaluator runs the same
bytes with status `0` and stdout `A`.

The second source reaches Delta's full 32,768 authored-function allowance and
then executes its generated receipt, including the extra Gamma helpers. The
fixture generator creates only repetitive ordinary source; `fixtures.tsv`
pins the exact source and receipt identities. Neither a compiler-owned Delta
refusal nor a successful Gamma execution is inferred by the host.

Run from the repository root with Python 3 and a POSIX shell, on macOS arm64
or Windows x64 in Git Bash:

```sh
sh tests/delta/generated-function-census/run.sh
```

Both hosts use the same Python framing and comparison code and the shared
platform-selecting Alpha seed materializer. macOS also requires `codesign`;
Windows requires `python3` on Git Bash's PATH. Windows execution has not been
validated here. A positive `OMEGA_DELTA_CENSUS_SECONDS` changes the per-invocation
host watchdog (default 1,200 seconds), not any language or evaluator bound.

This establishes these two generated programs, not universal Gamma-profile
admission of every Delta source. Normalization helpers, expression depth,
non-tail contexts, and immutable storage retain their own limits. The separate
[Gamma gate](../../gamma/evaluator-development/README.md) tests the expanded
census's exact boundary, adjacent refusal, and duplicate-before-provision rule.
