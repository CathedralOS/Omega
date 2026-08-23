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

No mutable memory, ownership, regions, runtime effects, or hardware boundaries;
those belong to [Delta](delta.md). Proof checking is not a Gamma language feature
or language rung. Gamma merely provides a safe language in which one independent
implementation of the generic proof kernel is small and auditable.

## Current repository reality

- `compiler/gamma/interp.beta` — canonical pure, fuel-bounded interpreter;
- `compiler/gamma/typeck.beta` — static checker for `Int`, declared ADTs,
  functions, constructors, and matches;
- `bootstrap/assurance/proof-kernel/implementations/gamma/` — independent
  proof-kernel implementations hosted by Gamma, owned by assurance rather than
  the language rung;
- `compiler/gamma/canonical-bytes/` and `terminal-codec-primitives/` — reusable
  typed canonical-byte decoding layers;
- `compiler/gamma/terminal-ledger-spike/` — bounded feasibility work for the
  terminal-Psi semantic-ledger generator.

The exact spike sizes, supported operation cohorts, and gate counts belong in
the spike's own README and live task status, not in this durable rung definition.

## Parked imperative surface

`gamma.alpha`, its native executable, `build.sh`, and the root `examples/`
directory implement the older compiler-first imperative language. It has fixed
variables, mutation, `if`/`while`, and decimal I/O. It remains a compatibility
and differential-testing artifact only. It does not define Gamma and must not
grow into a parallel meaning path.

## Implementation frontiers

- Retire or keep differential coverage for the parked imperative compiler.
- Keep the full canonical decoder auditable within Gamma's deliberately small
  type system; shared decoding primitives should absorb repeated mechanics.
- Improve reference-route performance without changing fuel visibility or
  semantic authority.
