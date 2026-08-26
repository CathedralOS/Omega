# Design Brief: Verified-Gated, ML-Native Optimizer

Scouted 2026-06-15. Status: OPEN QUESTIONS (direction only; no sign-off).

The concrete compiler architecture, build hook, verification boundary, and
folder ownership now live in
[`optimizer_architecture.md`](optimizer_architecture.md). The execution queue is
[`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md). This page remains the research
motivation and prior-art direction.

Omega's optimizer has two structural advantages over LLVM/GCC worth designing
FOR explicitly, rather than discovering later (LLVM retrofits both and fights its
own architecture doing so).

## Two structural advantages

1. **Higher ceiling from preserved semantics.** A large fraction of what LLVM/GCC
   do is heroically — and incompletely — *reconstructing* information C threw
   away: aliasing (the classic optimization killer; `restrict` exists because the
   compiler gives up), value ranges, effect/purity, whole-program shape. Omega
   keeps these as proved facts: ownership gives sound pervasive non-aliasing
   (when Rust first fed LLVM real `noalias` it exposed latent LLVM bugs — evidence
   LLVM was never stressed with that much alias info), domains give value ranges,
   empty reach gives purity, content-addressing gives whole-program inlining across
   what would be ABI boundaries. The ceiling is genuinely higher — the pedestal
   is partly unearned.

2. **A verified-equivalence acceptance gate.** Because the compiler proves its
   translation correct, an optimization can be admitted by *proof of equivalence*
   rather than human review + testing. That lets the optimizer accept a
   transformation from an untrusted source — a search, a learned policy, an LLM —
   *without trusting the source*: trust the checker, not the author. LLVM
   structurally cannot do this (it admits opts by review/testing), so it cannot
   safely ingest machine-generated optimizations at volume. This is the moat.

## The autotuning shape ("many inputs -> best function")

The goal "take many representative inputs and find the fastest function over
them" is the autotuning/search loop: propose candidate decisions, evaluate (run
or predict), keep the best. Battle-tested in narrow domains (Halide & TVM
auto-schedulers, PGO/BOLT, LLVM MLGO for inlining/regalloc, Souper/STOKE
search-then-verify). What makes it NATIVE rather than bolted-on is a set of
architectural seams to design in from the start:

- **Mechanism/policy separation** — every optimization DECISION externalized to a
  swappable oracle (heuristic | learned model | search), never buried in pass
  code.
- **Compiler as a deterministic callable function** — drivable in a tight
  `decisions -> binary (+ predicted perf)` loop, reproducible and parallel.
- **A learned cost model over the typed IR** — predict perf without running every
  candidate; the rich IR predicts better than opaque assembly.
- **The verified-equivalence gate as the universal admission path** — search/ML
  proposes, the prover disposes.
- **An IR that is a clean ML input** — the machine/state graph is GNN-ready, no
  structure to reverse-engineer.
- **Workload corpus/profiles as a first-class input**, plus native
  **specialization** (different best-functions per input distribution).

## Low-regret stance

- **Architect the seams now; build the ML optimizer incrementally later.** The
  seams cost little; retrofitting them (LLVM's pain) is the expensive path.
- **Keep the baseline sound + deterministic.** The OS must boot on a baseline
  that does not depend on ML. The search/ML/LLM layer TUNES the baseline; it is
  not a dependency.
- **Search is offline / build-time, emitting a fixed + verified result.** The
  shipped compiler stays deterministic and in-TCB (reproducible builds, the
  trusting-trust story); the ML never becomes a runtime oracle.

## Open questions

- What is the exact policy-oracle interface every pass calls, and is it uniform
  across high-level and backend passes?
- Can the verified-equivalence gate be made cheap enough to run per-candidate at
  search scale, or does it need a fast cost-model reject in front + full proof
  only on the winner?
- How much real-world performance is high-level (where preserved semantics help)
  vs backend (isel / regalloc / scheduling / uarch — a grind richer IR does not
  shortcut)? Where is the crossover?
- Does the learned cost model belong in the TCB? It only *ranks* candidates; the
  verified gate guarantees correctness regardless — so probably NOT, which is the
  point.
- How do profiles/corpora get captured, versioned, and fed without coupling the
  build to one workload?
- Does specialization / multi-versioning interact with content-addressed code
  dedup and hot-swap (chapter 22)?

## Cross-references

cathedral_alignment.md (TCB / trusting-trust, constant-time, IFC labels);
design_briefs/separate_compilation.md; Cathedral kernel_architecture.md (the
performance claim this raises the ceiling of) and omega_substrate.md.
