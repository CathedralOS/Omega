# Logical work and response bounds

[Terminal product](../terminal-psi/product.md) |
[Spatial resources](storage.md) | [Observation semantics](../terminal-psi/observations.md)

## Fuel is an accounting domain

A versioned schedule assigns deterministic logical cost to every closed Terminal
operation and terminator. Fuel is not native instructions, cycles, energy, or
elapsed time. Schedule identity is independent of semantic/program identity.
Extending the vocabulary requires an explicit cost classification; changing the
schedule does not change program meaning.

An evaluator or bounded compiler/bootstrap service may supply a budget.
Charge before each semantic site and report deterministic total and per-operation
and per-edge usage. Evaluated code cannot inspect the remaining budget, branch
on budget policy, or catch exhaustion as a machine result. Exhaustion is an
incomplete consumer outcome outside program semantics.

Native execution contains no implicit fuel counter, allowance, sponsor route,
dispatcher, charge/transfer/resume stub, or fuel-induced suspension. Suspension
facts come from real runtime operations. Static bounds and missing static bounds
alike leave native control flow unchanged. An explicitly authored bounded
algorithm is a different contract from this invisible service budget.

Build usage is deterministic for the concrete invocation, target description,
evaluator/Psi semantics, and schedule, never host load or elapsed time.
Long terminating builds remain legal. Progress reporting, warnings, cache
accounting, and optional root-selected ceilings consume the meter without
turning its policy into language semantics.

## Composition and charged work

Maximum logical work is the greatest total charge along any admitted path:

| Composition | Logical work |
| --- | --- |
| Sequential operations or calls | Sum of their charges. |
| Mutually exclusive paths | Maximum, not sum. |
| Call ending in a crash | Reached callee outcome only; no unreachable caller tail. |
| Caller segment continuing after a call | Callee normal-return bound plus the reached caller continuation. |

Work is consumed, not reclaimed. This differs from simultaneous stack use:
sequential call frames normally compose by maximum after the first is reclaimed.

Executable cleanup belongs to the edge's work: sum the actions on one edge and
take the maximum across mutually exclusive alternatives. A cycle executes its
backedge cleanup on each iteration, so a bounded-cycle certificate counts it
within the repeated work, not merely once in a maximum over edges. An iteration
bound with entry, per-iteration, and exit ceilings may compose as
`entry + bound * maximum_iteration_work + exit`; ranking without a quantitative
iteration bound does not establish this ceiling. Cleanup stack demand separately
uses ordinary peak composition, and its effects and guarantees remain part of
the enclosing machine contract.

Every retained executable operation is charged, including each arithmetic
operation, cast, widen, shift, call, and ordinary value leaf. `ReturnUnit` charges
one normal-return edge and no invented value-producing operation. Verifier work
introduces no executable fuel: equations, interval/preimage/hull calculations,
correlation and cancellation proofs, carrier intersections, and checked
substitution are proof work, not hidden program operations. Proving a retained
operation redundant does not remove its charge. Actual optimization must preserve
the logical-accounting contract of the exact semantic subject being analyzed;
a physical shortcut cannot silently select a new accounting subject.

## Restricted certificates

A maximum-logical-work certificate states: entry `E`, under preconditions `P`,
uses at most `K` units under schedule `S`. It binds exact Terminal identity,
entry, relevant preconditions, schedule, and scalar ceiling. A segment certificate
additionally binds its start and endpoint. Private maximizing-path evidence may
aid diagnosis but has no semantic identity and does not seed target WCET analysis.

The restricted fragment needs bounded iteration and call multiplicity,
acyclic or explicitly measured call structure, and finite contracts for every
reached blocking/foreign-completion edge. Static premises may be discharged at
installation; invocation-dependent premises remain ordinary call obligations.
Unsupported or unrepresentable numeric ceilings reject a certificate request.
Well-founded descent alone is not a quantitative bound on work.

Derivation reconstructs outcome-sensitive paths and the complete reachable
segment partition. Whole-entry, segment-local, and analysis-only safe-point
evidence have distinct roles. A segment or catalog cannot be smuggled into
whole-root composition, including beneath an opaque-provider or entry summary.
A complete safe-point catalog is sealed only after whole-roster replay;
borrowed rows do not separately authorize execution or composition.

Installed correspondence binds the exact Terminal subject, architecture, frozen
bytes/code context, function offset, entry stub, artifact, and selected entry.
A different occurrence or entry cannot reuse it. Retain the complete canonical
provider summary graph and all admitted opaque-provider receipts under the same
schedule. Compact fingerprints are report/cache coordinates; replay compares
the retained demand and evidence. A recomputable Psi theorem fabricates no
provider receipt.

A bound attached to installed code is PCC/report evidence only: no execution,
root, publication, bulk-charge, or runtime-meter authority follows. A proof that
arbitrary native bytes refine Terminal is a separate subject and trust closure.

## Cycles and unavailable bounds

A cyclic component is one verifier-derived semantic subject. Absence-of-bound
reports name its exact `CycleComponentId` and a directed cause: unranked,
unbounded rank, or the exact edge whose wait/foreign contract prevents closure.
Do not report whichever block a traversal happened to revisit first.
A separately admitted unranked cycle can be legal without a finite-work guarantee.

Selected-point reports distinguish:

- `Bounded(K, evidence)`: the restricted checker closed the bound.
- `Unknown(reason)`: it could not prove the requested bound.
- `NoFiniteGuarantee(subject, cause)`: an exact reachable edge or component
  publishes no finite bound.

None of these analysis verdicts creates an execution outcome or runtime meter.
A fixed-fuel request still rejects if no numeric certificate can be built.

## Response and physical time

Logical computation and waiting are different. A lock or I/O wait may have
bounded local computation but no finite response guarantee. A hard-control
profile requiring bounded response rejects unknown and no-finite-guarantee
root results. Force-terminating a blocked holder does not prove a response bound.

A clock/counter measures one observed run. Converting a logical or target-work
bound to elapsed time needs a separate derived or admitted worst-case timing
model. Fuel and target WCET optimize different cost functions and may have
different maximizing paths. Target analysis must re-search paths and account
for helper calls, expansions, and other lowering-introduced structure.
It may reuse enabling facts, not a presumed maximizing path, from Psi evidence.

A strict real-time profile needs analyzable evidence for every dependency:
Terminal semantics, a separately verifiable native WCET certificate, or an
admitted target-specific summary where policy permits it. Terminal is the
preferred distribution form, not the only possible mathematical evidence source.
Logical-work ceilings are not API/ABI identity unless an API explicitly promises
a deadline or fixed ceiling.
