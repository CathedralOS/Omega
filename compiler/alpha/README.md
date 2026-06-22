# `compiler/alpha/` — the seed (the binary from god)

Alpha is the trust root of the whole lattice: one tiny, hand-written, hand-auditable
binary. Everything above it (`../beta`, …, `../omega-rs`) is built and checked down onto
this. There is nothing below it.

- **`alpha_x64_windows.exe` — the seed binary. This is the thing you audit.** It's a
  ~5 KB tape VM (a tiny register machine with byte I/O) followed by a big zero "hole".
  A built program is this exact binary with the program's bytes memcpy'd into the hole —
  so the one thing you audit is the one thing that runs everywhere.
- `alpha_x64_windows.hex` — the annotated x64 listing the binary is built from. To audit:
  disassemble the `.exe` and read it against this.
- `dev/` — how the binary is *reproduced* from the listing with no Python: `hex0` (a
  tiny hand-assembled hex→bytes transcriber) turns the committed flat hex into the
  binary. Not needed to use alpha — only to rebuild or re-verify the seed.
  `dev/reproduce.sh` checks it; `dev/regen.sh` rebuilds it after editing the listing.

Each platform gets its own seed here as it's hand-written:
`alpha_aarch64_linux.elf`, `alpha_aarch64_macos.app`, `alpha_x86_windows.exe`, …

The next rung — the assembler that turns text into tapes — is `../beta/`, written in
alpha (so its source is `.alp`).
