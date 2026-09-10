# Provider selection

[Target slots](entry_roots.md) declare what can be selected.
[Boundary realization](../terminal-psi/boundary_calls.md) defines execution and
coverage evidence. Selection chooses a declared candidate; it does not construct
provider rows or grant runtime permission.
Receiving [permission policy](permissions.md) independently checks exact
service/schema permissions against exercised physical mechanisms.

## Declaration and choice

The selected target package supplies ordinary default provider types. The build,
test harness, or component manager owning a service slot may override them with
an admitted provider. Conformances and exact bodyless bindings declare candidates;
the compiler derives ProviderPlan; configuration selects it. Requirements do not
select their own providers. A boundary operator has no provider clause, and there
is no parallel top-level primitive-provider registry.

`Build::select_provider<Slot, Provider>()` is ordinary typed vocabulary. Slot
resolves to one exact boundary trait, package-qualified same-path boundary-
operator family, or top-level boundary requirement. Provider resolves to one
provider-data symbol. Both retain compiler-derived package ownership. Plans and
checked adapters rejoin exact slot, realizing machine, provider, schema, and
requirement owners. Leaf names, strings, normalized signatures, ordinals,
declaration order, and compact fingerprints cannot select overloads.

Selection is nominal and argument-free except for the declared composition mode.
Runtime-variable descriptors are ordinary capability values. Static target
handles and descriptors belong inside selected target realizations, not value
arguments to provider selection. Distinct authored configurations use distinct
nominal/static identities. A facade may compose checked target-specific leaves.
An exact call to a realization machine delegates directly; spelling its operator
inside the provider redispatches and can recurse.

[Foreign binding](foreign_bindings.md) identities are normalized evaluated values;
the requirement owns its
[calling-policy application](calling_plans.md#identity-and-selection).
A bodyless leaf uses `via`
only for payload its declaration and target cannot derive. Compiler intrinsics
are selected from exact declaration, signature, and target. The complete binding
producer closure and result enter final admission. Changing a typed foreign
locator, evaluated plan, or sealed catalog entry changes artifact identity and
requires fresh admission; Build cannot rewrite them during selection.

Overrides outrank target defaults and retain authored selection identity/order.
Missing, private, ambiguous, foreign, and duplicate candidates reject. A selected
operation's identity remains independent of its reach row; equal service sets
cannot correlate providers. Selection never permits mutation of derived plans.

After selection, [behavior exclusions](behavior_exclusions.md) may require stronger
absence properties of the complete selected product than its ordinary callable
contracts promise. A no-op body can contribute evidence for that independent
check, not a rewritten requirement or hidden service invocation. A remaining
forbidden behavior or insufficient evidence rejects the product. Dynamic
replacement must preserve the exclusions through its installation envelope;
selecting one benign provider now does not certify every later binding.

## Derivation and retained evidence

Candidate derivation follows explicit satisfaction edges and computes the complete
nominal provider conformance closure: coverage, overload signatures, effect
summaries, dependencies, calling/layout applications, and normalized identity.
Composite adapters are ordinary checked machines, not plan-shaped call sequences.
Build-time code may choose declared candidates or compute a leaf binding; it
cannot append plan rows. Structural coincidence never supplies a satisfaction.

Derivation, structural validation, semantic admission under boundary-grant
authority, and slot selection under the slot owner's capability are distinct
steps. Constructing a binding or choosing a candidate performs none of the other
steps. A selected plan must cover its complete schema and apply to the target.

Retain the selected plans as one immutable canonical fact set, with each exact
plan identity and a deterministic identity for the complete set. Missing,
duplicate, ambiguous, and identity-colliding selections reject. Later consumers
use that carrier rather than rediscovering declarations. Checked-adapter dispatch
rewrites only the exact selected overload row and retains its nominal machine
and entry-state identity; readable method names are drift checks, not selectors.
An external-binding projection retains calling/binding identity and order but
grants no admission, selection, ABI, or execution authority of its own.

Service schemas retain linear routed-parameter claims structurally: exact
parameter/result subject, carrier-aware semantic domain, authority-flow verb
such as `accepts`, carry policy, predicate discharge, and grant provenance.
Those rows participate in plan identity and reports, and travel to root selection.
Only the matching concrete entry receipt establishes a claim for an invocation;
type displays and schema promises are not that receipt.

## Operator families and indexed requirements

An authored operator override selects its complete package-qualified family
atomically. Every canonical overload coordinate must be covered or the whole
selection rejects, naming the missing member. Family membership is a semantic
set, deduplicated and encoded in canonical coordinate-identity order. Source
reordering does not change it. Adding/removing a public coordinate breaks existing
family overrides; no hidden signature syntax supplies per-coordinate overrides.

An overload coordinate and its static application are separate axes. Final
artifacts require exact closed applications, checked specialization, layout,
calling plans, target admission, and role-specific realization. Provider-authored
generic assertions do not establish coverage. A future checked universal theorem
could not replace those concrete obligations.

Universal generic coverage remains unimplemented and fail-closed without an
actual checked generic operator customer. An eligible future row must be
compiler-issued from a checked Omega body on the pristine pre-monomorphization
graph under its complete symbolic telescope. It binds requirement and realization
telescopes, binder categories/domains/bounds, exact mapping, requirement coordinate,
realization template, symbolic routing/dispatch, and transitive admissions.
The requirement domain must imply the realization domain. Bijective binder
reordering is permitted; collapsing independent binders or strengthening an
unrestricted input to `[copy]` is not.

The quantifier ranges over source applications satisfying the telescope and
`where` requirements, excluding target layout limits. Target-dependent routing
or admissions may qualify the row, but symbolic coverage proves only routing
and dispatch, not calling policy, byte layout, register classes, or stack
placement. Bodyless, external, opaque, intrinsic, and separately supplied
realizations remain exact-only; any future foreign universal contract needs its
own independent verifier, not a checked-body label or a successful concrete case.

An indexed requirement such as `ResidentContentTransfer<P,T>` is one schema
selected through one binding, not an ambient slot per monomorph. A provider may
offer a generic implementation or exact supported family, but final composition
must reconstruct every reachable closed demand and independently verify its
realization. Installation retains the admitted applications and issuance events.
Separate indexed slots are justified only by genuinely independent selection;
otherwise indices refine one provider's obligations. No ambient conformance
search or producer-authored total establishes the complete set.

[Opaque representations](opaque_representations.md) use a separate typed selection.
[Independent components](component_publication.md#selection-and-component-closure)
retain their own closure and installation obligations.

## Executable trust and containment

Selecting opaque in-process code adds a known executable dependency even when
source reach is unchanged. An isolated realization instead contributes its
endpoint and admitted execution requirements. A checked wrapper cannot erase
implementation evidence, execution scope, or containment requirements; these
dependencies propagate to consuming artifacts.

An executable manifest separates known entries from completeness and records
static versus Omega-mediated runtime origin. Uncontained opaque in-process code
makes caller-address-space completeness false and names the responsible provider:
it may introduce executable bytes without Omega admission. Profiles evaluate
exact identity, platform-baseline policy, evidence, scope completeness, and
memory/termination/fault/resource containment. They may permit and mark, or reject
before installation. Build API spelling for named profiles is not specified here.

A path or unresolved loader name is not executable identity. Ordinary package
policy rejects opaque code without pinned content, signer, or profile-owned
platform identity. Admission of a known binary still expands trusted code;
identity prevents substitution and enables revocation, not behavioral proof.

Each known entry retains exact provider, plan and executable/artifact identity,
implementation evidence and admission provenance, static/runtime origin,
execution scope, and independently evidenced containment guarantees. Trust is
classified per fact; a composite guarantee reports its weakest input and the
exact provider premise. An admitted hardware fact is not itself an opaque
executable in the caller's address space.

Containment names the actual guarantee, not its mechanism: memory isolation
outside explicitly shared authority, forcible termination, fault containment,
and bounded resource use are independent. A process supplies no resource bound
without explicit quotas. Platform baselines are policy allowlists, not different
language semantics. Static foreign selection contributes its abstract service
reach; an explicit runtime loader additionally reaches `DynamicLibraryLoading`.

The runtime executable ledger is append-only within one exact execution domain.
Omega-mediated admission supplies pinned executable, provider-plan,
implementation-evidence, and receipt identities; receipt replay rejects. Union
marks runtime origin and is idempotent. Without separate executable-closure
evidence it adds a known entry and attributed incompleteness. With that evidence
it may preserve a complete static scope, but cannot erase unrelated causes.
An enforceable dynamic-loading envelope requires containment controlling
executable admission; an uncontained binary may bypass Omega's ledger.

An isolated provider contributes a parent endpoint and its own separately
evaluated child manifest. Bind that exact child manifest and admission receipt
to one parent endpoint under a nonzero isolated scope; scope drift, duplicate
child scopes, and mixed-scope child entries reject. Endpoint containment remains
on the parent; child entries and completeness remain under the child scope.

Crash causes, routes, and abandonment-frontier lower bounds support deployment
checks, not safe continuation. Restart requires verified closed custody or
explicit crash-safe shared resources and reset/transaction protocols, retained
with isolation/restart evidence. Otherwise an uncontained crash terminates its
execution domain. Co-location, handlers, and physical isolation are installation
facts, not alternate meanings of source `crashes`.
