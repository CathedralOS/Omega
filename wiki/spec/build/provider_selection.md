# Provider selection

[Target slots](entry_roots.md) declare what can be selected.
[Boundary realization](../terminal-psi/boundary_calls.md) defines execution and
coverage evidence. Selection chooses a declared candidate; it does not construct
provider rows or grant runtime permission. An explicit
[assumed binding contract](assumed_binding_contracts.md) is the separate,
root-only runtime route for linking compatible requirements without a proved
semantic refinement. Ordinary selection and `satisfies` never imply that route.
Receiving [permission policy](permissions.md) independently checks exact
service/schema permissions against exercised physical mechanisms at explicit
receiver admission, not as a mandatory gate on ordinary artifact production.

## Declaration and choice

The selected target package supplies ordinary named default provider implementations. The build,
test harness, or component manager owning a service slot may override them with
an admitted provider. Conformances and exact bodyless bindings declare candidates;
the compiler derives ProviderPlan; configuration selects it. Requirements do not
select their own providers. A boundary operator has no provider clause, and there
is no parallel top-level primitive-provider registry.

[Test groups](testing.md) request providers for separate runner roots, not
application-wide replacements or new grants. Their selected closures remain
within root-delegated authority: a child cannot replace virtual filesystem
backing with host access. Test discovery collects exact checked satisfiers as
independent invocations; it is not provider selection among competing candidates.

A token-bearing boundary operation is a machine requirement in a boundary
trait. Its token is an ordinary explicit parameter, not a declaration owner or
separate operation species. Ordinary nonboundary token-bearing machines own
checked bodies and do not participate in Build provider selection.
Compiler-catalog semantic primitives are automatic. Target defaults still
supply boundary float implementations; overrides must satisfy the canonical
arithmetic contract, not choose a different meaning for `+`.
Selection alone contributes no service reach. Boundary requirements declare
their rows explicitly; omitted reach on a pure numeric requirement is empty.
Checked providers must fit that row, while imported or assumed implementations
retain their trust and physical-authority findings independently of reach.

Provider selection supplies no source-level token binder or conformance
evidence. Checked source must already select a token-bearing trait requirement
through one explicit proof-static implementation binder, or select its canonical
direct family, before provider planning. A provider row may realize that exact
boundary requirement; it cannot activate the trait's token, displace primitive
or direct operator meaning, resolve missing/ambiguous source evidence, or make
the same source select another operation on a different target.

`Build::select_provider<Slot, Provider>(builder)` is ordinary typed vocabulary. Slot
resolves to one exact boundary trait or package-qualified same-path boundary-
operator family. Provider resolves to one
named provider implementation, not a data type or a provider instance. Both retain compiler-derived package ownership. Plans and
checked adapters rejoin exact slot, realizing machine, provider, schema, and
requirement owners. Leaf names, strings, normalized signatures, ordinals,
declaration order, and compact fingerprints cannot select overloads.

The first value argument is the current activation's original mutable `Build`
value, passed explicitly as for other static Build operations. Helpers forward
that argument; neither the `Build::` namespace nor a same-shaped fabricated
value grants selection authority. The interpreter verifies the activation's
identity, not merely the argument's type. Receiver-style
`builder.select_provider<Slot, Provider>()` is not an alternate call form.

An explicit root trust grant uses
`Build::accept_boundary<Subject>(builder)`. Subject names an exact selected
provider or accepted machine under the existing admission policy; accepting it
does not select a provider or invent a new implementation. The Build argument
is explicit and must belong to the current activation. An ordinary declaration
named `accept_boundary` grants no authority. Receiver-style acceptance is
retired, with no implicit insertion of the Build argument.

Selection configuration is nominal, apart from the optional declared composition
mode supplied after `builder`:

```omega
Build::select_provider<Console, CapturedConsole>(builder);
Build::select_provider<Console, CapturedConsole>(builder, CompositionMode::Independent);
```

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

A selected provider's exact boundary calls create static transitive provider
dependencies. Provider planning closes those rows under target selection and
retains their requirement, implementation, target, reach, authority, trust,
replacement, and lifecycle identities. The closure, not the portable source
data shape, carries a target implementation's raw-family dependencies. Missing,
ambiguous, cyclic, cross-target, or unadmitted dependencies reject. Selection
does not manufacture source routing values. Source that needs another static
role names a distinct exact requirement application; runtime multiplicity uses
the protocol's ordinary data.

After selection, [behavior exclusions](behavior_exclusions.md) may require stronger
absence properties of the complete selected product than its ordinary callable
contracts promise. A no-op body can contribute evidence for that independent
check, not a rewritten requirement or hidden service invocation. A remaining
forbidden behavior or insufficient evidence rejects the product. Dynamic
replacement must preserve the exclusions through its installation envelope;
selecting one benign provider now does not certify every later binding.

## Toolchain-selected checked implementations

The `toolchain-settled-constant-binding` decision distinguishes who selects an
implementation from how that implementation executes. A toolchain-owned slot
may select ordinary checked provider machines supplied by its reviewed packages,
with explicit satisfaction edges and the same exact declaration, conformance,
schema, target, and body provenance as other checked implementations. A slot's
restriction on who may select or supply candidates does not restrict its
implementations to syscalls, imports, or compiler intrinsics.

A target-specific constant answer is an ordinary machine body, for example:

```omega
machine nanosecond_units() -> u64 {
    1_000_000_000
}
```

Its provider explicitly satisfies the relevant requirement and is selected for
the appropriate target. Returning the literal uses ordinary checked-body
lowering; removing the call is an ordinary optimization, not a prerequisite
for a valid binding. Runtime-derived values still require their actual
implementation: a process-stable clock frequency is not necessarily a
compile-time constant.

Do not add a constant-result provider kind, parallel constant table, intrinsic
sentinel, or requirement-name folding exemption for this case. Retain the
selected implementation through normal plan derivation, review, and lowering;
toolchain provenance does not waive conformance or coverage checks. Missing
support for selecting a checked body is an implementation gap, not permission
to invent an implementation or omit its selected row. Existing slot ownership
and authority restrictions remain unchanged.

## Derivation and retained evidence

Candidate derivation follows explicit satisfaction edges and computes the complete
nominal provider conformance closure: coverage, overload signatures, effect
summaries, dependencies, calling/layout applications, and normalized identity.
Composite adapters are ordinary checked machines, not plan-shaped call sequences.
Build-time code may choose declared candidates or compute a leaf binding; it
cannot append plan rows. Structural coincidence never supplies a satisfaction.

`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)`
records a relationship for the compiler to validate and derive, not authored
ProviderPlan rows. It retains the caller contract and actual supplied operation
separately. The supplier's body stays checked against its own contract; the
root assumption accounts for the exact admitted semantic difference. Complete
coverage, runtime-shape and operational compatibility, exact identity, receiving
policy and custody checks remain mandatory. It is not ordinary conformance.

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

An inherited receiver view stays beneath its selected whole conformance. It
retains the exact package-owned trait application, including phantom arguments,
and that application's complete original requirement roster. It is not another
provider plan or a union of independently selected rows. A whole descendant
choice can settle automatic ancestor selection only when the complete demanded
application agrees; it cannot suppress a competing authored choice. Shared
realizer code does not merge selecting owners. Erased runtime-carrier identities
cannot establish agreement between region-bearing applications.

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
