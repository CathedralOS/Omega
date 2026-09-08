# Provider selection

[Target slots](entry_roots.md) declare what can be selected.
[Boundary realization](../terminal-psi/boundary_calls.md) defines execution and
coverage evidence. Selection chooses a declared candidate; it does not construct
provider rows or grant runtime permission.

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

Binding identities are normalized evaluated values; the requirement owns its
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

Crash causes, routes, and abandonment-frontier lower bounds support deployment
checks, not safe continuation. Restart requires verified closed custody or
explicit crash-safe shared resources and reset/transaction protocols, retained
with isolation/restart evidence. Otherwise an uncontained crash terminates its
execution domain. Co-location, handlers, and physical isolation are installation
facts, not alternate meanings of source `crashes`.
