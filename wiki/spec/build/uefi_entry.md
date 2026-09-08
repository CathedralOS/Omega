# UEFI entry and firmware handoff

Freestanding is an immutable selected target/provider property, not a source
machine category. No hosted providers are ambient. The target supplies its
explicit firmware, storage, device, scheduling, entry, and exit contracts;
`build.omg` binds the [semantic program entry](entry_roots.md) and selects
provider slots. Cathedral owns its OS policies and lifecycle, not the compiler.

## Physical arrival and semantic roots

The UEFI x64 profile's physical `UefiPhysicalEntry::enter` accepts the platform's
image handle and system-table occurrence and returns `EfiStatus` under
the UEFI x64 convention and its exact selected calling policy. Its separate
semantic `ProgramStorageEntry::enter`
introduces exactly two `Extent in Granted` occurrences: image and initial
storage. Neither requirement replaces or refines the other.

A generated physical shell invokes the exact target-authored bootstrap adapter.
The adapter validates arrival, installs lifecycle-scoped providers, obtains
root geometry and correspondence evidence, then crosses the semantic
installation edge. The source continuation receives only the semantic schema's
visible arguments, with its optional provisioned receiver. It does not receive
hidden firmware arguments or project the private system table.

The physical arrival contract retains pointer validity, alignment and lifetime,
the paging/CPU regime, entry/exit control, and firmware lifecycle assumptions.
An ABI-compatible pointer or register arrangement establishes none of these.

The image handle is provenance-bearing identification, not storage authority.
The selected Loaded Image provider admits correspondence to the real mapped
image. Successful page allocation admits exclusive page custody; release admits
its return. Layout and arithmetic checks derive facts above these premises,
not the premises themselves. Retain exact selected provider postconditions,
input occurrences, phase, schema, calling plan, continuation, and receipts.

Both semantic root geometry obligations must hold before either complete root
is introduced. Failure before installation calls no continuation and returns
all moved bootstrap inputs. Target result mapping specifies recoverable
bootstrap errors; normal Unit return maps to success. Crash, trap, or abort
does not manufacture a returning status result.

Initial storage is a separately owned reserved region proved disjoint from the
image root or an admitted allocation. Image sections are subextents, not fresh
roots. Active stack, receiver, or retained bootstrap state cannot hide inside
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

The handoff uses a target-bounded linear `GetMemoryMap`/`ExitBootServices` cycle:

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

The [UEFI implementation note](../../../omega-rust/omega/backend/runtime/external-roots/uefi.md)
records the current bounded entrances without claiming complete boot execution.
