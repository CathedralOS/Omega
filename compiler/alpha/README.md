# `compiler/alpha/` — Alpha, the tape VM (rung 0 / v1)

Alpha is the Turing-minimal floor of the lattice: a tiny **register machine with byte
I/O**. Everything above is built and checked downward onto it. The whole point is that
the thing you must trust is small, hand-written, and hand-auditable — no Rust, no LLVM,
no Python in the loop.

## The whole bootstrap, in one breath

```
seed/hex0.exe   a hand-assembled hex->bytes transcriber  (YOU AUDIT THIS — ~5 KB of x64)
      │
      ├─ hex0 < seed/vm.flat.hex             →  the tape VM        (build/vm.exe)
      └─ hex0 < seed/assembler.tape.flat.hex →  the assembler      (build/assembler.tape)
                                                      │
            vm runs the assembler on its own source (seed/assembler.num)
                                                      │
                                          reproduces the assembler tape  ← self-verifies
```

`./build.sh` runs exactly that and prints OK. **It uses no Rust and no Python** — only
the committed seed and `sh`/`cmp`/`cat`. Trust reduces to: audit `hex0.exe`'s bytes
(disassemble, check against `seed/hex0.hex`), and the self-verification does the rest.

## Layout

```
build.sh        reproduce + self-verify the bootstrap  (no Rust, no Python)
dev-regen.sh    regenerate the committed seed after editing a source  (Rust + Python; DEV ONLY)
src/            Alpha-assembly source: assembler.alp, echo.alp, multiply.alp
seed/           the committed, hand-auditable bootstrap inputs:
                  hex0.hex / hex0.exe / hex0.flat.hex   the seed transcriber (listing, binary, flat)
                  vm.hex / vm.flat.hex                  the tape VM (listing, flat)
                  assembler.tape.flat.hex               the assembler, as a tape
                  assembler.num                         the assembler's source (numeric form)
                  materialize.py                        DEV-ONLY label resolver (regenerates flat hex)
build/          generated outputs (gitignored)
../alpha-rs/    DEV-ONLY Rust on-ramp: the `assembler` (mints the tape for dev-regen)
```

There is **one** VM — the hand-assembled `seed/vm.hex`. The labeled `.hex`/`.alp`
listings + `materialize.py` + the Rust `assembler` are the *authoring* tools (used by
`dev-regen.sh`); they are never in the reproduce/trust path.

## ISA

16 registers `r0..r15` (signed 64-bit), a flat zero-initialized byte memory, `PC` at 0.
One opcode byte + operands (`Reg`=1 byte, `Imm`/`Addr`=8-byte LE). Single source of
truth: `../alpha-rs/src/isa.rs`.

```
halt rS   imm rD,N   mov rD,rS   add/sub/mul/div/mod rD,rS   loadb/storeb/load/store rD,rS
jmp A     jz/jnz rS,A   jlt/jeq rA,rB,A   read rD   write rS   call A   ret
```

## How the seed is grown / audited

`seed/vm.hex` and `seed/hex0.hex` are commented x64 machine-code listings — every
instruction is hand-authored and annotated; `materialize.py` only resolves label
arithmetic (jump displacements, addresses), the way `hex0` (or hex1) would. Verify with
`llvm-objdump` (read-only) and by running. See `seed/README.md` for the M0–M5 build log
and what remains toward fully reproducible-from-bare-metal (hand-assembling the
assembler tape; a complete disassembly audit of `hex0.exe`/`vm.exe`).
