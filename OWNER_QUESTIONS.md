# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-08.

## Q1 — Declared crash-scope partial order

`ExecutionDomain` is settled as the permanent portable top, and exact nominal
identity is ordered reflexively. CRASH-CONTRACT also permits stable intermediate
containment scopes, but no source declaration, ownership boundary, or identity
rule currently defines them.

Choose how a package declares a crash scope and its order edges. The decision
must settle whether a scope may have multiple immediate parents, how imported
scope identities and order evidence compose across package boundaries, what
enters public semantic identity, and where cycle/conflicting-order diagnostics
are owned. Physical target realization remains an Omega installation concern;
the declaration must express only Psi's target-neutral nominal partial order.

## Q2 — Witness evidence introduction and elimination surface

A witness-bearing proposition already names its one carrierless trait
interface, and a concrete subjectless conformance now provides a named closed
implementation of such an interface. The source language does not yet say how
a proof that establishes the proposition selects that conformance, nor how a
consumer names and opens the exact retained evidence term. A bare proposition
fact currently records only the nominal application, so inferring a unique
visible conformance would erase the settled distinction between proposition
identity and witness-term identity.

Choose the proof-only introduction and elimination surface. The decision must
settle:

- where an establishing proof explicitly selects the complete named
  conformance and how its trait arguments are matched to the proposition's
  instantiated binder telescope;
- how a `requires` fact, cited theorem, returned proof, or forwarded fact
  retains that selected term rather than merely recreating the proposition;
- how proof-only code opens the term and scopes the stable opaque member
  symbols produced from its normalized rows;
- whether two introductions using the same conformance are the same term or
  distinct terms, and which authored or derived identity distinguishes them;
- how the surface composes with proof-only by-value carrierless `dyn` without
  exposing a runtime value or selected conformance through ordinary
  signatures.

The settled constraints remain: proposition identity is independent of the
selected witness, opening the same retained term twice yields the same opaque
symbols, distinct terms may contain distinct witnesses, and no carrier or row
may be inferred from ambient machine names.
