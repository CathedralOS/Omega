# `compiler/alpha/seed/` — the hand-assembled tape VM (the trust root)

The actual goal of the tape VM: a native interpreter whose provenance is
**hand-written, hand-audited machine code**, not Rust+LLVM. `compiler/alpha-rs`'s
Rust `vm` is the throwaway reference; *this* is the thing the trust chain is meant to
bottom out at.

- `vm.hex` — the VM as a commented x64 machine-code listing (the hand-assembled
  artifact you audit byte by byte). Every instruction is annotated; build resolves
  the label arithmetic (jump displacements, absolute/RIP-relative addresses).
- `materialize.py` — a ~40-line label-resolving hex assembler (hex listing → binary).
  Not part of the trust root any more than `xxd -r` is; it only does the address
  arithmetic a human would otherwise compute by hand (the hex0 → hex1 step), not
  compilation.

Materialize + run a tape (the parent `../build.sh` does the full self-host chain):

```
python materialize.py vm.hex ../build/vm.exe
( python -c "import sys,struct;sys.stdout.buffer.write(struct.pack('<I',N))"; \
  cat TAPE; cat INPUT ) | ../build/vm.exe            # tape from a 4-byte LE length prefix
```

Verification uses `llvm-objdump` to disassemble and confirm the bytes match the
intended instructions — a *read-only* check; it never produces the artifact.

## Status (incremental)

- **M0** — PE shell + `mov eax,42; ret` exit stub. Validated the PE32+ layout, the
  materializer, and the thread-return exit mechanism. Exits 42.
- **M1** — a real interpreter for `imm`/`mul`/`halt` over a tape embedded in `.text`,
  with the 16 virtual registers in a writable `.data` (BSS) region (`rbx=&vregs`,
  `rsi=pc`). The embedded tape computes 6*7 and halts r0 → exits 42.

- **M2** — the full non-I/O interpreter: all compute (`mov`/`add`/`sub`/`mul`/`div`/
  `mod`), memory (`load`/`store`/`loadb`/`storeb`), branch (`jmp`/`jz`/`jnz`/`jlt`/
  `jeq`), and `call`/`ret` (a VM call stack growing down from the top of a 64 MB
  memory in `.data`). The tape is embedded in `.text` and copied into VM memory at
  startup. Verified with two embedded tapes: sum 1..10 → 55 (imm/jlt/jmp/add), and
  a memory+subroutine program → 42 (store/load/mul/call/ret/sub). `read`/`write` are
  `ud2` placeholders until M3.

- **M3** — byte I/O. Adds a `.rdata` kernel32 import table (GetStdHandle/ReadFile/
  WriteFile), the `read`/`write` handlers, and the Win64 call ABI (`sub rsp,0x28` at
  entry for 16-aligned rsp + shadow space; kernel32 preserves the VM state regs
  rbx/rsi/rdi/r12). Verified: an embedded "write ABC\n" tape prints `ABC`, and an
  embedded echo tape cats stdin→stdout.

- **M4 — DONE: the trust root is off Rust.** The VM is now general: it reads the
  tape from a 4-byte little-endian length prefix on stdin, loads it into VM memory,
  then runs it (the rest of stdin is the program's input). I/O is factored into
  self-aligning `getbyte`/`putbyte` subroutines. The hand-assembled VM runs the
  Alpha assembler (`../src/assembler.alp`) on the assembler's own source and
  reproduces the assembler tape byte-for-byte (3003 bytes). **No Rust or LLVM is in
  the VM's provenance** — it is materialized from the hand-authored `vm.hex`. Run
  `../build.sh`.

## What this means

The trust chain now bottoms out at `vm.hex` — a few hundred hand-authored,
hand-auditable x64 instructions — instead of the Rust+LLVM toolchain. Everything
above (the assembler, and anything written in Alpha assembly) is a checkable tape
that the audited VM reproduces. The Rust `vm`/`assembler` in `../../alpha-rs` are now
only a convenience reference; `../build.sh` needs the Rust `assembler` solely to mint
the *initial* tape, which the hand-VM then reproduces from source.

Remaining toward full purity: hand-assemble (or independently re-derive) the
*assembler tape* too, so even the initial tape doesn't come from the Rust on-ramp;
and a `.hex`-level audit pass / a second materializer path (DDC) over `vm.hex`.
