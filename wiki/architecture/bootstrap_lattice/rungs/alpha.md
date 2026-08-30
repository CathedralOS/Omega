# Rung: Alpha — raw computation

[Lattice overview](../bootstrap_lattice.md) | Prev: — | Next: [Beta](beta.md)

Alpha is the native seed and the only rung realized in hand-written machine code.
It establishes **one thing**: given these bytes and this memory, these exact state
transitions occur. Nothing else — not types, not safety, not meaning.

Alpha tape is also the single canonical executable representation of every
bootstrap compiler from Beta through `omega`. This does not add higher-language
meaning to Alpha; it gives every source-to-artifact proof one common target
machine.

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
out-of-memory produce. [`source/alpha/SEMANTICS.md`](../../../../source/alpha/SEMANTICS.md)
is that specification. The `.hex` listing audited against the native binary is
an encoding, not a substitute for the semantics.

## Must not contain

No type system. No theorem prover. No proof objects. No compiler framework. No
language meaning. Alpha does not establish that a program is well-typed, safe,
meaningful, or correct — only that it computes deterministically.

## Current repo reality

`source/alpha/` is a 21-opcode register tape VM (`halt, imm, mov, add, sub,
mul, div, mod, loadb, storeb, load, store, jmp, jz, jnz, jlt, jeq, read, write,
call, ret`; unknown opcode → trap). Shipped as **two independent per-platform
seeds**, each hand-authored against the same semantics:

- `alpha_x64_windows.exe` (~37 KB) + annotated `.hex` listing — x86-64 Windows PE.
- `alpha_arm64_macos` (arm64 macOS Mach-O) + `alpha_arm64_macos.s` (the
  hand-authored source) + `alpha_arm64_macos.lst` (a committed disassembly).

A program is a "tape" stamped into a fixed `.tape`/`__tape` hole; the **same tape
runs on every platform's seed**. The Alpha assembler self-hosts on both
(byte-identical program-byte fixed point; on macOS the OS-imposed code signature
is excluded from the comparison — see below). `seed_env.sh` selects the seed and
stamps tapes per-platform, so one set of build scripts serves every host.

Beta, Gamma, Delta, `omega₀`, and `omega` therefore need no host-specific
backend for their compiler artifacts. Product Omega separately owns native
backends for user programs. A general checked Alpha-to-native realization may
accelerate tapes; special higher-level substitutions are not part of Alpha.

**The two seeds provide a cross-platform conformance check.** They are separate
realizations (different ISA, OS, and format), so the *same source* through both
must yield *byte-identical tapes*—verified: the arm64 macOS VM reproduces the
x64 VM's assembler bytecode from `assembler/assembler.alpha` byte-for-byte
(sha256 `15e75e68…`), the assembler self-hosts on macOS, and the example corpus
(`.alpha` and `.beta`) runs to identical answers on both. This is the
executable companion to the written Alpha semantics. Agreement is useful
evidence, but the semantics and audited implementation correspondence—not
multiplicity—supply authority.

Gaps versus this target, all small and self-contained:

- **Written small-step semantics — DONE.**
  [`source/alpha/SEMANTICS.md`](../../../../source/alpha/SEMANTICS.md) is the
  per-opcode operational spec the seeds are audited against, and
  [`conformance.sh`](../../../../source/alpha/conformance.sh) is its executable
  companion — hand-built tapes pinning every rule and edge (signed div/mod,
  signed `jlt`, EOF, the three traps) that any seed must pass. (The two committed
  seeds both implement it; div/mod now trap on `INT_MIN/-1` to match the x64
  `idiv` overflow.)
- **Fixed memory hole** — memory size should be an execution *parameter* with a
  defined out-of-memory result, not baked into the artifact. Both committed
  seeds currently reserve 256 KiB (`0x40000`). D23 requires the coherent
  `AlphaBootstrapV2` migration to a one-MiB hole and a maximum 1,048,572-byte
  raw tape, including the checker and every adjacent resource profile rather
  than enlarging only the seeds. Hole size remains capacity, not semantics—the
  realizations agree for any tape that fits the selected common profile.
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

## Implementation frontiers

- Implement D23's `AlphaBootstrapV2` atomically across seeds, compilers,
  generated memory maps, checker capacity, boundary outcome tables, and exact
  limit gates. Current 256-KiB artifacts remain V1 until that migration passes.
- Fixed-width vs variable-width instruction encoding (canonical-parsing
  simplicity vs density).
- Complete the memory-fault/`OutOfMemory` event surface when bounds checks land.
- Add more seed platforms when portability, hardware coverage, or concrete fault
  isolation justifies their audit and maintenance cost.
- Escalate before adding an opcode, changing the tape encoding, or accepting
  special native acceleration in response to higher-rung performance or
  verbosity. Those pressures question the common target and are not ordinary
  local Alpha maintenance.
