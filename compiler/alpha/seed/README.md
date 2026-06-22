# `compiler/alpha/seed/` — the hand-assembled tape VM (the trust root)

The actual goal of the tape VM: a native interpreter whose provenance is
**hand-written, hand-audited machine code**, not Rust+LLVM. `compiler/alpha-rs`'s
Rust `vm` is the throwaway reference; *this* is the thing the trust chain is meant to
bottom out at.

- `vm.hex` — the VM as a commented x64 machine-code listing (the hand-assembled
  artifact you audit byte by byte). Every instruction is annotated; every jump
  offset is computed by hand.
- `build.py` — a trivial materializer (hex listing → binary). Not part of the trust
  root any more than `xxd -r` is; it only concatenates bytes.

Build + run:

```
python build.py vm.hex vm.exe
./vm.exe ; echo $?
```

Verification uses `llvm-objdump` to disassemble and confirm the bytes match the
intended instructions — a *read-only* check; it never produces the artifact.

## Status (incremental)

- **M0** — PE shell + `mov eax,42; ret` exit stub. Validated the PE32+ layout, the
  materializer, and the thread-return exit mechanism. Exits 42.
- **M1** — a real interpreter for `imm`/`mul`/`halt` over a tape embedded in `.text`,
  with the 16 virtual registers in a writable `.data` (BSS) region (`rbx=&vregs`,
  `rsi=pc`). The embedded tape computes 6*7 and halts r0 → exits 42.

Next: the rest of the compute/memory/branch opcodes + `call`/`ret`, then byte I/O
via a kernel32 import table + tape loading, until the hand-assembled VM runs
`../as.tape` and reproduces the self-hosting fixed point with no Rust in the loop.
