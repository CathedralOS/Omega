# Design Brief — Privileged Effects, Inline Asm & Binary Trust

> **For:** Omega maintainer + Cathedral · **Status:** PARTIALLY SETTLED (chat
> 2026-07-11, Zach). Core effect/asm rules LOCKED; the binary/manifest format,
> the discharge-capability machinery, and the Thompson-attack layer are OPEN
> (marked below). · **Driver:** Cathedral M3 needs `hlt` + port I/O, which
> raised: no program may arbitrarily use inline asm / privileged instructions
> without proving it safe or surfacing its effects — package managers must be
> able to gatekeep rogue code. · **Depends on:** the effect system
> (`omega-effects`: closed effect set, transitive must-declare checking in
> `omega-validation/src/effects.rs`), [`freestanding_boot_and_hardware_facts.md`],
> [`static_root_and_constants.md`], the parked capability_lifecycle arc (Region
> minting), the Matrix manifest-vs-grant model. · **Companion:** M3 (serial
> driver + "Owned N MiB" report + `hlt` idle).

## Bottom line

Privileged instructions are **authority-bearing effects, gated by the effect
system that already exists** — not new machinery. The novelty is (a) treating
each asm instruction as a contract emitter, (b) a `machine_control` effect
distinct from `device_io`, and (c) the recognition that effect soundness rests
on *who compiled the binary*.

## LOCKED (2026-07-11)

1. **Specific effects, never a catch-all `privileged`.** `machine_control`
   (ring-0 CPU control: `hlt`, `cli`/`sti`, MSR/CR writes) is a DISTINCT effect
   from `device_io` (port I/O, already in the effect set) and `mmio` (future).
   They are decoupled because they have **different enforcement substrates**,
   not merely because it reads cleaner: `device_io` is hardware-mediated (the
   kernel maps specific ports into a ring-3 driver's TSS I/O bitmap; ungranted
   ports GP-fault) and is therefore safely grantable to an untrusted binary;
   `machine_control` is not grant-mediated at all (ring 3 faults, ring 0 runs it
   freely). Usermode drivers hold `device_io`/`mmio`, **never** `machine_control`.

2. **Each asm instruction emits contracts; only known-contract instructions
   compile.** `hlt` → `machine_control`; `in`/`out` → `device_io`. Opaque forms
   — `db`/raw bytes — are **BANNED** (no attributable contract), unless one day
   we parse the bytes and derive the contracts. There is **no "strictest
   default" escape hatch**: unknown/opaque asm simply does not compile. Every
   asm instruction is a known-contract intrinsic or it is rejected.

3. **Effects are transitive and must-declare (already enforced).** A machine's
   declared effects must contain all effects it transitively reaches
   (`effects.rs`; undeclared reach = compile error). The function AND every
   boundary that directly-or-indirectly calls it must declare what it emits.
   Privileged asm rides this existing check; a program's top-level declared
   effect set IS the manifest a package manager reads.

4. **Declare vs discharge.** *Declare* = truthfully label the signature (bubbles
   to the manifest; makes the effect visible). *Discharge* = prove at a boundary
   that you hold the capability (gates whether the code is PERMITTED, not just
   labeled). `machine_control` is the tier that should require **discharge**
   against an "owns-the-machine" capability — softer effects (`clock_read`) may
   be declare-only.

5. **Effect soundness rests on who compiled the binary.** I/O is
   binary-enforceable (hardware mediates it — a rogue ring-3 `out` faults);
   arbitrary asm is NOT (the CPU runs the bytes; no per-instruction runtime
   chokepoint). So a prebuilt exe's manifest is a **claim, not a grant** — a
   malicious binary declares nothing and runs the byte anyway. Package-
   enforceable (refuse to install), not binary-enforceable. **Source-distributed
   + host-compiled** closes this: the host's compiler *derives* effects from
   source (unfakeable) and enforces discharge at compile time; the binary is
   trusted *because the host built it*. A binary is a host **artifact, not a
   trusted input**.

6. **The narrow-gap corner.** The one op class that is NOT binary-enforceable
   (`machine_control`) is also the one that, by hardware, only ever executes in
   ring-0 code — which in Cathedral is host-compiled kernel/boot code by
   construction. So the ungrantable-and-unstoppable ops live *only* inside code
   the host itself built; grantable ops (`device_io`) are hardware-mediated. The
   residual risk is "a host-compiled-but-buggy kernel" — the TCB audited anyway.

## OPEN (not decided; do not build against as if settled)

- **Executable + manifest format.** Undecided. Point 3 assumes "the top-level
  effect set is the manifest," but the concrete on-disk format is not chosen.
- **We may relax host-compile-from-source to make progress.** Point 5 is the
  north star, not a hard near-term constraint; accepting some prebuilt binaries
  (with declare-only manifests) may be a pragmatic interim.
- **Thompson attacks.** Host-compilation only trivializes trust *if the compiler
  itself is trusted*. Eliminating trusting-trust (diverse double-compilation /
  the bootstrap tower — delta/epsilon) is a SEPARATE future effort; point 5 is
  sound only modulo a trusted compiler.
- **Proof-carrying code.** The future upgrade that lets an untrusted binary be
  trusted without recompiling (ship a checkable proof, verify cheaply). Relaxes
  point 5 when it lands.
- **The discharge capability for `machine_control`.** The "owns-the-machine"
  evidence token is the parked capability_lifecycle arc (same origin as Region
  minting) — not built.

## M3 actionable subset (sound TODAY)

M3's serial driver is our source, host-compiled by us, and IS the boot root, so
the fence is fully sound for it and none of the OPEN items block it. Build:

- `machine_control` added to the effect set as an authority-bearing effect.
- `hlt` and `in`/`out` as **known-contract asm intrinsics** emitting
  `machine_control` / `device_io`; `db`/opaque asm rejected.
- Discharge for `machine_control` = **v0 "permitted in the freestanding boundary
  root"** (the boot root trivially owns the machine) until the Region-style
  capability token lands.
