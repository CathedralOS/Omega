# `source/alpha/` — the native seed executor

Alpha is the native execution floor of the lattice: a small, hand-written,
hand-auditable VM checked against written semantics.
This owner contains the per-platform seed binaries and audited listings, the
written semantics and conformance tools, the executable reference realization,
the Alpha-written assembler, and the root derivation checker. Beta compiler
admission lives with the artifact it admits under
`source/beta/compiler/validation/`.

The Python executable reference is temporary development scaffolding. It has
no place in the completed offline bootstrap and is deleted once the checked
Alpha realization/conformance route subsumes its bounded comparison.

```
alpha_x64_windows.exe    seed binary, x86-64 Windows PE   (audit THIS)
alpha_x64_windows.hex    the annotated x64 listing it's built from  (audit AGAINST this)

alpha_arm64_macos        seed binary, arm64 macOS Mach-O  (audit THIS)
alpha_arm64_macos.s      the hand-authored arm64 source it's built from  (audit AGAINST this)
alpha_arm64_macos.lst    a committed disassembly, to ease reading the binary against the source

seed_env.sh              per-platform seed selection + tape-stamping, sourced by the build scripts

SEMANTICS.md             the written small-step operational semantics — the meaning a seed is audited AGAINST
ASSEMBLY.md              authoritative `.alpha` grammar and deterministic two-pass payload encoding
conformance.sh           executable companion: hand-built tapes pinning every opcode + edge; any seed must pass
verify.sh                full seed check; --edge omits the provenance diagnostic

assembler/               Alpha-written assembler, self-host gate, reference cross-check, and examples
checker/                 rooted derivation-checker service beside the compiler lattice
alpha_ref.py             untrusted executable reference realization of Alpha meaning
```

`sh verify.sh` runs the whole local trust check for the host's seed:

- **provenance** — re-derives the committed binary from its source and confirms a match
  (arm64: `clang -arch arm64 -Wl,-no_uuid …`, reproducible modulo the OS signature; x64:
  audit the `.exe` against its `.hex` by hand; the historical resize migration changed
  only three documented PE capacity fields and extended the zero-only tape section);
- **behavior** — `conformance.sh` (every opcode + edge realizes `SEMANTICS.md`);
- **reproduction** — `assembler/selfhost.sh` (the VM reproduces the canonical assembler bytecode).

`sh verify.sh --edge` is the direct-lattice mode. It retains behavior and exact
assembler construction but omits the native container rebuild: the selected
audited seed is the chain's floor, while reconstructing that container from its
assembly source is a supply-chain diagnostic and seed-admission aid rather than
another compiler-correctness premise.

To audit a seed: disassemble the binary and read it against its listing (the `.hex` for
x64, the `.s` + `.lst` for arm64), checking that each opcode realizes the transition in
`SEMANTICS.md`. `ASSEMBLY.md` separately fixes how readable Alpha source becomes
the exact cross-platform payload. That's the entire trust obligation for the platform — a few hundred
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
temporary verification instrument and regression oracle. It supplies no
authority and is not retained after the direct checked relation covers this
failure surface. Run `sh diamond-py.sh` while it remains.

macOS note: `dd`-stamping a tape into a Mach-O invalidates its code signature, and Apple
Silicon refuses to exec an invalid one, so a stamped seed is re-signed (`codesign -f -s -`)
by `seed_env.sh`. The signature blob is OS-imposed and non-reproducible; the bootstrap's
byte-identical guarantee therefore lives in the program bytes (the tape), not the
signature — which is exactly what `selfhost.sh` compares.

Both committed seeds reserve a 256 KiB tape hole; `stamp_seed` rejects the tape
before copying when its four-byte length prefix would exceed that extent. The
completed x64 32 KiB-to-256 KiB extent migration and its one-purpose script are
recoverable from Git history; retaining a completed mutation tool in the live
seed owner would add another apparent construction route.

## Retention inventory

| Retained child/files | Direct role | Deletion condition |
| --- | --- | --- |
| `assembler/` | The Alpha-written assembler and its exact self-host/reference gates. | Replace only with a smaller audited Alpha assembler that preserves exact encoding. |
| `checker/` | The rooted certificate-checker service used beside compiler edges. | Delete when an equally low or lower accepted checker service replaces it. |
| `SEMANTICS.md`, `ASSEMBLY.md` | Authoritative Alpha execution and assembly relations. | Replace only atomically with a ruled Alpha revision and its consumers. |
| `alpha_arm64_macos`, `alpha_arm64_macos.s`, `alpha_arm64_macos.lst` | Selected Darwin seed, its hand-authored source, and audit disassembly. | Delete only when Darwin arm64 support is retired or an equally audited conforming seed replaces all three. |
| `alpha_x64_windows.exe`, `alpha_x64_windows.hex` | Selected Windows seed and its annotated audit listing. | Delete only when Windows x64 support is retired or an equally audited conforming seed replaces both. |
| `seed_env.sh` | Select and stamp the exact host seed without changing tape identity. | Delete when every caller executes raw tape through an equally audited interface. |
| `conformance.sh`, `verify.sh` | Pin every opcode edge and run the canonical seed plus assembler-construction gate. | Delete a check only when a stronger checked seed admission subsumes it. |
| `alpha_ref.py`, `diamond-py.sh` | One temporary independent executable semantics and its bounded seed comparison. | Delete together when the checked Alpha realization/conformance relation subsumes the diagnostic; never retain them in the completed bootstrap. |

The duplicate random seed/reference fuzzer and its generator were deleted: the
retained conformance suite plus one independent diamond own that failure
surface. Completed mutation scripts and historical forges likewise remain only
in Git history.

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

Beta, Gamma, Delta, `omega₀`, and `omega` compiler artifacts are therefore
identified by exact Alpha tapes, never by stamped native containers. If
execution needs acceleration, only a general Alpha-to-native realization
checked against Alpha semantics is eligible. Source-, function-, hash-, or
workload-specific native substitutions are forbidden.

## Where the rest is

- `assembler/` — the assembler, written in alpha (`.alpha`); turns text into tape, then
  memcpy's it into a seed to make a standalone `.exe`.
- The hex→binary forge (a no-Python transcriber + label resolver) lived under `dev/`;
  it isn't trust-bearing (you audit the committed binary, not the tool that emitted it),
  so it's out of the bundle — recoverable from git history if a from-source rebuild is
  wanted.
