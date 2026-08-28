# Rung: Gamma — safe definitional computation

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is where computation becomes safe and definitional: enough to write
parsers, validators, canonical decoders, interpreters, and an independent
reference implementation of the cross-cutting [proof kernel](../proof_kernel.md).

## Adds

- algebraic data types and pattern matching;
- pure functions and recursion;
- a small monomorphic static type system;
- fuel-bounded reference evaluation with explicit exhaustion.

Reference evaluation is total because the interpreter always returns a value,
rejection, or exhaustion within its supplied fuel. That statement does not
pretend every unbounded mathematical computation terminates.

## Written in

Beta. Gamma's canonical reference interpreter and static type checker are Beta
programs compiled by the self-hosting `bc`.

## Meaning

A Gamma program means what the canonical, fuel-bounded Gamma reference
interpreter written in Beta does with it. Any compiler for Gamma is an
acceleration checked against that interpreter, never a second definition.

## Must not contain

No mutable memory, ownership, regions, runtime effects, or hardware boundaries.
Those are outside Gamma; [Delta](delta.md) retains only what its complete
compiler source and direct product edge justify, while Omega owns the complete
product model. Proof checking is not a
Gamma language feature or language rung. Gamma merely provides a safe language
in which one independent implementation of the generic proof kernel is small
and auditable.

## Current repository reality

- `source/gamma/interp.beta` — canonical pure, fuel-bounded interpreter;
  tail-position control is trampolined, while dense integers and ordinary
  two-field `Cons` values use semantics-transparent compact representations so
  compiler-sized reference evaluation remains bounded without enlarging Alpha's
  fixed memory;
- `source/gamma/typeck.beta` — static checker for `Int`, declared ADTs,
  functions, constructors, and matches;
- `source/alpha/checker/implementations/gamma/` — independent
  proof-kernel implementations hosted by Gamma, owned by Alpha's checker rather than
  the language rung;
- `source/gamma/canonical-bytes/` and
  `terminal-codec-primitives/` — reusable typed byte and terminal-grammar
  fragments, without a fixed-version live terminal decoder.

The exact spike sizes, supported operation cohorts, and gate counts belong in
the spike's own README and live task status, not in this durable rung definition.

The older compiler-first imperative prototype, its native artifact, scripts,
and private example corpus were retired after confirming that no lattice gate
or external consumer used them. Git history preserves that experiment; Gamma
does not own a parallel compatibility compiler.

## Implementation frontiers

- Keep the full canonical decoder auditable within Gamma's deliberately small
  type system; shared decoding primitives should absorb repeated mechanics.
- Improve reference-route performance without changing fuel visibility or
  semantic authority.
