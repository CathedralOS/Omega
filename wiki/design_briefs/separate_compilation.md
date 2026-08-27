# Design Brief: Separate Compilation And Replaceable Realizations

Current as of 2026-08-25. Status: semantic architecture settled; concrete
artifact encodings and runtime algorithms are implementation work.

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

## Composition authority

`build.omg` is the owner-controlled composer. Its typed selection names the
exact closed requirement application, the exact provider realization, and
whether that edge is fused into the surrounding artifact or independently
emitted:

```omega
builder.select_provider<ClockHost, MonotonicClock>(
    CompositionMode::Independent
);
```

The exact API spelling may evolve, but the authority split does not. Provider
source declares what it satisfies; it cannot make itself independently
loadable, select itself for a deployment, widen an installation envelope, or
authorize its own replacement.

The closed requirement application is the stable service-slot identity. A
slot family is distinguished by ordinary closed static arguments with nominal
or declared-domain identity, never by an authored string, ordinal, vtable
index, artifact address, or provider era. One package may contribute zero,
one, or several independently selected roots.

The compiler derives the selected closure. Its exports are the requirement
identities chosen by composition; its imports are requirement calls that leave
the closure. Concrete implementation edges remain inside the closure, pull
their target into it when legal, or reject. Two independent closures may share
duplicable immutable dependencies. Mutable state and linear custody have one
owner; if several closures require them, they must be owned at a fused position
dominating all users or mediated through a separately selected service.

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

## Static evidence and replacement granularity

Publication and mediation are different. A public conformance makes one exact
compile-time evidence or strategy identity selectable; it does not create a
replaceable call boundary. When a consumer selects executable behavior,
layout, cleanup, or another runtime-bearing row from that conformance, the
selected realization becomes a concrete edge in the consumer's closure. A
corrected provider implementation does not reach the already-built consumer
without rebuilding it. Proof-only erased evidence retains its theorem and
certificate dependency but does not by itself pin runtime code.

A public boundary trait selected through `Service<R>` has the opposite update
shape: the stable requirement application remains at the crossing and new
calls may resolve a new provider era. Code expected to change independently in
the field therefore belongs behind a service requirement, while public
conformances remain appropriate for proofs and deliberately static strategy
selection.

`CompositionMode::Independent` is a request for a checked deployment cut, not
a provider property. The composer computes the least fixed point of every
concrete implementation, selected conformance, layout, cleanup, state, and
custody edge. If a consumer has fused provider-specific identity, that consumer
joins the replacement cohort; its own fused consumers join transitively. The
toolchain never silently calls the originally requested smaller cut
independent. It either proves that cut, reports the enlarged cohort for explicit
owner acceptance, or rejects it. The cohort may legitimately become the whole
program.

Every artifact therefore retains attributable closure edges. A source-backed
diagnostic names the exact occurrence and span. A prebuilt artifact may lack a
span, but its manifest still names the consuming artifact and declaration, the
selected conformance or implementation, and the edge that enlarged the cohort.
The guarantee is not that every requested hot swap is possible; replacement
granularity is a computed, visible graph property and every coarsening edge is
attributable before deployment.

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

## Candidate capsules and execution modalities

A component capsule is a deployment-agnostic candidate for one exact service
slot. It retains canonical Terminal Psi, reconstructed obligation evidence,
symbolic imports and exports, resource and lifecycle demands, target-semantic
dependencies, and any native realizations and refinement certificates it
offers. The source/provider declaration remains in the manifest for audit and
source-correspondence policy; the runtime carrier hides the future provider
type behind the service contract.

Three execution modalities are coherent:

1. verified Terminal Psi interpreted by an interpreter already proved against
   the pinned Psi semantics;
2. native code whose target-specific realization from that Psi is checked,
   whether the bytes were shipped precompiled or lowered locally; and
3. opaque native code admitted as executable trusted-computing-base content.

The first two reconstruct and discharge the Psi obligation set. Native
realization additionally closes ABI, layout, instruction, stack, and target
semantics obligations. The third has no reconstructible Omega semantic subject
and is therefore a severe, explicitly disclosed admission rather than an
ordinary capability-reach expansion.

Source correspondence, semantic safety, and executable realization are
different edges. A deployment may require source-to-Psi correspondence for
owner-approved releases even when the Psi is otherwise safe. Producer identity
or reproducible pedigree never substitutes for any checked edge.

Capsule acceptance is deployment-local. The initial build freezes an envelope
for the slot: permitted imports and authority, contract and observation
profile, target semantics, resource ceiling, acceptable execution modalities,
admission policy, and replacement constraints. A later candidate can be
installed unattended only when the local verifier reconstructs its obligations
and proves it fits that already authorized envelope. Widening the graph or
envelope is a new owner-controlled build/composition transaction, not an
ordinary hot update.

## Bindings and era entry

The trait name is an interface, not a runtime carrier. Runtime call authority
uses an explicit carrier such as `Service<R> in Bound`. The carrier denotes a
stable selected slot; it contains neither a provider object nor a source-visible
vtable and does not permanently name one provider era. `Bound` is routed
authority established by component installation/publication, not by a record
literal, zero initialization, injection, or proof alone.

`Service<R>` is affine by default. A service may publish checked duplication or
stronger linear lifecycle obligations when its protocol requires them.
Multiplicity belongs to this carrier, never to the boundary trait. Generic and
heterogeneous storage likewise store the carrier, not a bare trait value.

For a fused selection, the compiler may erase the carrier and call the selected
provider directly. For an independent selection, each call resolves the slot's
current era and performs the entry/leave accounting below. After publication,
new calls resolve the new era; an already entered call or era-custodied session
retains the era it selected.

Every replaceable binding publishes an entry contract with these semantics:

1. each entry linearizes into exactly one era;
2. that era remains reachable until the matching leave;
3. closing an era prevents future entry into it;
4. reclamation waits for acknowledged quiescence; and
5. entry and leave publish operational and resource cost.

Values returned across the seam are accounted separately from active calls.
Call quiescence cannot retire a descriptor, handle, session, callback, cleanup
plan, state claim, or other value whose meaning still depends on its producing
era. Such an era-pinning carrier must be affine or linear, carry one compiler-
accounted era pin, and have an unavoidable terminal disposition that releases
that pin. It cannot be `[copy]`: implicit duplication has no corresponding pin
creation or individual terminal event. Checked explicit duplication instead
returns another non-copy carrier and increments the same era ledger. A copyable
permanent-gateway value is legal only because it resolves through stable
process-lifetime state and pins no provider era.

Affine movement transfers one pin; compiler-planned affine cleanup releases it
on every ordinary terminal edge. A linear carrier's authored terminal protocol
must release it exactly once. Abnormal process death needs no in-process
reclamation, while any admitted leak or foreign retention keeps the era live or
quarantined. Reclamation consults the explicit entry and pin ledgers—never a
garbage-collector-style search for arbitrary reachable values.

The current generic era-entry ledger binds each era to one exact binding and
entry contract, admitted plan, and profile-sealed executable manifest. Entry
linearizes once and retains that era across routing changes. Reclamation
requires closure to new entry, zero active entries, zero era pins and cohort
holds, complete dispositions, and a fresh release receipt. RCU, counters,
hazards, and similar mechanisms remain runtime policy.

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

Consequently, a replaceable requirement does not return a bare local `dyn`
descriptor. It returns detached data or an explicit affine/linear handle whose
hidden descriptor and era pin follow the accounting above. This is stricter
than an ordinary package API, which may carry a private conformance inside a
`dyn` value when no independently reclaimable component boundary is crossed.

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

The initial build establishes the stable slot and first era, supplies bounded
`Service<R>` values to the application, and gives a designated supervisor a
linear update authority. A downloaded candidate cannot publish itself. The
supervisor presents its capsule to the generic installer/verifier, which checks
the frozen envelope and returns staged Type-side lifecycle state. Publication,
entry freeze, installed roots, retirement, and release are linear operational
tokens; they never travel in the erased proof lane.

Continuity-free replacement publishes the new era first, routes new calls to
it, then drains the old era. State alone does not require a pause: caches and
other resettable state may use that route. A promised observable continuity
property does. The service contract states what callers may observe across
replacement, providers publish projections and checked migration theorems over
that seam, and composition selects a plan that proves the promise. There is no
`HotSwappable` marker; replaceability is a closure property checked from the
service, provider, composition, and runtime facts together.

When a continuity-preserving cutover must stop new entry, whether calls may
wait at the slot is part of the service contract. If entry may not block, the
runtime or OS must coordinate the relevant scheduler/caller graph instead.
Any pause token threads linearly through a crash-free replacement window or an
explicit supervisor-owned recovery disposition.

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

The generic process-static service carrier enforces service-authored
duplicate-key, versioned-key, and atomic-transfer policies. Atomic replacement
requires a non-replayed receipt binding the service, handover contract, key,
registrations, eras, and publication/retirement/obligation-transfer facts.
Cathedral owns concrete service policy and provider receipts.

Candidate admission checks the new era's selected-provider TCB manifest before
publication. While eras coexist, the live report is the union of their known
entries and the weakest applicable scope-completeness and containment evidence.
An opaque process-static platform provider remains a deployment baseline rather
than becoming private to either era; component-owned registrations and handles
still receive complete disposition.

The live executable-TCB carrier accepts only profile-sealed manifests. It keeps
the process-static baseline separate from component eras, preserves source
attribution and completeness evidence when equal executable subjects are
unioned, and treats any incomplete contributor as making the scope incomplete.
Shared containment claims require evidence from every contributor; era removal
still requires closing and quiescence from the entry ledger.

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

Opaque callback admission exposes only those two routes. Gateway admission
binds the installed realization and proves process-lifetime, current-era
dispatch. Direct registration consumes an external-root handle and returns it
only after exact provider unregistration plus independently proved
unreachability and quiescence; failure preserves the registration evidence.

Mapping reuse has one rule:

> Reuse is legal only after proof that no live authority reaches the mapping.

In checked Omega, inert `addr` values and sealed inert `Ptr<T>` carriers cannot
recreate memory or execution authority; any live authoritative reference
therefore remains visible to entry, installed-root, custody, or era-pin
accounting. This proof comes from closed ledgers and disposition receipts, not
from tracing arbitrary reachable values. Proven quiescence permits ordinary
virtual-address reuse. An incomplete drain or possible untracked opaque holder
leaves the range reserved and
unmapped/trapping until a wider isolation domain is retired. Quarantine detects
stale entry but discharges no lock, claim, or protocol obligation. Repeated
incomplete replacements consume reserved virtual-address capacity and report
the attributed loss.

The executable-installation carrier separates successful reclamation from
quarantine. A quiescence receipt returns placement for W+NX reuse; a quarantine
receipt proves execute removal and continued reservation, retains the artifact
and attributed cause, and reports the reserved extent as capacity loss. A
stale-entry fault discharges no obligations. Concrete entry accounting and
wider-domain retirement remain runtime work.

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

A custom dynamic container that transitively stores era-pinning values must
preserve their checked moves and terminal dispositions, so the era ledger stays
exact without enumerating arbitrary objects. Checked root enumeration is
additionally required when a replacement plan promises exact per-value
migration or custody transfer. Without it, reporting may stop at the containing
component or container and the outstanding pin keeps its era retained until
ordinary destruction releases it.

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

Omega exposes the facts needed to check a proposed update cut: requirement
edges, entry and callback roots, sessions and returned custody, state owners,
resource demands, and selected continuity obligations. Cathedral decides which
slots form an update cohort, how schedulers and devices quiesce, which mixed-era
states it permits, its rollback/retention policy, and which irreducible update
nucleus remains stable. A leaf may update while a live parent continues through
its stable service slot; a session or callback that entered the old era pins
that era until it drains, migrates, redirects, or is retained. These are graph
and lifecycle facts, not a source-module hierarchy rule.

The owner-authorized build record and the live deployment journal are distinct.
No transaction is atomic across an in-memory slot publication and durable
storage. A runtime update therefore records durable intent, activates the new
era, and records finalization; restart reconciliation has defined behavior for
`Prepared`, `Activated`, and `Finalized` states. The journal records which
preauthorized envelope accepted each candidate and retains all checked evidence
and disclosed admissions. Cathedral chooses rollback versus roll-forward policy.

The Rust on-ramp's canonical versioned journal record is the checkable core of
that protocol. Its typed phase transitions require the exact durable predecessor,
return candidate publication custody on rejection, and treat decoded bytes as
report/replay evidence rather than reconstructed authority. Restart
reconciliation reports the remaining rollback/roll-forward choices; the
Cathedral-selected durable-storage path owns runtime policy. The generic Rust
adapter writes one new canonical record through same-directory staged file
synchronization, atomic no-clobber hard-link publication, staging-name removal,
and directory synchronization. Its non-clonable receipt independently replays
the path and exact canonical bytes. A post-publication cleanup or directory-
sync failure is reported as possibly visible rather than rolled back, and no
existing destination is replaced. Cathedral still selects paths, retention,
cohorts, and rollback versus roll-forward, and owns restart-to-runtime
reconciliation.

## Implementation work

- exact component artifact and lifetime-cohort manifest;
- symbolic import/export and relocation encoding;
- durable runtime encoding for the generic entry-ledger receipts and the
  selected concrete entry-acquisition algorithm;
- replacement-plan and disposition-receipt representation;
- continuation/state migration interfaces and the stable-object table bundle;
- cross-component optimization and specialization rules; and
- target/runtime stack-provision plans.

These are tracked in `../../TASKS.md`. They are not unresolved owner questions,
reasons to add replacement syntax, or reasons to make packages semantic
components.
