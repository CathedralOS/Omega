# Design Brief: Freestanding Boot And Hardware Facts

Current direction as of 2026-08-21. Freestanding selection and the security
model are settled. The reusable memory/hardware primitives are specified in
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md);
their source APIs and backend support remain incomplete.

## Freestanding is a target/provider property

```omega
machine build(builder: &mut Build) {
    builder.roots.bind(
        cathedral::targets::uefi_x86_64::ProgramEntry,
        Application::start
    );
}
```

A concrete freestanding profile is selected immutably by the invocation; this
build only authors its target-qualified root binding. A freestanding target has
no ambient hosted provider set. Firmware, memory,
device, clock, scheduling, entry, and exit services must be selected from
explicit target packages and admitted providers. The selected profile supplies
the freestanding fact and ordinary provider defaults; `build.omg` binds required
external roots and may override individual provider slots. This is ordinary
deployment data, not compiler folklore.

## Typed entry handoff

Physical arrival and semantic program arrival are distinct contracts joined by
one target-owned entry schema. The target profile fixes the physical
requirement and its complete evaluated calling and machine-state policy;
`build.omg` binds only the semantic source continuation. Firmware, a loader, or
an OS supplies the physical values at launch. A generated ABI shell invokes the
exact target-authored bootstrap adapter, and that adapter establishes the
semantic arrival before calling the selected continuation.

For UEFI the physical contract has the platform's real two-argument ABI and
status result, conceptually:

```omega
pub boundary trait UefiPhysicalEntry {
    machine enter(
        image_handle: EfiImageHandle,
        system_table: &EfiSystemTable
    ) -> EfiStatus;
}
```

`EfiSystemTable` remains a validated native layout used privately by the target
provider. Applications do not project it. The target installs lifecycle-scoped
firmware provider realizations over that exact table occurrence instead.
`EfiImageHandle` is an opaque provenance-bearing physical input, not storage
authority; only admitted bootstrap operations accepting that occurrence can
derive correspondence facts from it.

The target-owned UEFI x64 layout plan retains the complete known 120-byte
`EFI_SYSTEM_TABLE` prefix as eighteen ordered rows: the flattened 24-byte
`EFI_TABLE_HEADER`, explicit four-byte ABI padding after `FirmwareRevision`,
and every console, service, table-count, and configuration-table field. Exact
target-package, entry-slot, Microsoft-x64 calling-plan, order, offset, width,
alignment, coverage, and layout identity replay independently. In particular,
the plan proves `ConOut` at byte 64 and `BootServices` at byte 96. The plan
itself does not inspect a runtime occurrence and grants no pointer, provider,
lifecycle, root, shell, or execution authority. A separate non-authorizing
header-integrity gate consumes the plan and borrows one supplied occurrence. It
checks the exact `EFI_SYSTEM_TABLE` signature, requires runtime `HeaderSize` to
cover the known prefix without exceeding the supplied bytes, requires zero
`Reserved`, and verifies the CRC across every `HeaderSize` byte with the CRC
field zeroed. The carrier retains revision for later capability-specific policy
and accepts CRC-covered forward-compatible suffixes, but projects no pointer
field. A lifecycle-scoped provider must still join that integrity evidence to
exact occurrence provenance and firmware phase before projecting any service.

```omega
machine Application::start(
    image: Extent in Granted,
    initial_storage: Extent in Granted
)
{
    ...
}
```

The UEFI profile's `ProgramEntry` schema records two differently bound
identities: target-fixed `UefiPhysicalEntry::enter` with
`Calling<UefiX86_64>`, and build-bound `ProgramStorageEntry::enter` with the two
source-visible qualified parameters. The physical requirement is not a
refinement of the semantic one, and the source continuation receives no hidden
firmware arguments. The generated shell and authored bootstrap together form
the installed bridge: they validate physical arrival, install the selected
scoped providers, obtain exact root geometry and correspondence evidence, and
then cross the semantic installation edge. That edge introduces the two
`Granted` occurrences once; the ordinary continuation call only forwards them.

The bootstrap contract states pointer provenance, alignment, lifetime, paging
and CPU regime, stack supply, entry/exit control, provider lifecycle, and every
accepted fact the checker cannot derive. The adapter itself is checked Omega;
its foreign premises are exact provider postconditions. Physical arrival admits
that the supplied system-table occurrence, machine regime, and entry stack
satisfy the selected profile. `HandleProtocol` admits the correspondence from
one successful Loaded Image result to this handle's real mapped image;
`AllocatePages` admits exact exclusive page custody on success; `FreePages`
admits its return. Layout validation and interval arithmetic derive facts above
those claims but never replace them. The ledger retains the postconditions and
selected bindings, not a blanket statement that firmware is trusted.

The first concrete `HandleProtocol` operand carrier retains the physical
entry's non-null image-handle value opaquely, rejoins it to the exact live
provider and Microsoft-x64 call plan, and borrows one initially zero interface
output slot. The Loaded Image GUID has an exact 16-byte native layout. The
service address, handle, GUID address, and output address remain private, while
the public carrier reports only identities and RCX/RDX/R8 destinations.
Address-free provenance or a stale output slot rejects transactionally. A
single target-runtime executor consumes the carrier, calls exactly its retained
service through the UEFI ABI, and seals the exact returned status and output in
a non-clone receipt. Callers cannot author those results. Only the closed
Success code with an unchanged non-null output can be decoded through the exact
target-owned 96-byte Loaded Image layout: revision `0x1000`, ImageBase at byte
64, and ImageSize at byte 72. Closed error statuses, unknown status, output
drift, and invalid revision or geometry retain the complete executed provider
custody for release. This bounded call still infers no image `Extent`, semantic
root, full bootstrap adapter, or semantic installation.

The selected profile exposes its minimum entry-stack bound as a target-semantic
observation, conceptually
`TargetSemantics::guaranteed_entry_stack<UefiX86_64>()`. It stays symbolic until
target closure and propagates into certificate and artifact compatibility
identity. The physical-arrival admission separately states that the executing
firmware conforms to that selected profile. For an adapter that remains on the
firmware stack, the checked inequality includes generated shell usage, live
adapter frames, maximum nested program and firmware-call WCSU, and one explicit
target reserve. A target may instead allocate and switch to a private Omega
stack through a checked stub, preserving the firmware return state for the
returning profile.

The closed UEFI x64 profile fixes that minimum at 128 KiB with 16-byte
alignment, following UEFI Specification 2.11 section 2.3.4. Numeric target
closure and the same-stack inequality are planning facts: generated-shell,
adapter-frame, continuation/provider WCSU, and explicit-reserve coordinates
must still be bound to their exact emitted or checked producers, and physical
arrival must separately admit firmware conformance. The planning result rejoins
the private firmware-ledger authority, both physical occurrences, and the live
phase lease before it can match the adapter-readiness carrier; equal public
report coordinates from another ledger do not substitute.

The first address-free adapter composition now consumes that private-ledger
readiness and numeric plan together with the compiler's optimized semantic
entry contract. It independently revalidates the receiver-free Microsoft-x64
two-root Unit ABI, exact Image/InitialStorage order, distinct physical and
semantic requirements, and byte-for-byte equality of the target-owned physical
contract retained on both sides. The non-clone result keeps the complete
semantic contract and stack plan private while exposing only stable identities,
the strong semantic calling-plan commitment, and bounded planning observations.
Rejection returns all three inputs. This establishes neither WCSU producer
provenance nor shell emission, provider installation, semantic roots,
invocation, or an `EfiStatus` result.

The installation receipt joins the physical invocation, target schema,
provider realizations, physical-input provenance, root geometry and evidence,
stack plan, semantic requirement, and selected continuation. Failure before
semantic installation calls no source continuation and introduces no complete
semantic roots. The target package maps each recoverable bootstrap failure to
an exact `EfiStatus`; normal return from today's Unit semantic continuation maps
to success. A crash, trap, or abort does not return through the physical result
register and remains a declared non-returning route. The compiler derives and
composes the generated and authored bridge contracts; it never interprets a
firmware handle by folklore.

Program storage begins from a small number of entry-provisioned content roots.
The image handle is only an identifier; the Loaded Image provider supplies its
base, size, lifetime, and correspondence. Initial storage comes from a
separately owned reserved region in the final image plan whose disjointness from
the installed image root is proved, or from an admitted firmware allocation.
`EntryStack::ProviderSelected` describes where the bootstrap executes;
it does not transfer that live stack to the program. The semantic entry
introduces the resulting program-visible image and initial-storage occurrences.

The compiler derives image sections as subextents of the image root; later
frames and task stacks are checked allocations from an existing root. Bootstrap
or receiver storage is never concealed inside an extent handed whole to source.
When a private stack and initial storage share one parent allocation, that
parent is conserved as an exact partition: the active stack and retained
bootstrap state remain in the target execution frontier, and only one disjoint
residual becomes source-visible `initial_storage`. The target chooses geometry
that leaves one contiguous residual when the semantic carrier is one `Extent`;
it may not encode an inaccessible hole in a `Granted` range. A receiver-bound
entry lends only its one checked `&mut self` occurrence.

Those roots use the core-owned stable `ProgramStorageEntry::enter` semantic
arrival requirement. Its two exact `Extent in Granted` parameter positions
identify the image and initial-storage roots inside the generated bridge.
Target entry schemas such as `UefiApplication` compose that requirement with a
separate target-fixed physical requirement. Calling policy and native-result
mapping belong to the physical side; generated stubs, target bootstrap, and the
source-visible continuation shape join the two without replacing either
identity.
`Extent::Granted` authorizes the core requirement as an alternative route, and
installation introduces the matching parameters. Core therefore never depends
on a UEFI/Cathedral domain, and the compiler never recognizes target-friendly
names as storage authority. The arrival requirement and qualified-position
identity are live. Installation requires the exact physical calling-plan
fingerprint, bootstrap/provider evidence, and generated capture for each
semantic position, validates both `Granted::no_wrap` obligations before
committing either semantic root, and returns every moved bootstrap input on
rejection. Compiler-derived image-section
ranges remain borrowed views under the installed image root. Initial-storage
allocations that leave the pool's ownership use an explicit conserved
partition retaining every remainder and can recompose the exact parent.

The first implementation profile is the returning `UefiApplication`. Boot
Services remain live, normal Unit completion returns success, and every
adapter-owned allocation is reclaimed. A successful OS-loader handoff is a
different lifecycle over the same physical ABI. Its bounded
`GetMemoryMap`/`ExitBootServices` state graph threads every allocation,
snapshot, map key, and boot-services capability linearly; stale-key rejection
returns all live custody before retry, while success consumes boot-scoped
authority and transfers surviving storage without returning to firmware. That
handoff profile remains implementation work and must not be inferred from the
returning application's Unit contract.

## Facts, authority, reach, and trust remain separate

- domains and contracts carry value/layout propositions;
- semantic qualifications are explicit author commitments;
- capability values carry authority;
- boundary-trait identities contribute service reach;
- provider admission carries accepted trust; and
- target assumptions are explicit environment/provider contracts.

An address value is never authority. A package cannot gain physical memory,
MMIO, artifact-installation, interrupt-table, or machine-control authority by
naming a type or manufacturing an integer.

## Firmware handoff and memory authority

UEFI's map/exit protocol is a bounded linear state machine:

1. obtain explicit storage for the runtime-sized map;
2. read descriptors using firmware's reported stride/version;
3. retain the associated `MapKey`;
4. call `ExitBootServices`;
5. on a stale key, explicitly return the live boot-services capability and
   allocation custody to the map-acquisition state with a smaller attempt
   measure; and
6. on success, consume firmware authority and establish one
   `FinalMemoryMap` obligation.

The attempt bound is target-authored. `remaining - 1` is formed only under the
guard `1 <= remaining`; exhaustion returns an authored EFI error status. Every
non-copy value is named in each arrival contract rather than ambient across the
cycle. Success consumes the boot-services capability and final map into the
exit receipt while transitioning allocation custody. A stale-key outcome
retires the stale snapshot and forwards boot services and allocations unchanged
to the next attempt. No destructor pretends to perform this fallible protocol.
Descriptor stepping uses the supplied stride, not `sizeof`, and every typed
projection is proven inside the returned byte extent.

The first runtime-independent handoff owner now models that bounded state graph
separately from the returning application ledger. Its non-clone arrival owns
the exact Boot Services occurrence, allocation roster, surviving-stack
evidence, physical invocation, and attempt measure. Acquiring a map binds one
fresh snapshot/key pair. Stale-key replay retires that pair and returns the
unchanged live custody at the next generation with exactly one fewer attempt;
exhaustion returns the target-authored EFI error while retaining Boot Services.
Success removes Boot Services and transfers the unchanged allocation lineage,
final snapshot, stack evidence, and exact exit receipt into a non-returning
carrier. Native calls, descriptor interpretation, and policy-qualified new
physical-memory introductions remain outside this first state-model rung.

`ExitBootServices` has per-claim dispositions. Boot services and boot-scoped
protocol providers end; already allocated storage preserves exact occurrence
lineage while transferring from firmware to program custody; separately
eligible conventional-memory descriptors may introduce new physical-memory
claims under the final-map snapshot, successful-exit receipt, and target memory
policy. Reserved, runtime, ACPI, device, active-bootstrap, and other excluded
descriptors are not thereby claimable. Runtime services survive only through
their separate post-exit contract.

The target does not assume that firmware's incoming stack survives the exit. It
switches before the final attempt to an explicitly accounted handoff stack, or
supplies evidence that the incoming stack has the required lifetime. The active
stack claim is threaded through the exit transition and retained by the stack
execution frontier. When it descends from the same parent allocation as initial
storage, the application receives only the conserved disjoint residual, never a
range containing live inaccessible stack bytes.

The final map supplies physical extents, not freely forged allocator values.
A bounded `Arena` is allocation authority over appropriate backing extents. Device,
reserved, firmware, and executable ranges retain different provenance and
rights. Metadata capacity is explicit.

## Placed hardware storage

MMIO, descriptor tables, framebuffers, DMA buffers, and shared IPC
pages use the same composition:

```text
qualified Extent borrow + provider-bound ResourceProfile receipt
    + evaluated PlacementPlan { LayoutPlan, AccessPlan, reach }
    -> checked admission + explicit content establishment
    -> Placed<P, T> / field accessors
```

`Extent` carries transparent geometry; its established facts and active loan
carry authority over a concrete range. `LayoutPlan` owns bit geometry.
`AccessPlan` states consumer demand. The provider's offset-keyed
`ResourceProfile` states supply. A nominal placement policy selects the plans
and static service reach, and admission checks that whole request against the
exact loan once before any accessor exists.

Projection is pure; accessors perform only their admitted exact-width stable,
external, or atomic operations. Device-specific operations such as W1C,
read-to-clear, FIFO, posted-write flush, and coherent snapshot remain package
machines over binding-private primitives. Address-translation policy and table
construction belong to the OS package, which composes generic extent, mapping,
layout, and checked-assembly contracts. DMA lends extents to an invisible
borrower represented by a linear completion token; completion may restore a
stable CPU loan after the device-owned phase.

CPU atomic fences, DMA publication/acquisition, cache maintenance, MMIO
notification, and posted-write completion remain distinct semantic operations.
A checked Cathedral driver composes provider primitives; a hosted OS boundary
may conform directly to a complete submission requirement. Publication
evidence is bound to an exact range and invalidated by an intersecting write.
Acquisition consumes completion tied to the same request and device instance,
and restores Stable CPU observation only when custody returns. Every provider
or target requirement emitted by these operations must be discharged or the
program rejects.

## Checked assembly

OS code uses parsed `asm {}` under compiler-known instruction contracts. The
first freestanding catalog must cover the actual x86 bringup path: interrupt
mask save/restore, `hlt`, port I/O, descriptor-table loads, control registers,
MSRs, fences/cache maintenance, atomics, and mode/entry transitions.

Instructions emit service-reach, authority, target, and state requirements
that must all be discharged, plus modeled register/flag/memory changes,
ordering, regime changes, and exits. A checked block may separately provide a
conformance when the modeled sequence proves it; policy-permitted admitted
provider evidence is still evidence, never an open requirement. Direct
assembly cannot be quieter than a boundary-trait operation. Unknown
instructions and raw emitted bytes are rejected; trusted foreign blobs use
provider admission.

Entry/exit-only operations such as `iretq` are deriver-only. Interrupt masking
is an ordinary linear save/restore token, distinct from a scheduler-switch
guard. Both restore prior state explicitly; neither relies on drop timing.

## Interrupt entry and the IDT

The language does not have an interrupt declaration or IDT DSL. The x86 IDT is
a Cathedral acceptance slice that composes the common pieces:

1. ordinary `data` for the gate schema;
2. an x86 layout policy with bit and fragmented placements;
3. a target-specific boundary requirement carrying `Calling<C>`, `CallPlan`,
    `StatePlan`, stack/preemption class, service/suspension/blocking and guarded
    crash ceilings, and acknowledgement protocol;
4. an ordinary exact machine satisfying the entry requirement;
5. binding that machine to the target-owned indexed interrupt-root slot;
6. symbolic entry-stub identity resolved by a phase-aware materializer;
7. a Cathedral-authored checked writer establishing a content-bound table fact
   over an exclusive unpublished placement;
8. a separate Cathedral installer presenting table and CPU publication
   authority to checked `lidt`, with roots recorded before hardware
   reachability; and
9. a linear acknowledgement token for exactly-once EOI.

The generic obligations are distinct linear data values with consuming
settlement operations: restoring prior CPU interrupt state is not the same
fact as acknowledging a device. Their normalized provider mint and entry
settlement are recorded in the installed-root ledger. An exact entry receipt
binds the installed root, selected provider execution, invocation, and
acknowledgement policy before the values exist; replay rejects; nested
saved-state guards restore exact prior states in LIFO order; and exit requires
all obligations selected by the admitted plan to be settled. The concrete
types, PIC/LAPIC protocol, timer source, vector policy, and transition machines
belong to Cathedral.

The provider-neutral completion boundary publishes a bounded abstract service
row beneath `MachineControl + PortIo`. Its exact normalized requirement path
identifies the row; a selected PIC completion resolves it to `PortIo`, while a
selected LAPIC/x2APIC completion resolves it to `MachineControl`. The entry and
completion rows remain distinct because row equality cannot establish
provider identity. The installed-root receipt binds their exact operations,
provider execution, acknowledgement policy, and token lineage instead.
Unresolved rows and bounds remain visible in the installation manifest and may
propagate only inside that root's installation closure; final admission rejects
any unresolved row.

Program entry, reset vectors, interrupt vectors, and callbacks are instances of
the same target-declared slot model. Direction distinguishes roots, which the
environment activates, from providers, which the program calls. Lifecycle,
indexing, sparseness, and cardinality remain orthogonal: an interrupt family may
declare legal, required, optional, and reserved vectors, while a program-entry
slot is one required build-bound root. Runtime installation performs the same
binding-shape and demand/supply checks before publishing a handler to hardware.

If Cathedral defers acknowledgement to a bottom half, the linear token leases
the interrupt root and controller configuration until completion. Shutdown,
controller reconfiguration, CPU removal, relevant power transitions, and root
retirement drain those tokens first; carry policy must authorize the transfer.
The lease is not asynchronously revocable.

The selected provider plan may keep entry identity private for static tables;
the program does not need a source-visible function pointer or numeric code
address. Installation records the handler as an external root. The root ledger
then includes its reach, trust receipt, state plan, stack domain, nesting graph,
and version/liveness pins.

Stack/preemption class is authored once and drives both the gate's concrete IST
field and WCSU composition. Two separately-authored facts would be unsound.
The installed gate and admitted target profile also determine the complete set
of arrival contexts. A sealed target rule maps each context—such as same-
privilege entry versus a privilege transition—to its hardware-atomic arrival
epochs. The provider does not separately cite an arrival-size row. Software
stack transitions in the emitted adapter delimit further epochs, and nesting
resolves `Interrupted` against the active domain of the exact epoch in which it
occurs.

The live x86 installation join now consumes the validated gate/TSS realization
in that shape. An opaque table/profile-validation carrier retains the complete
context roster, exact boundary, validation receipt, and `InstalledCodeContext`;
the public gate/TSS details must equal its full roster before derivation. The
join resolves the gate's zero/nonzero IST field through the exact TSS privilege
and IST stack-class maps, takes nesting from the matching boundary plan, and
replays the descriptor's symbolic entry against that exact installed-code
occurrence. Thus a table consumer cannot omit an arrival or replace the
physical selection, compiler entry, occurrence, or public stack policy with a
parallel target-fact fixture.

## Symbolic materialization and rebasing

Toolchain-known addresses are symbolic relocation targets, not `addr` values in
user code. Layout fragment entries allow an entry-stub identity to fill the
split x86 gate offset. The materializer resolves it by static placement, native
relocation, loader relocation, or a generated runtime writer.

A rebasing PE loader cannot generally repair an arbitrary split 64-bit IDT
offset with one ordinary pointer relocation, so boot-time materialization after
the image base is known is the canonical path. Fields consumed by the loader
before the first Omega instruction must remain expressible in the object
format's native relocation vocabulary.

The generic materializer is not a public address-resolution escape hatch. It
receives one exact mapped/pinned/writable unpublished placement and a sealed
resolver restricted to the boot-admitted artifact's root set. It writes that
destination directly; failure produces no established consumer value. Layout
validation checks geometry. Omega's live provider carrier now consumes the
activated mapping plus exact pin/unpublished/write-rights receipt and returns
the written mapping without publishing it; failed transitions return all
authority. Successful writing retains a hash-free exact copy of the complete
destination image, and every existing outward consumer replays current bytes
against that producer output before observation. That custody proves neither
the IDT's meaning nor publication. Cathedral's separate IDT validator checks
selectors, gates, privilege levels, IST assignments, reserved bits, and exact
admitted roots before Cathedral establishes its materialized-table fact.

The Cathedral writer does not hold CPU publication authority and cannot make
the table live. A separate Cathedral installer prepares the external-root
records, completes required visibility, and invokes checked `lidt`. Root
records precede hardware reachability. Omega owns the instruction contract,
sealed symbolic targets, generic materialization guarantees, root ledger, and
receipts; Cathedral owns the table-state carriers and the lifecycle connecting
them. Audit fingerprints identify reports and caches, never authority.

The earliest writer's software-fault-free claim is an admitted conjunction, not
an absolute promise that hardware cannot fail. Its destination and stack are
mapped, pinned, writable, and provisioned; its layout operations and CPU
instruction support are validated; its path is bounded and cannot allocate,
block, suspend, dynamically dispatch, or use an unsupported instruction. NMI,
machine check, and physical failure remain named platform assumptions.

### Who establishes what during boot

The build produces a signed PE/COFF boot envelope containing checked kernel
code, generated entry stubs and table writers, normalized plans, the admitted
root manifest, evidence, ordinary native relocations, and reserved data/BSS.
UEFI authenticates the outer image when Secure Boot policy requires it, loads
and relocates the sections, and transfers control to the typed Omega entry. It
does not understand Omega PCC, split IDT offsets, root-ledger policy, or
Cathedral's materialized-table fact.

While firmware services remain available, Cathedral learns the actual image
placement, fixes its final virtual-address plan, and reserves the mapped/pinned
IDT, TSS, and stack placements. Once final addresses are stable, it should
materialize and validate the table before `ExitBootServices` where practical.
The post-exit critical sequence then contains only final mapping/stack
transition where needed, visibility, prepared-root publication, checked
`lidt`, and installation finalization. External interrupts remain disabled
until the complete exception floor is installed.

The boot image can perform this work because firmware authenticated and entered
the initially admitted artifact. That is the explicit trust base, not an
ordinary package granting itself authority. The platform provider attenuates
that initial authority into exact placements, the compiler-derived static
subextents, the sealed artifact resolver, and Cathedral's CPU-scoped
publication authority.

## Admitted executable installation and AP bringup

Omega does not convert arbitrary bytes into host code. An immutable executable
artifact is admitted first, with eligibility bound to normalized content,
identity, relocations, footprint, and placement plan. A contracted installation
provider borrows that reusable artifact and consumes extent-backed linear
`CodePlacement`. Materialization spends write access; freezing ends every
writer; validation checks the exact final bytes; installation produces the
linear `InstalledCode` claim. The provider owns final validation, W^X
transition, target-specific cache maintenance, ordering, and instruction-fetch
synchronization. Callers never reproduce an architecture maintenance sequence.

The compiler foundation now has an executable form of that state machine in
`executable-installation`: reusable exact-evidence admission; one-shot
Extent-backed placement; frozen materialization; artifact/placement/final-byte/
footprint-bound validation; scoped installation authority; synchronous
visibility; and explicit W^X enforcement reporting. Every failed consuming
transition returns its authority inputs. The reusable artifact retains exact
code and canonical relocations. Its pure provider materializer resolves only
sealed targets, patches a private copy with checked target semantics, validates
AArch64 instruction shapes, and derives a placement/content-bound final-byte
SHA-256 digest without acquiring destination-write or execute authority.
Materialization receipts retain that complete canonical output, while final
validation evidence is minted from and retains the exact frozen artifact and
byte snapshot, exact Extent-backed placement evidence, and strong digest.
Artifact content and validated-container proof payloads likewise use distinct
domain-framed SHA-256 digest types whose construction remains inside their
canonical normalizers. Compact normalized identities remain report keys rather
than collision-resistant authority. Installation and retirement continue that rule:
their authorities and receipts retain the complete validated placement or
installed realization, including exact bytes, Extent authority facts, scope,
audience, validation, and W^X state. Compact lifecycle IDs never substitute for
that evidence. Higher-level certificates may test a bounded materialized byte
interval beginning at an exact admitted entry through a sealed equality result;
the installed image and executable address remain provider-side. Retirement
completion facts are domain-framed SHA-256
commitments over provider-canonical bytes. Failed drain quarantine retains the
complete installed realization and provider receipt; stale-entry evidence
requires the exact opaque installed-code context rather than an equal compact
ID. The schema-driven native-container decoder, real PCC and final-code validators,
destination write/freeze and installer operations, Omega linearity, and live
replacement remain implementation work.

Extent-backed placement claims are also connected to the normalized placement
plan: actual destination base/length must satisfy its range, alignment, phase,
machine-regime, and installation-scope constraints before materialization.

Normalized retirement also enforces the opposite lifecycle edge: exact scoped
authority and a provider receipt must prove executors quiesced, X removed,
write authority restored, and target completion facts before the placement can
return to W+NX. Visibility never substitutes for quiescence. Runtime provider
execution and component replacement orchestration remain.

AP trampoline installation consumes a compiler-produced admitted artifact and
is followed by a target boot protocol; it is not runtime code generation. A
dormant/local target needs local completion, while a future remote fetcher needs
visibility completion before entry. Code that may already be executing instead
requires quiescence/component replacement. Visibility and quiescence are
distinct linear obligations with opposite lifecycle roles. The loader completes
visibility inside the loader; it exposes no asynchronous token without a real
provider customer.

Every executable mapping and relevant checked-assembly operation
requires admitted-artifact provenance. There is no `ExecutableMemory`
capability, JIT path, self-modifying code, or alternate raw-byte route.
Replaceable requirement binding is a later logical dispatch/versioning operation, not
part of code placement. Installation prevents injection. Backward-edge returns
in checked Omega remain compiler-owned, non-addressable control state across
both execution and parking. Forward-edge indirect calls separately require
sealed requirement-compatible entry references or descriptors.

The normalized Omega-native container byte decoder and validator are live.
The decoder uses the ordinary validated scalar-layout consumer rather than a
bespoke pointer parser. Its canonical little-endian form is deliberately
small:

- a 64-byte `OMEGAXE!` header fixes the current format marker, architecture,
  total length, artifact report identity, non-authoritative content
  compatibility fingerprint, and a section count;
- the section directory starts immediately after the header and uses bounded
  32-byte records (`kind`, required flag, normalized identity, offset, length);
- code and proof remain exact opaque byte spans; contract and footprint
  sections contain one normalized identity; placement has one fixed 64-byte
  constraint record; entries are fixed 16-byte identity/offset records; and
  relocations are one checked count followed by fixed 32-byte records from the
  closed relocation/target vocabulary;
- all reserved fields are zero, semantic sections are required, informational
  sections are optional, and an unknown required section rejects before any
  admission candidate exists.

Every count and byte range is bounded and checked before slicing. Sections
cannot overlap the canonical header/directory prefix or one another. Their
sorted ranges must tile every byte after the directory exactly, so gaps and
unreferenced trailing bytes cannot become an identity-invisible smuggling
channel. Payload identities must match their directory entries, and the exact
input length must match the header. The semantic validator then enforces one
exact copy of every semantic section, checks the legacy content fingerprint,
and derives normalized executable-content and proof SHA-256 digests. The result
is an immutable admission candidate, never executable eligibility.

Optional known and unknown informational sections remain opaque and
identity-invisible to executable admission, but they are not allowed to
self-name: the decoder derives their trace identity from the section kind and
exact bytes and rejects a directory restatement that differs. This preserves
normalizer-owned reporting identity without granting the payload semantic or
admission authority.

The inverse compiler-side encoder is live over the same layout records. Its
ordinary path emits container v2 with the seven original semantic sections plus
one required authority-commitment section in canonical order, derives the proof
digest from the exact payload, checks configured section/relocation/total-size
bounds before allocation, and routes its completed bytes back through the
hostile-input decoder before returning them. Producer and consumer therefore
share one schema and fail closed on drift; optional informational decoration is
intentionally a later packaging step with no admission role. A separately named
compatibility encoder reproduces the seven-section v1 bytes exactly, but v1
candidates cannot pass executable admission.
The ordinary artifact writer now exposes the only compiler-packaging seam for
this form: it accepts an already-normalized `Artifact` plus exact proof bytes,
invokes the canonical encoder, and atomically installs the resulting file. It
does not accept a final PE/ELF/Mach-O image or a caller-selected byte buffer as
an executable candidate.
Verifier evidence retains that exact immutable candidate, its strong content
and proof digests, and the exact proof bytes. The remaining FNV content field is
container compatibility reporting only, while informational-section
fingerprints stay authority-free.
Normalization binds the exact code bytes, instruction-set architecture,
contracts, footprint, placement, entries, and canonical relocations into
content identity; proof evidence remains outside that promise. The artifact
retains its immutable bytes, architecture, and canonical relocation set through
admission, and relocation lowering rejects cross-architecture substitution even
when a relocation kind is otherwise shared. Signed relocation addends survive
the validated artifact, canonical materializer, object plan, image application,
report, and fingerprint. Retaining and translating the compiler's semantic
code, relocation, contract, footprint, placement, and entry facts into that
packaging seam remains engineering, as does wrapping the canonical result in
the target's firmware envelope.

Container v2 additionally requires four independent domain-separated SHA-256
commitments over canonical evidence for the imported contract set, declared
machine footprint, machine regime, and installation scope. Their historical
`u64` values are report coordinates only. All four strong commitments enter
artifact content identity, admission replay, retained materialization custody,
and final-byte identity; changing any commitment while keeping every compact
coordinate fixed rejects. The production compiler-to-container translator that
supplies those canonical upstream evidence bytes remains engineering work in
the quarantined runtime lane. Placement and entry/data-symbol authority remain
bound by their exact retained structures and the artifact content commitment,
not by compact equality.

The initial image uses the same trust discipline at an earlier phase: the
current trusted build validates the artifact and signs its admitted identity,
secure boot authenticates that identity and gates entry, and measured boot
records what entered. The boot-admitted installer then loads later admitted
artifacts. Future independent PCC/final-byte validation reduces reliance on the
compiler; it is not a prerequisite for the boot semantics. Measurement is
evidence, never the admission gate.

The normalized admission seam already binds a verifier decision to the exact
checked container and its proof payload, while keeping informational sections
authority-free. That prevents a trusted-build or future PCC acceptance from
being replayed onto different semantic content or different proof evidence; it
does not claim that the independent PCC verifier itself is implemented.

AP bringup is a mandatory foundation test: low-memory placement, alignment,
real/protected/long-mode code regions, checked regime-changing instructions,
runtime materialization, artifact-installation authority, cross-core visibility,
AP entry as an external root, and per-CPU stack/state. Calling plans describe
stable regimes; checked instructions describe transitions between them.

## Required artifact report

A freestanding build reports normalized, package-qualified identities rather
than relying on friendly names:

- selected boundary requirements and providers;
- evaluated `CallPlan + StatePlan` identities;
- accepted target/environment assumptions and receipt identities;
- physical/mapped/executable extents and granted scopes;
- admitted artifact identities, symbolic materializations, placement
  constraints, installation phases, and visibility evidence;
- external roots, effect closure, stack domains, nesting/WCSU, and version pins;
- checked assembly footprint and any accepted leaf claims; and
- all remaining authority and linear obligations at image handoff.

The external-root portion is implemented as a live provider/runtime artifact,
not a guessed build-time table. `artifacts` writes
`external_roots.json` from the installed ledger, including its deterministic
snapshot fingerprint and complete normalized entry plans while omitting numeric
code addresses. Static builds may emit it at handoff; dynamic providers may
emit or attest fresh snapshots after later installations.

## Implementation boundary

Cathedral is the acceptance customer for this brief; it owns the UEFI, memory,
IDT, timer, and device lifecycles. Omega owns only the general checked assembly,
evaluated calling/state plans, materialization, entry lowering, final-footprint
validation, and installed-root machinery. `TASKS.md` owns their current order;
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md)
owns the general engineering contract. Boot work must not invent local grammar
to bypass either document or `OWNER_QUESTIONS.md`.
