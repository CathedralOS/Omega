# Design Brief: Freestanding Boot And Hardware Facts

Current direction as of 2026-07-18. Freestanding selection and the security
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
   `StatePlan`, stack/preemption class, effect ceiling, and acknowledgement
   protocol;
4. an ordinary `boundary machine ... satisfies ...` handler;
5. provider/build selection of the handler;
6. symbolic entry-stub identity resolved by a phase-aware materializer;
7. checked `lidt` installation under IDT authority; and
8. a linear acknowledgement token for exactly-once EOI.

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
part of code placement. Installation prevents injection; sealed entries and
protected-return/final CFI validation remain a separate control-transfer gate.

The initial image uses the same trust discipline at an earlier phase: the build
checks PCC/CFI and signs the admitted artifact identity, secure boot
authenticates that identity and gates entry, and measured boot records what
entered. The boot-admitted installer then loads later admitted artifacts.
Measurement is evidence, never the admission gate.

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

## Status and remaining work

The first two Cathedral milestones already validate the typed UEFI entry,
runtime firmware-table calls, memory-map walk, `ExitBootServices`, first
physical-Extent mint (still spelled `Region` in the bootstrap sample), port I/O,
and `hlt` path. The timer/IDT slice is not yet specified or
implemented end to end.

The next implementation order is:

1. full parsed checked-assembly frontend and initial x86 catalog;
2. generic trait-parent composition for `Calling<C>`;
3. fragmented layouts and symbolic materialization;
4. `CallPlan + StatePlan` entry-stub derivation and footprint validation;
5. external-root ledger and IDT/timer slice; and
6. placed views, external loans, and the wider driver gauntlet.

Open design details live in
[`os_memory_and_hardware_foundation.md`](os_memory_and_hardware_foundation.md)
and `OWNER_QUESTIONS.md`. Implementations must not invent local grammar to
bypass them.
