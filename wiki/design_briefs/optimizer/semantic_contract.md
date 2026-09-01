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

Structural qualification rosters are non-recomputable authority. Terminal now
supports exact canonical path-indexed rows rooted at parameters, function
results, and structural operation results in addition to whole-root rosters.
Verification binds each nonempty path to its leaf carrier, calls copy only the
callee result's exact roster, and returns rejoin the source and declared result
contracts. Optimizer identity, independent replay, abstract projection, and
prephysical custody retain every row exactly. Target lowering now has one
bounded exception to the general rejection fence: an exact two-function owned
linear structural call/return closure retains one identical canonical nonempty
roster across caller parameter, call result, both declared results, callee
parameter, and callee source. Independent plan and local-function replay
reconstruct native ABI placement and every roster location. Boundary/provider,
unrelated, and all other projected-roster shapes still reject, and legalization
has no authority to consume this carrier. No stage may infer a field
qualification from root shape or carrier equality.

At control-flow joins, an output may retain only qualifications carried by
every incoming occurrence through valid establishment lineage. CSE and GVN
must distinguish unequal rosters unless a named transformation deliberately
forms their common intersection and independently revalidates every use.

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

The optimizer currently admits one deliberately smaller cyclic subset: the
existing exact unsigned countdown. A distinct optimizer-only verifier carrier
confers analysis authority without conferring execution, interpretation,
fixed-fuel, native, or publication authority. Verified-context validation
independently rederives the Terminal and current SCC membership, freezes the
ranked component at the optimization-unit level, and scopes cycle admission to
that function. Current ownership reaches its fixed point only when all incoming
and backedge states are equal. A canonical `CycleComponentId` now binds machine
and internal edges, with derived member, entry, and exit rosters; Terminal and
current graphs must independently produce the same structural component before
analysis custody is issued. An adjacent exact countdown-ranking certificate
binds the positive guard, unsigned rank carrier, minus-one backedge transfer,
and subtract obligation to that component. Its first revision-bound consumer
consumes the ordinary `LoopForest`, authenticates the exact reducible region
against component and ranking custody, and exposes the preheader argument as
the symbolic exact trip count. Independent replay reconstructs current edges,
boundaries, reachability, and header dominance without reusing CFG, dominator,
SCC, or loop producers. General ranking
certificates, productive unranked components, finite-work failures, and
transforms that invalidate SCC evidence remain closed.

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
