# `compiler/alpha/` — the seed (the binary from god)

Alpha is the trust root of the whole lattice: a tiny, hand-written, hand-auditable VM.
This folder is just **a bundle of seed binaries + the hex they're built from** — nothing
else lives here.

```
alpha_x64_windows.exe    the seed binary for this platform  (audit THIS)
alpha_x64_windows.hex    the annotated x64 listing it's built from  (audit AGAINST this)
```

To audit a seed: disassemble the `.exe` and read it against its `.hex` listing. That's
the entire trust obligation for the platform — a few hundred instructions.

## Per-platform vs cross-platform

The seed **cannot** be cross-platform: it's raw machine code in an OS executable format,
so x64-Windows and aarch64-Linux are different bytes. Each platform therefore gets its
own tiny hand-written seed here:

```
alpha_x64_windows.exe   alpha_aarch64_linux.elf   alpha_aarch64_macos.app   ...
```

What **is** cross-platform is everything *above* the seed. The seed is a tape VM (a
register machine with byte I/O) with a zero "hole" in it; a program is **tape** (VM
bytecode), and a built `.exe` is a seed with the program's tape memcpy'd into its hole.
The *same* tape runs on every platform's seed — so the assembler (`../beta`) and every
rung up are written once, cross-platform, and only the ~5 KB VM differs per machine.

So the "cross-platform thing that reproduces the rest of itself" is the **tape**, not
the binary. The binary is the small per-platform shim the tape runs on.

## Where the rest is

- `../beta/` — the assembler, written in alpha (`.alp`); turns text into tape, then
  memcpy's it into a seed to make a standalone `.exe`.
- The hex→binary forge (a no-Python transcriber + label resolver) lived under `dev/`;
  it isn't trust-bearing (you audit the committed binary, not the tool that emitted it),
  so it's out of the bundle — recoverable from git history if a from-source rebuild is
  wanted.
