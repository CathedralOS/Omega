# Bootstrap Lattice — Status & Onboarding

> Live status of the **self-building lattice** only — decoupled from the main
> compiler's [TASKS.md](TASKS.md). If you are a fresh agent, read this top to
> bottom, then the architecture overview, and you are caught up.

## 30-second orientation

Omega is being built **from a tiny hand-audited seed, rung by rung**, so the
trusted toolchain has no Rust/LLVM in its lineage. The principle is *trust by
checking, not by pedigree*. The full architecture (trust model, the rungs, the
honest edges) lives in
**[wiki/architecture/bootstrap_lattice/bootstrap_lattice.md](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md)
— read it first.**

## Read-first

| Doc | What |
| --- | --- |
| [bootstrap_lattice.md](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md) | The overview: trust-by-checking, two stacks, rung table, honest edges, the two-roles-for-Rust cut |
| [rungs/*.md](wiki/architecture/bootstrap_lattice/rungs/) | One doc per rung (alpha … omega) |
| [compiler/beta/CALLING_CONVENTION.md](compiler/beta/CALLING_CONVENTION.md) | The calling convention (proven on the seed) |
| [compiler/beta/LANGUAGE.md](compiler/beta/LANGUAGE.md) | The Beta language surface (v0) |
| [compiler/beta-lang-rs/README.md](compiler/beta-lang-rs/README.md) | The Beta compiler on-ramp + slice status |

## The map (what's on disk)

| Path | What it is | Status |
| --- | --- | --- |
| `compiler/alpha/` | **The seed**: a 21-opcode register tape VM. **Two independent hand-authored realizations** — x64 Windows PE (`.exe` + audited `.hex`) and arm64 macOS Mach-O (`alpha_arm64_macos` + `.s` + `.lst`). The provenance root, and the first lattice diamond (same source → byte-identical tapes on both). | DONE (x64 + arm64-macOS) |
| `compiler/beta/` | **The assembler** (`assembler.alpha`, written in Alpha assembly) + the Beta-language docs/examples. | DONE — self-hosts byte-identically |
| `compiler/beta-rs/` | Throwaway Rust on-ramp for the *assembler* (cold-start only). | parked |
| `compiler/beta-lang-rs/` | Throwaway Rust on-ramp for the **Beta-language compiler** (`.beta` → Alpha asm). | slices 1–6 done; self-check passed |
| `compiler/beta-lang/` | The Beta compiler **written in Beta** (`bc.beta`) — the self-hosting path (slice 7). | **ACTIVE** — slice 1 (arithmetic) done |
| `compiler/gamma/` | A v13 imperative language; compiler hand-written in Alpha asm (`gamma.alpha`). The thing Beta exists to **supersede**. | parked at v13 |
| `compiler/epsilon*`, `compiler/alpha-rs` | Old/renamed experiment soup. | **IGNORE** |
| `compiler/omega-rs/` | The real Omega compiler, in Rust. Separate concern: the *producer*, not the lattice. | (other workstream) |

## Where we are RIGHT NOW

**Active workstream: building Beta-the-language** — a small structured systems
language (tiny-C / Oberon-0 shape) so we stop hand-writing compilers in assembly.
`beta-lang-rs` compiles `.beta` → Alpha assembly; the assembler lowers it; the seed
runs it. (It's a *throwaway Rust on-ramp* — see "on-ramp pattern" below.)

Slices 1–6 done and **verified on the seed**:

1. arithmetic
2. procedures, ≤4 params, calls (the calling convention, generated mechanically)
3. `if`/`else`, `while`, `let` locals, assignment, comparisons → **recursion + loops**
4. explicit memory (`byte[]` / `word[]`) → raw arrays/buffers
5. ergonomics + host boundary: char literals (`'a'` + escapes), `read_byte()`/
   `write_byte(x)` intrinsics, call-as-statement
6. **the self-check** (see below)

8 example programs pass (`answer 42 · double 42 · calls 10 · factorial 120 · fib 55
· sumto 55 · arrays 30 · bytes 131`). `factorial.beta`/`fib.beta` produce the same
answers as the hand-written `.alpha` proofs in `compiler/beta/examples/`.

**Self-check PASSED (slice 6).** [`beta-lang-rs/examples/calc.beta`](compiler/beta-lang-rs/examples/calc.beta)
is a **recursive-descent calculator written in Beta** — reads an arithmetic
expression from stdin (decimal ints, `+ - * /`, parens, precedence, whitespace),
evaluates it, prints the decimal result, returns it as the exit code. It exercises
the whole surface (char literals, the I/O boundary, a memory-backed buffer +
cursor, recursion through the grammar). `2+3*4`→14, `(2+3)*4`→20, `2*(3+4)*5`→70;
calc tape ≈ 6.7 KB. `sh compiler/beta-lang-rs/test.sh` runs the gate (8 examples +
9 calc cases, all green). **Beta is now demonstrably compiler-grade** — the key
judgment below is confirmed, not just plausible.

**Key judgment (confirmed):** Beta is *more capable than the assembler ever was*
(the assembler was hand-written in raw asm with none of procedures/recursion/
locals/memory), and the self-check proves a real parser/evaluator is pleasant to
write in it. The next move is the transcription, not more features.

## Roadmap (next)

7. **Self-hosting Beta compiler — IN PROGRESS.** Rather than hand-write the
   compiler in assembly (the old framing), it is being written *in Beta*
   (`compiler/beta-lang/bc.beta`) and lowered through the on-ramp — the same
   on-ramp-then-discard pattern the assembler uses, reaching a self-hosting fixed
   point (`bc` compiles `bc.beta` to a tape that recompiles `bc.beta` identically).
   Slices **1, 2a, 2b done + gated** (`sh compiler/beta-lang/test.sh`, 20/20):
   arithmetic; a real tokenizer + per-proc symbol table with `let` locals,
   assignment, variable refs; `if`/`else`/`while` + the six comparisons. bc has a
   full single-proc front end (compact-asm output, pre-scan frame sizing).
   **Blocker now HIT:** bc's tape is **30.7 KB of the 32 KB hole** — the next slice
   (memory, char/string literals, or procedures/calls) cannot fit. `emit` lowers
   each output byte to an `imm`+`write` (~12 tape bytes/char). Two resolutions:
   (a) **assembler `.db` data section + `write_str`** — ~1 tape byte/char,
   *preserves both-seeds lockstep* and shrinks tapes ~10×, but is surgery on the
   trusted self-hosting assembler (must stay byte-identical); **recommended**.
   (b) **enlarge the tape hole** — one-line `.space`/`.hex` change, but must be
   done on *both* seeds to keep the diamond; the x64 forge isn't available on this
   host, so doing only arm64 introduces a flagged asymmetry. See
   `compiler/beta-lang/README.md`.
8. **Rewrite gamma in Beta**, retiring `gamma.alpha` (this is the whole point:
   never hand-write a compiler in assembly again).
9. **Climb:** Delta (the checker / evidence rung — where trust actually starts),
   then up.

## How to build & verify (repo root; Git Bash on Windows, plain `sh` on macOS; `cargo` needed for `beta-lang-rs`)

The build scripts are **platform-aware** (`compiler/alpha/seed_env.sh` picks the
seed + stamping per host), so the same commands work on Windows (x64 PE seed) and
macOS arm64 (Mach-O seed, auto re-signed after stamping). On macOS the self-host
fixed point is asserted on the program bytes, not the OS-imposed code signature.

```sh
# beta self-hosts (byte-identical program-byte fixed point):
sh compiler/beta/selfhost.sh                                   # -> "self-host ✓"

# gamma still rebuilds + runs from .gamma source:
sh compiler/gamma/rebuild.sh
sh compiler/gamma/build.sh && ./compiler/gamma/build/answer.exe ; echo $?   # -> 42

# the Beta compiler, end to end (Beta -> asm -> tape -> seed):
sh compiler/beta-lang-rs/build.sh examples/factorial.beta
./compiler/beta-lang-rs/build/factorial.exe ; echo $?          # -> 120

# the hand-written calling-convention proofs:
sh compiler/beta/build.sh examples/fib.alpha
./compiler/beta/build/fib.exe ; echo $?                        # -> 55
```

A program is a *tape* memcpy'd into the seed's hole; exit code = the low byte of the
result, so examples return small numbers on purpose.

## Things a fresh agent MUST know (gotchas)

- **Two graphs — don't conflate.** The *implementation* graph (what each tool is
  written in: the assembler in Alpha asm, the Beta compiler prototyped in Rust) is
  separate from the *lowering* graph (how a program becomes machine code). Delta /
  Epsilon are **not** IRs that `omega-rs` emits; they are rungs whose *tools* are
  written in the rung below.
- **On-ramp-then-discard.** The `*-rs` crates (`alpha-rs`, `beta-rs`, `beta-lang-rs`)
  are **throwaway Rust** used to design each rung ergonomically. The *trusted*
  version is later transcribed into the rung below and the Rust is discarded. Rust
  is the drafting table, never the foundation.
- **Naming.** "beta" currently means **two** things: the *assembler*
  (`compiler/beta/`, `beta-rs`) and the *Beta language* (`LANGUAGE.md`,
  `beta-lang-rs`). Long-view cleanup (the assembler is really *Alpha's* assembler,
  freeing "beta" for the language) is deferred. Names across the lattice are
  emergent, not fixed.
- **Extensions** (renamed this session): `.alpha` = Alpha assembly, `.gamma` =
  Gamma source, `.beta` = Beta language, `.omg` = Omega.
- **The 32 KB hole.** The seed memcpy's a program tape into a *fixed 32 KB hole*.
  gamma's tape is ~21 KB; the Beta compiler's own tape will eventually outgrow it.
  The fix (make the arena a parameter) is a small VM change — flagged, not done.
  Watch for `exceeds the seed's 32 KB hole` build failures.
- **The calling convention** (`CALLING_CONVENTION.md`): two stacks — control on the
  VM's hidden `call`/`ret` stack, data on an explicit stack via `r15` (sp), `r14` =
  frame pointer; args `r0`–`r3`, result `r0`.
- **Determinism is load-bearing.** Builds are byte-identical/reproducible (the
  basis of the provenance story). `selfhost.sh` asserts beta reproduces itself
  exactly; don't introduce nondeterminism (clocks, hash-order, timestamps).
- **Bash cwd persists** between tool calls — use absolute paths or `git -C` to
  avoid drift.

## Why all this (trust-story north star)

The seed is the **provenance** root (lineage + determinism), **not** a correctness
oracle — a self-hosting fixed point proves *consistency*, not *faithfulness*. Real
trust eventually comes from a small **checker** (the Delta rung) that validates
*evidence*, not from self-hosting. Rust leaves the trusted base entirely (it may
remain an untrusted *producer* in the interim). The endpoint and the TCB ledger
({seed, checker, specs, hardware}) are in
[proof_engine_north_star.md](wiki/design_briefs/proof_engine_north_star.md) and
[cathedral_alignment.md](wiki/cathedral_alignment.md); the full reasoning is in the
[architecture overview](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md).
