# `source/alpha/` — the native seed executor

Alpha is the native execution floor of the lattice: a small, hand-written,
hand-auditable VM checked against written semantics.
This owner contains the per-platform seed binaries and audited listings, the
written semantics and conformance tools, the executable reference realization,
and the Alpha-written assembler. Cross-rung Beta-to-Alpha refinement
reconstruction lives under `source/refinement/alpha-beta/`; temporary
symlinks here preserve its historical entry points without assigning it to the
Alpha rung.

```
alpha_x64_windows.exe    seed binary, x86-64 Windows PE   (audit THIS)
alpha_x64_windows.hex    the annotated x64 listing it's built from  (audit AGAINST this)

alpha_arm64_macos        seed binary, arm64 macOS Mach-O  (audit THIS)
alpha_arm64_macos.s      the hand-authored arm64 source it's built from  (audit AGAINST this)
alpha_arm64_macos.lst    a committed disassembly, to ease reading the binary against the source

seed_env.sh              per-platform seed selection + tape-stamping, sourced by the build scripts
resize-x64-tape-hole.py  one-purpose audited 32 KiB -> 256 KiB PE extent migration

SEMANTICS.md             the written small-step operational semantics — the meaning a seed is audited AGAINST
conformance.sh           executable companion: hand-built tapes pinning every opcode + edge; any seed must pass
verify.sh                the per-platform acceptance gate: provenance + conformance + reproduction

assembler/               Alpha-written assembler, self-host gate, reference cross-check, and examples
alpha_ref.py             untrusted executable reference realization of Alpha meaning
refinement*              compatibility symlinks to cross-rung assurance tooling
```

`sh verify.sh` runs the whole local trust check for the host's seed:

- **provenance** — re-derives the committed binary from its source and confirms a match
  (arm64: `clang -arch arm64 -Wl,-no_uuid …`, reproducible modulo the OS signature; x64:
  audit the `.exe` against its `.hex` by hand; the committed resize helper only changes
  three documented PE capacity fields and extends the zero-only tape section);
- **behavior** — `conformance.sh` (every opcode + edge realizes `SEMANTICS.md`);
- **reproduction** — `assembler/selfhost.sh` (the VM reproduces the canonical assembler bytecode).

To audit a seed: disassemble the binary and read it against its listing (the `.hex` for
x64, the `.s` + `.lst` for arm64), checking that each opcode realizes the transition in
`SEMANTICS.md`. That's the entire trust obligation for the platform — a few hundred
instructions. `conformance.sh` mechanically checks the runtime behavior against the spec
(`sh conformance.sh` runs the host seed through every case).

## Platform realizations and executable references

The x64 and arm64 seeds are **independently hand-authored** (different ISA, OS, executable
format, and author). They are not transcriptions of each other — they are two
realizations of the same 21-opcode small-step semantics. Feeding the *same source*
through both VMs produces **byte-identical tapes**, so disagreement exposes a
conformance or implementation problem. The written semantics and audited
implementation correspondence supply authority; multiplicity supplies useful
evidence. Verified: the arm64 macOS VM reproduces the x64 VM's assembler
bytecode from `assembler/assembler.alpha` byte-for-byte (sha256 `945c8061…`), and the full
example corpus runs to the same answers on both.

**Third point — `alpha_ref.py`.** The two seeds are hand-authored *assembly*, hard to audit.
`alpha_ref.py` is a third, independent realization of the same semantics in ~150 lines of
Python, written straight from `SEMANTICS.md` and short enough to read line by line against
it. It is **UNTRUSTED and checked**: `diamond-py.sh` runs
opcode edges (signedness, traps, EOF) *and* real bc-compiled programs through both the host
seed and `alpha_ref.py` and asserts they agree — so the semantics is now pinned by two opaque
seeds *and* one auditable reference. The runtime lineage never runs `alpha_ref.py`; it is a
verification instrument and regression oracle. Run `sh diamond-py.sh`.

macOS note: `dd`-stamping a tape into a Mach-O invalidates its code signature, and Apple
Silicon refuses to exec an invalid one, so a stamped seed is re-signed (`codesign -f -s -`)
by `seed_env.sh`. The signature blob is OS-imposed and non-reproducible; the bootstrap's
byte-identical guarantee therefore lives in the program bytes (the tape), not the
signature — which is exactly what `selfhost.sh` compares.

Both committed seeds reserve a 256 KiB tape hole; `stamp_seed` rejects the tape
before copying when its four-byte length prefix would exceed that extent. For
x64, the one-purpose `resize-x64-tape-hole.py` records the complete migration
from the old 32 KiB PE:
`SizeOfImage`, `.tape` virtual size, and `.tape` raw size change, followed only by
zero extension. It refuses any other input shape, which makes the capacity-only
change independently reproducible without restoring the historical general forge.

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
The *same* tape runs on every platform's seed — so the assembler (`assembler/`) and every
rung up are written once, cross-platform, and only the ~5 KB VM differs per machine.

So the "cross-platform thing that reproduces the rest of itself" is the **tape**, not
the binary. The binary is the small per-platform shim the tape runs on.

## Where the rest is

- `assembler/` — the assembler, written in alpha (`.alpha`); turns text into tape, then
  memcpy's it into a seed to make a standalone `.exe`.
- The hex→binary forge (a no-Python transcriber + label resolver) lived under `dev/`;
  it isn't trust-bearing (you audit the committed binary, not the tool that emitted it),
  so it's out of the bundle — recoverable from git history if a from-source rebuild is
  wanted.
