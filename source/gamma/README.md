# `source/gamma/` — safe definitional computation

Gamma is the pure functional rung above Beta. It supplies algebraic data,
pattern matching, recursion, bounded reference evaluation, and a small static
type system. It is suitable for implementing the Delta compiler as well as
parsers, validators, and interpreters.

The reference evaluator implements proper tail calls and uses variable-size AST
nodes, immediate nonnegative `u32` integers with a boxed fallback, and a
headerless two-word representation for ordinary `Cons` cells. Its parsed AST and
evaluation values occupy one stable-address checked 16 MiB bump arena. The
interpreter deliberately does not inspect compiler-generated Beta frames as
source-visible memory; exhaustion is fail-closed rather than reclaimed by a
collector with an incomplete explicit root set. Known `Nil` and `Cons` patterns
are classified once while parsing; expression and value constructors retain
their ordinary representation. Direct call
expressions also cache their resolved function-table index after the first
lookup; this is interpreter-private AST metadata and does not alter name
resolution, evaluation order, fuel, or printed values. Variable expressions
likewise cache their frame-relative lexical slot after the first complete
lookup; subsequent reads use the current invocation's frame base, so recursion
and repeated calls do not retain a prior invocation's value. These are bounded
execution properties of the current oracle. Its checked evaluator entry
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
    bounded interpreter / type-checker oracles consuming Gamma source
```

D11 requires one Beta-written Gamma compiler artifact that accepts
Gamma and emits Alpha tape. `interp.beta` and `typeck.beta` are reusable
specification/implementation material for that edge, but the interpreter is not
permission to keep an older compiler as an external runtime dependency.

Principal artifacts:

- `interp.beta` — bounded Gamma evaluation oracle, written in Beta;
- `typeck.beta` — bounded monomorphic Gamma checking oracle, written in Beta;
- future `compiler/gamma_compiler.beta` — immediate-predecessor compiler emitting
  `gamma_compiler_bytecode.tape`;
- `reference/` — independent Python evaluator, fuzz generator, and differential
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

`LANGUAGE.md` records the two pre-Q8 oracle surfaces. `interp.beta` and
`typeck.beta` are candidate compiler material and bounded failure detectors,
not a coherent executable contract or accepted compiler edge. The independent
Python evaluator and focused gates are diagnostics; agreement cannot promote
either oracle into the missing compiler.

The old generic canonical-byte and terminal-codec prototype was retired because
no live artifact admission consumed it. Being written in Gamma did not make it
part of Gamma meaning; artifact-specific reconstruction belongs beside the
artifact being admitted.

The older imperative experiment does not select the new compiler edge. OWNER Q7 must
first select Gamma's typed executable contract; the resulting compiler must
emit Alpha tape and is not a revival of an unrelated historical language.

See [LANGUAGE.md](LANGUAGE.md) for the current oracle surfaces and
[`rungs/gamma.md`](../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `compiler/` | The sole owner of the future Beta-written compiler accepting Gamma and its exact Alpha-tape edge. | Replace only atomically with the admitted immediate-predecessor compiler edge. |
| `reference/` | One independent executable Gamma meaning reference and bounded differential. | Delete when a stronger checked semantic relation subsumes every retained case. |
| `LANGUAGE.md` | The current accepted Gamma surface pending OWNER Q7's executable-contract ruling. | Replace only atomically with the ruled contract and synchronized compiler/reference tests. |
| `interp.beta`, `test-interp.sh`, `test-interp-arena.sh` | Candidate compiler component plus bounded execution, failure, and arena-profile discriminators. | Absorb into the standalone compiler or delete when Q8 makes the interpreter noncanonical; delete a gate when its exact failure surface is subsumed. |
| `typeck.beta`, `test-typeck.sh` | Candidate compiler component plus bounded static-semantics discriminators. | Absorb into the standalone compiler or delete when Q8 makes it noncanonical. |

At the root, `interp.beta` and `typeck.beta` remain candidate implementation
components and executable semantic oracles for the blocked compiler edge. Each
must be absorbed, adapted, or deleted when OWNER Q7 freezes the executable Gamma
contract; neither is itself a compiler artifact.
