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

Program storage begins from a small number of entry-provisioned content roots.
The loader, firmware, or OS admits the image mapping and initial stack/storage
roots as part of the typed entry handoff. The compiler derives statics and
sections as subextents of the image root; later frames and task stacks are
checked allocations from an existing root. It does not admit every object
individually, and a static array cannot originate physical-memory content merely
because its size is known.

Those roots use one core-owned stable program-storage entry requirement. Its
exact qualified parameter positions identify the image and initial storage
roots. Target entry traits such as `UefiApplication::entry` inherit that
semantic requirement; `Calling<C>`, target policy, and generated stubs refine
its plan and ABI without replacing its identity. `Extent::Granted` authorizes
the core requirement as an alternative route, and installation introduces the
matching parameters. Core therefore never depends on a UEFI/Cathedral domain,
and the compiler never recognizes target-friendly names as storage authority.

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
    `StatePlan`, stack/preemption class, service/suspension/blocking ceilings,
    and acknowledgement protocol;
4. an ordinary `boundary machine ... satisfies ...` handler;
5. provider/build selection of the handler;
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
validation checks geometry. Cathedral's separate IDT validator checks
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
The ordinary artifact writer now exposes the only compiler-packaging seam for
this form: it accepts an already-normalized `Artifact` plus exact proof bytes,
invokes the canonical encoder, and atomically installs the resulting file. It
does not accept a final PE/ELF/Mach-O image or a caller-selected byte buffer as
an executable candidate.
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
report, and fingerprint. Retaining and translating the compiler's semantic
code, relocation, contract, footprint, placement, and entry facts into that
packaging seam remains engineering, as does wrapping the canonical result in
the target's firmware envelope.

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

The remaining hardware-foundation engineering contract lives in
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md).
Unrelated unresolved source-language decisions remain in
`OWNER_QUESTIONS.md`; this boot sequence must not invent local grammar to bypass
either document.
