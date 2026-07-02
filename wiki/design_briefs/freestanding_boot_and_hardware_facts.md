# Design Brief: Freestanding Boot & Hardware Facts — Worked Samples

Scouted 2026-07-02. Status: THEORETICAL — worked code samples, not decided
design. **All syntax is provisional.**

Chapter 18's "Freestanding Targets And Hardware Facts" records the direction —
freestanding = an empty host-provider set, boundary providers declare hardware
claims instead of OS claims — and honestly footnotes it as "largely
undesigned." The problem: the language has not yet *encountered* boot. Nothing
in the current corpus forces the entry-contract spelling, hardware-facts-as-
domains, region-capability minting, or interrupt entry to get concrete.

This brief forces the encounter: six samples walking a UEFI x86-64 handoff
from firmware jump to a running kernel, each ending with **what it forces the
language to decide**. Written from Cathedral's boot needs
(`Cathedral/wiki/design/part_5_lifecycle/02_boot_and_trust_chain.md`,
`04_kernel_architecture.md`); this is Tier-1 item 7 in
[cathedral_alignment.md](../cathedral_alignment.md).

The through-line: **Omega has no `unsafe` block, and boot does not add one.**
Every "unsafe" act below is a *declared, audited fact at an enumerable
boundary* — an axiom sourced from the UEFI spec or the Intel SDM instead of a
parent process — with proved code above it. Boot is the place with the most
axioms, not a different regime.

## Sample 1 — the entry provider: the UEFI handoff as declared guarantees

Who calls `main`, in what machine state? Today every target block names a host
package; a freestanding target instead names an **entry provider** whose
`ensures` are the firmware's spec-guaranteed handoff state.

```omega
// provisional syntax throughout

// UefiHandoff is NOT pub: this module declares exactly one minter — the entry
// provider — so holding one is proof control arrived via the declared handoff
// (the ch21 IrqCtx provenance pattern: private constructor, single minter).
data UefiHandoff {
    image_handle: EfiHandle;
    system_table: EfiSystemTablePtr;
}

boundary trait Uefi64Entry {
    // The firmware jump target. There is no caller to prove `requires` for —
    // the guarantees are ACCEPTED from the provider, audited against the
    // UEFI spec. This is the axiom set everything else descends from.
    machine entry() -> UefiHandoff
        ensures cpu in Cpu::LongMode
        ensures paging_active(Identity)      // UEFI: paging on, identity-mapped
        ensures interrupts in Irq::Enabled   // UEFI: interrupts are ON at entry
        ensures stack.aligned<16> and stack.len >= 128 * KiB
        ensures result.system_table.valid    // points at a live EFI system table
        effects device_io, memory_map;
}
```

**Forces the language to decide:**
- The target-declaration shape for "no host" — how a build names an entry
  provider instead of a host package.
- What an `ensures` over *machine state* (`cpu in Cpu::LongMode`,
  `interrupts in Irq::Enabled`) even is — these are not facts about values,
  they are facts about the world. Are they domains over distinguished
  zero-sized state objects? A new fact kind? (See sample 4.)
- Whether `entry()` is a machine at all — nothing calls it; hardware does. The
  same question as interrupt entry (sample 6) in its simplest form.

## Sample 2 — the memory map: untrusted bytes, a vouched meaning, and the exit dance

`GetMemoryMap` returns bytes. Parsing them is the ordinary untrusted-data path
(empty-invariant slice → validate into typed values). But the *meaning* —
"this range genuinely is free RAM" — no validation can conjure; it is the
provider's vouched claim. And UEFI's `ExitBootServices` key-matching dance is
a *freshness contract*: the map is only authoritative if boot services exited
against that exact map version.

```omega
// Plain data — a firmware struct never crosses a schema-evolution edge, so no
// identity numbers. Its BYTE layout comes from a layout policy (`Uefi`, the
// C-ABI/MS-x64 plan — see design_briefs/programmable_layouts.md); the schema
// itself stays layout-free. (An earlier draft wrongly wrote this as `wire data`
// with tag numbers — that would have implied Omega tag-framing over firmware
// bytes, which is exactly backwards.)
data EfiMemoryDescriptor {
    kind: u32;
    physical_start: u64;
    page_count: u64;
    attributes: u64;
}

// FinalMemoryMap is private; exit_boot_services is its only minter.
data FinalMemoryMap { entries: Vec<EfiMemoryDescriptor>; }

boundary trait BootServices {
    // Bytes with an EMPTY invariant set: the caller must validate every
    // descriptor before use (snapshot-then-validate — cannot typecheck
    // otherwise). The `ensures` is the audited SEMANTIC vouch: the firmware's
    // map describes physical memory. Validation checks shape; the axiom
    // supplies meaning.
    machine get_memory_map(buffer: &mut [u8, [writable]]) -> MapKey
        ensures buffer describes_physical_memory   // the axiom, spec-audited
        effects device_io;

    // The freshness dance: succeeds only against the CURRENT map key. On
    // success, firmware is gone and the map is final — mint the token.
    machine exit_boot_services(handoff: UefiHandoff, key: MapKey)
        -> FinalMemoryMap | StaleMap
        requires key.fresh
        ensures  result is FinalMemoryMap implies boot_services in Efi::Exited
        effects device_io;
}

// The origin-of-authority moment: the first Region capabilities are minted
// FROM the final map. Region is private with mint_regions the only minter, so
// every later memory authority descends from this call, by construction.
machine mint_regions(map: FinalMemoryMap) -> Vec<Region>
    requires boot_services in Efi::Exited
    ensures  forall r in result: r.exclusive and r.backed_by_ram
    effects  alloc;
```

**Forces the language to decide:**
- How a *semantic* provider vouch (`describes_physical_memory`) attaches to a
  value the caller then validates structurally — two different fact sources on
  one buffer.
- The `Region` minting story ch19 names open ("how a region capability is
  constructed at boot"): private-mint + a required world-fact
  (`Efi::Exited`) looks sufficient — confirm no new core feature is needed.
- Whether `MapKey.fresh` — a fact invalidated by *someone else's* later call
  to `get_memory_map` — is expressible, or the freshness stays a runtime
  `StaleMap` retry loop with no static help (UEFI reality: loop until it
  sticks).
- **Runtime record stride.** The UEFI spec *forbids* striding the map array by
  `sizeof(descriptor)` — firmware may append fields; you must stride by the
  runtime `DescriptorSize` it returns. So the walk's obligation must cite the
  handoff value, not the type's comptime size — a `::size`-based stride should
  *fail to discharge*, making the classic C bootloader bug untypeable. Derived
  static codecs assume fixed record size; this needs a runtime-stride walk form.
- **Function-pointer tables.** BootServices/RuntimeServices are structs of
  pointers called MS-x64 — `SystemTable → BootServices → GetMemoryMap` is a
  pointer read from a laid-out struct, then a call through it. The extern brief
  deliberately kept machine-as-C-function-pointer out of scope (bind-by-symbol
  only); boot re-opens it: the win64 call encoder pointed at a runtime pointer
  instead of an import symbol.

## Sample 3 — an MMIO region and volatile operators

Chapter 19's direction: device memory is reached through boundary operators
with volatile contracts (each source-level access happens exactly once, at
declared width, in program order relative to other volatile accesses on the
same region) — never ordinary loads a compiler may coalesce.

```omega
// A serial UART at a fixed physical region, claimed from a minted Region.
// MmioRegion<Uart16550> is private-minted by claim_mmio, so holding it is
// proof the region was carved from boot authority, not fabricated.
machine claim_mmio<Dev>(r: Region, base: u64, len: u64) -> MmioRegion<Dev>
    requires r.exclusive and r.contains(base, len)
    effects  memory_map;

boundary trait Mmio {
    machine read8(region: &MmioRegion<Dev>, offset: u64) -> u8
        requires offset < region.len
        ensures  access.exactly_once and access.width<8>
                 and access.ordered_within(region)
        effects  device_io;

    machine write8(region: &mut MmioRegion<Dev>, offset: u64, value: u8)
        requires offset < region.len
        ensures  access.exactly_once and access.width<8>
                 and access.ordered_within(region)
        effects  device_io;
}

// A driver machine over it — ordinary proved Omega above the boundary.
machine uart_put(uart: &mut MmioRegion<Uart16550>, byte: u8)
    effects device_io
{
    loop bounded 65536 {                       // the totality-brief bound
        if Mmio::read8(uart, LSR) & THR_EMPTY != 0 { break; }
    }
    Mmio::write8(uart, THR, byte);
}
```

**Forces the language to decide:**
- The operator surface: one generic `read8/write8` pair over `MmioRegion<Dev>`
  (above) vs per-register typed operators generated from a device description.
  The generic pair is the smaller language ask.
- What `access.exactly_once` / `ordered_within(region)` are as contract
  vocabulary — these constrain the *compiler's own lowering*, not a runtime
  value. A new contract kind: guarantees about emitted code.
- Confirming ch19's note that volatile ordering is *not* hardware ordering —
  fences/barriers stay separate boundary machines with their own contracts.

## Sample 4 — CR3, MSRs, and hardware facts that later code requires

The contract-heavy asm form (ch22) is the vehicle; the open question ch18
names — "is 'paging enabled' a fact a provider establishes and later providers
require?" — is the design content. The sharp detail: `mov cr3` has a real
*requires* (the new table must map the currently-executing code, or the next
fetch faults) that only the proof layer above can discharge.

```omega
machine load_page_table(table: PageTableRoot) 
    requires table in PageTable::Valid
    requires table.maps_executing_code        // or the next instruction faults
    ensures  paging_active(table)             // a WORLD fact, not a value fact
    effects  memory_map
{
    asm where
        requires table.phys.aligned<4096>
        clobbers tlb                          // the WHOLE point of the write
    {
        mov cr3, table.phys
    }
}

machine write_msr(msr: MsrId, value: u64)
    requires msr in Msr::KnownWritable        // no blind wrmsr
    effects  device_io
{
    asm where
        requires target_feature<msr>
        ensures  msr_state(msr) == value
    {
        wrmsr
    }
}

// Later code REQUIRES the established world fact:
machine map_kernel_heap(...) 
    requires paging_active(kernel_table)
    ...
```

**Forces the language to decide:**
- **World facts as first-class:** `paging_active(table)`, `msr_state(msr)`,
  `interrupts in Irq::Masked` are facts about machine state — established by
  one machine's `ensures`, required by another's `requires`, *invalidated* by
  a third (the next `load_page_table` kills the old `paging_active`). That is
  affine/state-like fact flow, not value invariants. This is the single
  biggest gap the samples surface — nothing in the current fact system holds,
  threads, or revokes a global machine-state fact.
- `clobbers tlb` — clobber vocabulary beyond registers (TLB, caches, pipeline
  serialization).
- `table.maps_executing_code` — a proof obligation relating a data structure
  (the page table) to the *current instruction pointer*. Stateable? Or is this
  exactly where a human `boundary` assertion is honest?

## Sample 5 — interrupt masking: a bracketed world fact

UEFI hands over with interrupts *enabled*, so masking is among the first real
acts. "This sequence masks interrupts until the matching unmask" is a
*scoped* world fact — the ch21 provenance pattern again, plus drop-semantics.

```omega
// IrqGuard is private; mask() is its only minter. Holding it IS the fact
// "interrupts are masked". Dropping it (or unmask()) revokes the fact.
machine mask() -> IrqGuard
    ensures interrupts in Irq::Masked
    effects device_io
{
    asm where ensures interrupts in Irq::Masked { cli }
}

machine unmask(guard: IrqGuard)
    ensures interrupts in Irq::Enabled
    effects device_io
{
    asm where ensures interrupts in Irq::Enabled { sti }
}

// A machine that must not be preempted takes the guard as evidence:
machine switch_context(guard: &IrqGuard, ...) ...
```

**Forces the language to decide:**
- Whether holding-a-value-as-evidence-of-a-world-fact (the guard) is *the*
  blessed encoding of scoped machine state — it composes with ownership (the
  fact dies when the guard drops) and needs no new core feature — or whether
  world facts (sample 4) need their own tracking and the guard is sugar.
- Nesting/re-entrancy: two guards, the inner unmask must not unmask — a
  counting guard is a library answer *if* the evidence pattern is blessed.

## Sample 6 — interrupt entry: hardware calls a machine (mostly a question)

The least designed, recorded here mostly to hold its shape. Hardware pushes a
frame and vectors to an address: something must declare that convention and
enter the state graph.

```omega
boundary trait InterruptEntry {
    // The provider declares what the CPU did before our first instruction:
    // what is on the stack, what is masked, alignment. The handler machine is
    // entered with a typed frame minted by the vector stub.
    machine on_vector(vector: u8, frame: &mut TrapFrame)
        // Accepted, not proved: the CPU pushed this frame and masked delivery
        // for this vector before we ran. We cannot prove what hardware did.
        ensures interrupts in Irq::Masked        // arrived with delivery off
        ensures frame.saved_rip.valid
        effects device_io;
}

// A concrete handler is ordinary proved Omega — but note what it CANNOT assume:
// the code it interrupted holds borrows and world facts this handler did not
// establish. `&mut self` on a preempted machine is the open wound.
machine timer_tick(entry: &InterruptEntry, frame: &mut TrapFrame)
    requires interrupts in Irq::Masked           // handler runs masked
    effects device_io
{
    // ... acknowledge the interrupt controller, bump the tick, maybe reschedule
    Mmio::write8(apic, EOI, 0);
}
```

**Forces the language to decide:**
- The **entry convention**: hardware, not a caller, transfers control — sample
  1's `entry()` shape again, but re-entrant and mid-execution. The vector stub
  (asm) mints the typed `TrapFrame` and calls the handler machine — is that
  stub expressible, or is it irreducibly a hand-audited boundary blob?
- **The `&mut self`-under-preemption question ch18 flags directly:** a handler
  runs *inside* another machine's execution. What happens to the interrupted
  machine's in-flight borrows and its established world facts (sample 4)? The
  honest answer is probably "the affected region ran under sample 5's mask
  guard, so no machine is preempted mid-borrow" — which means **world facts and
  the borrow checker together define what an interrupt may touch.** A real
  interaction to design, not a stub.
- Whether nested/prioritized interrupts are a language concern or pure runtime
  config (interrupt-controller priorities).

## What the six samples force, rolled up

Boot does not want a new `unsafe` regime. Across all six samples, exactly **one
genuinely new fact kind** recurs; everything else is existing machinery pointed
at hardware.

**The one real gap — world facts.** `cpu in Cpu::LongMode`,
`paging_active(table)`, `interrupts in Irq::Masked`, `boot_services in
Efi::Exited`, `msr_state(msr) == v` are facts about *machine state*, not about
values. They are **established** by one machine's `ensures`, **required** by
another's `requires`, and **invalidated** by a third (the next
`load_page_table` kills the prior `paging_active`; `unmask` kills
`Irq::Masked`). That is affine/state-threaded fact flow — nothing in the
current value-invariant system holds, threads, or revokes a global state fact.
This is the load-bearing decision the brief exists to surface. The candidate
encoding that needs no new core feature: **hold-a-value-as-evidence** (sample
5's `IrqGuard`) — a private-minted token whose ownership *is* the fact, so
ownership/drop already gives establish/revoke. Whether that also covers the
*non-scoped* facts (`paging_active` has no natural guard lifetime) or those
need first-class world-fact tracking is the question to answer first.

| Sample | Mechanism exercised | New language ask? |
|---|---|---|
| 1 Entry provider | `boundary trait`, `ensures` over world state, no-host target | Target-decl for "no host"; world-fact `ensures` → **the gap** |
| 2 Memory map | untrusted-bytes validation + semantic vouch; private-mint Region | Two fact sources on one buffer; `MapKey.fresh` — mostly existing |
| 3 MMIO / volatile | boundary operators, volatile contracts | `exactly_once` / `ordered_within` = contracts on *lowering* — new contract kind |
| 4 CR3 / MSR | contract-heavy asm, established world facts | **World facts → the gap**; `clobbers tlb` vocabulary |
| 5 IRQ mask | provenance guard = scoped world fact | Bless evidence-as-fact, or world facts need own tracking |
| 6 IRQ entry | hardware-enters-machine; `&mut self` under preemption | Entry convention; borrow × world-fact under preemption |

Everything not in the "the gap" rows is either the **existing provenance
pattern** (private type + single minter = "holding this proves how it was
obtained", already used for `IrqCtx` in ch21) or the **existing boundary/asm
machinery** (ch18 providers, ch22 instruction contracts, ch19 volatile) simply
declaring hardware axioms instead of OS ones.

**Sequencing.** None of this blocks current compiler work. It should be
*designed* before the freestanding target and interrupt model are implemented,
because the world-fact representation is shared with ordinary `requires` /
`ensures` and constrains the fact system as a whole. Recommended first move:
take up **world facts** as its own fact-system decision, with the
evidence-token pattern as the leading candidate and `paging_active` (the
unscoped case) as the test that decides whether tokens suffice.

## Status

THEORETICAL — worked samples to force the encounter, not decisions. Feeds
Tier-1 item 7 in `cathedral_alignment.md`. When world facts get a real design
call, record it in TASKS.md and collapse the relevant rows above.