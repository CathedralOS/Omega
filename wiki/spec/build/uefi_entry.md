# UEFI entry and firmware handoff

Freestanding is an immutable selected target/provider property, not a source
machine category. No hosted providers are ambient. The target supplies its
explicit firmware, storage, device, scheduling, entry, and exit contracts;
`build.omg` binds the [semantic program entry](entry_roots.md) and selects
provider slots. Cathedral owns its OS policies and lifecycle, not the compiler.

## Authored firmware definitions and adapters

UEFI is not an exception to source-owned platform code. The selected target
package authors firmware data declarations, layout policies, calling policies,
constants, integrity checks, and bootstrap/handoff machines in ordinary Omega.
System Table, Boot Services, and Loaded Image geometry comes from checked
[layout plans](../layouts/plans.md), not a second firmware-field catalog built
into the compiler. Source packages may keep those representations private;
opacity to application code does not require compiler-owned definitions.

The compiler supplies generic checking, evaluation, lowering, and independent
evidence replay, plus the necessary target entry shell and instruction/ABI
primitives. It binds exact selected source contracts and admitted environment
premises to their generated realization. Independent replay must check the
source/plan/realization correspondence; a source digest alone does not establish
it. Firmware policies have one source definition; independent replay must not
depend on a separately authored Rust copy of that policy.

Firmware invocation leaves retain explicit validity, provenance, lifecycle,
and service-behavior assumptions. Authored table validation alone cannot prove
a pointer is valid or manufacture firmware authority. Conversely, admitting those
assumptions does not make ordinary parsing, allocation sequencing, retry logic,
or status handling compiler primitives. If a source implementation is blocked,
name the missing general capability on the execution board rather than expanding
a firmware-specific intrinsic.

## Physical arrival and semantic roots

The UEFI x64 profile's physical `UefiPhysicalEntry::enter` accepts the platform's
image handle and system-table occurrence and returns `EfiStatus` under
the UEFI x64 convention and its exact selected calling policy. Its separate
semantic `ProgramStorageEntry::enter`
introduces exactly two `Extent in Granted` occurrences: image and initial
storage. Neither requirement replaces or refines the other.

A generated physical shell invokes the exact source-authored target bootstrap
adapter. The adapter validates arrival, installs lifecycle-scoped providers, obtains
root geometry and correspondence evidence, then crosses the semantic
installation edge. The source continuation receives only the semantic schema's
visible arguments. There is no optional automatically provisioned receiver or
context object. Additional state uses ordinary storage or an argument explicitly
established by that installed contract. The continuation does not receive hidden
firmware arguments or project the private system table.

The physical arrival contract retains pointer validity, alignment and lifetime,
the paging/CPU regime, entry/exit control, and firmware lifecycle assumptions.
An ABI-compatible pointer or register arrangement establishes none of these.

### Firmware API authority and storage grants

The installed physical entry contract establishes which firmware operations the
adapter may invoke through the handles and table borrows received on this call.
The adapter delegates that authority to the subsystem doing the work by passing
a qualified handle or borrow. The subsystem may use it only for the permitted
operations and within its firmware phase. Each protected domain uses
`Issuance::Only` in its `AccessPlan` to name its authorized issuers. The installed
physical requirement permits the `UefiBootServices` reach required by page
allocation. The adapter’s synchronous handoff to the
Build-bound continuation contributes `invokes ProgramStorageEntry`. Calls to
outbound firmware and grant providers contribute their published effect rows. [Initial-call authority](../resources/authority.md#initial-call-authority)

Firmware supplies the image handle for this invocation. The selected Loaded Image
operation establishes which mapping belongs to that handle. A successful page
allocation establishes exclusive custody of the pages returned by that call.
A successful release returns that custody. A single target package may implement
both lookup and allocation. Each storage grant must retain its own evidence for
the exact range, including when a checked helper forwards the pair. The
provider and invocation records retain the inputs, postconditions, firmware
phase, schema, calling plan, continuation and receipts.
[External roots and issuance](../resources/authority.md#external-roots-and-issuance)

Before introducing either complete root, the checked adapter proves that both
ranges fit their backing, are disjoint and exclude the live stack and retained
state. It then forwards both grants to `ProgramStorageEntry::enter`. Failure
before installation calls no continuation and returns all moved bootstrap inputs.
Target result mapping specifies recoverable bootstrap errors; normal Unit return maps to success. Crash, trap, or abort
does not manufacture a returning status result.

Initial storage is a separately owned reserved region proved disjoint from the
image root or an admitted allocation. Image sections are subextents, not fresh
roots. Active stack, application state, or retained bootstrap state cannot hide inside
an extent forwarded whole. A shared parent allocation must partition exactly,
leaving one contiguous disjoint residual when source receives one `Extent`.

## Stack contract

The closed UEFI x64 profile supplies a canonical target-semantic minimum of
128 KiB available stack with 16-byte alignment. It remains symbolic until
target closure and participates in observation/artifact identity. Physical
arrival separately admits that the actual firmware invocation meets the
profile; evaluating the constant proves no runtime backing.

Same-stack admission includes generated shell, live adapter frames, nested
program/provider WCSU, and the explicit target reserve. Each term binds its
actual checked or emitted producer. Alternatively, a checked stub may switch
to privately provisioned Omega storage while retaining firmware return state
for a returning profile. Numeric totals from a different invocation or private
firmware ledger cannot replace that exact evidence.

## Returning application versus OS handoff

A returning `UefiApplication` keeps Boot Services live, reclaims adapter-owned
allocations, and returns through its physical result map. A successful OS-loader
handoff is non-returning and has a different lifecycle, despite sharing the
physical calling convention. One cannot be inferred from the other's Unit
source signature.

An ordinary source-authored adapter implements the bounded linear
`GetMemoryMap`/`ExitBootServices` cycle under the selected target/provider contract:

1. Acquire explicit map-buffer storage and retain allocations, Boot Services,
   surviving-stack evidence, physical invocation, and remaining attempts.
2. Read the map using its returned descriptor stride/version and retain the
   exact snapshot and map key. Every projection fits the returned byte range.
3. Attempt exit. A stale-key outcome retires that snapshot and returns the
   unchanged live custody for retry, decreasing the measure only under
   `1 <= remaining`. Exhaustion returns the target-authored EFI error.
4. Success consumes Boot Services and the final map into the exit receipt,
   transitions allocation custody, and retains the final-map obligation.

No destructor conceals that fallible protocol. Boot-scoped providers end;
already allocated storage preserves its occurrence lineage while custody
passes to the program. New physical-memory claims require final-map, successful
exit, and target-policy evidence. Reserved, runtime, ACPI, device, active
bootstrap, and otherwise excluded ranges are not implicitly claimable. Runtime
Services survive only under their separate post-exit contract.

A bodyless handoff declaration backed by a compiler-specific retry-loop
realization is not the completed source implementation. Generated entry and
stack-transition mechanics remain compiler responsibilities; the protocol
body uses ordinary machine calls, control flow, and custody checking.

The incoming firmware stack is not assumed to survive exit. Switch to an
accounted handoff stack before the final attempt or establish the incoming
stack's required lifetime. Retain its claim through success and expose only
disjoint residual storage to source.

## Hardware preparation and reporting

Use ordinary checked [materialization](hardware_materialization.md) and
[interrupt obligations](interrupt_obligations.md), not an IDT language or
compiler-owned OS tables. Stack/preemption selection drives both concrete gate
IST placement and WCSU; these cannot be independently authored facts. Split IDT
addresses normally materialize after rebasing, whereas loader-consumed fields
must fit native relocation forms.

Where practical, reserve and validate final table/stack placements before exit.
The post-exit critical sequence then performs only required mapping/stack
transition, visibility, prepared-root publication, checked table installation,
and finalization. External interrupts remain disabled until the complete
exception floor exists. Software-fault-free claims retain explicit platform
assumptions; they do not prove NMI, machine-check, or physical failure absent.

Reports bind selected requirements/providers, calling/state plans, admitted
environment facts, extent scopes, artifact/materialization/visibility identities,
external roots with reach/stack/nesting/version pins, instruction footprints,
and remaining authority debts. Installed-ledger snapshots are not guessed
build tables. [Executable installation](executable_installation.md) owns boot
authentication, AP artifacts, visibility, and quiescence independently.
