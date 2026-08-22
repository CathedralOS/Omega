# Alpha — Small-Step Operational Semantics

> **The written meaning of the alpha seed.** A seed binary is *audited against
> this document*: it is correct iff every opcode realizes the transition below.
> The two committed seeds (x64 Windows PE, arm64 macOS Mach-O) are independent
> implementations of exactly these rules; [`conformance.sh`](conformance.sh) is
> the executable companion (hand-built tapes that pin each rule, which any
> faithful seed must pass).
>
> This fills the trust-root gap the lattice flagged: "you cannot audit the binary
> against a spec that does not exist." See
> [bootstrap_lattice rungs/alpha.md](../../../wiki/architecture/bootstrap_lattice/rungs/alpha.md).

Alpha is **role #1, an executor** (lattice overview, "Five roles"). It is not a
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

`MEMSIZE` is an implementation parameter; the committed seeds use 64 MiB
(`0x04000000`). `sp` is initialized to `0x04000000` and the stack grows **down**.

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

A program is a **tape**: a `[4-byte LE length L][L bytes of bytecode]` block
stamped into the seed's fixed hole. The loader:

1. zero-fills `M` (memory starts clean),
2. copies the `L` bytecode bytes into `M[0 .. L-1]`,
3. sets `pc = 0`, all `R[i] = 0`, `sp = 0x04000000`,
4. begins fetch/dispatch.

The same tape runs on every platform's seed — only the surrounding executable
shim differs per ISA/OS.

## 5. Transition rules

Fetch reads `op = M[pc]` and advances past the opcode; the columns below give the
operands (consumed in order, each advancing `pc`) and the effect. `R[d]`, `R[s]`
denote register-indexed slots; `k`/`a` are 8-byte immediate/address operands;
`zext8` zero-extends a byte to 64 bits.

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
| 0x08 | `loadb`| `d, s` | `R[d] = zext8(M[R[s]])` | `pc+2` |
| 0x09 | `storeb`| `d, s`| `M[R[d]] = R[s] mod 2⁸` | `pc+2` |
| 0x0A | `load` | `d, s` | `R[d] = M[R[s] .. R[s]+8]` (LE 64-bit) | `pc+2` |
| 0x0B | `store`| `d, s` | `M[R[d] .. R[d]+8] = R[s]` (LE 64-bit) | `pc+2` |
| 0x0C | `jmp`  | `a` | — | `a` |
| 0x0D | `jz`   | `c, a` | — | `a` if `R[c]=0` else `pc+10` |
| 0x0E | `jnz`  | `c, a` | — | `a` if `R[c]≠0` else `pc+10` |
| 0x0F | `jlt`  | `a, b, a₂` | — (signed `<`) | `a₂` if `R[a] <ₛ R[b]` else `pc+11` |
| 0x10 | `jeq`  | `a, b, a₂` | — | `a₂` if `R[a] = R[b]` else `pc+11` |
| 0x11 | `read` | `d` | consume head byte `x` of `in`: `R[d] = zext8(x)`; at EOF `R[d] = 0xFFFFFFFFFFFFFFFF` | `pc+2` |
| 0x12 | `write`| `s` | append `R[s] mod 2⁸` to `out` | `pc+2` |
| 0x13 | `call` | `a` | `sp -= 8`; `M[sp..sp+8] = (pc+8)` (the offset just past `a`, LE); | `a` |
| 0x14 | `ret`  | — | `r = M[sp..sp+8]` (LE); `sp += 8`; | `r` |
| other | — | — | **Trap** (unknown opcode) | — |

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

1. an unknown opcode (`> 0x14`),
2. `div`/`mod` with divisor `0`,
3. `div`/`mod` signed overflow (`INT64_MIN / -1`).

## 8. Currently undefined (the honest edges)

These are real gaps versus an ideal trust-root executor, tracked in the alpha
rung doc; they are **not** yet specified behavior:

- **Out-of-range memory** (`M[i]` for `i ∉ [0, MEMSIZE)`, including `sp` under/
  overflow): currently unchecked — the implementations may corrupt adjacent state
  or fault. A trust root *should* trap; until it does, programs must stay in
  bounds and this document does not assign a meaning to violations.
- **Memory size is fixed** (`MEMSIZE`, and the tape hole) rather than an
  execution parameter with a defined out-of-memory result. The tape hole is
  currently 256 KiB on both committed seeds. This capacity is not part of Alpha's
  language semantics; the same tape runs identically on both platform realizations.

Everything in §5–§7 is pinned by `conformance.sh`; §8 is deliberately out of
scope until the hardening lands.

## 9. Conformance

`conformance.sh` runs hand-built bytecode tapes — one per rule and per edge
(signed division/remainder, signed `jlt`, EOF, the three traps) — against the
host's seed and checks exit code and stdout. A faithful seed on any ISA passes
all of them; a divergence between two realizations on the same tape exposes a
conformance or implementation problem. Run it after touching a seed and as the
acceptance gate for a new platform realization.
