# Delta-generated function census

This gate compiles three ordinary Delta programs through the selected compiler
and executes the complete generated Gamma receipts. It does not extract helpers
or substitute a host compiler. In the two authored-census controls, every helper
returns `65`; `main : Bytes -> Bytes` calls the last authored helper and returns
one byte, `A`.

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

The normalization control has five authored functions. A shared 256-parameter
function forms a two-level call tree with 65,536 leaves, each `(g (f))`.
Wrapping the root call in 252 conditional expressions leaves normalization
height budgets three, two, and one at those levels. Every height-two leaf is
therefore extracted into one ordinary helper. The 530,514-byte source stays
within the authored source, syntax, arity, and depth provisions, but its
3,066,611-byte successful receipt exceeded the former 65,536-function census.
The baseline returned raw status 3 with no output. The receipt's `Bytes`
identity main must now return sealed input `00 41 80 ff` exactly. All unused
helper bodies still undergo Gamma static validation. Source and receipt
identities are pinned alongside this generator in `gate.py`; the receipt is
always produced by the selected Delta compiler, never constructed by the host.

Run from the repository root with Python 3 and a POSIX shell, on macOS arm64
or Windows x64 in Git Bash:

```sh
sh tests/delta/generated-function-census/run.sh
```

Use `sh tests/delta/generated-function-census/run.sh --normalization` for the
single helper-fanout regression without repeating the two authored-census cases.

Both hosts use the same Python framing and comparison code and the shared
platform-selecting Alpha seed materializer. macOS also requires `codesign`;
Windows requires `python3` on Git Bash's PATH. Windows execution has not been
validated here. A positive `OMEGA_DELTA_CENSUS_SECONDS` changes the per-invocation
host watchdog (default 1,200 seconds), not any language or evaluator bound.

The evaluator's request-byte bound now establishes function-storage fit for
every admitted receipt, including normalization helpers. This is not universal
Gamma-profile admission of every Delta source: body depth, validation bindings,
non-tail contexts, and immutable storage remain separate obligations. The
[Gamma gate](../../gamma/evaluator-development/README.md) crosses the former
census ceiling, retains duplicate rejection, and checks exact/adjacent request
extent. Physical function-table exhaustion cannot precede request refusal.
