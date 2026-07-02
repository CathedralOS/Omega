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
    // The firmware jump target. There is no caller to prove `requires` for.
    // The initial machine state is an AUDITED AXIOM LIST — prose statements
    // checked by a human against the UEFI spec and enumerated in the build
    // artifact next to the provider, NOT typed ensures clauses:
    //
    //   axiom: CPU is in long mode with paging on, identity-mapped
    //          (= the state this target's codegen assumes; audited once,
    //          never threaded through proofs — a TARGET invariant)
    //   axiom: interrupts are enabled at entry (so masking is an early act)
    //   axiom: >= 128 KiB of 16-aligned stack
    //   axiom: result.system_table points at the live EFI system table
    //          (provenance only — the STRUCTURE is minted, sample 2)
    //
    machine entry() -> UefiHandoff
        effects device_io, memory_map;
}
```

An earlier draft wrote these axioms as typed clauses (`ensures cpu in
Cpu::LongMode`, `ensures paging_active(Identity)`) — wishcasts: there is no
`Cpu` value, no mint, no checker story. The trust *is* the boundary; the
machine-checkable things downstream are mints on structures, invariants on
values, and tokens for transitions (see the roll-up).

**Forces the language to decide:**
- The target-declaration shape for "no host" — how a build names an entry
  provider instead of a host package.
- The form of the entry axiom list: prose in the provider declaration,
  enumerated in the boundary report like unchecked-assembly obligations —
  probably no new mechanism beyond a doc-comment convention the artifact
  surfaces.
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
    // otherwise). Validation checks SHAPE. The MEANING — "this map genuinely
    // describes physical memory" — is not a postcondition; it is the decision
    // to trust firmware, i.e. the boundary itself. (An earlier draft wrote
    // `ensures buffer describes_physical_memory` — a wishcast: unfalsifiable
    // from inside; not a fact any type could hold.)
    machine get_memory_map(buffer: &mut [u8, [writable]]) -> MapKey
        effects device_io;

    // The freshness dance: succeeds only against the CURRENT map key —
    // a runtime retry loop returning a sum (UEFI reality: loop until it
    // sticks). On success, firmware is gone and the map is final: MINT THE
    // TOKEN. FinalMemoryMap is the evidence — holding it IS "boot services
    // have exited"; no ambient fact needed.
    machine exit_boot_services(handoff: UefiHandoff, key: MapKey)
        -> FinalMemoryMap | StaleMap
        effects device_io;
}

// The origin-of-authority moment: the first Region capabilities are minted
// FROM the final map. Region is private with mint_regions the only minter,
// and the FinalMemoryMap token is the precondition — every later memory
// authority descends from this call, by construction, with zero ambient
// state consulted.
machine mint_regions(map: FinalMemoryMap) -> Vec<Region>
    ensures  forall r in result: r.exclusive and r.backed_by_ram
    effects  alloc;
```

**Forces the language to decide:**
- ~~Two fact sources on one buffer~~ — RESOLVED: the mint supplies the
  structural fact; the semantic "meaning" is not a fact at all, it is the
  boundary's trust decision, enumerated in the audit trail. Facts attach where
  they're true.
- The `Region` minting story ch19 names open ("how a region capability is
  constructed at boot"): private mint gated by the `FinalMemoryMap` evidence
  token — CONFIRMED sufficient, no new core feature.
- `MapKey.fresh` stays a runtime `StaleMap` retry with no static help — a
  fact invalidated by someone else's later call is not worth a type.
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

## Sample 4 — CR3 and MSRs: value invariants and owned state, not world facts

The open question ch18 names — "is 'paging enabled' a fact a provider
establishes and later providers require?" — resolves **no**. Chase every
would-be consumer of `paging_active(table)` and it wants one of two existing
things:

- *"The new table must map the currently-executing kernel code (or the next
  fetch faults)"* — an **invariant of the table value**: every `PageTableRoot`
  is constructed from the kernel prototype (higher-half mappings shared by
  construction), so safety is minted where the table is *built* and
  `load_page_table` is safe for any well-typed argument. Constrain the value,
  don't assert the world.
- *"Which table is live right now"* — the kernel is a program; "current X" is
  a **field it owns**: `per_cpu.address_space`. Loading CR3 is a method on
  that owned structure (swap + TLB maintenance as one operation — a data
  structure maintaining its own hardware shadow). Virt↔phys translation is a
  function of the table object you hold, not an ambient registry.

```omega
// PageTableRoot's INVARIANT carries the safety condition; minted at build.
machine load_page_table(space: &mut per_cpu.AddressSpace, table: PageTableRoot)
    effects memory_map
{
    asm where
        requires table.phys.aligned<4096>
        clobbers tlb                          // the WHOLE point of the write
    {
        mov cr3, table.phys
    }
    space.current = table;                    // owned state, ordinary assignment
}

machine write_msr(msr: MsrId, value: u64)
    requires msr in Msr::KnownWritable        // no blind wrmsr
    effects  device_io
{
    asm where requires target_feature<msr> { wrmsr }
    // No `ensures msr_state(msr) == value` — config writes are EFFECTS, not
    // facts. Downstream code never proves things from an MSR's value; the
    // write's consequence is hardware behavior. Tracking register state in
    // the type system is modeling the whole machine — the wishcast line.
}
```

**Forces the language to decide:**
- `clobbers tlb` — clobber vocabulary beyond registers (TLB, caches, pipeline
  serialization). The one genuine ask left in this sample.
- Nothing else: no world-fact machinery, no `maps_executing_code` obligation
  against the instruction pointer (dissolved into the table type's
  construction invariant).

## Sample 5 — interrupt masking: an evidence token

UEFI hands over with interrupts *enabled*, so masking is among the first real
acts. "This sequence masks interrupts until the matching unmask" is a *scoped*
machine-state transition — and the ch21 provenance pattern plus drop-semantics
carries it whole. No typed "world fact" anywhere: the token IS the fact.

```omega
// IrqGuard is private; mask() is its only minter. Holding it IS
// "interrupts are masked". Dropping it (or unmask()) revokes it.
machine mask() -> IrqGuard
    effects device_io
{
    asm { cli }        // the asm's contract is the instruction's; the FACT
                       // lives in the minted token, not an ensures clause
}

machine unmask(guard: IrqGuard)
    effects device_io
{
    asm { sti }
}

// A machine that must not be preempted takes the guard as evidence:
machine switch_context(guard: &IrqGuard, ...) ...
```

**Resolved (was open):** evidence-as-fact IS the blessed encoding of scoped
machine state — it composes with ownership (the fact dies when the guard
drops) and needs no new core feature. Nesting/re-entrancy (the inner unmask
must not unmask) is a counting guard — a library answer, now that the pattern
is blessed.

## Sample 6 — interrupt entry: hardware calls a machine (mostly a question)

The least designed, recorded here mostly to hold its shape. Hardware pushes a
frame and vectors to an address: something must declare that convention and
enter the state graph.

```omega
boundary trait InterruptEntry {
    // The provider's axiom list (prose, audited against the SDM, enumerated
    // in the artifact — same form as sample 1's entry axioms):
    //   axiom: the CPU pushed this frame and masked delivery for this vector
    //          before our first instruction
    // The vector stub mints BOTH values: the typed frame AND an IrqGuard —
    // "arrived masked" is evidence the handler HOLDS, not a clause.
    machine on_vector(vector: u8, frame: &mut TrapFrame, guard: IrqGuard)
        effects device_io;
}

// A concrete handler is ordinary proved Omega — but note what it CANNOT
// assume: the code it interrupted holds borrows this handler did not make.
// `&mut self` on a preempted machine is the open wound.
machine timer_tick(frame: &mut TrapFrame, guard: &IrqGuard)   // masked: evidenced
    effects device_io
{
    // ... acknowledge the interrupt controller, bump the tick, maybe reschedule
    Mmio::write8(apic, EOI, 0);
}
```

**Forces the language to decide:**
- The **entry convention**: hardware, not a caller, transfers control — sample
  1's `entry()` shape again, but re-entrant and mid-execution. The vector stub
  (asm) mints the typed `TrapFrame` + `IrqGuard` and calls the handler machine
  — is that stub expressible, or is it irreducibly a hand-audited boundary
  blob?
- **The `&mut self`-under-preemption question ch18 flags directly:** a handler
  runs *inside* another machine's execution. What happens to the interrupted
  machine's in-flight borrows? The honest answer is probably "the affected
  region ran under sample 5's mask guard, so no machine is preempted
  mid-borrow" — which means **guard tokens and the borrow checker together
  define what an interrupt may touch.** A real interaction to design, not a
  stub.
- Whether nested/prioritized interrupts are a language concern or pure runtime
  config (interrupt-controller priorities).

## What the six samples force, rolled up

Boot does not want a new `unsafe` regime — **and it does not want world facts
either.** The first draft of this brief named "world facts" (ambient
machine-state predicates established/required/revoked across machines) as its
#1 gap. Deflated 2026-07-02: chase every would-be consumer and it wants one of
four **existing** things. Nothing true is ambient.

| Would-be "world fact" | Actual consumer need | Mechanism (exists) |
|---|---|---|
| `boot_services in Efi::Exited` | gate `mint_regions` | **evidence token** (`FinalMemoryMap` — private mint; holding it is the fact) |
| `interrupts in Irq::Masked` | non-preemptible sections | **evidence token** (`IrqGuard`, threaded as a borrow; drop = revoke) |
| `cpu in Cpu::LongMode` | every instruction | **target invariant** — the state codegen assumes; audited once at the entry boundary, never threaded |
| `msr_state(msr) == v` | ~nobody as a premise | config writes are **effects, not facts**; consequences are hardware behavior, not propositions |
| `paging_active(table)` | table safety; "what's live" | **value invariant** (tables built from the kernel prototype) + **owned state** (`per_cpu.address_space`) |

The residue at the entry boundary is an **audited axiom list** — prose checked
by a human against the spec, enumerated in the build artifact next to the
provider (the same visibility discipline as unchecked-assembly obligations) —
never typed `ensures` clauses pretending to flow. The wishcast to guard
against: writing a machine-state predicate as if some value carried it.

| Sample | Mechanism exercised | Remaining language ask |
|---|---|---|
| 1 Entry provider | boundary trait + audited axiom list | target-decl for "no host"; axiom-list surfacing in the artifact |
| 2 Memory map | mint for shape, boundary for trust; token-gated Region mint | none new (`MapKey.fresh` stays a runtime retry) |
| 3 MMIO / volatile | boundary operators, volatile contracts | `exactly_once` / `ordered_within` = contracts on *lowering* — new contract kind |
| 4 CR3 / MSR | value invariants + owned per-CPU state | `clobbers tlb` — clobber vocabulary beyond registers |
| 5 IRQ mask | evidence token (blessed) | none — counting guards are a library |
| 6 IRQ entry | vector stub mints frame + guard | entry convention; borrow-checker × preemption interaction |

Everything above is the **existing provenance pattern** (private type + single
minter, ch21's `IrqCtx`) or the **existing boundary/asm machinery** (ch18
providers, ch22 instruction contracts, ch19 volatile) declaring hardware
axioms instead of OS ones — plus the layout-policy machinery
(`programmable_layouts.md`) for every foreign struct in the chain.

**Sequencing.** None of this blocks current compiler work. The remaining asks
are the no-host target/entry spelling, the lowering-contract vocabulary
(volatile, clobbers), and the interrupt-entry convention — all scoped to the
freestanding target arc.

## The firmware seam: one UEFI-subset contract, two implementations

Decided direction (2026-07-02): the boot handoff is **UEFI-shaped on every
path**, and who implements the firmware side varies:

- **Commodity x86:** vendor UEFI → Cathedral. Works everywhere; uses exactly
  the glue in samples 1–2.
- **Reference x86:** coreboot (+ the vendor FSP blob for DRAM init —
  immovable) → **Omega UEFI payload** → Cathedral. The payload slot exists
  today (coreboot's `UefiPayloadPkg` pattern).
- **Reference ARM:** open init → Omega UEFI. Proven territory: U-Boot's
  `EFI_LOADER` is precisely a small to-spec UEFI surface over one's own
  firmware, shipping in products.

The kernel side is **identical on all paths and honors the boundary
identically** — mints the tables, validates the map, trusts nothing
structurally, with no "it's our firmware, skip validation" special case (that
would be a sandbox-escape in boot clothing). What varies is the *residual risk
behind the same boundary*: audited prose about vendor C vs proved Omega. And
"did we implement the spec right" becomes **differentially testable** — boot
the same kernel against vendor UEFI and ours; divergence = someone misread the
spec.

What the Omega implementation buys beyond boot: **runtime services in proved
Omega** (vendor UEFI runtime services are foreign code the OS must keep mapped
and callable forever — a classic persistent attack surface); **no ACPI/AML on
the reference platform** (static tables instead of a firmware bytecode
interpreter in the TCB); the measured chain rooted in our key from near-reset;
and the reference firmware boots other OSes (Linux's EFI stub is just another
loader), keeping it independently testable.

What it costs, honestly: the **bounded C-ABI export table** — implementing
UEFI puts Omega on the *provider* side of a C function-pointer table (~40
compile-time-known entries a foreign loader calls at MS-x64). This is the
bounded version of the callback problem the extern brief deliberately scoped
out: a static export table (a `provides` mapping in reverse — each entry = one
Omega machine + a declared C-ABI contract + a compiler-emitted thunk), not
general first-class function pointers. Plus boot-time storage drivers on our
side, and a firmware fork's maintenance tail. DRAM init stays a vendor blob on
x86 regardless; ME/PSP sit below everything either way.

## Status

THEORETICAL samples; the world-facts deflation, the evidence-token blessing,
and the firmware-seam direction are SETTLED (2026-07-02). Feeds Tier-1 item 7
in `cathedral_alignment.md`. Remaining open: no-host target + entry spelling,
lowering-contract vocabulary, interrupt-entry convention, the bounded C-ABI
export table (extern-brief adjacent), and — Cathedral-side, un-owned by any
chapter — the ACPI/AML question on the commodity path.