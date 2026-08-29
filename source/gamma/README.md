# `source/gamma/` — safe definitional computation

Gamma is the pure functional rung above Beta. It supplies algebraic data,
pattern matching, recursion, bounded reference evaluation, and a small static
type system. It is suitable for implementing the Delta compiler as well as
parsers, validators, and interpreters.

The reference evaluator implements proper tail calls and uses variable-size AST
nodes, immediate nonnegative `u32` integers with a boxed fallback, and headerless
two-word representations for ordinary `Cons` cells and the `Node` and `Chunks`
constructors used by bounded persistent-array workloads. Its parsed AST and
evaluation values occupy one stable-address checked 16 MiB bump arena. The
interpreter deliberately does not inspect compiler-generated Beta frames as
source-visible memory; exhaustion is fail-closed rather than reclaimed by a
collector with an incomplete explicit root set. Known `Nil`, `ZeroTree`, `Cons`,
`Node`, and `Chunks` patterns are classified once while parsing; expression and
value constructors retain their ordinary representation. Direct call
expressions also cache their resolved function-table index after the first
lookup; this is interpreter-private AST metadata and does not alter name
resolution, evaluation order, fuel, or printed values. Variable expressions
likewise cache their frame-relative lexical slot after the first complete
lookup; subsequent reads use the current invocation's frame base, so recursion
and repeated calls do not retain a prior invocation's value. These are bounded
execution properties of the canonical interpreter. Its checked evaluator entry
establishes positive fuel; subexpression evaluation preserves that invariant,
and every function-call decrement is checked at the tail-transfer boundary.
The internal evaluator therefore does not repeat the same fuel test for every
AST child. Literal, arithmetic, comparison, and condition hot paths encode or
decode the canonical nonnegative-u32 immediate representation directly;
negative and oversized results still rejoin the single boxed implementation,
and explicit boundary cases pin both paths. This is not extra Gamma syntax;
printed values and pattern matching
retain the language definition in [`LANGUAGE.md`](LANGUAGE.md). Canonical
source input has a checked 4 MiB ceiling; the adjacent byte exits 252 without
evaluation or output rather than overlapping the function table. Evaluated call arguments use
a checked 4 KiB interpreter-private scratch stack, and exhaustion exits 253
without printing a partial result. Tail transfers therefore do not allocate
source-visible persistent lists merely to move values between evaluator frames.
The arena occupies `[16 MiB,32 MiB)` of Beta's logical raw-memory profile.
Exhaustion exits 254 without partial output instead of falling into
Alpha's undefined out-of-range-memory edge.

The current reference implementation path is Rust-free but is migration and
semantic-oracle infrastructure, not a canonical compiler edge:

```text
interp.beta / typeck.beta --beta_compiler.alpha--> Alpha tape --seed-->
    canonical interpreter / type checker consuming Gamma source
```

D11 requires one Beta-written Gamma compiler artifact that accepts
Gamma and emits Alpha tape. `interp.beta` and `typeck.beta` are reusable
specification/implementation material for that edge, but the interpreter is not
permission to keep an older compiler as an external runtime dependency.

Principal artifacts:

- `interp.beta` — canonical Gamma reference interpreter, written in Beta;
- `typeck.beta` — monomorphic Gamma type checker, written in Beta;
- future `compiler/gamma_compiler.beta` — immediate-predecessor compiler emitting
  `gamma_compiler_bytecode.tape`;
- `reference/` — optional Python evaluator, fuzz generator, and differential
  runner;

Run the principal gates from the repository root:

```sh
sh source/gamma/test-interp.sh
sh source/gamma/test-interp-arena.sh
sh source/gamma/test-typeck.sh
sh source/gamma/reference/gamma-diamond-py.sh
```

The former terminal-ledger spike was retired after its format-18/vocabulary-20
decoder became stale. Its architectural result is retained in the terminal-Psi
documentation and Git history (`a5cfd83cc`); the live production semantic tables
now own its useful closed-row decomposition. Do not recreate format-specific
verification permutations under the Gamma language owner.

## Ownership classification

`LANGUAGE.md` defines Gamma's accepted surface. `interp.beta` and `typeck.beta`
are the current canonical executable interpretation and checking components
pending the standalone compiler. The independent Python
evaluator and the focused gates are conformance tools; agreement with them does
not override `interp.beta`.

The old generic canonical-byte and terminal-codec prototype was retired because
no live artifact admission consumed it. Being written in Gamma did not make it
part of Gamma meaning; artifact-specific reconstruction belongs beside the
artifact being admitted.

The older compiler-first imperative experiment does not select the new compiler
edge. The required Gamma compiler must implement the current Gamma language and
emit Alpha tape; it is not a revival of an unrelated historical language.

See [LANGUAGE.md](LANGUAGE.md) for the canonical surface and
[`rungs/gamma.md`](../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.
