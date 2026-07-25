# Design Brief: Freestanding Boot And Hardware Facts

Current direction as of 2026-07-22. Freestanding selection and the security
model are settled. The reusable memory/hardware primitives are specified in
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md);
their source APIs and backend support remain incomplete.

## Freestanding is a target/provider property

```omega
machine build(b: &mut Build, fs: &mut Filesystem) {
    b.freestanding = true;
    b.entry = Main::run;
}
```

A freestanding target has no ambient hosted provider set. Firmware, memory,
device, clock, scheduling, entry, and exit services must be selected from
explicit target packages and admitted providers. `build.omg` supplies defaults
and may override individual provider slots; this is ordinary deployment data,
not compiler folklore.

## Typed entry handoff

The image exports an ordinary boundary callable. The target requirement pins
its complete evaluated calling and machine-state plan; the generated entry stub
translates the platform arrival into typed Omega values.

```omega
data UefiHandoff {
    image: EfiHandle;
    system_table: &mut EfiSystemTable;
}

boundary machine Main::run(
    handoff: UefiHandoff,
) -> BootOutcome
    satisfies UefiApplication::entry
{
    ...
}
```

The stub/provider contract states pointer provenance, alignment, lifetime,
paging and CPU regime, stack demand, entry/exit control, and any facts the
checker cannot derive. Accepted facts appear in receipts and the boundary
report. The old `boundary(<Plan>)` syntax is retired; plan identity belongs to
the satisfied requirement through `Calling<C>`.

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

UEFI's map/exit protocol is a linear state transition:

1. obtain explicit storage for the runtime-sized map;
2. read descriptors using firmware's reported stride/version;
3. retain the associated `MapKey`;
4. call `ExitBootServices`;
5. retry with new capacity/key when firmware requests it; and
6. on success, consume firmware authority and establish one
   `FinalMemoryMap` obligation.

No destructor pretends to perform this fallible protocol. Descriptor stepping
uses the supplied stride, not `sizeof`, and every typed projection is proven
inside the returned byte extent.

The final map supplies physical extents, not freely forged allocator values.
A bounded `Arena` is allocation authority over appropriate backing extents. Device,
reserved, firmware, and executable ranges retain different provenance and
rights. Metadata capacity is explicit.

## Placed hardware storage

MMIO, page tables, descriptor tables, framebuffers, DMA buffers, and shared IPC
pages use the same composition:

```text
Extent capability + validated LayoutPlan + validated AccessPlan
    -> sealed placed view / field-access values
```

`Extent` owns authority over a concrete range. `LayoutPlan` owns bit geometry.
`AccessPlan` owns primitive transfer semantics and statically pinned service
reach. There is no general `Mmio<T>` magic wrapper and no public arbitrary-offset
volatile primitive.

Device-specific operations such as W1C remain library machines over private
field access. Page-table mapping additionally requires frame authority and
establishes an `Installable` domain before checked installation. DMA lends
extents to an invisible borrower represented by a linear completion token.

## Checked assembly

OS code uses parsed `asm {}` under compiler-known instruction contracts. The
first freestanding catalog must cover the actual x86 bringup path: interrupt
mask save/restore, `hlt`, port I/O, descriptor-table loads, control registers,
MSRs, fences/cache maintenance, atomics, and mode/entry transitions.

Contracts emit service reach, authority requirements, register/flag/memory
changes, ordering, regime changes, and exits. Direct assembly cannot be quieter
than a boundary-trait operation. Unknown instructions and raw emitted bytes are
rejected; trusted foreign blobs use provider admission.

Entry/exit-only operations such as `iretq` are deriver-only. Interrupt masking
is an ordinary linear save/restore token, distinct from a scheduler-switch
guard. Both restore prior state explicitly; neither relies on drop timing.

## Interrupt entry and the IDT

The language does not have an interrupt declaration or IDT DSL. The x86 IDT
vertical slice composes the common pieces:

1. ordinary `data` for the gate schema;
2. an x86 layout policy with bit and fragmented placements;
3. a target-specific boundary requirement carrying `Calling<C>`, `CallPlan`,
    `StatePlan`, stack/preemption class, service/suspension/blocking ceilings,
    and acknowledgement protocol;
4. an ordinary `boundary machine ... satisfies ...` handler;
5. provider/build selection of the handler;
6. symbolic entry-stub identity resolved by a phase-aware materializer;
7. a generated checked writer producing a validated, content-bound
   `MaterializedIdt` from an exclusive unpublished placement;
8. separate checked `lidt` installation under `IdtControl`, with roots recorded
   before hardware reachability; and
9. a linear acknowledgement token for exactly-once EOI.

The provider-neutral obligation spelling is live in
`omega::language::core::interrupt`: the mask guard and acknowledgement are
distinct opaque linear values with consuming `restore` and `complete`
operations. Their normalized provider mint and entry settlement are live in
the installed-root ledger: an exact entry receipt binds the installed root,
selected provider execution, invocation, and acknowledgement policy before the
opaque values exist; replay rejects; nested mask guards restore exact prior
states in LIFO order; and exit requires both the entry mask state and the exact
completed acknowledgement. Cathedral now carries pure xAPIC/x2APIC register
and timer encodings alongside its PIC/PIT facts. Those values grant no MMIO/MSR
authority and deliberately leave frequency enumeration/calibration to the
selected provider. Cathedral's checked x2APIC helpers now program one-shot
mode, arm/stop, and EOI with parsed `wrmsr` contracts retaining
`MachineControl` reach. They cannot enable x2APIC/IF or publish a root. The
concrete PIC/LAPIC providers still owe execution of the normalized entry and
acknowledgement transitions in their generated paths.

The selected provider plan may keep entry identity private for static tables;
the program does not need a source-visible function pointer or numeric code
address. Installation records the handler as an external root. The root ledger
then includes its reach, trust receipt, state plan, stack domain, nesting graph,
and version/liveness pins.

Stack/preemption class is authored once and drives both the gate's concrete IST
field and WCSU composition. Two separately-authored facts would be unsound.

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

The generated writer is not a general table-construction or address-resolution
API. It receives one exact mapped/pinned/writable unpublished placement and a
sealed resolver restricted to the boot-admitted artifact's root set. It writes
that destination directly; failure produces no established table claim.
Layout validation checks geometry, while the target IDT validator separately
checks selectors, gates, privilege levels, IST assignments, reserved bits, and
the exact admitted roots. Only then may the writer produce `MaterializedIdt`.

The writer does not hold `IdtControl` and cannot publish. A separate installer
prepares the external-root records, completes required visibility, executes
checked `lidt`, and returns `InstalledIdt` plus its installation receipt. Root
records precede hardware reachability. The live preparation carrier is sealed:
it is minted only for the exact materialized content/destination, live root
handles, ledger fingerprint, and `IdtControl`. Compiler lowering from that
carrier produces the generated-only target/machine `lidt [r10]` operation with
its retained identities and exact R10 + control-state footprint. The
materialization receipt binds the writer, plan, artifact, entries, destination,
and exact final content bytes; the installation receipt separately binds the
granted CPU/table scope, prepared roots, visibility, and `lidt` operation while
retaining the exact materialized table evidence prepared for publication.
Compact FNV fingerprints remain audit/report identities, not authorization.

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
`MaterializedIdt`.

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
that initial authority into exact placements, the sealed artifact resolver,
and CPU-scoped `IdtControl`.

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
`omega-executable-installation`: reusable exact-evidence admission; one-shot
Extent-backed placement; frozen materialization; artifact/placement/final-byte/
footprint-bound validation; scoped installation authority; synchronous
visibility; and explicit W^X enforcement reporting. Every failed consuming
transition returns its authority inputs. The reusable artifact retains exact
code and canonical relocations. Its pure provider materializer resolves only
sealed targets, patches a private copy with checked target semantics, validates
AArch64 instruction shapes, and derives a placement/content-bound final-byte
identity without acquiring destination-write or execute authority.
Materialization receipts retain that complete canonical output, while final
validation evidence is minted from and retains the exact frozen artifact and
byte snapshot; compact normalized identities remain report keys rather than
collision-resistant authority. Installation and retirement continue that rule:
their authorities and receipts retain the complete validated placement or
installed realization, including exact bytes, Extent authority facts, scope,
audience, validation, and W^X state. Compact lifecycle IDs never substitute for
that evidence. The
schema-driven native-container decoder, real PCC and final-code validators,
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
distinct linear obligations with opposite lifecycle roles. V1 completes
visibility inside the loader; it exposes no asynchronous token without a real
provider customer.

Every executable page-table mapping and relevant checked-assembly operation
requires admitted-artifact provenance. There is no `ExecutableMemory`
capability, JIT path, self-modifying code, or alternate raw-byte route.
Component-slot binding is a later logical dispatch/versioning operation, not
part of code placement. Installation prevents injection. Backward-edge returns
in checked Omega remain compiler-owned, non-addressable control state across
both execution and parking. Forward-edge indirect calls separately require
sealed requirement-compatible entry references or descriptors.

The normalized Omega-native container byte decoder and validator are live.
The decoder uses the ordinary validated scalar-layout consumer rather than a
bespoke pointer parser. Its canonical little-endian v2 form is deliberately
small:

- a 64-byte `OMEGAXE2` header fixes version, architecture, total length,
  artifact/content identities, and a section count;
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
exact copy of every semantic section and derives the normalized executable-
content and proof identities. The result is an immutable admission candidate,
never executable eligibility.

Optional known and unknown informational sections remain opaque and
identity-invisible to executable admission, but they are not allowed to
self-name: the decoder derives their trace identity from the section kind and
exact bytes and rejects a directory restatement that differs. This preserves
normalizer-owned reporting identity without granting the payload semantic or
admission authority.

The inverse compiler-side encoder is live over the same layout records. It
emits only the seven required semantic sections in canonical order, derives the
proof identity from the exact payload, checks configured section/relocation/
total-size bounds before allocation, and routes its completed bytes back
through the hostile-input decoder before returning them. Producer and consumer
therefore share one schema and fail closed on drift; optional informational
decoration is intentionally a later packaging step with no admission role.
Verifier evidence retains that exact immutable candidate rather than using its
compact FNV identities as collision-resistant authority; the proof-payload
identity is normalizer-derived from and retained beside the exact proof bytes,
while informational sections remain authority-free.
Normalization binds the exact code bytes, instruction-set architecture,
contracts, footprint, placement, entries, and canonical relocations into
content identity; proof evidence remains outside that promise. The artifact
retains its immutable bytes, architecture, and canonical relocation set through
admission, and relocation lowering rejects cross-architecture substitution even
when a relocation kind is otherwise shared. Signed relocation addends survive
the validated artifact, canonical materializer, object plan, image application,
report, and fingerprint. Connecting the canonical encoder to final compiler
artifact packaging and wrapping its result in the target's firmware envelope
remain engineering.

The initial image uses the same trust discipline at an earlier phase: the
current trusted build validates the artifact and signs its admitted identity,
secure boot authenticates that identity and gates entry, and measured boot
records what entered. The boot-admitted installer then loads later admitted
artifacts. Future independent PCC/final-byte validation reduces reliance on the
compiler; it is not a prerequisite for the v1 boot semantics. Measurement is
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
not a guessed build-time table. `omega-artifacts` writes
`external_roots.json` from the installed ledger, including its deterministic
snapshot fingerprint and complete normalized entry plans while omitting numeric
code addresses. Static builds may emit it at handoff; dynamic providers may
emit or attest fresh snapshots after later installations.

## Status and remaining work

The first two Cathedral milestones already validate the typed UEFI entry,
runtime firmware-table calls, memory-map walk, `ExitBootServices`, first
physical-Extent mint, port I/O,
and `hlt` path. The timer/IDT slice is not yet specified or
implemented end to end.

Generic trait-parent composition for `Calling<C>` and source-policy evaluation
are implemented. The compiler discovers concrete policy relationships,
evaluates `CallingPolicy::plan` through the build-time interpreter, validates
and canonicalizes accepted `CallPlan + StatePlan` results, publishes only the
evaluated-plan fingerprint, and retains the complete plan for lowering. The
remaining implementation order is:

1. complete the checked-assembly catalog required by the entry provider;
2. finish IDT1 after the implemented symbolic, phase-aware direct-destination
   materializer and checked `lidt` carrier: insert and execute the generated
   checked Omega writer/load helpers through their plan-selected opaque
   pointers in the provider (R10 materialization and the private packed IDTR
   descriptor are already emitted/sealed);
3. `CallPlan + StatePlan` entry-stub derivation, state-ceiling-aware codegen,
   footprint evidence, and final-artifact validation;
4. external-root ledger and IDT/timer slice; and
5. placed views, external loans, and the wider driver gauntlet.

Open design details live in
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md)
and `OWNER_QUESTIONS.md`. Implementations must not invent local grammar to
bypass them.
