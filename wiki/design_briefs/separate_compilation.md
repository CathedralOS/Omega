# Design Brief: Separate Compilation And Replaceable Realizations

Current as of 2026-07-24. Status: architecture settled; artifact and runtime
representations remain open.

## Terms that must not collapse

- A **package** is a source, naming, and dependency-reach unit.
- A **requirement** is a behavioral contract that a provider realization may
  satisfy.
- A **component** is a provider realization selected for independent
  deployment or replacement, plus the closed code, state, resource, and
  version graph that realization owns.
- A **boundary** is a trust, ABI, or external-supply crossing.

A package is not automatically a component or a boundary. Packages normally
compose statically into an artifact and may inline across package edges.
`pub` publishes source names; it does not by itself publish a component ABI.

Calls naming a concrete machine bind that implementation identity. Calls
naming a requirement bind the requirement contract and may be selected
statically or preserved as a replaceable edge by the build. No `slot` keyword,
hot-swap call syntax, or replacement DSL is implied.

## Component closure

The build selects a provider realization as independently replaceable. The
toolchain computes and validates the closure containing:

- implementation machines, generated stubs, and private helpers;
- code, constants, relocations, unwind data, and other executable metadata;
- owned mutable state, continuation/frame pools, and external registrations;
- admitted providers and resources;
- requirements called outside the closure;
- migration, coexistence, cancellation, or restart policy for versioned state;
  and
- every entry by which control may reach the realization.

Concrete calls are legal inside the closure. A concrete-identity edge crossing
the replaceable boundary rejects; the crossing must name a requirement whose
evaluated calling, state, representation, and semantic contracts are
component-compatible. The same source requirement may still be statically
selected and inlined in another build.

Component validation is two-sided. The producer proves that internal identities
and ownership do not leak and exports only requirement identities. A consuming
build or final composer proves that every import names an exported requirement
and that normalized contracts match. Separate compilation does not require one
omniscient source pass.

## Resources and stack provision

Semantic compatibility and resource admission are separate:

- stable requirement promises describe behavior;
- every candidate publishes target-specific realized stack, structural-work,
  and machine-state demand with validation evidence; and
- the selected runtime establishes current provision.

Admission checks the candidate demand against current provision. A fixed
resource ceiling enters requirement identity only when policy intentionally
promises replacement without reprovisioning that resource. Otherwise a new
candidate may demand more resources if the runtime can provision them before
publication.

Stack provision is provider-owned. A precommitted stack needs sufficient total
capacity. A growable hosted stack additionally needs the target's probing/growth
contract. Unknown foreign headroom cannot admit a stack-bounded root.
Unconstrained resource renegotiation needs an independently provisionable
execution domain; a component-owned stack is one realization, not a language
requirement.

Static selection remains more precise: whole-program analysis may use the
selected realization's actual demand. A replaceable edge must remain valid for
the admitted candidate and therefore composes through requirement promises and
candidate admission records rather than private implementation evidence.

## Replacement protocol

Replacement accounts for every live object belonging to the old realization;
it does not require every object to migrate. An old activation, continuation,
state object, registration, authority, or device claim must be drained,
retained with its era, migrated, restarted/cancelled under contract, redirected,
or transferred to a named receiver that acknowledges the obligation.

The runtime ledger tracks a set of live eras. A provider may impose a bounded
`max_live_eras`; increasing that implementation limit only admits more
replacements. Admission provisions peak coexistence for all retained eras plus
the candidate.

Before publication, the replacement plan declares:

- its drain/coexistence policy and retention budget;
- the disposition of owned state;
- its point of no return; and
- failure behavior on each side of that point.

Entry acquisition has an era linearization rule: a racing caller is accounted
either in the closing era or in the new era, never between them. Visibility of
new code gates future entry; quiescence of the old era gates reclamation. They
may share completion-obligation infrastructure but establish different facts.

A parked continuation ordinarily pins the code and metadata needed to resume
its era. Reclamation waits for drain, valid cancellation, explicit continuation
migration, or an admitted indefinite-retention policy. "Routing switched" and
"old era reclaimed" are separate completion states.

Objects with independently reclaimable lifetimes occupy separate mapping
cohorts; unrelated lifetimes must not share a page that one side expects to
unmap.

## Current whole-program rework inventory

- one global runtime frame region with absolute offsets;
- one fused dispatch loop with dense global indices;
- per-call-context specialization crossing future component edges;
- final-image-only relocation/data planning;
- compilation-local arena indices instead of stable exported identities; and
- entry/provider wiring resolved once for the complete image.

See `../architecture/whole_program_assumptions.md` for the live backend
inventory.

## Honest staging

An implementation restriction is legitimate only when removing it admits more
programs without changing the meaning, identity, or guarantees of programs that
already compiled.

The first implementation may accept only component closures that coincide with
one package. That is a monotone restriction over the general realization-closure
model. It must be diagnosed as an implementation fence, never documented as
"component means package."

## Ownership

Omega owns normalized requirement/artifact identities, relocatable component
artifacts, contract validation, candidate resource reports, and the generic
entry/quiescence obligations needed for safe reclamation.

A runtime or OS such as Cathedral owns provider selection policy, resource
provisioning, era limits, drain/cancellation policy, state migration scheduling,
loader mappings, and the concrete binding-era algorithm. RCU, counters, hazard
references, or another algorithm may realize the same entry-accounting
contract.

## Open representation work

- exact component artifact and lifetime-cohort manifest;
- symbolic import/export and relocation encoding;
- concrete entry-acquisition and era-ledger representation;
- replacement-plan and disposition-receipt representation;
- continuation/state migration interfaces;
- cross-component optimization and specialization rules; and
- target/runtime stack-provision plans.

These are not reasons to add replacement syntax or make packages semantic
components.
