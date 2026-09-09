# Alpha — Small-Step Operational Semantics

> **The written meaning of the alpha seed.** A seed binary is *audited against
> this document*: it is correct iff every opcode realizes the transition below.
> The two committed seeds (x64 Windows PE, arm64 macOS Mach-O) are independent
> implementations of exactly these rules;
> [`tests/alpha/conformance.sh`](../../tests/alpha/conformance.sh) is
> the executable companion (hand-built tapes that pin each rule, which any
> faithful seed must pass).

Alpha is the [bootstrap chain's](../README.md) native execution floor. It is not a
type system, checker, or proof kernel — only a deterministic register machine
with byte I/O. Its job is to make "given these bytes and this memory, exactly
these state transitions occur" precise.

## 1. Machine state

A configuration is `σ = (pc, M, R, sp)` plus the two I/O byte streams:

| Component | Meaning |
| --- | --- |
| `pc` | program counter: a byte offset into `M` (the next opcode is `M[pc]`) |
| `M`  | memory: a flat array of bytes, indices `0 .. MEMSIZE-1`, initially all `0` |
| `R`  | registers: `R[i]` is a 64-bit value, `i ∈ 0..255`, initially all `0` |
| `sp` | call-stack pointer: a byte offset into `M`, the top of the return-address stack |
| `in` | input stream: a sequence of bytes (process stdin); `read` consumes its head |
| `out`| output stream: bytes appended by `write` (process stdout) |

`MEMSIZE` is an implementation parameter; the selected AlphaBootstrapV4 seeds
use 1.75 GiB (`0x70000000`). `sp` remains initialized to `0x10000000` and the stack
grows **down**. The added memory is above that unchanged stack origin; raising
the memory extent does not move the stack or change any opcode transition.

## 2. Values and arithmetic

Registers hold 64-bit bit patterns. Operations are two's-complement:

- `add`, `sub`, `mul` are **wrapping** mod 2⁶⁴ (the result is the low 64 bits;
  `mul` discards the high half).
- `div`, `mod` are **signed**, truncating toward zero, with the remainder taking
  the sign of the dividend (`-7/2 = -3`, `-7%2 = -1`). They **trap** when the
  divisor is `0`, or on the single signed-overflow case `INT64_MIN / -1`. (This
  matches x86 `idiv`, which raises `#DE` for both.)
- `jlt` compares **signed**. `jeq` compares full 64-bit equality. Equality and
  the `jz`/`jnz` zero-tests are sign-agnostic.

## 3. Encoding

Instructions are variable-length. The first byte is the **opcode**; operands
follow immediately:

- a **register operand** is one byte (the register index `0..255`);
- an **immediate** or **address** operand is 8 bytes, little-endian, read as a
  64-bit value.

An **address** operand is an absolute offset into `M` (a jump/call target, where
`M[0]` is the first tape byte — see §4). "`pc += n`" below means the instruction
consumed `n` bytes total (opcode + operands).

## 4. Initial configuration (loading the tape)

A program is a raw **tape** containing `L` bytes of bytecode. Seed stamping
places it in the fixed hole as `[4-byte LE length L][raw tape]`; the prefix and
host container are not part of the `.tape` or compiler identity. The loader:

1. checks `L <= 16,777,212` and `L <= MEMSIZE`; otherwise it performs a loader
  trap before copying bytes or starting execution,
2. zero-fills `M` (memory starts clean),
3. copies the `L` bytecode bytes into `M[0 .. L-1]`,
4. sets `pc = 0`, all `R[i] = 0`, `sp = 0x10000000`,
5. begins fetch/dispatch.

The same tape runs on every platform's seed — only the surrounding executable
shim differs per ISA/OS.

## 5. Transition rules

Fetch checks `pc < MEMSIZE` before reading `op = M[pc]`. After the opcode selects
an instruction width, every operand byte must also be in `M`; a failed fetch or
operand-range check traps before the instruction has any effect. Checks use
nonwrapping comparisons. The columns below give the operands (consumed in order,
each advancing `pc`) and the effect. `R[d]`, `R[s]` denote register-indexed
slots; `k`/`a` are 8-byte immediate/address operands; `zext8` zero-extends a
byte to 64 bits.

| Op | Mnemonic | Operands | Effect | Next `pc` |
| --- | --- | --- | --- | --- |
| 0x00 | `halt` | `d` | **Exit.** code = `R[d] mod 2³²` (a shell observes `mod 2⁸`) | — (halts) |
| 0x01 | `imm`  | `d, k` | `R[d] = k` | `pc+9` |
| 0x02 | `mov`  | `d, s` | `R[d] = R[s]` | `pc+2` |
| 0x03 | `add`  | `d, s` | `R[d] = (R[d] + R[s]) mod 2⁶⁴` | `pc+2` |
| 0x04 | `sub`  | `d, s` | `R[d] = (R[d] - R[s]) mod 2⁶⁴` | `pc+2` |
| 0x05 | `mul`  | `d, s` | `R[d] = (R[d] · R[s]) mod 2⁶⁴` | `pc+2` |
| 0x06 | `div`  | `d, s` | trap if `R[s]=0 ∨ (R[d]=INT_MIN ∧ R[s]=-1)`; else `R[d] = R[d] ÷ₛ R[s]` | `pc+2` |
| 0x07 | `mod`  | `d, s` | same traps; else `R[d] = R[d] −ₛ (R[d] ÷ₛ R[s])·R[s]` | `pc+2` |
| 0x08 | `loadb`| `d, s` | trap unless `R[s] < MEMSIZE`; else `R[d] = zext8(M[R[s]])` | `pc+2` |
| 0x09 | `storeb`| `d, s`| trap unless `R[d] < MEMSIZE`; else `M[R[d]] = R[s] mod 2⁸` | `pc+2` |
| 0x0A | `load` | `d, s` | trap unless `[R[s], R[s]+8)` is in `M`; else load that range as LE 64-bit | `pc+2` |
| 0x0B | `store`| `d, s` | trap unless `[R[d], R[d]+8)` is in `M`; else store `R[s]` there as LE 64-bit | `pc+2` |
| 0x0C | `jmp`  | `a` | — | `a` |
| 0x0D | `jz`   | `c, a` | — | `a` if `R[c]=0` else `pc+10` |
| 0x0E | `jnz`  | `c, a` | — | `a` if `R[c]≠0` else `pc+10` |
| 0x0F | `jlt`  | `a, b, a₂` | — (signed `<`) | `a₂` if `R[a] <ₛ R[b]` else `pc+11` |
| 0x10 | `jeq`  | `a, b, a₂` | — | `a₂` if `R[a] = R[b]` else `pc+11` |
| 0x11 | `read` | `d` | consume head byte `x` of `in`: `R[d] = zext8(x)`; at EOF `R[d] = 0xFFFFFFFFFFFFFFFF` | `pc+2` |
| 0x12 | `write`| `s` | append `R[s] mod 2⁸` to `out` | `pc+2` |
| 0x13 | `call` | `a` | trap unless `[sp-8, sp)` is in `M`; else decrement `sp` by 8 and store `(pc+8)` there as LE | `a` |
| 0x14 | `ret`  | — | trap unless `[sp, sp+8)` is in `M`; else load `r` there and increment `sp` by 8 | `r` |
| 0x15..0xFE | — | — | **Trap** (unknown opcode) | — |
| 0xFF | — | — | **Trap**; permanently reserved as the first byte of an out-of-band producer-diagnostic frame and never assignable as an opcode | — |

Notes:
- `÷ₛ` / `−ₛ` are signed truncating division / the matching remainder.
- `call` pushes a **return offset** (relative to `M[0]`), not an absolute
  pointer, so the call stack is position-independent within `M`.

## 6. I/O and halting

- `read` performs one input event. A successful read yields a byte `0..255`
  (zero-extended); end-of-input yields all-ones (`-1` as a 64-bit pattern), which
  is the conventional EOF sentinel a program tests for.
- `write` performs one output event of a single byte.
- `halt` ends execution. The VM's exit code is the low 32 bits of `R[d]`; because
  process exit status is a byte on Unix shells, examples return small numbers on
  purpose (the low byte is what you observe).

## 7. Traps

A trap is an abnormal, non-resumable halt (the implementations raise an illegal
instruction; a shell observes exit `132 = 128 + SIGILL`). The defined trap
conditions are:

1. an unknown opcode (`0x15..0xFE`) or the permanently reserved diagnostic
   marker (`0xFF`),
2. `div`/`mod` with divisor `0`,
3. `div`/`mod` signed overflow (`INT64_MIN / -1`),
4. an out-of-range instruction fetch, operand fetch, data access, or call/return
   stack access,
5. a stamped tape length exceeding the physical tape hole or `MEMSIZE`.

Runtime traps preserve the exact stdout prefix written before the failing
transition and append no diagnostic bytes. A loader trap occurs before execution
and therefore has an empty stdout prefix. Both have the same external Trap
observation; “loader” names the cause, not a second outcome.

## 8. Bounds and fixed capacity

Every memory range is checked in full before it is read, written, or used to
change machine state. Implementations use nonwrapping range checks; an address
near `2^64` cannot wrap into valid memory. A failed `call` does not move `sp` or
write a return address, and a failed `ret` does not read memory or move `sp`.

Execution is not restricted to the original tape extent or instruction
boundaries. Mutable code and unaligned data remain legal. A taken control target
is checked when its next instruction is fetched; an untaken branch does not
access or validate its target. The stack remains ordinary memory: Alpha adds no
stack partition, does not require a preceding `call` for `ret`, and does not
trap merely because `sp` rises above its initial value while its next access
remains in range.

`MEMSIZE` and the tape hole are fixed execution-profile parameters rather than a
recoverable allocation service. AlphaBootstrapV4 selects 1.75 GiB of semantic
memory and an exact 16 MiB stamped hole, including the four-byte length, for a
16,777,212-byte raw-tape maximum. A bounds trap is an Alpha failure and must not
be relabeled as a successful Gamma or compiler-owned resource refusal.

The current native sources/listings implement these checks. The
[realization evidence](README.md#ratified-bounds-hardening-contract) distinguishes
macOS execution from Windows listing reconstruction and records a separate
unresolved Windows register-storage defect; source changes alone do not establish
complete conformance.

## 9. Conformance

`tests/alpha/conformance.sh` runs hand-built bytecode tapes against the host's
seed and checks exit code and stdout. It pins in-bound rules, signed
division/remainder, signed `jlt`, EOF, and arithmetic/unknown-opcode traps.
The linked bounds cases exercise exact and adjacent fetch, operand, byte/word
data, call/return stack, and stamped-tape ranges, including stdout-prefix
preservation. A faithful seed on any ISA
passes all applicable cases; a divergence between two realizations on the same
tape exposes a conformance or implementation problem.
