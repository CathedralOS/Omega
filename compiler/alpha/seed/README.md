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

- **M2** — the full non-I/O interpreter: all compute (`mov`/`add`/`sub`/`mul`/`div`/
  `mod`), memory (`load`/`store`/`loadb`/`storeb`), branch (`jmp`/`jz`/`jnz`/`jlt`/
  `jeq`), and `call`/`ret` (a VM call stack growing down from the top of a 64 MB
  memory in `.data`). The tape is embedded in `.text` and copied into VM memory at
  startup. Verified with two embedded tapes: sum 1..10 → 55 (imm/jlt/jmp/add), and
  a memory+subroutine program → 42 (store/load/mul/call/ret/sub). `read`/`write` are
  `ud2` placeholders until M3.

Next: byte I/O (`read`/`write`) via a kernel32 import table, then tape loading from
stdin/argv, until the hand-assembled VM runs `../as.tape` and reproduces the
self-hosting fixed point with no Rust in the loop.
