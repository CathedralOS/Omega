# Private callback realization

A callback binder selects a nominal machine satisfying one exact requirement.
Structural signature coincidence does not select it. The callback's native
address is a private realization result, never a source runtime value or
semantic input.

## Authored selection

A registrar's static machine binder declares
`where machine Selected satisfies CallbackRequirement::call`. That
signature-free path must select one exact overload at the binder declaration;
signature coincidence or a uniquely visible machine cannot choose it later.
Adding an overload that makes such a reference ambiguous is a compatibility
break, diagnosed at the declaring trait and affected uses. No callback-specific
expanded-signature or `as Name` overload-selector form exists.

A direct destination is written `native callback name from Selected` at its
actual native argument position. It has no source runtime argument. A nested
destination uses the explicitly cited `PrivateCallbackSlot<Requirement>`
conformance in its owning layout. The [calling plan](calling_plans.md) places
these declared demands; it does not invent hidden trailing parameters.

The binder receives its signature, contracts, operational ceilings, and entry
plan from the requirement. Per-instance state uses the protocol's context
parameter, a checked generational handle recoverable from callback arguments,
or package-owned stable state. No implicit closure environment rides on a
function pointer. Raw addresses and pointers remain inert representation tokens,
not readable/writable views.

## Required identities

| Record | Binds |
| --- | --- |
| Callback use | Registration call/operation, static-machine argument ordinal, selected machine and satisfaction row, exact canonical requirement overload, and target entry recipe. |
| Registrar materialization | Exact binder slot and one declared native parameter or private layout-field destination, independently of the later callback selection. |
| Callback root | Exact canonical entry, activation-local flow/dispatch/storage/frame identities, validated inbound entry plan, and internal argument/result bridge. |

Retain the requirement's published envelope separately from the selected
machine's actual envelope and the checked actual-refines-published judgment.
The actual envelope is not an ambient caller fact. Registration provenance
retains selected identity and lease disposition without itself granting a lease.

Terminal Psi remains target-neutral. Target-specific recipes and placement
evidence travel beside it as build/native realization inputs, joined to its
exact boundary operation. A source-free artifact cannot reconstruct an authored
callback selection merely from that operation.

## Declared native destinations

A direct callback destination is an interleaved native-only parameter on the
registrar requirement. It contributes no semantic runtime formal, Terminal
operand, or address value. One ordered native telescope contains both these
entries and entries originating in ordinary semantic formals. Each entry has a
compiler-issued nominal identity; a callback entry also retains its exact
binder/requirement source and target-closed function-pointer shape.

`NativePlace::Parameter` and the root parameter of `NativePlace::Field` use
that same identity space. Calling policy places declared entries; it cannot
create, reorder, retarget, or infer a trailing callback argument.

A nested private field destination originates in an explicitly selected named
`PrivateCallbackSlot<Requirement>` conformance cited by the layout plan. An
uncited declaration is inert. Retain conformance-owned slot identity, exact
target-neutral requirement overload, target-closed placement, and the complete
layout-plan fingerprint. An authored byte offset is placement data, neither
slot identity nor a calling-plan coordinate. Private slots are not ordinary
source-visible fields or addresses.

## Plan and occurrence validation

[Calling-plan identity](calling_plans.md#identity-and-selection) distinguishes
the reusable physical recipe from its nominal boundary application. Callback
materializations participate in the latter; equal register layouts cannot hide
a reordered telescope.

Keep the callback's inbound plan separate from the registrar's outbound plan.
Join each binder/requirement/destination to exactly one private function and
callback-root schedule, preserving order. Destination replay checks selected
target/layout, slot and data-symbol identity, offset, pointer extent, alignment,
and containing storage. Missing, duplicate, reordered, overlapping,
shape-incompatible, unresolved, or substituted materializations reject.

Materialization uses only a fully validated outbound plan. The private function's
artifact-local machine identity stays nested in its source artifact; it cannot
impersonate a machine in the enclosing program. Object and final-image replay
bind the private symbol, relocation, executable region, and patched address to
the same function and native argument destination.

Independent receiving validation needs both authenticated plan-application
identity and replayable authored-use-to-Terminal-operation correspondence.
A retained producer digest or placement index alone does not establish either.

## Registration and lifetime

Emission and image replay establish code and placement, not successful foreign
registration, invocation permission, installed-address lifetime, capacity,
external-root ownership, or component publication. Those require the actual
registrar outcome and the separate root/lease protocol. Registration capacity
counts live registrations, not emitted thunks; unregister and required quiescence
precede release of code/component leases.

The registrar is an ordinary runtime boundary operation. Build selects its
realization and resource profile, not successful registration. Success creates
the admitted future root and moves the exact live-registration capacity into
the linear registration. Failure creates no root and preserves that authority;
successful teardown returns the same capacity occurrence. Call-scoped borrows
create no durable registration. A consumable lifetime budget is a different
resource.

Materialization retains binder and destination, not a duplicate lifetime field.
The native parameter's ordinary custody contract determines whether its storage
is call-scoped or retained. Foreign internal tables are provider state; retained
caller storage needs the general foreign-retention contract.

`invokes` describes possible synchronous entry before the registrar returns;
it is separate from creating a future root. Bodyful machines infer direct
invocations; bodyless requirements declare them. Omitted `invokes` on a bodyless
requirement means no synchronous invocation. The handler's service and
selected operational envelope contribute to current-invocation reach. The direct
synchronous invocation graph must be acyclic; inserting another synchronous
trait does not break a cycle. A genuine new-activation boundary does. A package
may handle restricted synchronous queries and queue ordinary application events
without inferring an opaque provider's internal call graph.

The registration retains the selected concrete envelope but does not import it
automatically into the caller's proof context. An API exposing those facts must
forward them in its contract. [Installed roots](external_roots.md) and
[entry stacks](../resources/entry_stacks.md) govern later entry and resource
admission.

Compact provider-execution coordinates remain reports. Admission retains the
selected provider authority, strong closure identity, and exact requirement;
equal compact coordinates cannot authorize requirement substitution.

See [component publication](component_publication.md) for runtime custody and
[native realization](../../../omega-rust/omega/compiler/native-realization/README.md)
for the current implementation boundary. Unsupported callback forms reject;
carrying an opaque companion intact is not interpreting or admitting it.
