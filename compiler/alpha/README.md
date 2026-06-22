# `compiler/alpha/` — Alpha, the tape VM (rung 0 / v1)

Alpha is the Turing-minimal floor of the lattice: a tiny **register machine with byte
I/O**. It is *not* a member of the Omega language family — it's the substrate the
trust chain bottoms out at. The whole point is that the thing you must hand-audit is
as small and regular as possible: a uniform fetch/decode/execute loop over a fixed
opcode table. Everything above is built and checked downward onto this.

## Layout

```
src/       Alpha-assembly source (.alp): assembler.alp, echo.alp, multiply.alp
seed/      the HAND-ASSEMBLED VM — the trust root (vm.hex + materialize.py)
build/     generated artifacts (gitignored): *.tape, vm.exe
build.sh   builds everything and verifies both self-hosting facts
```

The VM has two implementations of one rung — not two rungs:
- **`seed/vm.hex`** — the hand-assembled x64 VM (a few hundred hand-authored,
  hand-auditable instructions). This is the **trust root**; no Rust or LLVM is in its
  provenance. Per-arch by nature (x64 today; a sibling `.hex` per future target).
- **`../alpha-rs`** — the *throwaway* Rust reference: `vm` (interprets a tape) and
  `assembler` (assembles `.alp` text → a tape). Used for development + to mint the
  initial tape; not part of the trust root.

## ISA

16 registers `r0..r15` (signed 64-bit), a flat zero-initialized byte memory, `PC`
starting at 0. The tape loads at address 0; the program owns all memory past it.
Each instruction is one opcode byte + operands (`Reg`=1 byte, `Imm`/`Addr`=8-byte
little-endian). See `../alpha-rs/src/isa.rs` — the single source of truth shared by
`vm`, `assembler`, and the hand-assembled `seed/vm.hex`.

```
halt rS              imm rD,N    mov rD,rS
add/sub/mul/div/mod rD,rS        loadb/storeb/load/store rD,rS
jmp A   jz/jnz rS,A   jlt/jeq rA,rB,A
read rD              write rS    call A   ret
```

## Build / run

`build.sh` does the full build + both verifications. By hand:

```
cd ../alpha-rs && cargo build            # builds vm + assembler
../alpha-rs/target/debug/assembler src/multiply.alp build/multiply.tape
../alpha-rs/target/debug/vm        build/multiply.tape; echo $?    # -> 42
printf 'hi' | ../alpha-rs/target/debug/vm  build/echo.tape         # echo -> hi
```

Alpha-assembly source files are `.alp`; the bytecode tapes they assemble to are
`.tape`.

## ✅ Self-hosting, and the trust root off Rust

`src/assembler.alp` is an assembler written **in Alpha-assembly** (two-pass,
label-resolving). `build.sh` verifies two things:

1. **Self-host** — the assembler assembles its own source and reaches a byte-identical
   fixed point (`assembler.tape == gen1 == gen2`), run on the Rust reference VM.
2. **Grounding** — the **hand-assembled `seed/vm.hex`** (no Rust/LLVM in the VM path)
   loads that tape and reproduces it. So the trust chain bottoms out at hand-authored,
   auditable machine code, not the Rust+LLVM toolchain.

`assembler.alp` is authored in mnemonics for readability; `assembler --num` lowers it
to the numeric-opcode form the assembler itself reads (no mnemonic table in the
Alpha-side assembler keeps it small). That numeric form is *piped*, never written —
the mnemonic alias is the only thing outside the self-hosting loop (a trivial 1:1
map; teaching the Alpha assembler a mnemonic table would retire it entirely).
