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
| `compiler/beta-lang/` | The Beta compiler **written in Beta** (`bc.beta`) — slice 7. | **DONE — self-hosts** (byte-for-byte fixed point) |
| `compiler/delta/` | **The certificate checker** (`check.beta`) — the trust anchor. A natural-deduction / simply-typed-lambda proof checker, compiled by `bc`. | **PROTOTYPE** (in Beta; target is a Gamma program) |
| `compiler/gamma/` | `gamma.alpha` = parked v13 imperative compiler. **`interp.beta`** = interpreter-first reference interpreter (functional, ADTs + pattern matching, fuel-bounded) + **`typeck.beta`** = a static type checker + **`checker.gamma`** = the Delta checker in gamma, all in Beta. | interpreter + type system done |
| `compiler/epsilon*`, `compiler/alpha-rs` | Old/renamed experiment soup. | **IGNORE** |
| `compiler/omega-rs/` | The real Omega compiler, in Rust. Separate concern: the *producer*, not the lattice. | (other workstream) |

## Where we are RIGHT NOW

**The lattice has a complete vertical slice, seed → checker, with no Rust in the
runtime lineage.** One command verifies it all:

```sh
sh compiler/verify-lattice.sh        # -> "LATTICE VERIFIED ✓"
```

It walks every rung in dependency order:

- **alpha** — seed re-derives from source + conforms to `SEMANTICS.md` + diamond
  (`verify.sh`); 256 KB tape hole; both seeds in lockstep.
- **beta** — the assembler self-hosts byte-identically; gained a `db` data directive.
- **Beta language** — `beta-lang-rs` (throwaway Rust on-ramp) compiles the corpus.
- **bc** — the Beta compiler **written in Beta** (`beta-lang/bc.beta`) **self-hosts**:
  it compiles its own source to a compiler that reproduces that compilation
  byte-for-byte. From `bc` on, Rust is out of the lineage.
- **delta** — the **certificate checker** (the trust anchor): `check.beta` does full
  intuitionistic propositional logic (`-> & + ⊥`) **plus equality with the
  conversion rule** (`2+2=4` proved by computation), and `eq.beta` does definitional
  equality by fuel-bounded normalization. Both compiled by `bc`, run on the seed.
- **gamma** — `interp.beta`, the interpreter-first reference interpreter (stage 1:
  functional, fuel-bounded).

The lattice's thesis — *trust by checking, not pedigree* — is now a working stack.

## Roadmap (next)

7. **Self-hosting Beta compiler — DONE.** Rather than hand-write the compiler in
   assembly (the old framing), it is written *in Beta* (`compiler/beta-lang/bc.beta`)
   and cold-started through the on-ramp — the same on-ramp-then-discard pattern the
   assembler uses. **`bc` self-hosts:** it compiles its own source to a compiler
   that reproduces that compilation byte-for-byte (`sh compiler/beta-lang/selfhost.sh`),
   so from that compiler on, Rust is out of the lineage. bc implements the whole
   language — arithmetic, locals + symbol table, `if`/`else`/`while` + comparisons,
   procedures/params/calls/recursion, `byte[]`/`word[]` memory, char literals +
   `read_byte`/`write_byte` + call statements, and string literals via `emit("...")`.
   30/30 per-feature gate + the self-host fixed point. Enabled by the assembler
   `db` data directive (#46) and growing the arm64 tape hole 32 KB → 256 KB (bc's
   self-tape is ~45 KB; logic-dominated, not emit-dominated). The Rust on-ramp
   (`beta-lang-rs`) is now discardable from the steady state.
8. **Gamma — STAGE 1 DONE (interpreter-first).** Rather than grow the parked
   imperative `gamma.alpha`, the target gamma is the *interpreter-first, functional,
   ADT + pattern-matching* language the Delta checker is meant to be written in
   (rungs/gamma.md). [`compiler/gamma/interp.beta`](compiler/gamma/interp.beta) is
   the reference interpreter — *meaning is the interpreter*, fuel-bounded (totality)
   — stage 1: a pure functional core (ints, top-level recursive functions, `if`,
   `let`, arithmetic/comparisons; `fac 5→120`, `fib 10→55`, `gcd→12`), compiled
   Rust-free by `bc`. 11/11 in `test-interp.sh`. Stage 2 (ADTs + pattern matching) DONE; a static type system (`typeck.beta`,
   Int + ADTs, catches Int-vs-List etc.) DONE; the Delta checker rewritten in gamma
   (`checker.gamma`, full parity, ~12 functions vs ~350 lines) DONE — runs on the
   reference route and agrees with check.beta (`checker-diamond.sh`). The fully
   type-annotated `checker_typed.gamma` (which `typeck.beta` accepts) is DONE too,
   and is now mechanically tied to the trusted checker: `erase_types.py` type-erases
   it and the checker diamond runs it as a THIRD oracle agreeing with `checker.gamma`
   on ALL 83 cases (`frule_to_flat.py` rewrites the user-function proofs from the
   wrapper rule form to the typed flat form, so even `fdisp` — the one helper where the
   two representations diverge — is cross-checked). So "the checker is statically
   type-safe" and "the checker is behaviorally correct" are now claims about the SAME
   artifact, enforced every lattice run.
9. **Delta — the checker / evidence rung (where trust actually starts): PROTOTYPE
   DONE + GROWN.** [`compiler/delta/check.beta`](compiler/delta/check.beta) is now a
   **full intuitionistic propositional** proof checker (`-> & + ⊥`; Curry-Howard =
   simply-typed-lambda type checker) **plus equality with the conversion rule** —
   `(= t1 t2)` over Peano terms, with `refl` discharged by definitional computation,
   so `2+2=4` is proved *by reduction* and equations compose with the connectives.
   [`eq.beta`](compiler/delta/eq.beta) is a standalone definitional-equality checker
   (fuel-bounded normalization). Both compiled by `bc`, run on the seed; 32/32 in
   `compiler/delta/test.sh`. Demonstrates the architecture (tiny trusted checker,
   unbounded untrusted producer) AND the logic↔computation seam. Caught a real
   calling-convention bug (the prologue clobbered argument 3 — the checker's first
   3-arg `alloc`). Still a *Beta* prototype (target: a Gamma program once stage 2
   lands) and with **no soundness bridge** to actual program execution — the deep
   open problem. What exists is bounded *evidence* for that bridge, at the seam where
   a checker bug is most dangerous: `semantics-diamond.sh` (definitional `=` vs the
   interpreter's operational eval), `induction-soundness.sh` (inductive universals
   confirmed at concrete instances), and `soundness-sweep.sh` (curated corpus
   theorems that must be BOTH proved by check.beta AND computed true by the gamma
   interpreter — sourced straight from `proofs/*.elab`, so widening it is one line).

## How to build & verify (repo root; Git Bash on Windows, plain `sh` on macOS; `cargo` needed for `beta-lang-rs`)

The build scripts are **platform-aware** (`compiler/alpha/seed_env.sh` picks the
seed + stamping per host), so the same commands work on Windows (x64 PE seed) and
macOS arm64 (Mach-O seed, auto re-signed after stamping). On macOS the self-host
fixed point is asserted on the program bytes, not the OS-imposed code signature.

```sh
# verify the WHOLE lattice in one command (seed -> assembler -> Beta -> bc -> checker):
sh compiler/verify-lattice.sh                                  # -> "LATTICE VERIFIED ✓"

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
