# Provider selection

Source: `wiki/spec/build/provider_selection.md` lines 1–427 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| PROVIDER_SELECTION1 | "Selection chooses a declared candidate; it does not construct provider rows or grant runtime permission." | Provider selection | Selection must pick an existing declared candidate and must not fabricate provider rows or grant runtime permission by selecting. | not stated | 9 |
| PROVIDER_SELECTION2 | "Ordinary selection and `satisfies` never imply that route." | Provider selection | Ordinary selection and `satisfies` edges must not be treated as implying the assumed-binding runtime route. | not stated | 9 |
| PROVIDER_SELECTION3 | "Ordinary programs should reject assumed bindings." | Provider selection | Ordinary programs must reject assumed bindings. | not stated | 9 |
| PROVIDER_SELECTION4 | "If a provider fails to satisfy a requirement, repair its implementation or contract, or use a checked adapter; do not substitute an assumed relationship merely to make selection succeed." | Provider selection | Failed satisfaction must be fixed by implementation, contract, or checked adapter; assumed relationships must not be used to force selection success. | not stated | 9 |
| PROVIDER_SELECTION5 | "Receiving [permission policy](permissions.md) independently checks exact service/schema permissions against exercised physical mechanisms at explicit receiver admission, not as a mandatory gate on ordinary artifact production." | Provider selection | Permission policy must be checked at explicit receiver admission only; it must not gate ordinary artifact production by default. | at entry | 9 |
| PROVIDER_SELECTION6 | "Requirements do not select their own providers." | Declaration and choice | Requirements must not self-select providers. | not stated | 9 |
| PROVIDER_SELECTION7 | "A boundary operator has no provider clause, and there is no parallel top-level primitive-provider registry." | Declaration and choice | Boundary operators must have no provider clause; no parallel top-level primitive-provider registry may exist. | not stated | 9 |
| PROVIDER_SELECTION8 | "Their selected closures remain within root-delegated authority: a child cannot replace virtual filesystem backing with host access." | Declaration and choice | Test-group provider selections must stay within root-delegated authority; children must not replace virtual FS with host access. | not stated | 9 |
| PROVIDER_SELECTION9 | "Test discovery collects exact checked satisfiers as independent invocations; it is not provider selection among competing candidates." | Declaration and choice | Test discovery must treat satisfiers as independent invocations, not as competing provider selection. | not stated | 9 |
| PROVIDER_SELECTION10 | "Its token is an ordinary explicit parameter, not a declaration owner or separate operation species." | Declaration and choice | Boundary token-bearing operations must use tokens as ordinary explicit parameters, not as declaration owners or separate operation species. | not stated | 10 |
| PROVIDER_SELECTION11 | "Ordinary nonboundary token-bearing machines own checked bodies and do not participate in Build provider selection." | Declaration and choice | Nonboundary token-bearing machines must own checked bodies and must be excluded from Build provider selection. | not stated | 10 |
| PROVIDER_SELECTION12 | "canonical float arithmetic is [realized by the selected target](../language/numeric_values.md#target-realization-and-control-state), not selected or overridden here" | Declaration and choice | Canonical float arithmetic must come from selected target realization, not from this selection surface. | not stated | 9 |
| PROVIDER_SELECTION13 | "Selection alone contributes no service reach." | Declaration and choice | Selection must not by itself add service reach. | not stated | 9 |
| PROVIDER_SELECTION14 | "Boundary requirements declare their rows explicitly, including `reaches none` for service-free arithmetic." | Declaration and choice | Boundary requirements must declare reach rows explicitly, including `reaches none` when service-free. | not stated | 9 |
| PROVIDER_SELECTION15 | "A non-boundary numeric requirement that promises service-free implementations also writes `reaches none`; omission instead follows [deferred requirement reach](../language/effects.md#deferred-requirement-reach)." | Declaration and choice | Service-free non-boundary numeric requirements must write `reaches none`; otherwise deferred-requirement reach rules apply. | not stated | 9 |
| PROVIDER_SELECTION16 | "Checked providers must fit that row, while imported or assumed implementations retain their trust and physical-authority findings independently of reach." | Declaration and choice | Checked providers must match the declared reach row; imported or assumed implementations keep separate trust and physical-authority findings. | not stated | 9 |
| PROVIDER_SELECTION17 | "Provider selection supplies no source-level token binder or conformance evidence." | Declaration and choice | Selection must not supply source-level token binders or conformance evidence. | not stated | 9 |
| PROVIDER_SELECTION18 | "Checked source must already select a token-bearing trait requirement through one explicit proof-static implementation binder, or select its canonical direct family, before provider planning." | Declaration and choice | Before provider planning, source must already have selected the token-bearing requirement via one proof-static binder or canonical direct family. | not stated | 8 |
| PROVIDER_SELECTION19 | "A provider row may realize that exact boundary requirement; it cannot activate the trait's token, displace primitive or direct operator meaning, resolve missing/ambiguous source evidence, or make the same source select another operation on a different target." | Declaration and choice | Provider rows may realize the boundary requirement but must not activate tokens, displace primitive/direct meaning, fix missing evidence, or retarget operations across targets. | not stated | 9 |
| PROVIDER_SELECTION20 | "Slot resolves to one exact boundary trait or package-qualified same-path boundary-operator family." | Declaration and choice | `select_provider` slot operand must resolve to exactly one boundary trait or package-qualified same-path operator family. | at a call | 9 |
| PROVIDER_SELECTION21 | "Provider resolves to one named provider implementation, not a data type or a provider instance." | Declaration and choice | Provider operand must name one provider implementation, not a type or instance. | at a call | 9 |
| PROVIDER_SELECTION22 | "Both retain compiler-derived package ownership." | Declaration and choice | Slot and provider operands must keep compiler-derived package ownership. | not stated | 9 |
| PROVIDER_SELECTION23 | "Plans and checked adapters rejoin exact slot, realizing machine, provider, schema, and requirement owners." | Declaration and choice | Plans and adapters must rejoin exact owners for slot, machine, provider, schema, and requirement. | not stated | 9 |
| PROVIDER_SELECTION24 | "Leaf names, strings, normalized signatures, ordinals, declaration order, and compact fingerprints cannot select overloads." | Declaration and choice | Those textual or ordering artifacts must not be used to select overloads. | not stated | 9 |
| PROVIDER_SELECTION25 | "The first value argument is the current activation's original mutable `Build` value, passed explicitly as for other static Build operations." | Declaration and choice | First `select_provider` argument must be the activation's original mutable `Build`, passed explicitly. | at a call | 9 |
| PROVIDER_SELECTION26 | "neither the `Build::` namespace nor a same-shaped fabricated value grants selection authority." | Declaration and choice | `Build::` qualification and same-shaped fabricated values must not grant selection authority. | at a call | 9 |
| PROVIDER_SELECTION27 | "The interpreter verifies the activation's identity, not merely the argument's type." | Declaration and choice | Build activation identity must be verified, not only argument type. | at a call | 9 |
| PROVIDER_SELECTION28 | "Receiver-style `builder.select_provider<Slot, Provider>()` is not an alternate call form." | Declaration and choice | Receiver-style `select_provider` must be rejected or unsupported as an alternate form. | at a call | 9 |
| PROVIDER_SELECTION29 | "accepting it does not select a provider or invent a new implementation." | Declaration and choice | `accept_boundary` must not select a provider or invent implementations. | at a call | 9 |
| PROVIDER_SELECTION30 | "The Build argument is explicit and must belong to the current activation." | Declaration and choice | `accept_boundary` Build argument must be explicit and belong to current activation. | at a call | 9 |
| PROVIDER_SELECTION31 | "An ordinary declaration named `accept_boundary` grants no authority." | Declaration and choice | User declarations spelled `accept_boundary` must not grant boundary-acceptance authority. | not stated | 9 |
| PROVIDER_SELECTION32 | "Receiver-style acceptance is retired, with no implicit insertion of the Build argument." | Declaration and choice | Receiver-style acceptance must be unavailable; implicit Build insertion must not occur. | at a call | 9 |
| PROVIDER_SELECTION33 | "Runtime-variable descriptors are ordinary capability values, not provider selectors." | Declaration and choice | Runtime variable descriptors must not act as provider selectors. | not stated | 9 |
| PROVIDER_SELECTION34 | "An explicit compile-time target-catalog reference keys a Build row; it is not a runtime routing descriptor or an observation of the realized target." | Declaration and choice | Compile-time catalog references key Build rows only; they must not serve as runtime routing or target observation. | not stated | 9 |
| PROVIDER_SELECTION35 | "Physical handles and runtime descriptors stay inside selected implementations." | Declaration and choice | Physical handles and runtime descriptors must remain inside selected implementations. | not stated | 9 |
| PROVIDER_SELECTION36 | "Distinct authored configurations use distinct nominal/static identities." | Declaration and choice | Distinct configurations must get distinct nominal/static identities. | not stated | 9 |
| PROVIDER_SELECTION37 | "An exact call to a realization machine delegates directly; spelling its operator inside the provider redispatches and can recurse." | Declaration and choice | Direct realization calls must delegate directly; re-spelling inside provider must be treated as redispatch with recursion risk. | at a call | 9 |
| PROVIDER_SELECTION38 | "Target-prefixed machine declarations are retired." | Build-owned target selection | Target-prefixed machine declarations must not be accepted. | not stated | 9 |
| PROVIDER_SELECTION39 | "The target key belongs to the Build row, not to the source machine's declaration, name, or body." | Build-owned target selection | Target key must live on the Build row, not on source machine declaration/name/body. | not stated | 9 |
| PROVIDER_SELECTION40 | "There is no implicit same-path target-body family, target-sensitive source overload, or additional body-selection pass." | Build-owned target selection | Compiler must not apply implicit target-body families, target-sensitive overloads, or extra body-selection passes. | not stated | 9 |
| PROVIDER_SELECTION41 | "Psi checks every supplied implementation independently of target selection." | Build-owned target selection | Every supplied implementation must be checked in Psi regardless of target selection. | not stated | SHARED |
| PROVIDER_SELECTION42 | "Omega validates each selected closure against the row's target, including ABI, imports, layout, ISA, contracts, and authority." | Build-owned target selection | Each selected closure must be validated against its row's target for ABI, imports, layout, ISA, contracts, and authority. | not stated | 9 |
| PROVIDER_SELECTION43 | "An unselected implementation's foreign locator need not be realizable on the selected target, but its source must still be checked." | Build-owned target selection | Unselected implementations must still pass source checking even if locator is unrealizable on selected target. | not stated | SHARED |
| PROVIDER_SELECTION44 | "Missing, duplicate, incompatible, or unadmitted selected supply rejects rather than choosing a body by spelling or source order." | Build-owned target selection | Bad selected supply must reject; must not fall back to spelling or source order. | not stated | 9 |
| PROVIDER_SELECTION45 | "Concrete layout evaluation follows [layout demand](../layouts/plans.md#layout-demand-and-validation), not a pre-pass over all supplied bodies." | Build-owned target selection | Layout evaluation must follow layout demand rules, not a blanket pre-pass on all bodies. | not stated | 9 |
| PROVIDER_SELECTION46 | "Every declared target row demands the layouts of its selected closure, including helpers and callbacks, even when that target is not being emitted." | Build-owned target selection | Each declared target row must demand layouts for its selected closure and helpers/callbacks even if not emitted. | not stated | 9 |
| PROVIDER_SELECTION47 | "selecting it later can expose a latent layout error and must validate it before successful consumption or realization." | Build-owned target selection | Newly selected previously-unreached bodies must pass layout validation before consumption or realization. | not stated | 9 |
| PROVIDER_SELECTION48 | "The selected generic member must implement the public contract for every admitted application." | Generic selected requirements | Selected generic member must satisfy public contract for all admitted applications. | not stated | 7 |
| PROVIDER_SELECTION49 | "Build neither enumerates all possible tuples nor chooses the caller's evidence as a side effect of platform selection." | Generic selected requirements | Build must not enumerate all generic tuples or pick caller evidence as side effect of platform selection. | not stated | 9 |
| PROVIDER_SELECTION50 | "Specialization retains the caller's exact static selections and the selected provider identity." | Generic selected requirements | Specialization must preserve caller static selections and selected provider identity. | not stated | 7 |
| PROVIDER_SELECTION51 | "This is not a runtime machine-value lookup and introduces no unrestricted generic vtable, runtime generic engine, or generic foreign ABI." | Generic selected requirements | Must not introduce runtime generic vtable/engine/foreign ABI or machine-value lookup for this mechanism. | not stated | 7 |
| PROVIDER_SELECTION52 | "absence of local trait dispatch is not a promise of direct native call encoding." | Generic selected requirements | Must not treat absence of local trait dispatch as guaranteeing direct native call encoding. | not stated | 3 |
| PROVIDER_SELECTION53 | "The requirement operand is an exact designated product reference; target is an ordinary validated catalog reference, and the locator is ordinary compile-time data with literal byte lengths inferred." | Declaration and choice | `Build::bind` operands must meet designated-product, validated-catalog, and compile-time locator rules with inferred byte lengths. | at a call | 9 |
| PROVIDER_SELECTION54 | "Source-level `via` clauses and one-to-one bodyless realization declarations are retired." | Declaration and choice | `via` clauses and bodyless one-to-one realization declarations must not be used. | not stated | 9 |
| PROVIDER_SELECTION55 | "complete schema coverage remains mandatory." | Declaration and choice | Schema coverage must remain complete for native rows. | not stated | 9 |
| PROVIDER_SELECTION56 | "Rows keyed to the same target from different suppliers conflict and reject, and an explicit selection still outranks both." | Declaration and choice | Conflicting same-target rows from different suppliers must reject; explicit selection outranks conflicts. | not stated | 9 |
| PROVIDER_SELECTION57 | "Build-host readiness stays separate from these product rows." | Declaration and choice | Build-host readiness must be kept separate from product native rows. | not stated | 9 |
| PROVIDER_SELECTION58 | "Compiler intrinsics are selected from exact declaration, signature, and target." | Declaration and choice | Intrinsics must be selected by exact declaration, signature, and target. | not stated | 9 |
| PROVIDER_SELECTION59 | "Changing a typed foreign locator, evaluated plan, or sealed catalog entry changes artifact identity and requires fresh admission;" | Declaration and choice | Those binding changes must change artifact identity and require fresh admission. | not stated | 9 |
| PROVIDER_SELECTION60 | "Build records binding intent, not mutable trusted plans." | Declaration and choice | Build must record binding intent only, not mutable trusted plans. | not stated | 9 |
| PROVIDER_SELECTION61 | "Overrides outrank target defaults and retain authored selection identity/order." | Declaration and choice | Overrides must beat defaults and keep authored selection identity and order. | not stated | 9 |
| PROVIDER_SELECTION62 | "Missing, private, ambiguous, foreign, and duplicate candidates reject." | Declaration and choice | Those candidate defects must cause rejection. | not stated | 9 |
| PROVIDER_SELECTION63 | "A selected operation's identity remains independent of its reach row; equal service sets cannot correlate providers." | Declaration and choice | Operation identity must not depend on reach row; equal service sets must not correlate providers. | not stated | 9 |
| PROVIDER_SELECTION64 | "Selection never permits mutation of derived plans." | Declaration and choice | Derived plans must not be mutated by selection. | not stated | 9 |
| PROVIDER_SELECTION65 | "Missing, ambiguous, cyclic, cross-target, or unadmitted dependencies reject." | Declaration and choice | Transitive provider dependency defects must reject. | not stated | 9 |
| PROVIDER_SELECTION66 | "Selection does not manufacture source routing values." | Declaration and choice | Selection must not fabricate source routing values. | not stated | 9 |
| PROVIDER_SELECTION67 | "Source that needs another static role names a distinct exact requirement application; runtime multiplicity uses the protocol's ordinary data." | Declaration and choice | Extra static roles need distinct requirement applications; runtime multiplicity uses ordinary protocol data only. | not stated | 9 |
| PROVIDER_SELECTION68 | "A no-op body can contribute evidence for that independent check, not a rewritten requirement or hidden service invocation." | Declaration and choice | No-op bodies may supply exclusion evidence only; must not rewrite requirements or hide services. | not stated | 9 |
| PROVIDER_SELECTION69 | "A remaining forbidden behavior or insufficient evidence rejects the product." | Declaration and choice | After selection, forbidden behavior or insufficient exclusion evidence must reject the product. | not stated | 9 |
| PROVIDER_SELECTION70 | "Dynamic replacement must preserve the exclusions through its installation envelope; selecting one benign provider now does not certify every later binding." | Declaration and choice | Dynamic replacement must preserve exclusions; current benign selection must not certify later bindings. | not stated | 9 |
| PROVIDER_SELECTION71 | "requires the caller to name both the exact boundary requirement application and its exact named implementation:" | Build provider readiness | `await_provider` caller must name exact requirement application and exact named implementation. | at a call | 9 |
| PROVIDER_SELECTION72 | "`topology_api::TopologyBuild` must be available when its invoking continuation is checked, not necessarily before the first Build stage." | Build provider readiness | Named requirement must be available when invoking continuation is checked, not necessarily before first Build stage. | not stated | 9 |
| PROVIDER_SELECTION73 | "unresolved names cannot execute or be substituted structurally." | Build provider readiness | Unresolved names must not execute or be structurally substituted before preparation. | not stated | 9 |
| PROVIDER_SELECTION74 | "The backing operand is an inert designated selection reference until dependency preparation resolves it against that exact immutable package instance." | Build provider readiness | Backing operand must stay inert until resolved against exact immutable package instance. | not stated | 9 |
| PROVIDER_SELECTION75 | "This uses Build's designated-selection mechanism, not execution through an unresolved import." | Build provider readiness | Resolution must use designated-selection mechanism, not unresolved import execution. | not stated | 9 |
| PROVIDER_SELECTION76 | "Provider installation itself grants no general nameability, host authority or new source-level conformance evidence;" | Build provider readiness | Installation must not grant general nameability, host authority, or new conformance evidence. | not stated | 9 |
| PROVIDER_SELECTION77 | "Ordinary imports follow the separately admitted dependency scope." | Build provider readiness | Imports must follow separately admitted dependency scope only. | not stated | 9 |
| PROVIDER_SELECTION78 | "preparation checks the implementation's visibility, exact satisfaction of that requirement, and admission for the Build execution profile before installing it." | Build provider readiness | Before install, must check visibility, exact satisfaction, and Build-profile admission. | not stated | 9 |
| PROVIDER_SELECTION79 | "Matching names or signatures cannot replace nominal identity." | Build provider readiness | Names or signatures alone must not substitute for nominal identity. | not stated | 9 |
| PROVIDER_SELECTION80 | "Missing, inaccessible, mismatched, or unadmitted supply rejects rather than selecting another implementation." | Build provider readiness | Bad supply must reject; must not pick another implementation. | not stated | 9 |
| PROVIDER_SELECTION81 | "Readiness never searches for a satisfier, chooses the sole visible candidate, or consumes a dependency-selected default." | Build provider readiness | Readiness must not search, auto-pick sole visible candidate, or use dependency default. | not stated | 9 |
| PROVIDER_SELECTION82 | "An already-installed different backing does not satisfy this request or get silently replaced;" | Build provider readiness | Different installed backing must not satisfy or be silently replaced. | not stated | 9 |
| PROVIDER_SELECTION83 | "distinct static roles require distinct exact requirement applications." | Build provider readiness | Distinct static roles must use distinct exact requirement applications. | not stated | 9 |
| PROVIDER_SELECTION84 | "After installation, calls use the exact static `R::operation(arguments)` slot." | Build provider readiness | Post-install calls must use exact static requirement operation slot. | at a call | 3 |
| PROVIDER_SELECTION85 | "Readiness returns no routing value and resumes the same Terminal continuation without replay." | Build provider readiness | Readiness must not return routing values and must resume same Terminal continuation without replay. | across a pause | 9 |
| PROVIDER_SELECTION86 | "Ordinary product `Build::select_provider` rows do not select or authorize this Build-host slot." | Build provider readiness | Product `select_provider` must not select or authorize Build-host readiness slot. | not stated | 9 |
| PROVIDER_SELECTION87 | "A slot's restriction on who may select or supply candidates does not restrict its implementations to syscalls, imports, or compiler intrinsics." | Toolchain-selected checked implementations | Slot ownership restrictions must not force implementations to be syscalls/imports/intrinsics only. | not stated | 9 |
| PROVIDER_SELECTION88 | "Runtime-derived values still require their actual implementation: a process-stable clock frequency is not necessarily a compile-time constant." | Toolchain-selected checked implementations | Runtime-derived values must use real implementations; must not treat them as compile-time constants without proof. | not stated | 5 |
| PROVIDER_SELECTION89 | "Do not add a constant-result provider kind, parallel constant table, intrinsic sentinel, or requirement-name folding exemption for this case." | Toolchain-selected checked implementations | Those shortcut mechanisms must not be added for toolchain constant binding. | not stated | 9 |
| PROVIDER_SELECTION90 | "toolchain provenance does not waive conformance or coverage checks." | Toolchain-selected checked implementations | Toolchain provenance must not waive conformance or coverage checks. | not stated | 9 |
| PROVIDER_SELECTION91 | "Missing support for selecting a checked body is an implementation gap, not permission to invent an implementation or omit its selected row." | Toolchain-selected checked implementations | Missing selection support must be treated as gap; must not invent implementations or omit rows. | not stated | 9 |
| PROVIDER_SELECTION92 | "Build-time code may choose declared candidates or compute a leaf binding; it cannot append plan rows." | Derivation and retained evidence | Build-time code may pick candidates or leaf bindings but must not append plan rows. | not stated | 9 |
| PROVIDER_SELECTION93 | "Structural coincidence never supplies a satisfaction." | Derivation and retained evidence | Structural coincidence must not count as satisfaction. | not stated | 8 |
| PROVIDER_SELECTION94 | "`Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)` records a relationship for the compiler to validate and derive, not authored ProviderPlan rows." | Derivation and retained evidence | Assume-binding records relationship for validation/derivation only, not authored plan rows. | at a call | 9 |
| PROVIDER_SELECTION95 | "Complete coverage, runtime-shape and operational compatibility, exact identity, receiving policy and custody checks remain mandatory." | Derivation and retained evidence | Those checks remain mandatory for assume-binding; it is not ordinary conformance. | not stated | 9 |
| PROVIDER_SELECTION96 | "Constructing a binding or choosing a candidate performs none of the other steps." | Derivation and retained evidence | Binding construction and candidate choice must not perform derivation, admission, or slot selection steps. | not stated | 9 |
| PROVIDER_SELECTION97 | "A selected plan must cover its complete schema and apply to the target." | Derivation and retained evidence | Selected plan must have complete schema coverage and target applicability. | not stated | 9 |
| PROVIDER_SELECTION98 | "Missing, duplicate, ambiguous, and identity-colliding selections reject." | Derivation and retained evidence | Plan-set defects must reject. | not stated | 9 |
| PROVIDER_SELECTION99 | "Later consumers use that carrier rather than rediscovering declarations." | Derivation and retained evidence | Consumers must use retained canonical plan carrier, not rediscover declarations. | not stated | 9 |
| PROVIDER_SELECTION100 | "Checked-adapter dispatch rewrites only the exact selected overload row and retains its nominal machine and entry-state identity; readable method names are drift checks, not selectors." | Derivation and retained evidence | Adapter dispatch must rewrite only selected overload row; method names are drift checks only. | not stated | 9 |
| PROVIDER_SELECTION101 | "An external-binding projection retains calling/binding identity and order but grants no admission, selection, ABI, or execution authority of its own." | Derivation and retained evidence | External-binding projection must not grant admission, selection, ABI, or execution authority. | not stated | 9 |
| PROVIDER_SELECTION102 | "It is not another provider plan or a union of independently selected rows." | Derivation and retained evidence | Inherited receiver view must not be a separate plan or union of independent rows. | not stated | 9 |
| PROVIDER_SELECTION103 | "Whole descendant choice can settle automatic ancestor selection only when the complete demanded application agrees; it cannot suppress a competing authored choice." | Derivation and retained evidence | Descendant choice may auto-settle ancestor only on full application agreement; must not suppress competing authored choice. | not stated | 9 |
| PROVIDER_SELECTION104 | "Shared realizer code does not merge selecting owners." | Derivation and retained evidence | Shared realizer code must not merge selecting owners. | not stated | 9 |
| PROVIDER_SELECTION105 | "Erased runtime-carrier identities cannot establish agreement between region-bearing applications." | Derivation and retained evidence | Erased runtime-carrier identities must not prove agreement between region-bearing applications. | not stated | 2 |
| PROVIDER_SELECTION106 | "Only the matching concrete entry receipt establishes a claim for an invocation; type displays and schema promises are not that receipt." | Derivation and retained evidence | Invocation claims need concrete entry receipt; displays and schema promises must not suffice alone. | at a call | 9 |
| PROVIDER_SELECTION107 | "Every canonical overload coordinate must be covered or the whole selection rejects, naming the missing member." | Operator families and indexed requirements | Operator family override must cover every overload coordinate or reject naming missing member. | not stated | 9 |
| PROVIDER_SELECTION108 | "Adding/removing a public coordinate breaks existing family overrides; no hidden signature syntax supplies per-coordinate overrides." | Operator families and indexed requirements | Public coordinate changes break family overrides; per-coordinate hidden override syntax must not exist. | not stated | 9 |
| PROVIDER_SELECTION109 | "Final artifacts require exact closed applications, checked specialization, layout, calling plans, target admission, and role-specific realization." | Operator families and indexed requirements | Final artifacts must have closed applications, specialization, layout, calling plans, admission, and realization. | not stated | SHARED |
| PROVIDER_SELECTION110 | "Provider-authored generic assertions do not establish coverage." | Operator families and indexed requirements | Provider generic assertions must not establish coverage. | not stated | 8 |
| PROVIDER_SELECTION111 | "Universal generic coverage is fail-closed without an actual checked generic operator customer." | Operator families and indexed requirements | Universal generic coverage must fail closed without checked generic operator customer. | not stated | 7 |
| PROVIDER_SELECTION112 | "The requirement domain must imply the realization domain." | Operator families and indexed requirements | Future universal rows must have requirement domain implying realization domain. | not stated | 7 |
| PROVIDER_SELECTION113 | "Bijective binder reordering is permitted; collapsing independent binders or strengthening an unrestricted input to `[copy]` is not." | Operator families and indexed requirements | Only bijective binder reordering allowed; binder collapse and unrestricted-to-`[copy]` strengthening forbidden. | not stated | 7 |
| PROVIDER_SELECTION114 | "symbolic coverage proves only routing and dispatch, not calling policy, byte layout, register classes, or stack placement." | Operator families and indexed requirements | Symbolic universal coverage must not prove calling policy, layout, registers, or stack placement. | not stated | 7 |
| PROVIDER_SELECTION115 | "Bodyless, external, opaque, intrinsic, and separately supplied realizations remain exact-only;" | Operator families and indexed requirements | Those realization forms must stay exact-only for universal coverage. | not stated | 9 |
| PROVIDER_SELECTION116 | "final composition must reconstruct every reachable closed demand and check its realization." | Operator families and indexed requirements | Indexed requirements must reconstruct and check every reachable closed demand at composition. | not stated | 9 |
| PROVIDER_SELECTION117 | "No ambient conformance search or producer-authored total establishes the complete set." | Operator families and indexed requirements | Complete indexed sets must not come from ambient search or producer-authored totals. | not stated | 9 |
| PROVIDER_SELECTION118 | "A checked wrapper cannot erase implementation evidence, execution scope, or containment requirements; these dependencies propagate to consuming artifacts." | Executable trust and containment | Checked wrappers must not erase evidence, scope, or containment; dependencies propagate to consumers. | not stated | 9 |
| PROVIDER_SELECTION119 | "Uncontained opaque in-process code makes caller-address-space completeness false and names the responsible provider:" | Executable trust and containment | Uncontained opaque in-process code must mark caller-address-space completeness false and attribute provider. | not stated | 9 |
| PROVIDER_SELECTION120 | "A path or unresolved loader name is not executable identity." | Executable trust and containment | Paths and unresolved loader names must not count as executable identity. | not stated | 9 |
| PROVIDER_SELECTION121 | "Ordinary package policy rejects opaque code without pinned content, signer, or profile-owned platform identity." | Executable trust and containment | Opaque code without pinned content, signer, or profile platform identity must reject. | not stated | 9 |
| PROVIDER_SELECTION122 | "A process supplies no resource bound without explicit quotas." | Executable trust and containment | Process containment must not claim resource bounds without explicit quotas. | not stated | 9 |
| PROVIDER_SELECTION123 | "Platform baselines are policy allowlists, not different language semantics." | Executable trust and containment | Platform baselines must be treated as policy allowlists only. | not stated | 9 |
| PROVIDER_SELECTION124 | "Static foreign selection contributes its abstract service reach; an explicit runtime loader additionally reaches `DynamicLibraryLoading`." | Executable trust and containment | Static foreign selection adds abstract reach; runtime loader adds DynamicLibraryLoading reach. | not stated | 9 |
| PROVIDER_SELECTION125 | "The runtime executable ledger is append-only within one exact execution domain." | Executable trust and containment | Runtime executable ledger must be append-only per execution domain. | not stated | 9 |
| PROVIDER_SELECTION126 | "receipt replay rejects." | Executable trust and containment | Executable admission receipt replay must reject. | not stated | 9 |
| PROVIDER_SELECTION127 | "An enforceable dynamic-loading envelope requires containment controlling executable admission; an uncontained binary may bypass Omega's ledger." | Executable trust and containment | Dynamic-loading envelope must have admission-controlling containment or ledger may be bypassed (must enforce when required). | not stated | 9 |
| PROVIDER_SELECTION128 | "scope drift, duplicate child scopes, and mixed-scope child entries reject." | Executable trust and containment | Isolated child binding defects must reject. | not stated | 9 |
| PROVIDER_SELECTION129 | "Endpoint containment remains on the parent; child entries and completeness remain under the child scope." | Executable trust and containment | Parent owns endpoint containment; child owns entries and completeness under child scope. | not stated | 9 |
| PROVIDER_SELECTION130 | "Restart requires verified closed custody or explicit crash-safe shared resources and reset/transaction protocols, retained with isolation/restart evidence." | Executable trust and containment | Restart after crash must meet custody or crash-safe shared-resource protocols with retained evidence. | not stated | 9 |
| PROVIDER_SELECTION131 | "Otherwise an uncontained crash terminates its execution domain." | Executable trust and containment | Uncontained crash without restart preconditions must terminate execution domain. | not stated | 9 |
| PROVIDER_SELECTION132 | "Co-location, handlers, and physical isolation are installation facts, not alternate meanings of source `crashes`." | Executable trust and containment | Installation facts must not redefine source `crashes` semantics. | not stated | 10 |

## Ambiguous sentences

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "Ordinary programs should reject assumed bindings." | Programs authored for ordinary use must be rejected at compile time if they use assumed bindings. | The specification recommends that authors write programs that avoid assumed bindings; rejection is guidance not a compiler obligation. |
| "Receiving [permission policy](permissions.md) independently checks exact service/schema permissions against exercised physical mechanisms at explicit receiver admission, not as a mandatory gate on ordinary artifact production." | Permission checks never run during ordinary artifact production. | Permission checks are mandatory only at explicit receiver admission; other artifact paths may still run optional checks not described here. |
| "If a provider fails to satisfy a requirement, repair its implementation or contract, or use a checked adapter; do not substitute an assumed relationship merely to make selection succeed." | The compiler must reject selection that relies on assumed relationships instead of repair/adapter. | This is author guidance only; the compiler may still permit assumed bindings on non-ordinary paths per assumed-binding contract. |
| "The build, test harness, or component manager owning a service slot may override them with an admitted provider." | Only listed owners may override defaults; other Build callers may not. | Any Build code running in those contexts may override when it owns the slot; "owning" is defined only by those subsystems' rules elsewhere. |
| "Conformances and exact Build-owned native bindings declare candidate intent; the compiler derives ProviderPlan; configuration selects it." | Selection happens only via configuration after plan derivation, not at conformance declaration time. | Conformances declare intent but configuration might be implicit default when only one candidate exists. |
| "Checked providers must fit that row, while imported or assumed implementations retain their trust and physical-authority findings independently of reach." | Reach row compatibility is enforced only on checked providers at selection time. | Imported/assumed implementations must also match reach for invocation even though trust findings stay separate. |
| "The interpreter verifies the activation's identity, not merely the argument's type." | Build interpreter must compare activation identity (e.g. object identity) for select_provider. | "Activation identity" means the same lexical Build activation in the build script, verified structurally not by reference equality. |
| "Distinct authored configurations use distinct nominal/static identities." | Each distinct select_provider call site configuration gets a unique static identity. | Each distinct combination of slot+provider+composition mode gets an identity; duplicate spellings at different sites might share or differ—unspecified. |
| "An exact call to a realization machine delegates directly; spelling its operator inside the provider redispatches and can recurse." | Compiler must inline/delegate direct realization calls without provider indirection. | Authors must avoid spelling realization operators inside providers; compiler behavior on recursion is rejection vs stack overflow unspecified. |
| "A body reached by no row or entry remains source-checked without speculative target geometry;" | Unselected bodies skip target layout until selected. | Bodies with no row still get partial target checks; only layout geometry is deferred. |
| "Build selects the realizing implementation for the target; a static caller supplies the requirement's generic arguments, including the exact evidence map." | Platform selection must not override caller-supplied generic evidence map. | Build picks implementation body while caller supplies evidence; if they conflict, reject vs caller-wins not stated in this sentence. |
| "A native row keyed to the selected target is that target's authored supply for its slot: checked implementations in scope, including those other targets' rows select, stay checked but do not compete with it." | Other targets' selected implementations are checked but excluded from competing on this target's row. | In-scope implementations from other targets' rows are non-candidates only at selection; they might still affect validation reports. |
| "Provider planning closes those rows under target selection and retains their requirement, implementation, target, reach, authority, trust, replacement, and lifecycle identities." | Closure is computed per target row after selection. | Closure is global across targets with per-row identities retained; cross-target closure interaction not fully specified here. |
| "`topology_api::TopologyBuild` must be available when its invoking continuation is checked, not necessarily before the first Build stage." | Name resolution for requirement may be deferred until continuation typecheck. | Requirement symbol must exist in environment when continuation is checked, which might be after first Build stage executes. |
| "Explicit dependency preparation admits ordinary shared non-core interfaces before that continuation executes;" | Preparation may run before continuation and admit shared interfaces early. | "Before continuation executes" means before runtime of continuation, not before its static check. |
| "Matching names or signatures cannot replace nominal identity." | Two implementations with same name/signature but different nominal identity must not be interchangeable for readiness. | Name/signature match might warn but nominal identity still required for acceptance. |
| "Readiness never searches for a satisfier, chooses the sole visible candidate, or consumes a dependency-selected default." | No implicit default or search even when only one implementation is exported. | Search is forbidden for readiness only; ordinary select_provider might still have defaults elsewhere. |
| "Removing the call is an ordinary optimization, not a prerequisite for a valid binding." | Constant folding may remove calls without invalidating toolchain constant providers. | Valid binding requires retaining the call unless optimization proves equivalence; removal optional not guaranteed. |
| "Derivation, structural validation, semantic admission under boundary-grant authority, and slot selection under the slot owner's capability are distinct steps." | Compiler must implement these as separate phases that cannot be collapsed. | Conceptual separation only; implementation may interleave if observable outcomes match. |
| "Whole descendant choice can settle automatic ancestor selection only when the complete demanded application agrees; it cannot suppress a competing authored choice." | Descendant can auto-select ancestor only if applications are identical; authored ancestor choice always wins over auto. | Auto ancestor selection applies when applications agree; competing authored choice at ancestor blocks auto-settle only when applications disagree. |
| "Universal generic coverage is fail-closed without an actual checked generic operator customer." | No universal coverage row exists until a concrete checked customer exists. | Fail-closed applies to admitting coverage claims; provisional rows might exist but cannot be used. |
| "Target-dependent routing or admissions may qualify the row, but symbolic coverage proves only routing and dispatch, not calling policy, byte layout, register classes, or stack placement." | Qualified rows still need separate checks for layout and calling policy. | Symbolic coverage might suffice for routing on some targets when qualifications are absent—unspecified. |
| "An indexed requirement such as `ResidentContentTransfer<P,T>` is one schema selected through one binding, not an ambient slot per monomorph." | Each monomorph does not get its own selection slot by default. | One binding covers the schema family; per-monomorph obligations are refined inside one provider, not separate slots. |
| "Profiles evaluate exact identity, platform-baseline policy, evidence, scope completeness, and memory/termination/fault/resource containment. They may permit and mark, or reject before installation." | Profile evaluation is mandatory before installation when opaque executables are selected. | Profiles are optional policy layers; default behavior without named profile not specified in this sentence. |
| "Without separate executable-closure evidence it adds a known entry and attributed incompleteness." | Ledger union without closure evidence must record incompleteness. | Incompleteness is diagnostic only; admission might still proceed with warning. |
| "With that evidence it may preserve a complete static scope, but cannot erase unrelated causes." | Closure evidence can restore completeness for static scope only for attributed causes. | "Unrelated causes" means other incompleteness sources stay reported; whether completeness boolean becomes true is partial. |
| "Crash causes, routes, and abandonment-frontier lower bounds support deployment checks, not safe continuation." | Compiler must not use crash metadata to justify continuing after crash without restart protocol. | Deployment checks may use crash metadata; runtime may still continue if platform allows—outside language guarantee. |

## Terms used without definition in this chapter

- Target slots
- Boundary realization
- Coverage evidence
- Assumed binding contract
- Provider rows
- Runtime permission
- Semantic refinement
- Permission policy
- Service slot
- Default provider implementations
- Admitted provider
- Candidate intent
- ProviderPlan
- Boundary operator
- Primitive-provider registry
- Test groups
- Runner roots
- Root-delegated authority
- Virtual filesystem backing
- Host access
- Checked satisfiers
- Token-bearing boundary operation
- Machine requirement
- Boundary trait
- Compiler-catalog semantic primitives
- Canonical float arithmetic
- Service reach
- `reaches none`
- Deferred requirement reach
- Token binder
- Conformance evidence
- Proof-static implementation binder
- Canonical direct family
- Package-qualified same-path boundary-operator family
- Named provider implementation
- Realizing machine
- CompositionMode::Independent
- Runtime-variable descriptors
- Target-catalog reference
- Nominal/static identities
- Facade
- Realization machine
- Target-prefixed machine declarations
- Build row
- Same-path target-body family
- Target-sensitive source overload
- Body-selection pass
- Foreign locator
- Layout demand
- Generic machine requirements
- Evidence map
- Generic vtable
- Runtime generic engine
- Generic foreign ABI
- Indirect-entry
- Transport mechanism
- Foreign binding
- Calling-policy application
- Designated product reference
- Validated catalog reference
- Native rows
- Schema coverage
- Build-host readiness
- Compiler intrinsics
- Binding producer closure
- Final admission
- Sealed catalog entry
- Stable native-provider identity
- Transitive provider dependencies
- Raw-family dependencies
- Source routing values
- Behavior exclusions
- Installation envelope
- explicit-build-provider-backing
- staged-build-capability-attenuation
- Dependency preparation
- Designated selection reference
- Build execution profile
- Terminal continuation
- toolchain-settled-constant-binding
- Toolchain-owned slot
- Constant-result provider kind
- Nominal provider conformance closure
- Effect summaries
- Composite adapters
- Plan-shaped call sequences
- Leaf binding
- Structural coincidence
- ProviderPlan rows
- Boundary-grant authority
- Canonical fact set
- Checked-adapter dispatch
- Entry-state identity
- Drift checks
- External-binding projection
- Inherited receiver view
- Phantom arguments
- Requirement roster
- Region-bearing applications
- Linear routed-parameter claims
- Entry receipt
- Operator override
- Canonical overload coordinate
- Coordinate-identity order
- Static application
- Checked specialization
- Role-specific realization
- Pristine pre-monomorphization graph
- Symbolic telescope
- Requirement coordinate
- Realization template
- Foreign universal contract
- Indexed requirement
- Ambient conformance search
- Producer-authored total
- Opaque representations
- Independent components
- Executable manifest
- Caller-address-space completeness
- Platform-baseline policy
- Scope completeness
- Executable identity
- Profile-owned platform identity
- Executable/artifact identity
- Composite guarantee
- Platform baselines
- DynamicLibraryLoading
- Runtime executable ledger
- Omega-mediated admission
- Executable-closure evidence
- Dynamic-loading envelope
- Isolated provider
- Parent endpoint
- Child manifest
- Isolated scope
- Abandonment-frontier lower bounds
- Verified closed custody
- Crash-safe shared resources

CHAPTER COMPLETE
