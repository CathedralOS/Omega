# Rung: Gamma — safe definitional computation

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is where computation becomes *safe and definitional*: enough to write
parsers, validators, interpreters, and the reference implementation of the
[proof kernel](../proof_kernel.md) in a substantially safer language than raw
assembly.

## Adds

- algebraic data types and pattern matching
- pure functions and structural recursion
- a simple static type system
- possibly **total** computation (every function terminates — see honest edge #4
  below)

## Written in

Beta. Gamma's type checker and reference interpreter are beta programs.

## Meaning

A gamma program means what a **gamma reference interpreter written in beta** does
with it. The gamma *compiler* (to alpha/beta) is then an acceleration that must
behave like the interpreter — not the definition.

## Must not contain

No mutable memory, ownership, regions, or effects (those are
[Delta](delta.md)). Proof checking is a cross-cutting service rather than a
Gamma language feature; its reference implementation is written in Gamma.
Keeping Gamma pure, safe, and definitional makes that implementation small and
auditable.

## Totality (honest edge)

If gamma is total, it cannot contain a plain interpreter for a Turing-complete
language — the interpreter would loop forever on looping input. Reference
interpreters then become **fuel-bounded**:
`interp(program, fuel) -> Result | OutOfFuel`. This is a feature: it makes the
slow reference route's cost bounded and explicit, and it is exactly
[`totality_and_bounded_computation.md`](../../../design_briefs/totality_and_bounded_computation.md).
Decide it deliberately; it reshapes every interpreter in the spine.

## Current repo reality

Two things live here, pulling in opposite directions:

- `gamma.alpha` (~3540 lines) — the **old compiler-first imperative** v13:
  variables `a`–`j` in registers, arithmetic, `if`/`else`, `while`, `print`/`read`.
  Compiler written in alpha-asm, assembled by beta; not self-hosting. Parked.
- `interp.beta` — a **new interpreter-first reference interpreter**, stage 1
  (`test-interp.sh`), written in **Beta** (compiled by the self-hosting `bc`). A
  pure functional, **fuel-bounded** core: integers, top-level recursive functions,
  `if`, `let`, arithmetic/comparisons (`fac 5 → 120`, `fib 10 → 55`, `gcd → 12`).
  This is the architecturally-favored shape — *meaning is the interpreter* — and
  resolves the first divergence below. Stage 2 (DONE) adds the
  gamma-defining features (ADTs + pattern matching), so the proof kernel can now
  be rewritten in it cleanly (its hand-encoded tagged nodes are exactly that pull).

The old gamma diverges from the target in two ways the architecture wants
reconciled — `interp.beta` is the start of that reconciliation:

- **Compiler-first, not interpreter-first** — `gamma.alpha`'s compiler *is* the
  definition; the lattice wants a reference interpreter to define meaning, with
  the compiler checked against it. `interp.beta` is that reference interpreter.
- **Imperative, not functional/total** — `gamma.alpha` has no algebraic data,
  pattern matching, or totality. `interp.beta` is functional + fuel-bounded;
  ADTs + pattern matching landed in stage 2 (`Z`/`S`, `Nil`/`Cons`, `Pair`; `match` with nullary, applied-binding, and catch-all patterns).

The **simple static type system** gamma.md calls for now exists too:
[`typeck.beta`](../../../../compiler/gamma/typeck.beta) (run by `test-typeck.sh`),
a monomorphic, fully-annotated type checker — Int + `(data T (C ArgTy...)...)`
declarations, typed functions, and type checking of `if`/`let`/calls/constructor
application/`match`. It catches the errors you want (an Int op on a `List`, a
constructor given the wrong argument type, divergent `match` arms, a pattern from
the wrong type, a return-type mismatch) and accepts well-typed `Nat`/`List` code —
"just enough to make the proof kernel safe to write, no more."

Gamma now also hosts the bounded Q7 canonical semantic-ledger feasibility spike
in `compiler/gamma/terminal-ledger-spike/`. The typed program consumes exact
current terminal-Psi bytes, validates a closed subset, and emits/audits ranked
semantic rows. Both the Beta-written reference interpreter and the independent
Python evaluator agree on the positive fixture, an asymmetric join rejection,
and malformed-byte rejection. The assembled core is 4,545 lines rather than a
permutation-expanded verifier. Its bounded thirty-two-kind scalar leaf semantics
now live in five composed exact-unique policy-cohort schema tables; one generic
interpreter consumes result, denotation, goal, fact, crash, fuel, and frontier
fields, while calls and control remain separate algebra. Missing, duplicate,
and altered schema rows reject end to end. Exact cast, exact-right-shift, and
exact-left-shift retain distinct canonical goals; widening and wrapping shifts
remain total. Its value environment retains exact typed declarations across i8
and i16, so duplicate IDs and type drift cannot cross the generic schema
boundary. A separate 695-byte fixture and exact-unique table now cover
`BooleanStructuralField`, `EstablishTrivialAffineLocal`, and `PortWrite` through
their own place/frontier/effect vocabulary; they produce a 3-row, 185-byte
ledger and reject relevance, custody, service, effect, cleanup, and retirement
drift. Structural/effect byte decoding is isolated from its schema/evaluator.
The three call variants likewise live in a separate exact-unique composition
table. A dedicated 697-byte canonical fixture decodes exact `CallUnit` and
`BoundaryCall` resource, requirement, claim-transfer, completion-receipt, and
boundary custody through that table. One generic checker keeps signature, state
version, movement, requirement
coverage, substitution, outcome, crash-route, evidence-lifetime, fuel, and
frontier custody independent; missing, duplicate, altered, cross-kind, and
per-axis drift reject without adding call-specific evaluator branches.
That decomposition into decoder, typed row vocabulary, schema tables,
validators, and sequencing helpers is the intended shape for the production low
generator.

The spike also makes one scaling limit concrete: the monomorphic type system
requires a distinct parse-result ADT for each decoded type. Completing the
structural/effect and canonical call slices grows the assembled core to 180,717
bytes, 166 data declarations, and 389 typed functions, at nesting depth 25.
That repetition is an engineering/audit cost, not a reason to weaken the
canonical-byte endpoint.
The reusable PSITERM-neutral byte cursor and checked fixed-width primitives are
now factored and independently gated in `compiler/gamma/canonical-bytes/`.
Type-specific results and the bounded `u64` identity limitation remain explicit
in the spike. If the complete closed vocabulary cannot remain auditable after
that extraction, that is the point for an explicit Gamma rung-design
correction.

The architectural fork is now settled: the functional interpreter-first Gamma
defines meaning, while the old imperative compiler-first surface remains a
parked compatibility artifact. Future acceleration must be checked against the
reference interpreter; it cannot become a second definition.

## Open questions

- Retirement or differential validation of the parked compiler-first surface.
- Totality vs partiality, and the fuel discipline for interpreters.
- Whether the full canonical decoder remains auditable with monomorphic result
  ADTs or needs one explicit, minimal type-system ergonomics correction.
