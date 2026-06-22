# `compiler/alpha/` — Alpha, the tape VM (rung 0 / v1)

Alpha is the Turing-minimal floor of the lattice: a tiny **register machine with byte
I/O**. It is *not* a member of the Omega language family — it's the substrate the
trust chain bottoms out at. The whole point is that the thing you must hand-audit is
as small and regular as possible: a uniform fetch/decode/execute loop over a fixed
opcode table. Everything above is built and checked downward onto this.

Two roles, both currently throwaway Rust (in `../alpha-rs`):
- **vm** — interprets a bytecode tape. This is the *seed*; the trust-root version is
  a hand-rolled assembly port of this same loop (and, per arch, a sibling port — the
  tape stays neutral).
- **asm** — assembles Alpha-assembly text into a bytecode tape. This is the *on-ramp*.

## ISA

16 registers `r0..r15` (signed 64-bit), a flat zero-initialized byte memory, `PC`
starting at 0. The tape loads at address 0; the program owns all memory past it.
Each instruction is one opcode byte + operands (`Reg`=1 byte, `Imm`/`Addr`=8-byte
little-endian). See `../alpha-rs/src/isa.rs` for the encoding (single source of truth
shared by `vm` and `asm`).

```
halt rS              imm rD,N    mov rD,rS
add/sub/mul/div/mod rD,rS        loadb/storeb/load/store rD,rS
jmp A   jz/jnz rS,A   jlt/jeq rA,rB,A
read rD              write rS
```

## Build / run

```
cd ../alpha-rs && cargo build            # builds vm + asm
../alpha-rs/target/debug/asm arith.asm arith.tape
../alpha-rs/target/debug/vm  arith.tape; echo $?      # -> 42
../alpha-rs/target/debug/asm echo.asm echo.tape
printf 'hi' | ../alpha-rs/target/debug/vm echo.tape   # -> hi
```

## ✅ Self-hosting reached

`as.asm` is an assembler written **in Alpha-assembly** (a two-pass, label-resolving
assembler). The VM runs it to reproduce its own bytecode tape:

```
asm as.asm as0.tape          # Rust on-ramp builds the assembler
asm --num as.asm > as.num     # numeric form of its own source
vm as0.tape < as.num > as1.tape   # the assembler assembles itself
vm as1.tape < as.num > as2.tape   # ...and again
cmp as1.tape as2.tape         # byte-identical  (and as1 == as0)
```

`build.sh` runs the whole chain. So the **trust root is the tiny VM alone** — the
assembler (labels, two passes, encoding) is a checkable tape that reproduces itself.

`as.asm` is authored in mnemonics for readability; `asm --num` lowers it to the
numeric-opcode form the assembler itself reads (mnemonics -> opcode numbers, labels
preserved). The asm-in-asm carries no mnemonic table — that keeps it small; the
mnemonic alias is the only thing outside the self-hosting loop (a trivial 1:1 map).
