# Design Brief: Separate Compilation And Replaceable Realizations

Current as of 2026-07-26. Status: architecture settled; artifact and runtime
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

## Bindings and era entry

A boundary-trait value names a selected provider slot. It does not contain a
local dynamic-dispatch table and does not name one provider era permanently.
After publication, new calls resolve the slot to the new era; an already
entered call or era-custodied session retains the era it selected.

Multiplicity and rebinding are independent. A `[copy]` binding may be
duplicated because every copy still resolves the current slot. Duplication
does not create old-era retention. An affine or linear binding restricts
authority duplication for its own contract, not because replacement requires
it.

Every replaceable binding publishes an entry contract with these semantics:

1. each entry linearizes into exactly one era;
2. that era remains reachable until the matching leave;
3. closing an era prevents future entry into it;
4. reclamation waits for acknowledged quiescence; and
5. entry and leave publish operational and resource cost.

The algorithm is runtime policy. Epochs, RCU, counters, hazard references, or a
target-specific single-core scheme may satisfy the same contract. Whether a
bounded or interrupt context may call the binding is derived from the admitted
entry/leave work, stack, suspension, blocking, effect, capability, `CallPlan`,
and `StatePlan` contracts. There is no separate `isr_safe` switch.

Binding identity contains the abstract requirement contract, selected
`CallPlan` and `StatePlan`, replacement-facing guarantees, and a
`BindingEntryCeiling`. A target or runtime supplies a concrete
`BindingEntryPlan` whose realized demand fits that ceiling. Provider
realizations may change without changing the binding identity; changing the
ceiling is a contract change.

Local `dyn` tables never cross this boundary. A consumer that wants a local
dynamic interface owns a local proxy whose methods call the boundary binding.
The proxy localizes the ABI, entry, effect, and resource costs while the
descriptor remains an ordinary within-artifact two-word value.

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

## Coexisting eras and shared services

Checked Omega components have no duplicable component-local ambient runtime.
Allocator access, output, cleanup, failure, and other services are explicit
values or named process-static custodians rooted outside every replaceable era.
Two component eras therefore coexist as distinct owned subtrees rather than as
two hidden heaps or cleanup registries.

Process-static services still publish an era-coexistence contract. General
lifecycle machinery accounts for holdings: queued work and callbacks retain
the code era they may enter, registrations return linear claims, and values
retain an era only when their meaning depends on its state. Each service
separately defines logical name collision and handover. A registry may reject
duplicate key `K`, version it, or provide an atomic transfer; the component
framework cannot infer that policy.

Candidate admission checks the new era's selected-provider TCB manifest before
publication. While eras coexist, the live report is the union of their known
entries and the weakest applicable scope-completeness and containment evidence.
An opaque process-static platform provider remains a deployment baseline rather
than becoming private to either era; component-owned registrations and handles
still receive complete disposition.

## Opaque providers and mapping quarantine

An uncontained opaque library private to a component defeats provable native
unloading. It may retain threads, callbacks, native pointers, loader state, TLS,
or process-global resources that the Omega claim graph cannot enumerate.
Coexisting versions can also collide through those hidden resources. A
deployment requiring reliable replacement therefore selects checked Omega or
verified portable IR interpreted or locally lowered through its trusted path,
or a provider with enforced containment and a separately replaceable execution
scope.

If an opaque provider retains a callback into replaceable Omega code, the
foreign address must target a process-lifetime gateway that dispatches into the
current era, unless the provider supplies an accepted unregistration and
quiescence contract. The gateway preserves replaceability of the Omega target;
it does not make the opaque library reclaimable or complete its TCB manifest.

Mapping reuse has one rule:

> Reuse is legal only after proof that no live authority reaches the mapping.

In checked Omega, inert `addr` values and sealed inert `Ptr<T>` carriers cannot
recreate memory or execution authority; any live authoritative reference
therefore remains visible to quiescence accounting. Proven quiescence permits
ordinary virtual-address reuse. An incomplete drain or possible untracked
opaque holder leaves the range reserved and
unmapped/trapping until a wider isolation domain is retired. Quarantine detects
stale entry but discharges no lock, claim, or protocol obligation. Repeated
incomplete replacements consume reserved virtual-address capacity and report
the attributed loss.

## Claim custody and retention reporting

Claim metadata separates historical origin from current custody. Origin remains
audit history. Custody names the owner whose state currently gives the claim
meaning and whose era must remain reachable.

Custody follows checked establishment, not opacity or representation. A
transparent provider-local key can retain an era. Moves, returns, and stores
preserve custody; consumption discharges it. Transfer requires a named receiver
and checked acknowledgment. A boundary transfer requires an admitted receipt.
An implementation cannot create custody merely by writing a postcondition.

The runtime need not instrument every local move. Compiler root maps identify
claim-bearing places at durable roots and suspension points; continuation maps
are a projection of the canonical place-liveness analysis already needed for
carry and stack/resource checking. A coarse holder directory can identify the
component or container, while exact paths are reported when root metadata
supports them.

A custom dynamic container that transitively stores claim-carrying values must
provide checked root enumeration before it may appear in independently
replaceable component state. Failure is a compile error at the state-field
declaration, not a surprise during deployment.

Retention reports name old-era edges and the most precise known holding path,
for example:

```text
v1 retained by Cache.sessions[14]
  claim: Transaction::Live
  custodian: PaymentProvider@v1
```

Long-lived old-era custody is sound. Deployment policy decides whether to wait,
retain the era, request an authored custody transfer, or reject the
replacement.

## Replacement capability tiers

Publication always redirects new binding calls to the new era. The tiers differ
only in what happens to existing sessions:

| Tier | Existing sessions | Application participation |
|---|---|---|
| drain or coexist | continue in the old era until they end | none |
| explicit migration | holder consumes the old handle and receives a new-era handle | required |
| stable object identity | a stable object table redirects the durable handle | none |

The first tier uses era-bound handles such as `{era, local_key}`. An optional
generation hardens admitted or foreign holders but is not required for
correctness in fully checked Omega code: a live claim keeps its era and slot
unreclaimable.

Explicit migration is an authored protocol. It transfers state, custody, and
claims and changes the application-visible handle. A stable object table is
warranted only when replacement must not require holder cooperation. It
provides handle transparency, not semantic correctness.

Stable-object migration uses a state machine:

```text
Live(v1)
  -> Preparing(current = v1, candidate = v2)
  -> Cutover(point of no return; publish v2 atomically)
  -> Live(v2)
```

Calls during `Preparing` continue to use v1. At cutover, the requirement
contract must permit the chosen racing-call disposition: continue on the old
era, bounded wait when allowed, or a declared retry result. The provider proves
that migrated state denotes the same logical object and that custody and
resource obligations transfer correctly.

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
- continuation/state migration interfaces and the stable-object table bundle;
- cross-component optimization and specialization rules; and
- target/runtime stack-provision plans.

These are not reasons to add replacement syntax or make packages semantic
components.
