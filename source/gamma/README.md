# `source/gamma/` — safe definitional computation

Gamma is the pure functional rung above Beta. It supplies algebraic data,
pattern matching, recursion, fuel-bounded reference evaluation, and a small
static type system. It is suitable for parsers, validators, interpreters, and an
implementation of the cross-cutting proof kernel.

The reference evaluator implements proper tail calls and uses variable-size AST
nodes, immediate nonnegative `u32` integers with a boxed fallback, and headerless
two-word representations for ordinary `Cons` cells and the `Node` and `Chunks`
constructors used by the bootstrap translator's bounded persistent-array
carrier. Its parsed AST is an immutable pinned heap prefix. Evaluation values
occupy a stable-address 40 MiB heap with a separate byte-per-word allocation,
representation-kind, and mark map. When that heap fills, a non-moving
conservative mark/sweep validates every candidate root against an exact
allocation start, follows ordinary and compact constructor children to a fixed
point, and reuses dead blocks with next-fit search. It scans the Beta compiler's
explicit frame/data-stack reserve, live Gamma environments, and live argument
scratch; the separate Alpha return stack contains return addresses rather than
Gamma values. Conservative false roots can delay reclamation but cannot move or
prematurely reclaim an observable Gamma value. Known `Nil`, `ZeroTree`, `Cons`,
`Node`, and `Chunks` patterns are classified once while parsing; expression and
value constructors retain their ordinary representation. Direct call
expressions also cache their resolved function-table index after the first
lookup; this is interpreter-private AST metadata and does not alter name
resolution, evaluation order, fuel, or printed values. These are bounded
execution properties of the canonical
interpreter, not extra Gamma syntax; printed values and pattern matching retain
the language definition in [`LANGUAGE.md`](LANGUAGE.md). Canonical source input
has a checked 4 MiB ceiling; the adjacent byte exits 252 without evaluation or
output rather than overlapping the function table. Evaluated call arguments use
a checked 4 KiB interpreter-private scratch stack, and exhaustion exits 253
without printing a partial result. Tail transfers therefore do not allocate
source-visible persistent lists merely to move values between evaluator frames.
The heap occupies `[16 MiB,56 MiB)`, its map occupies `[56 MiB,61 MiB)`, and the
top 1 MiB remains reserved below Alpha's descending return stack. Exhaustion
after collection exits 254 without partial output instead of falling into
Alpha's undefined out-of-range-memory edge.

The canonical implementation path is Rust-free:

```text
interp.beta / typeck.beta --bc.beta--> Alpha assembly --assembler--> tape --seed-->
    canonical interpreter / type checker consuming Gamma source
```

Principal artifacts:

- `interp.beta` — canonical Gamma reference interpreter, written in Beta;
- `typeck.beta` — monomorphic Gamma type checker, written in Beta;
- `source/proof-kernel/implementations/gamma/` — independent
  proof-kernel implementations hosted by Gamma and owned by assurance;
- `canonical-bytes/` — reusable typed byte-cursor primitives;
- `terminal-codec-primitives/` — reusable typed scalar, identity, type, integer-
  value, UTF-8, and structural-leaf grammar fragments. It deliberately owns no
  fixed terminal-format header or complete live codec;

Run the principal gates from the repository root:

```sh
sh source/gamma/test-interp.sh
sh source/gamma/test-interp-gc.sh
sh source/gamma/test-interp-arena.sh
sh source/gamma/test-typeck.sh
sh source/proof-kernel/gates/gamma-checker.sh
sh source/gamma/test-canonical-bytes.sh
sh source/gamma/test-terminal-codec-primitives.sh
```

The former terminal-ledger spike was retired after its format-18/vocabulary-20
decoder became stale. Its architectural result is retained in the terminal-Psi
documentation and Git history (`a5cfd83cc`); the live production semantic tables
now own its useful closed-row decomposition. Do not recreate format-specific
verification permutations under the Gamma language owner.

## Ownership classification

Only `LANGUAGE.md`, `interp.beta`, and `typeck.beta` define Gamma's accepted
surface, canonical evaluation, and static checking. The independent Python
evaluator and the focused gates are conformance tools; agreement with them does
not override `interp.beta`.

`canonical-bytes/` and `terminal-codec-primitives/` are reusable programs in
Gamma. Being written in and evaluated by Gamma does not make a consumer part of
the language or its meaning. Artifact-specific obligation reconstruction belongs
under assurance, not under the Gamma rung.

## Parked imperative Gamma

`gamma.alpha`, `gamma_x64_windows.exe`, `build.sh`, `rebuild.sh`, and the root
`examples/` directory implement the older compiler-first language with variables
`a`–`j`, mutation, `if`/`while`, and decimal I/O. They remain compatibility and
differential-testing artifacts only. They do not define canonical Gamma and
must not grow into a second meaning path.

The parked files remain co-located for compatibility only. Their classification,
not their host-language suffix or directory proximity, determines their
architectural role. The former `compiler/gamma` compatibility entry has been
retired.

See [LANGUAGE.md](LANGUAGE.md) for the canonical surface and
[`rungs/gamma.md`](../../wiki/architecture/bootstrap_lattice/rungs/gamma.md) for
Gamma's architectural role.
