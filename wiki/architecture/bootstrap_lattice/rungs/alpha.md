# Rung: Alpha — raw computation

[Lattice overview](../bootstrap_lattice.md) | Prev: — | Next: [Beta](beta.md)

Alpha is the native seed and the only rung realized in hand-written machine code.
It establishes **one thing**: given these bytes and this memory, these exact state
transitions occur. Nothing else — not types, not safety, not meaning.

## Adds

Raw computation, and only computation:

- bytes and fixed-width integer arithmetic
- bounded, flat memory with loads and stores
- branches
- byte-stream input/output
- halt and trap

It should have an extremely simple, near-trivially-parsed binary format (ideally
fixed-width instructions). It is a substrate, **not** a miniature proof kernel.

## Written in

Nobody — alpha is the floor. It is hand-authored machine code, one realization per
ISA (x86-64, ARM, RISC-V, …). Everything above alpha is portable; only this seed
is per-platform.

## Meaning

Alpha is role #1 (executor); its meaning is pinned by a **small-step operational
semantics** — a written, per-opcode description of how `(pc, memory, registers,
stack)` transitions, what `getbyte`/`putbyte` observe, and what `halt`/`trap`/
out-of-memory produce. The `.hex` listing you audit the binary against is an
*encoding*, not a semantics; the semantics doc is a separate, required artifact
(**action item — does not exist yet**). You cannot audit the binary against a
spec that does not exist.

## Must not contain

No type system. No theorem prover. No proof objects. No compiler framework. No
language meaning. Alpha does not establish that a program is well-typed, safe,
meaningful, or correct — only that it computes deterministically.

## Current repo reality

`compiler/alpha/` is a 21-opcode register tape VM (`halt, imm, mov, add, sub,
mul, div, mod, loadb, storeb, load, store, jmp, jz, jnz, jlt, jeq, read, write,
call, ret`; unknown opcode → trap). Shipped as **two independent per-platform
seeds**, each hand-authored against the same semantics:

- `alpha_x64_windows.exe` (~37 KB) + annotated `.hex` listing — x86-64 Windows PE.
- `alpha_arm64_macos` (arm64 macOS Mach-O) + `alpha_arm64_macos.s` (the
  hand-authored source) + `alpha_arm64_macos.lst` (a committed disassembly).

A program is a "tape" stamped into a fixed `.tape`/`__tape` hole; the **same tape
runs on every platform's seed**. [Beta](beta.md) self-hosts on both (byte-identical
program-byte fixed point; on macOS the OS-imposed code signature is excluded from
the comparison — see below). `seed_env.sh` selects the seed and stamps tapes
per-platform, so one set of build scripts serves every host.

**The two seeds form the first real diamond.** They are independent realizations
(different ISA, OS, format, author), not transcriptions, so the *same source*
through both must yield *byte-identical tapes* — verified: the arm64 macOS VM
reproduces the x64 VM's assembler bytecode from `assembler.alpha` byte-for-byte
(sha256 `945c8061…`), the assembler self-hosts on macOS, and the example corpus
(`.alpha` and `.beta`) runs to identical answers on both. This is the
"diversity is the security" property established at the seed, not bolted on.

Gaps versus this target, all small and self-contained:

- **Written small-step semantics — DONE.**
  [`compiler/alpha/SEMANTICS.md`](../../../../compiler/alpha/SEMANTICS.md) is the
  per-opcode operational spec the seeds are audited against, and
  [`conformance.sh`](../../../../compiler/alpha/conformance.sh) is its executable
  companion — hand-built tapes pinning every rule and edge (signed div/mod,
  signed `jlt`, EOF, the three traps) that any seed must pass. (The two committed
  seeds both implement it; div/mod now trap on `INT_MIN/-1` to match the x64
  `idiv` overflow.)
- **Fixed memory hole** (currently 32 KB) — memory size should be an execution
  *parameter* with a defined out-of-memory result, not baked into the artifact.
- **Memory accesses are unchecked** — out-of-bounds is silent, not a defined
  trap. A trust-root executor should trap, not corrupt. (Spelled out as the only
  *undefined* corner in SEMANTICS.md §8; the hardening is the next step here.)
- **The seed is large** (x64 ~37 KB / ~400 lines; arm64 ~290 lines of asm)
  versus a stage0-scale seed (~256 bytes). Acceptable, but it is a per-platform
  audit cost; track it.

See [`alpha_language.md`](../../../design_briefs/alpha_language.md) for the
salvageable constraint list (resource budgets, banned features, trap-everything),
noting its trust-architecture framing is superseded by the
[lattice overview](../bootstrap_lattice.md).

## Open questions

- Fixed-width vs variable-width instruction encoding (canonical-parsing
  simplicity vs density).
- The exact I/O and trap event alphabet the semantics names
  (`Read/Write/Exit/Trap/OutOfMemory`).
- The diversity plan: how many independent alpha implementations, on which ISAs,
  authored how — this is what turns lattice diamonds into real Thompson
  resistance. **First step taken:** two now exist (x86-64 Windows PE, arm64 macOS
  Mach-O), independently hand-authored, producing byte-identical tapes. Open:
  more ISAs (RISC-V, aarch64 Linux), and ideally an implementation authored by a
  *different party / toolchain* so the diamond resists a shared-author bug, not
  just a shared-machine one.
