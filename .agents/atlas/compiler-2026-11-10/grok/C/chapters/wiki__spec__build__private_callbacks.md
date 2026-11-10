# Private callback realization

Source: `wiki/spec/build/private_callbacks.md` lines 1–154 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| PRIVATE_CALLBACKS1 | "A callback binder selects a nominal machine satisfying one exact requirement." | Private callback realization | The binder must name one nominal machine that satisfies exactly one callback requirement | not stated | 3 |
| PRIVATE_CALLBACKS2 | "Structural signature coincidence does not select it." | Private callback realization | Machine selection must not rely on structural signature coincidence alone | not stated | 3 |
| PRIVATE_CALLBACKS3 | "The callback ABI address and its private destination are realization results, not source projections of the callback slot." | Private callback realization | ABI address and private destination must come from realization, not from source-visible slot projection | not stated | 9 |
| PRIVATE_CALLBACKS4 | "Explicit [machine address exposure](../language/counts_and_addresses.md#address-exposure) is a separate inert observation: it does not expose the slot, supply a callback requirement, or replace its selected ABI adapter and registration contract." | Private callback realization | Address-exposure observations must not expose the slot, supply a requirement, or substitute for ABI adapter or registration contract | not stated | 9 |
| PRIVATE_CALLBACKS5 | "That signature-free path must select one exact overload at the binder declaration;" | Authored selection | The signature-free binder path must fix one exact requirement overload at the declaration site | not stated | 7 |
| PRIVATE_CALLBACKS6 | "signature coincidence or a uniquely visible machine cannot choose it later." | Authored selection | Later selection by signature coincidence or unique visibility must not change the bound overload or machine | not stated | 7 |
| PRIVATE_CALLBACKS7 | "Adding an overload that makes such a reference ambiguous is a compatibility break, diagnosed at the declaring trait and affected uses." | Authored selection | New overloads that make an existing binder reference ambiguous must be diagnosed at the declaring trait and uses as a compatibility break | not stated | 7 |
| PRIVATE_CALLBACKS8 | "No callback-specific expanded-signature or `as Name` overload-selector form exists." | Authored selection | Callback overload selection must not use expanded-signature or `as Name` selector forms | not stated | 7 |
| PRIVATE_CALLBACKS9 | "A direct destination is written `native callback name from Selected` at its actual native argument position." | Authored selection | Direct destinations must use that syntax at the correct native argument position | not stated | 9 |
| PRIVATE_CALLBACKS10 | "It has no source runtime argument." | Authored selection | Direct callback destinations must not introduce a source runtime formal | not stated | 3 |
| PRIVATE_CALLBACKS11 | "A nested destination uses the explicitly cited `PrivateCallbackSlot<Layout, Requirement>` implementation whose `Layout` argument is its owning layout policy." | Authored selection | Nested destinations must cite an explicit `PrivateCallbackSlot<Layout, Requirement>` with layout policy owning the `Layout` argument | not stated | 9 |
| PRIVATE_CALLBACKS12 | "The [calling plan](calling_plans.md) places these declared demands; it does not invent hidden trailing parameters." | Authored selection | Calling plan may place authored callback demands only; it must not add hidden trailing parameters | not stated | 3 |
| PRIVATE_CALLBACKS13 | "The binder receives its signature, contracts, operational ceilings, and entry plan from the requirement." | Authored selection | Binder metadata must be taken from the requirement, not authored separately | not stated | 3 |
| PRIVATE_CALLBACKS14 | "Per-instance state uses the protocol's context parameter, a checked generational handle recoverable from callback arguments, or package-owned stable state." | Authored selection | Per-instance state must use only those three mechanisms | not stated | 3 |
| PRIVATE_CALLBACKS15 | "No implicit closure environment rides on a function pointer." | Authored selection | Function pointers must not carry implicit closure environments | not stated | 3 |
| PRIVATE_CALLBACKS16 | "Raw addresses and pointers remain inert representation tokens, not readable/writable views." | Authored selection | Raw addresses and pointers must stay inert tokens without read/write view authority | not stated | 2 |
| PRIVATE_CALLBACKS17 | "Callback use" (Record column: "Registration call/operation, static-machine argument ordinal, selected machine and satisfaction row, exact canonical requirement overload, and target entry recipe.") | Required identities | Callback-use records must bind all listed identities | not stated | 3 |
| PRIVATE_CALLBACKS18 | "Registrar materialization" (Record column: "Exact binder slot and one declared native parameter or private layout-field destination, independently of the later callback selection.") | Required identities | Registrar materialization must bind binder slot and one declared destination independent of later selection | not stated | 9 |
| PRIVATE_CALLBACKS19 | "Callback root" (Record column: "Exact canonical entry, activation-local flow/dispatch/storage/frame identities, validated inbound entry plan, and internal argument/result bridge.") | Required identities | Callback-root records must bind all listed identities | not stated | 3 |
| PRIVATE_CALLBACKS20 | "Retain the requirement's published envelope separately from the selected machine's actual envelope and the checked actual-refines-published judgment." | Required identities | Published envelope, actual envelope, and actual-refines-published judgment must be kept distinct | not stated | 8 |
| PRIVATE_CALLBACKS21 | "The actual envelope is not an ambient caller fact." | Required identities | Actual envelope must not become ambient caller proof | not stated | 8 |
| PRIVATE_CALLBACKS22 | "Registration provenance retains selected identity and lease disposition without itself granting a lease." | Required identities | Provenance must record identity and lease disposition but must not grant a lease by itself | not stated | 2 |
| PRIVATE_CALLBACKS23 | "Terminal Psi remains target-neutral." | Required identities | Terminal Psi for callbacks must stay target-neutral | not stated | SHARED |
| PRIVATE_CALLBACKS24 | "Target-specific recipes and placement evidence travel beside it as build/native realization inputs, joined to its exact boundary operation." | Required identities | Target recipes and placement evidence must be realization inputs attached to the boundary operation, not inside portable Psi | not stated | 9 |
| PRIVATE_CALLBACKS25 | "A source-free artifact cannot reconstruct an authored callback selection merely from that operation." | Required identities | Reconstruction of authored selection from a source-free operation alone must be impossible | not stated | 9 |
| PRIVATE_CALLBACKS26 | "A direct callback destination is an interleaved native-only parameter on the registrar requirement." | Declared native destinations | Direct destinations must be native-only parameters interleaved on the registrar requirement | not stated | 3 |
| PRIVATE_CALLBACKS27 | "It contributes no semantic runtime formal, Terminal operand, or address value." | Declared native destinations | Direct callback entries must not add semantic formals, Terminal operands, or address values | not stated | 3 |
| PRIVATE_CALLBACKS28 | "One ordered native telescope contains both these entries and entries originating in ordinary semantic formals." | Declared native destinations | Callback and ordinary semantic entries must share one ordered native telescope | not stated | 3 |
| PRIVATE_CALLBACKS29 | "Each entry has a compiler-issued nominal identity;" | Declared native destinations | Every telescope entry must have compiler-issued nominal identity | not stated | 3 |
| PRIVATE_CALLBACKS30 | "a callback entry also retains its exact binder/requirement source and target-closed function-pointer shape." | Declared native destinations | Callback entries must retain binder/requirement source and target-closed function-pointer shape | not stated | 9 |
| PRIVATE_CALLBACKS31 | "`NativePlace::Parameter` and the root parameter of `NativePlace::Field` use that same identity space." | Declared native destinations | Parameter and field-root native places must share the parameter identity space | not stated | 3 |
| PRIVATE_CALLBACKS32 | "Calling policy places declared entries; it cannot create, reorder, retarget, or infer a trailing callback argument." | Declared native destinations | Calling policy may place declared entries only; it must not create, reorder, retarget, or infer trailing callback arguments | not stated | 3 |
| PRIVATE_CALLBACKS33 | "A nested private field destination originates in an explicitly selected named `PrivateCallbackSlot<Layout, Requirement>` implementation cited by the plan of its `Layout` policy;" | Declared native destinations | Nested field destinations must originate from explicitly selected cited `PrivateCallbackSlot` in the owning layout policy's plan | not stated | 9 |
| PRIVATE_CALLBACKS34 | "another policy cannot place it." | Declared native destinations | A layout policy other than the owner must not place the nested destination | not stated | 9 |
| PRIVATE_CALLBACKS35 | "An uncited declaration is inert." | Declared native destinations | Uncited private slot declarations must have no effect | not stated | 9 |
| PRIVATE_CALLBACKS36 | "Retain conformance-owned slot identity, exact target-neutral requirement overload, target-closed placement, and the complete layout-plan identity." | Declared native destinations | Must retain slot identity, requirement overload, placement, and full layout-plan identity together | not stated | 9 |
| PRIVATE_CALLBACKS37 | "A compact fingerprint alone is not that identity." | Declared native destinations | Compact fingerprints must not substitute for retained slot/plan identity | not stated | 9 |
| PRIVATE_CALLBACKS38 | "An authored byte offset is placement data, neither slot identity nor a calling-plan coordinate." | Declared native destinations | Byte offsets count as placement data only, not slot identity or calling-plan coordinates | not stated | 9 |
| PRIVATE_CALLBACKS39 | "Private slots are not ordinary source-visible fields or addresses: they have no source projection, read/write, serialization, or runtime topology." | Declared native destinations | Private slots must lack source projection, access, serialization, and runtime topology | not stated | 1 |
| PRIVATE_CALLBACKS40 | "The cited conformance binds the layout owner;" | Declared native destinations | Cited conformance must bind the layout owner for the slot | not stated | 7 |
| PRIVATE_CALLBACKS41 | "visible conformance enumeration and layout ownership do not grant an exception." | Declared native destinations | Visible conformance lists and ownership alone must not authorize private slot placement without citation | not stated | 7 |
| PRIVATE_CALLBACKS42 | "Every private demand must be supplied exactly once before layout is complete, including for an `Access` view." | Declared native destinations | Each private demand must be supplied once before layout completes, including `Access` views | not stated | 9 |
| PRIVATE_CALLBACKS43 | "Physical offset ownership remains in layout, not the binding or calling plan." | Declared native destinations | Physical offsets must be owned by layout, not binding or calling plan | not stated | 9 |
| PRIVATE_CALLBACKS44 | "[Calling-plan identity](calling_plans.md#identity-and-selection) distinguishes the reusable physical recipe from its nominal boundary application." | Plan and occurrence validation | Compiler must keep physical recipe identity separate from boundary application identity | not stated | 3 |
| PRIVATE_CALLBACKS45 | "Callback materializations participate in the latter;" | Plan and occurrence validation | Callback materializations must participate in boundary application identity, not merely physical recipe | not stated | 3 |
| PRIVATE_CALLBACKS46 | "equal register layouts cannot hide a reordered telescope." | Plan and occurrence validation | Equal register layouts must not mask telescope reordering across materializations | not stated | 3 |
| PRIVATE_CALLBACKS47 | "Keep the callback's inbound plan separate from the registrar's outbound plan." | Plan and occurrence validation | Inbound callback plan and outbound registrar plan must remain separate | not stated | 3 |
| PRIVATE_CALLBACKS48 | "Join each binder/requirement/destination to exactly one private function and callback-root schedule, preserving order." | Plan and occurrence validation | Each binder/requirement/destination must map to exactly one private function and root schedule in order | not stated | 3 |
| PRIVATE_CALLBACKS49 | "Destination replay checks selected target/layout, slot and data-symbol identity, offset, pointer extent, alignment, and containing storage." | Plan and occurrence validation | Replay must verify target/layout, identities, offset, extent, alignment, and storage | not stated | 9 |
| PRIVATE_CALLBACKS50 | "Missing, duplicate, reordered, overlapping, shape-incompatible, unresolved, or substituted materializations reject." | Plan and occurrence validation | Those materialization defects must cause rejection | not stated | 3 |
| PRIVATE_CALLBACKS51 | "Materialization uses only a fully validated outbound plan." | Plan and occurrence validation | Materialization must require a fully validated outbound plan first | not stated | 3 |
| PRIVATE_CALLBACKS52 | "The private function's artifact-local machine identity stays nested in its source artifact; it cannot impersonate a machine in the enclosing program." | Plan and occurrence validation | Private function machine identity must stay artifact-local and must not impersonate enclosing-program machines | not stated | 9 |
| PRIVATE_CALLBACKS53 | "Object and final-image replay bind the private symbol, relocation, executable region, and patched address to the same function and native argument destination." | Plan and occurrence validation | Image replay must tie symbol, relocation, region, and patched address to the same function and native destination | not stated | 9 |
| PRIVATE_CALLBACKS54 | "Independent receiving validation needs both authenticated plan-application identity and replayable authored-use-to-Terminal-operation correspondence." | Plan and occurrence validation | Receiving validation must have both authenticated plan-application identity and replayable use-to-Terminal mapping | not stated | 8 |
| PRIVATE_CALLBACKS55 | "A retained producer digest or placement index alone does not establish either." | Plan and occurrence validation | Producer digest or placement index alone must not satisfy receiving validation | not stated | 8 |
| PRIVATE_CALLBACKS56 | "Emission and image replay establish code and placement, not successful foreign registration, invocation permission, installed-address lifetime, capacity, external-root ownership, or component publication." | Registration and lifetime | Emission and replay must not be treated as establishing registration success, invocation rights, lifetimes, capacity, root ownership, or publication | not stated | 9 |
| PRIVATE_CALLBACKS57 | "Those require the actual registrar outcome and the separate root/lease protocol." | Registration and lifetime | Registration-related facts require registrar outcome and root/lease protocol, not emission alone | not stated | 3 |
| PRIVATE_CALLBACKS58 | "Registration capacity counts live registrations, not emitted thunks;" | Registration and lifetime | Capacity must count live registrations only, not emitted thunks | not stated | 3 |
| PRIVATE_CALLBACKS59 | "unregister and required quiescence precede release of code/component leases." | Registration and lifetime | Unregister and required quiescence must occur before releasing code or component leases | not stated | 2 |
| PRIVATE_CALLBACKS60 | "The registrar is an ordinary runtime boundary operation." | Registration and lifetime | Registrar must be modeled as ordinary runtime boundary operation | at a call | 3 |
| PRIVATE_CALLBACKS61 | "Build selects its realization and resource profile, not successful registration." | Registration and lifetime | Build must select realization and resources only, not predetermine registration success | not stated | 9 |
| PRIVATE_CALLBACKS62 | "Success creates the admitted future root and moves the exact live-registration capacity into the linear registration." | Registration and lifetime | Successful registration must create admitted future root and move exact capacity into linear registration | at normal return | 2 |
| PRIVATE_CALLBACKS63 | "Failure creates no root and preserves that authority;" | Registration and lifetime | Failed registration must create no root and preserve registration authority | at normal return | 2 |
| PRIVATE_CALLBACKS64 | "successful teardown returns the same capacity occurrence." | Registration and lifetime | Successful teardown must return the same capacity occurrence | at normal return | 2 |
| PRIVATE_CALLBACKS65 | "Call-scoped borrows create no durable registration." | Registration and lifetime | Call-scoped borrows must not create durable registration | not stated | 2 |
| PRIVATE_CALLBACKS66 | "A consumable lifetime budget is a different resource." | Registration and lifetime | Lifetime budget must be distinct from registration capacity | not stated | 2 |
| PRIVATE_CALLBACKS67 | "Materialization retains binder and destination, not a duplicate lifetime field." | Registration and lifetime | Materialization must retain binder and destination without duplicating lifetime fields | not stated | 9 |
| PRIVATE_CALLBACKS68 | "The native parameter's ordinary custody contract determines whether its storage is call-scoped or retained." | Registration and lifetime | Storage scope must follow the native parameter custody contract | not stated | 2 |
| PRIVATE_CALLBACKS69 | "Foreign internal tables are provider state; retained caller storage needs the [general foreign-retention contract](foreign_storage.md)." | Registration and lifetime | Provider tables are provider state; retained caller storage must meet foreign-retention contract | not stated | 2 |
| PRIVATE_CALLBACKS70 | "`invokes` describes possible synchronous entry before the registrar returns;" | Registration and lifetime | `invokes` must document possible synchronous entry before registrar return | not stated | 3 |
| PRIVATE_CALLBACKS71 | "it is separate from creating a future root." | Registration and lifetime | `invokes` facts must be separate from future-root creation | not stated | 3 |
| PRIVATE_CALLBACKS72 | "Bodyful machines infer direct invocations; bodyless requirements declare them." | Registration and lifetime | Direct invocations must be inferred for bodyful machines and explicitly declared for bodyless requirements | not stated | 10 |
| PRIVATE_CALLBACKS73 | "Omitted `invokes` on a bodyless requirement means no synchronous invocation." | Registration and lifetime | Missing `invokes` on bodyless requirements must mean no synchronous invocation | not stated | 3 |
| PRIVATE_CALLBACKS74 | "The handler's service and selected operational envelope contribute to current-invocation reach." | Registration and lifetime | Current-invocation reach must include handler service and operational envelope | not stated | 3 |
| PRIVATE_CALLBACKS75 | "The direct synchronous invocation graph must be acyclic;" | Registration and lifetime | Direct synchronous invocation graph must be acyclic | at a call | 4 |
| PRIVATE_CALLBACKS76 | "inserting another synchronous trait does not break a cycle." | Registration and lifetime | Adding another synchronous trait must not be treated as breaking an invocation cycle | not stated | 4 |
| PRIVATE_CALLBACKS77 | "A genuine new-activation boundary does." | Registration and lifetime | Genuine new-activation boundaries must break cycles for synchronous invocation analysis | not stated | 4 |
| PRIVATE_CALLBACKS78 | "A package may handle restricted synchronous queries and queue ordinary application events without inferring an opaque provider's internal call graph." | Registration and lifetime | Packages may queue events and handle restricted queries without inferring opaque internal call graphs | not stated | 3 |
| PRIVATE_CALLBACKS79 | "Opaque native re-entry remains trust-relative." | Registration and lifetime | Opaque native re-entry must be treated as trust-relative, not structurally disproved | not stated | 3 |
| PRIVATE_CALLBACKS80 | "A binding may enforce an active or depth limit for one call chain only when its protocol supplies a valid unavailable result." | Registration and lifetime | Active or depth limits on one chain require protocol support for valid unavailable results | at a call | 3 |
| PRIVATE_CALLBACKS81 | "Otherwise finite mixed-chain admission requires a checked provider contract or structural isolation." | Registration and lifetime | Without that protocol, finite mixed-chain admission needs checked provider contract or structural isolation | not stated | 8 |
| PRIVATE_CALLBACKS82 | "A native declaration alone does not prove non-re-entry." | Registration and lifetime | Native declarations alone must not prove absence of re-entry | not stated | 3 |
| PRIVATE_CALLBACKS83 | "The registration retains the selected concrete envelope but does not import it automatically into the caller's proof context." | Registration and lifetime | Registration keeps concrete envelope but must not auto-import it into caller proof | not stated | 8 |
| PRIVATE_CALLBACKS84 | "An API exposing those facts must forward them in its contract." | Registration and lifetime | APIs exposing envelope facts must declare them in their contracts | not stated | 8 |
| PRIVATE_CALLBACKS85 | "Compact provider-execution coordinates remain reports." | Registration and lifetime | Compact provider-execution coordinates must be reports only, not authority | not stated | SHARED |
| PRIVATE_CALLBACKS86 | "Admission retains the selected provider authority, strong closure identity, and exact requirement;" | Registration and lifetime | Admission must retain provider authority, strong closure identity, and exact requirement | not stated | 3 |
| PRIVATE_CALLBACKS87 | "equal compact coordinates cannot authorize requirement substitution." | Registration and lifetime | Equal compact coordinates must not permit requirement substitution | not stated | 8 |
| PRIVATE_CALLBACKS88 | "Unsupported callback forms reject;" | Registration and lifetime | Unsupported callback forms must be rejected | not stated | 3 |
| PRIVATE_CALLBACKS89 | "carrying an opaque companion intact is not interpreting or admitting it." | Registration and lifetime | Intact opaque companions must not count as interpretation or admission | not stated | 3 |
| PRIVATE_CALLBACKS90 | "A registrar's static machine binder declares `requires machine Selected satisfies CallbackRequirement::call`." | Authored selection | Registrar static machine binders must declare `requires machine Selected satisfies CallbackRequirement::call` | not stated | 7 |
| PRIVATE_CALLBACKS91 | "[Installed roots](external_roots.md) and [entry stacks](../resources/entry_stacks.md) govern later entry and resource admission." | Registration and lifetime | Later entry and resource admission must follow installed-roots and entry-stack rules | not stated | 3 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "A callback binder selects a nominal machine satisfying one exact requirement." | Selection is static at binder declaration and names one machine. | "Nominal machine" may allow symbolic identity resolved only at realization if still one requirement overload. |
| "Structural signature coincidence does not select it." | No fallback selection when machines share structure but differ in requirement binding. | Structural coincidence may inform diagnostics but must never be the selection rule. |
| "The callback ABI address and its private destination are realization results, not source projections of the callback slot." | Source cannot name or read ABI address or destination as ordinary values. | They exist in artifacts only; whether intermediate IR may reference them symbolically before emission is not stated. |
| "Explicit machine address exposure is a separate inert observation" | Address exposure is a distinct compile-time or runtime report with no callback side effects. | "Inert" limits only mutation authority; exposure might still affect proof if another chapter says so. |
| "That signature-free path must select one exact overload at the binder declaration;" | Overload is fixed where `requires machine Selected satisfies CallbackRequirement::call` appears. | "Signature-free" might mean no expanded signature in source while overload still comes from requirement name alone. |
| "Adding an overload that makes such a reference ambiguous is a compatibility break, diagnosed at the declaring trait and affected uses." | Ambiguity is a hard error on both trait definition and downstream uses. | Compatibility break might be warning-on-upgrade for some uses and error for others; severity is not stated here. |
| "Per-instance state uses the protocol's context parameter, a checked generational handle recoverable from callback arguments, or package-owned stable state." | Exactly one of the three mechanisms per callback protocol instance. | Any combination of the three is allowed if each piece of state maps to one listed carrier. |
| "The actual envelope is not an ambient caller fact." | Callers cannot assume actual envelope without explicit contract forward. | Actual envelope may appear in registration objects but not in default proof environment. |
| "Registration provenance retains selected identity and lease disposition without itself granting a lease." | Provenance records lease disposition only; lease still requires protocol steps. | Provenance might grant disposition metadata usable by later lease operations without being a lease itself. |
| "A source-free artifact cannot reconstruct an authored callback selection merely from that operation." | Reverse engineering selection from Terminal operation boundary alone must fail. | Applies only to artifacts with no auxiliary realization inputs; digests beside Psi might still reconstruct. |
| "Calling policy places declared entries; it cannot create, reorder, retarget, or infer a trailing callback argument." | Policy placement is order-preserving and limited to authored entries. | "Infer" forbids synthesis; whether policy may reject incompatible declaration order is not stated. |
| "An uncited declaration is inert." | Declarations without plan citation have zero effect on layout or calling. | They might still parse and type-check but emit no demands. |
| "Every private demand must be supplied exactly once before layout is complete, including for an `Access` view." | Layout validation fails unless each demand has one supplier, even for access-only layout views. | "Supplied" might mean authored in source vs satisfied by conformance; timing relative to layout phases is not split here. |
| "equal register layouts cannot hide a reordered telescope." | Application identity must change when telescope order changes despite same registers. | Register layout equality is insufficient for fingerprint equality of callback materializations. |
| "Join each binder/requirement/destination to exactly one private function and callback-root schedule, preserving order." | Bijective mapping with order preserved across join. | Multiple destinations might share one function if schedule distinguishes them; "exactly one" might forbid sharing. |
| "Materialization uses only a fully validated outbound plan." | Outbound plan must pass all calling-plan validation before any callback materialization step. | Materialization reads outbound plan fields only after validation flag is set; partial validation is forbidden. |
| "The private function's artifact-local machine identity stays nested in its source artifact; it cannot impersonate a machine in the enclosing program." | Private thunks keep artifact-local IDs and cannot be invoked as enclosing-program machines by ID collision. | Impersonation might mean ABI or symbol collision at link time rather than identity tables. |
| "Emission and image replay establish code and placement, not successful foreign registration, invocation permission, installed-address lifetime, capacity, external-root ownership, or component publication." | Those runtime facts require registrar execution and separate protocols. | Emission might still emit static capacity slots without counting as establishing live capacity. |
| "Build selects its realization and resource profile, not successful registration." | Build cannot encode registration success in artifacts. | Resource profile might include capacity limits without predicting runtime registration outcome. |
| "Success creates the admitted future root and moves the exact live-registration capacity into the linear registration." | Capacity is a linear token consumed on success into the registration object. | "Moves" might be logical ownership transfer while numeric capacity counter decrements separately. |
| "Call-scoped borrows create no durable registration." | Temporary borrows during registrar call do not register callbacks permanently. | Borrows might still register if protocol says so; this sentence forbids durable effect only. |
| "Bodyful machines infer direct invocations; bodyless requirements declare them." | Inference applies only where machine has body; bodyless must spell `invokes`. | "Infer" might mean compiler derives graph from body calls vs requiring explicit edge list. |
| "inserting another synchronous trait does not break a cycle." | Trait insertion alone cannot certify acyclicity. | New trait might add synchronous path; cycle detection must still flag cycles despite trait boundaries. |
| "A genuine new-activation boundary does." | New activation breaks synchronous cycle for analysis purposes. | "Break" might mean splits graph for ranking only, not permit otherwise cyclic direct calls. |
| "Opaque native re-entry remains trust-relative." | Compiler does not disprove re-entry for opaque natives without contract. | Trust-relative might mean assume provider honesty unless structural isolation proved. |
| "A binding may enforce an active or depth limit for one call chain only when its protocol supplies a valid unavailable result." | Limits require protocol-defined unavailable outcomes on exceed. | "One call chain" might mean single registrar invocation stack vs entire program thread. |
| "Otherwise finite mixed-chain admission requires a checked provider contract or structural isolation." | Without unavailable results, admission needs contract proof or isolation proof. | "Mixed-chain" scope and what counts as structural isolation are not defined in this chapter. |
| "The registration retains the selected concrete envelope but does not import it automatically into the caller's proof context." | Envelope is stored on registration handle; caller proofs need explicit import. | Retention might be runtime-only with no checked import path unless API forwards. |
| "equal compact coordinates cannot authorize requirement substitution." | Report coordinates with equal compact values must not swap requirements. | Compact coordinate equality is insufficient for admission even if digests differ elsewhere. |
| "Unsupported callback forms reject;" | Compiler must reject unsupported forms at compile time. | "Unsupported" might mean specific enumerated forms; whether link-time rejection exists is not stated. |
| "carrying an opaque companion intact is not interpreting or admitting it." | Presence of opaque bytes does not admit callback. | Intact carry might still affect layout size without semantic admission. |

## Terms used without definition in this chapter

- callback binder
- nominal machine
- exact requirement
- structural signature coincidence
- callback ABI address
- private destination
- callback slot
- source projection
- machine address exposure
- ABI adapter
- registration contract
- registrar
- static machine binder
- `CallbackRequirement::call`
- signature-free path
- exact overload
- uniquely visible machine
- compatibility break
- expanded-signature overload-selector
- `as Name` overload-selector
- direct destination
- `native callback name from Selected`
- native argument position
- nested destination
- `PrivateCallbackSlot<Layout, Requirement>`
- layout policy
- calling plan
- hidden trailing parameters
- operational ceilings
- entry plan
- context parameter
- checked generational handle
- package-owned stable state
- implicit closure environment
- function pointer
- inert representation tokens
- Callback use (record)
- Registration call/operation
- static-machine argument ordinal
- satisfaction row
- canonical requirement overload
- target entry recipe
- Registrar materialization
- binder slot
- private layout-field destination
- Callback root
- activation-local flow/dispatch/storage/frame identities
- inbound entry plan
- argument/result bridge
- published envelope
- actual envelope
- actual-refines-published judgment
- ambient caller fact
- Registration provenance
- lease disposition
- Terminal Psi
- target-specific recipes
- placement evidence
- build/native realization inputs
- boundary operation
- source-free artifact
- authored callback selection
- interleaved native-only parameter
- semantic runtime formal
- Terminal operand
- ordered native telescope
- compiler-issued nominal identity
- target-closed function-pointer shape
- `NativePlace::Parameter` / `NativePlace::Field`
- identity space
- nested private field destination
- uncited declaration
- conformance-owned slot identity
- target-closed placement
- layout-plan identity
- compact fingerprint
- authored byte offset
- calling-plan coordinate
- source-visible fields
- runtime topology
- cited conformance
- layout owner
- visible conformance enumeration
- private demand
- `Access` view
- physical offset ownership
- reusable physical recipe
- nominal boundary application
- callback materializations
- reordered telescope
- inbound plan / outbound plan
- private function
- callback-root schedule
- destination replay
- data-symbol identity
- pointer extent
- containing storage
- materializations (missing/duplicate/etc.)
- artifact-local machine identity
- enclosing program
- object replay / final-image replay
- private symbol
- patched address
- receiving validation
- authenticated plan-application identity
- authored-use-to-Terminal-operation correspondence
- producer digest
- placement index
- emission
- image replay
- foreign registration
- invocation permission
- installed-address lifetime
- registration capacity
- emitted thunks
- unregister / required quiescence
- code/component leases
- runtime boundary operation
- realization and resource profile
- admitted future root
- linear registration
- call-scoped borrows
- consumable lifetime budget
- duplicate lifetime field
- custody contract
- foreign internal tables
- general foreign-retention contract
- `invokes`
- bodyful machines / bodyless requirements
- direct invocations
- current-invocation reach
- handler's service
- selected operational envelope
- direct synchronous invocation graph
- synchronous trait
- new-activation boundary
- restricted synchronous queries
- opaque provider's internal call graph
- opaque native re-entry
- trust-relative
- active or depth limit
- unavailable result
- finite mixed-chain admission
- checked provider contract
- structural isolation
- selected concrete envelope
- caller's proof context
- installed roots (linked)
- entry stacks (linked)
- Compact provider-execution coordinates
- provider authority
- strong closure identity
- requirement substitution
- component publication (linked)
- unsupported callback forms
- opaque companion

CHAPTER COMPLETE
