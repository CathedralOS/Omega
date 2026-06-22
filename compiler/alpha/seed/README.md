# `compiler/alpha/seed/` — the hand-assembled tape VM (the trust root)

The goal of the tape VM: a native interpreter whose provenance is **hand-written,
hand-audited machine code**, not Rust+LLVM — and a build with **no Python in the
loop**. `compiler/alpha-rs`'s Rust `vm` is just a throwaway reference; *this* is what
the trust chain bottoms out at.

The committed seed:
- `hex0.hex` / `hex0.exe` — a tiny hand-assembled flat-hex transcriber (a Windows PE)
  that reads hex digit pairs from stdin and writes the bytes. `hex0.exe` is the
  trust-root binary; audit it by disassembling and checking it against `hex0.hex`.
- `hex0.flat.hex` — hex0's own flat hex, so it reproduces itself
  (`hex0 < hex0.flat.hex == hex0.exe`) with nothing but itself.
- `vm.flat.hex` — the tape VM's flat hex; `hex0 < vm.flat.hex` materializes `vm.exe`,
  **no Python, no LLVM, no other toolchain**.
- `vm.hex` — the VM as a commented x64 listing (the human-readable source you audit
  byte by byte; every instruction annotated).
- `materialize.py` — a ~40-line label-resolving hex assembler. **Dev-only**: it
  regenerates the flat hexes when `vm.hex`/`hex0.hex` change. It is *not* in the build
  or trust path — `../build.sh` uses `hex0.exe`, never Python.

Build (Python-free): `../build.sh` reproduces hex0, materializes the VM with it, and
runs the grounding self-host. To regenerate a flat hex after editing a listing (dev):

```
python materialize.py vm.hex build/vm.exe && xxd -p build/vm.exe > seed/vm.flat.hex
```

Verification uses `llvm-objdump` to disassemble `vm.exe`/`hex0.exe` and confirm the
bytes are the intended instructions — a *read-only* check; it never produces the
artifact.

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
  the VM's provenance** — it is materialized from the hand-authored `vm.hex`.

- **M5 — DONE: Python out of the loop.** `materialize.py` (which ran on CPython — a
  huge trusted interpreter) is replaced in the build by `hex0`, a tiny hand-assembled
  flat-hex transcriber (above). `../build.sh` is now Python-free: hex0 reproduces
  itself, materializes `vm.exe` from `vm.flat.hex`, and the hand VM grounds the
  self-host. Python survives only as a dev tool to regenerate the committed flat hexes.

## What this means

The build now turns source into the VM binary with **no Python and no LLVM** — only
the hand-assembled, hand-auditable `hex0` (~5 KB; audit it by disassembly). hex0
reproduces itself and materializes the VM; the VM reproduces the assembler tape.
The Rust `vm`/`assembler` in `../../alpha-rs` are now only a convenience reference;
`../build.sh` needs the Rust `assembler` solely to mint the *initial* assembler tape,
which the hand VM then reproduces.

Remaining toward full purity (the goal is reproducible-from-bare-metal):
1. hand-assemble (or independently re-derive) the **assembler tape**, so even the
   initial tape doesn't come from the Rust on-ramp;
2. a complete `.hex`-level audit of `hex0.exe` and `vm.exe` (annotated disassembly),
   the real trust anchor;
3. optionally a second, independent materializer path (DDC) over the flat hexes.
