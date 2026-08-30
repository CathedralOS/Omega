# Optimizer Semantic Contract

This brief defines what an Omega optimizer must preserve. The architecture
entrance is [optimizer_architecture.md](../optimizer_architecture.md).

## Observational equivalence

A rewrite preserves every observation admitted by the source program and its
target contract:

- returned values and selected state transitions;
- exact trap kind and ordering where the language makes them observable;
- effect, service, atomic, volatile, placed-memory, and cleanup ordering;
- ABI-visible calls, arguments, results, clobbers, unwind/exit behavior, and
  externally visible storage;
- ownership, borrow, address-stability, and alias restrictions;
- debug/provenance roots required by the selected reporting contract; and
- logical fuel or progress accounting when it belongs to source semantics.

Native instruction count, code size, register pressure, compilation work, and
wall-clock time are cost observations, not source logical fuel.

## Arithmetic and floats

The operation identity includes width, signedness, domain/provider, and policy:

- `Exact` may be folded only when the same proof or check obligation is
  discharged and retained;
- `Wrapping`, `Saturating`, and `Trapping` are not interchangeable;
- shifts preserve the language's count and signedness rules;
- fused and unfused floating-point operations differ;
- NaN, signed zero, infinities, payload behavior, and rounding follow the named
  operation contract.

There is no ambient fast-math switch. A future lossy float family would need an
explicit source-visible name, declared observable differences, separate rule
identity, and tests.

## Proof and ownership capabilities

Accepted proof obligations and borrow-checker products are identity-bearing
capabilities. They can justify transformations unavailable to conventional
compilers, including:

- removing a check whose exact obligation is accepted;
- proving an exact arithmetic identity for all admitted inputs;
- proving non-aliasing across a mutation or load;
- proving field or variant irrelevance during a state interval;
- proving cleanup or transition reachability; and
- proving bounded/terminating loop transformations.

The optimizer may consume these facts only by recording their identities in
the candidate and validation receipt. It must not erase them before the last
dependent transformation or diagnostic boundary.

## Effects and control flow

Purity is a closed classification reconstructed from operation semantics.
Unused results alone never authorize removal. Calls, traps, services, atomics,
volatile/placed memory, cleanup, and transitions are barriers unless an exact
rule proves otherwise.

Control-flow analyses use explicit entry, exit, exceptional, cleanup, and
transition edges. Suspension remains an interprocedural state of the exact call
rather than a second local successor. Finite cyclic Terminal Psi is established
through verifier-derived SCC topology, loop-carried block arguments, ownership
fixed points, and distinct ranked, bounded, or unranked progress authority. The
remaining ordinary cyclic execution and optimizer-consumer work is engineering,
not an unresolved language meaning.

## Provenance

Every surviving or synthesized value, instruction, block, edge, and emitted
byte retains roots sufficient to answer:

- which source construct it implements;
- which semantic operation it preserves;
- which optimization rule changed it;
- which facts justified the change; and
- how to reconstruct diagnostics or a human report.

Provenance is not publication authority. Validation and custody receipts grant
authority at explicit stage boundaries.
