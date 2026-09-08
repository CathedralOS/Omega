# Native products and component publication

[Terminal Psi](../terminal-psi/product.md) is the portable compilation boundary.
This contract distinguishes compilation products from installed component
publication. Writing artifact bytes for later consumption does not establish a
running installation. The deployment requirements below apply to the installed
component claim, not to ordinary artifact serialization. Platform container layouts
are separate, and specified deployment routes are not necessarily implemented.

## Selection and component closure

A package owns source naming and dependency reach; a requirement owns behavior;
a component is a selected independently deployable realization and its closed
code/state/resource/version graph. A boundary is a trust, ABI, or external-supply
crossing. These are distinct: `pub` alone exports neither a component ABI nor
a replaceable edge.

Provider selection is Fused by default; omission and explicit `CompositionMode::Fused`
agree. Root Build may request `Independent`, but provider source, target defaults,
and unique automatic selection cannot. Conflicting modes reject, and an operator
family cannot split modes across overload coordinates. Independent is semantic,
not packaging: it requires a closed component graph, symbolic imports/exports,
entry/leave and resource demands, and installation/replacement obligations.
Until those exist, reject rather than silently fall back to Fused.

Composition mode is compiler-owned build vocabulary; an authored same-named or
same-shaped value cannot select it. Provider source declares satisfaction, not
its own deployment, selection, replacement authority, or wider envelope.

Slot identity is the exact closed requirement application. Independently selectable
families use ordinary closed static arguments with nominal or declared-domain
identity, not strings, ordinals, vtable indices, addresses, or artifact generations.
A package may contribute several independent roots.

Derive exports from selected satisfied requirements and imports from requirement
calls leaving the closure. Concrete-identity edges remain inside, pull in their
targets when legal, or reject. Duplicable immutable dependencies may be shared or
copied. Mutable state and linear custody have one owner: fuse them above dependent
closures or mediate them through a selected service.

Closure includes implementation/helpers/stubs, constants and relocations,
unwind/executable metadata, mutable state and continuation/frame pools,
external registrations, selected providers/resources, outgoing requirements,
versioned-state disposition, and every possible entry. Producer validation
proves internal identity/custody does not leak; independent consumer validation
matches every import to an exported requirement and its normalized calling,
state, representation, and semantic contracts. No omniscient source pass is
required.

Initial composition freezes an independent slot's replacement envelope: imports
and authority, compatibility/observation profile, target semantics, resource
ceilings, execution modalities, admission policy, and continuity constraints.
A runtime verifier may accept a candidate inside that envelope without rerunning
the build program. Widening requires a new owner-controlled composition, never
permission issued by provider code or downloaded artifacts.

A first implementation may restrict independent closures to whole packages;
that does not define components as packages. Calls crossing a replaceable closure
name ordinary requirements, not concrete machine identities. The same requirement
may be statically selected and inlined elsewhere; no hot-swap call syntax is implied.

### Replacement granularity

Publishing a conformance makes exact static evidence/strategy selectable, not
replaceable. Selecting executable behavior, layout, cleanup, or a runtime-bearing
row creates a concrete closure edge: changing that provider requires rebuilding
already-built consumers. Erased proof evidence retains theorem/certificate
dependencies without necessarily pinning runtime code. Independently updated
behavior instead crosses a stable requirement through a service binding.

The requested cut is checked by a least fixed point over concrete implementations,
selected conformances, layout, cleanup, state, and custody edges. A fused consumer
joins the replacement cohort, as do its fused consumers transitively. The compiler
proves the requested cut, reports an enlarged cohort for explicit owner acceptance,
or rejects; it cannot silently label a smaller cut independent. The cohort may
be the whole program.

Artifacts retain attributable closure edges. Source diagnostics identify the
occurrence/span; a source-free manifest still names consuming artifact and
declaration, selected conformance/implementation, and the edge enlarging the cut.
Package-shaped closures are a permissible initial restriction only when removing
that fence preserves all previously accepted meanings and identities.

## Candidate acceptance and resources

A deployment-agnostic checked capsule for one exact slot retains canonical
Terminal Psi, reconstructed obligations, symbolic imports/exports,
resource/lifecycle demands, target dependencies, and any offered native
realizations/refinement certificates.
Source/provider identity remains audit/correspondence data, not the runtime
service carrier's provider type.

An envelope may admit verified Psi on an interpreter proved against pinned
semantics, checked target-native realization of that Psi (shipped or lowered
locally), or disclosed opaque-native executable-TCB admission. The last has no
reconstructible Omega semantic subject; it is not merely more capability reach.
Source correspondence, semantic safety, and executable realization are separate
edges. Producer identity or reproducibility substitutes for none of them.

Semantic compatibility and resource admission are separate. Candidates publish
realized stack, work, and machine-state demand with evidence; runtime provision
must cover the admitted candidate before publication. A fixed ceiling enters
requirement identity only when the contract promises replacement without
reprovisioning. Otherwise a larger candidate may be admitted after additional
provision, within the owner-authorized envelope.

Stack supply is provider-owned. Precommitted stacks need sufficient capacity;
growable hosted stacks also need a probing/growth contract. Unknown foreign
headroom cannot admit a bounded root. Unconstrained renegotiation requires an
independently provisionable execution domain, not necessarily a component-owned
stack. Static builds may use selected actual demand; replaceable crossings use
public promises and candidate admission, not a previous provider's private proof.

## Service bindings and era entry

Runtime call authority uses a carrier such as affine `Service<R> in Bound`,
not a bare trait value, provider object, or source-visible vtable. It names the
stable slot, not one permanent era. Installation/publication establishes routed
`Bound`; literals, ZII, injection, or proof alone cannot. A protocol may publish
checked explicit duplication or stronger linear lifecycle obligations on the
carrier; the requirement trait itself acquires no multiplicity.

Fused lowering may erase an established carrier into direct dispatch.
Independent calls resolve the current era, enter exactly that era, and retain
it until matching leave. Closing prevents future entry; reclamation waits for
acknowledged quiescence. Entry/leave publish operational and resource costs.
Racing entry belongs to either the closing or new era, never neither. New calls
follow publication; entered calls and era-custodied sessions keep their era.

Binding identity includes the requirement, `CallPlan`, `StatePlan`, replacement
guarantees, and `BindingEntryCeiling`. The selected `BindingEntryPlan` must fit
that ceiling. Provider changes need not rename the binding; changing its ceiling
does. Interrupt/bounded-context admissibility derives from complete entry/leave
work, stack, suspension, blocking, reach/authority, and calling/state contracts,
not an `isr_safe` switch. RCU, epochs, counters, or hazards are runtime choices.

### Returned values and custody

Call quiescence does not release returned sessions, descriptors, callbacks,
cleanup plans, state claims, or handles that still depend on the producing era.
Each such carrier is affine/linear, retains one accounted era pin, and has an
unavoidable terminal disposition releasing it. `[copy]` cannot invent pins;
checked duplication returns another non-copy carrier and increments the ledger.
An ordinary move transfers its pin. Affine cleanup releases pins on ordinary
terminal edges; linear terminal protocols release exactly once. Admitted leaks
or foreign retention keep an era live/quarantined. Process death requires no
in-process reclamation. A copyable permanent gateway is valid only when it
resolves through process-lifetime state and pins no era.

Historical origin and current custody differ. Custody follows checked
establishment, even for transparent provider-local keys. Moves/returns/stores
preserve it; consumption discharges it; transfer requires a named receiver's
checked acknowledgement or an admitted boundary receipt. A postcondition alone
does not create custody.

Root maps at durable roots and suspension points identify claim-bearing places
using canonical place liveness. The runtime need not instrument every local
move or trace arbitrary objects. Dynamic containers preserve checked movements
and terminal dispositions. Exact per-value migration additionally requires
checked root enumeration; otherwise reports may identify only the holding
container/component and pins retain the era until ordinary destruction.
Retention diagnostics name the old era, claim, custodian, and most precise
known holding path. Long-lived custody is sound; policy chooses wait, retention,
authored transfer, or rejection.

By-value opaque crossings also require exact declaration, target-closed shape,
movement, and physical finalization under
[opaque representation composition](opaque_representations.md#evidence-and-composition).
A state-migration theorem cannot reinterpret a live inline carrier under a new
descriptor: rebuild its fused producers/consumers, enlarge the cohort, or reject.
Stable indirect descriptors can retain varying provider-owned backing while
outstanding non-copy handles pin the interpreting era. Local `dyn` tables do not
cross an independently reclaimable boundary; a local proxy may call the service,
and the boundary returns detached data or an explicit era-pinning handle instead.

## Replacement and coexistence

Capture owns device, clock, scheduler, and other boundary reach. Replayable
upgrade code operates on owned old state and captured context, writes an
exclusive output, observes no shared or atomic racing state, and calls only
deterministic providers. Empty reach is necessary but not sufficient to prove
that determinism.

Every old activation, continuation, state object, registration, authority, and
device claim receives a disposition: drain, era retention, migration,
contracted cancellation/restart, redirection, or acknowledged transfer. Before
publication, declare coexistence/drain policy, retention budget, state disposition,
point of no return, and failure behavior on both sides. Provision peak coexistence
for retained eras plus the candidate. A runtime `max_live_eras` limit is a private
capacity, not a change to replacement meaning.

Continuity-free replacement publishes/routes new calls first and drains the old
era afterward. Resettable state alone requires no pause. Observable continuity
is an explicit service promise proved through provider projections and migration
theorems under the selected composition plan; no `HotSwappable` marker supplies it.
If a cutover freezes entry, waiting must be allowed by the service contract.
Otherwise the OS coordinates scheduler/caller topology. Pause authority threads
linearly through a crash-free window or explicit supervisor recovery.
Parked continuations retain resumable code/metadata until drained, cancelled,
migrated, or admitted for indefinite retention. Routing and reclamation are
separate completion states. Independently reclaimable mappings cannot share a
page that one owner expects to unmap.

The initial build supplies the stable slot/first era and service carriers, and
grants one designated supervisor linear update authority. Candidates cannot
publish themselves. The installer checks the frozen envelope and returns
staged lifecycle custody; publication, entry freeze, roots, retirement, and
release are operational Type-side tokens, never erased theorem conclusions.

| Replacement tier | Existing session behavior |
| --- | --- |
| Drain/coexist | Old-era sessions finish without holder participation. |
| Explicit migration | Holder consumes its old handle and receives a new-era handle with transferred state/custody. |
| Stable object identity | A stable object table redirects the durable handle without holder cooperation. |

An era/local-key handle needs no generation for checked-code correctness while
its claim keeps the era/slot unreclaimable; generations may harden foreign or
admitted holders. Stable-object preparation keeps calls on the old era. Atomic
cutover selects new state, with racing calls continuing old, bounded-waiting,
or returning a declared retry as the service permits. Stable identity supplies
handle transparency, not migration correctness.

### Shared services and executable trust

Replaceable components have no duplicated ambient local runtime. Allocation,
output, cleanup, and failure services are explicit values or named process-static
custodians rooted outside eras. Registrations/queued callbacks retain the code
they may enter. Each process-static service owns key-collision/coexistence policy:
duplicate rejection, versioned keys, or atomic handover. Atomic handover needs
a non-replayed receipt binding service, contract, key, registrations, eras, and
publication/retirement/obligation-transfer facts; the framework cannot infer policy.

Before publication, check the candidate's selected-provider executable-TCB manifest.
The coexisting report unions profile-sealed era manifests with a separate
process-static baseline, preserving attribution. Any incomplete contributor
makes that scope incomplete; shared containment needs every contributor's
evidence. Remove an era only after closed entry, zero active entries/pins/cohort
holds, complete dispositions, and a fresh release receipt.

### Opaque retention and quarantine

Uncontained opaque component-local code may retain hidden threads, callbacks,
pointers, TLS, loader state, or global resources. It defeats a proof of reliable
native unloading. Select checked code, verified Psi through an admitted execution
route, or enforced containment with a separately replaceable scope when reliable
replacement is required.

Foreign-retained callbacks must target a process-lifetime current-era gateway
or carry accepted unregistration and independent unreachability/quiescence
evidence. Direct registration consumes its external-root handle and returns it
only after those checks; rejection preserves custody. A gateway makes the Omega
target replaceable, not the opaque library reclaimable or its manifest complete.

Mapping reuse requires proof that no live authority reaches it, from closed
entry/root/pin/custody ledgers and dispositions, not pointer tracing. Inert `addr`
and `Ptr<T>` cannot recreate authority. Complete quiescence permits ordinary
address reuse. Incomplete drain or unknown opaque holders leave an unmapped,
reserved range until wider-domain retirement. Quarantine detects stale entry
but discharges no lock, claim, or protocol debt; repeated quarantines report
attributed reserved-address capacity loss. [Executable retirement](executable_installation.md#visibility-and-retirement)
distinguishes reclaimed W+NX placement from continued quarantined reservation.

## Durable deployment recovery

The owner-authorized composition record and live deployment journal are distinct.
Memory publication and durable storage are not one atomic transaction. Record
durable intent, activate the era, then record finalization; restart reconciliation
defines `Prepared`, `Activated`, and `Finalized` behavior. Retain the preauthorized
envelope, checked evidence, and disclosed admissions used for each candidate.
The runtime/OS chooses rollback versus roll-forward, not downloaded code.

Decoded journal bytes are replay/report evidence, not reconstructed authority.
Recovery rejoins exact durable predecessor, live ledger, installed occurrence,
and runnable custody. Compact report IDs cannot replace strong occurrence
identity or retained installed-code context. Rejection preserves journal receipt,
explicit recovery choice, live ledger, and every unconsumed authority; a retry
cannot repeat an already spent publication operation.

Omega owns normalized identities, contract/resource validation, relocatable
artifacts, and generic entry/quiescence obligations. Cathedral or another runtime
owns selection policy, provision, mapping cohorts, scheduler/device quiescence,
era/drain limits, migration, rollback/retention, and concrete entry algorithms.
The [runtime implementation note](../../../omega-rust/omega/backend/runtime/component-publication/README.md)
records current journal/ledger support; neither it nor these contracts claims
ordinary source `Independent` execution is implemented.

## Products and authority

| Product | Contains | Does not establish |
| --- | --- | --- |
| Canonical Terminal artifact | Exact semantic, proof, optional debug, and reconstructed manifest sections. | Native target, installed provider, or publication authority. |
| Native artifact | Verified Terminal product, exact selected provider closure/executions, target realization, object/image, and replay evidence. | Output path, installed occurrence, progress receipt, or `InstalledCode` custody. |
| Native component candidate | The same native artifact plus exact source-selected provider facts and pending component-progress manifest. | A second lowering route or runnable installation. |
| Installed runnable | Replayed installation and the real installed-code/provider/progress custody for a live component era. | A new installation merely by recompilation. |
| Publication receipt | Exact installation/image/path relationship and required file-mode validation. | Authority inferred from a filename or compact report identifier. |

A source-free native consumer accepts canonical Terminal plus explicit realization
inputs, not checked/typed/source representations. It verifies, lowers, emits,
and replays exact bytes. Unresolved/duplicate settlements, executions outside
the selected requirement closure, target substitution, and object/image drift
reject before a product exists.

Realization retains the exact Terminal authority policy/review and the application
and physical-child evidence required by the
[boundary contract](../terminal-psi/boundary_calls.md#operator-applications-and-physical-children).

A direct native request cannot silently discard pending component progress.
An authority-distinct dynamic ELF request with an exact normalized interpreter
retains its complete Terminal/object/selection/evidence/image relationship in a
separate non-installable product. Producing it grants no loader-policy,
installation, publication, or execution admission.

Opaque callback companions remain exact by-value custody on success and rejection.
Retaining or validating them does not infer source origin, registration,
invocation, address, or lifetime authority. Interpreting an admitted callback
requires its separate exact source/realization join.

## Installed provider and progress closure

Installation seals the complete selected provider-plan set to exact installed
occurrences and a domain-separated strong digest. Include selected plans with no
execution in this image. Compact `u64` summaries are report coordinates, not
identity sufficient for acceptance.

A `ProgressProfile` receipt is admissible only when its exact issuer occurrence
realizes an owner-authorized boundary route and the receipt qualifies its exact
subject occurrence. Issuer and subject need not be the same occurrence.

Component closure checks every pending row and replays the manifest's own
domain-separated digest. Retain the original manifest and exact evidence;
compact-equal but structurally different inputs reject.

External-root summaries likewise retain exact validated roots, boundary/resource
columns, provider exit assurance, installed occurrences, and the strong selected
closure digest. Compact root-policy or execution-summary equality grants no
authority.

## Deployment transaction

Deployment receives the candidate and independently acquired installed code,
provider-occurrence bindings, progress attestations, and profile decision.
Compilation cannot supply substitutes for those authorities.

The transaction proceeds through installation claim, provider closure, progress
closure, canonical installation finalization, and publication. Failure retains
the exact current deployment carrier and every unconsumed later input. A one-shot
registry claim must not become repeatable because a later step failed.

Runnable binding joins the complete Terminal object/image, canonical installation,
real linear `InstalledCode` claim, and opaque acceptance. It compares the selected
provider closure even when no progress rows exist. The live era retains the
registry and runnable custody; successful retirement alone releases them.
Binding or retirement failure preserves exact custody.

## Visible component publication

The output owner consumes a deployment-finalized runnable, not selected plans or
compiler trust labels as a substitute. It derives the requested destination from
the build output and sealed image filename, then delegates consuming publication.

Publishing an installed component as a flat executable replays the
installation/image relation, stages exact sealed bytes and executable mode,
validates before atomic rename, and
replays the visible file before reporting success. Rejection returns the runnable
and requested path for retry. Receipt replay detects later byte or mode drift.

Flat installation v1 retains its fixed `0` destination byte under the existing
digest domain. No destination enum or optional second-copy receipt is required.
Whole [macOS packages](macos_application.md) have their own complete scope;
they cannot reinterpret the old executable-copy receipt. Flat installation,
output-kind, and general report validation remain required independently.

Reports retain the non-clonable published carrier, permitting borrowed inspection,
validated path projection, or consuming transfer to the next owner. They must
not reduce a failed linear transaction to diagnostics while dropping its custody.

Container assembly, metadata, signing, and other platform publication requirements
need their own exact artifact scope. An executable receipt alone does not certify
an application package.

## Report and entry-bridge consistency

The output category is explicit, not inferred from a path or a `wrote_output`
Boolean. Native executable output requires its publication custody;
object-container output has no executable receipt; check-only has neither output
nor receipt. A failed native-custody check cannot turn the result into a valid
object fallback. Written-output handoff paths match the receipt before reporting.

A retained program-storage binding and bridge agree exactly. Check-only bridges
remain pending, native bridges retain final wrapper evidence, and object-only
output carries no bridge. Final wrapper evidence rejoins the same executable
inventory, compiler-text derivation, function evidence, and boundary contract
as publication; independently valid evidence from another image is insufficient.

Execution consumers use the report's verified executable path, not a conventional
filename reconstructed under the build directory. Check-only, object-only, or
internally inconsistent reports provide no executable path. For a bundle, the
checked inner executable and package root retain one verified relationship.
