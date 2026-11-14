# Machine values and some packages replace dyn

Ratified 2026-11-06. Runtime behavior is selected through machine values in
ordinary data. `some S` lets each data value package a concrete type while
keeping dependent fields related. All source `dyn` forms are retired, including
the earlier captured extension. Static traits, named implementations, checked
laws and static machine parameters, written as
[typed static parameters](../../spec/language/generics.md#typed-static-parameters)
such as `const F: machine(value: u32) -> u32`, remain.

The source contracts are [machine values](../../spec/language/machine_values.md)
and [some packages](../../spec/language/some_packages.md); the
[Terminal contract](../../spec/terminal-psi/dynamic_dispatch.md) owns retained
evidence and indirect realization. Baseline representation is indirect, with
sized borrowed or owned state and uniform field layouts. No global target/witness
roster, implicit allocation or anonymous closure syntax is required; owned hidden
state follows [owning some packages](owning_some_packages.md). Optimizer
devirtualization remains optional.

## Migration ownership

`RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` on [TASKS.md](../../../TASKS.md)
owns this replacement, covering the historical
`DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` work. Its existing descriptor,
envelope and host coverage must migrate, not disappear. This is documentation
ratification; no new compiler support or fixture outcome is claimed here.

1. Parse/elaborate callable types using complete ordinary structural contracts;
   select closed named machines and conformance members as runtime values.
   Preserve static machine binders. Retire `dyn` syntax and coercion routes.
2. Elaborate `some` data binders, infer construction witnesses, and reject
   inconsistent or nonuniform fields. Carry binder/field relationships as typed
   evidence. Qualify literals, fields, parameters, results, arrays and containers.
3. Produce checked plans for stable subject/version openings, correlated
   projections, current contracts, fresh reborrows, moves, copies, joins and
   no-escape. Use exact places and captured indexes, not syntax recognizers.
4. Carry descriptor and package evidence through lowering, Terminal encoding,
   external verification, interpretation, native calls and replay. Remove the
   legacy whole-trait runtime table ownership; retain static conformance evidence.
   Change codec markers coherently rather than accepting old bytes as new values.
5. Migrate all affected `traits/`, callback, effect and resource fixtures and
   samples with actual behavior preserved. Retire `_` capture-map/Fused-roster
   design dependencies and update compiler READMEs as implementation lands.
6. Integrate code/borrow custody with replacement accounting and component proxy
   calls. Enforce boundary plans; do not equate indirect layout stability with
   permission to unload live code or state.

## Acceptance

Carry the same object across all nine ordinary value placements, including
container insertion from generic helpers and runtime branches. Use focused
positive/negative controls rather than new frozen facts matrices. Run affected
`runtime_dyn_*_exit` and other existing customers under migrated syntax on all
four hosted targets with `omega run --both`, and attribute every pinned pass/fail
movement. Record clean source identities. Required semantic coverage includes:

- Static `const F: machine ...` direct selection alongside runtime machine fields; rebinding
  descriptors, passing operations records and independently selected source/sink
  callables. Reject wrong signatures, stronger target preconditions, weaker
  guarantees, undeclared effects and null/forged descriptors.
- Heterogeneous `Reader` array/vector entries with matching state and callables;
  reject mismatched state, inline unknown-layout `S` and unsized witnesses.
- Same stable indexed subject succeeds; changed index, replaced package, stale
  projection or unrelated package pairing rejects without losing disjoint facts.
  Coherent moves/copies preserve supported equality evidence and custody.
- Local-state escape, duplicate exclusive access and witness escape reject;
  complete repackaging and concrete returned borrows obey normal lifetimes.
  Removing an entry transfers its loans; consuming it releases them only when
  no dependent loan remains. Mutable calls update the original referent.
- Current `requires` and retained `ensures`, effects, progress, crash and resource
  evidence hold through unknown targets. Package construction is not a permanent
  precondition proof. Cross-operation laws require explicit evidence.
- Finite value-family contracts retain complete declared tuples independently
  of target discovery. A new callable or package witness adds no semantic roster
  or layout identity. Optimizations preserve indirect-call behavior and costs.
- Terminal/native equivalence, forged evidence rejection and versioned codec
  migration; live code/state dependency retention, coherent drain/migration and
  component-boundary proxy behavior.
