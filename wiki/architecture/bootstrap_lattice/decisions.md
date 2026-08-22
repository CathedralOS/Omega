# Lattice — ratified decisions

[Lattice overview](bootstrap_lattice.md)

The overview decides the *principles* (trust-by-checking; meaning = reference
interpreter; checked refinement across every compiler edge; Rust dies) and deliberately leaves a set of
**open questions**. This document resolves them into standing decisions and an
execution order. It is the executive layer on top of the design: when the
overview says "emergent / to be decided," the calls here are the decision.

Format: each decision is **D#**, states the call, the rationale, and the resulting
policy. Decisions bind the construction; they do not touch language *meaning*
(owned by the language guide) nor `omega-rs` (the untouched reference producer).

---

## D1 — Rust exits by ROLE, not by rung. Kill it as meaning/checker first; as producer last.

The overview's "Two roles for Rust" is the ordering law. Made concrete, per artifact:

| Where Rust sits | Role | Status / plan |
| --- | --- | --- |
| `check.beta` / `checker.gamma` (the proof kernel) | **trusted base** | **DEAD** — Beta + Gamma implementations, cross-checked against shared seams. |
| cold-started `bc` artifact vs `bc.beta` | **trusted compilation edge** | **OPEN** — fixed point exists; complete lower-rooted source-to-artifact refinement does not. |
| `interp.beta` / `typeck.beta` (γ meaning) | **trusted base** | **DEAD** — Beta, on the seed lineage. |
| Delta's **meaning** (`gamma_emit.rs`) | **trusted base** | **DYING** — the broad Beta-written `omega2gamma` route and Gamma execution path exist, including checked D0 storage and real byte-I/O certifiers; exact coverage of the eventual Omega0 Delta source remains open. |
| Psi/Omega's **meaning** | **trusted base** | Follows the same elaboration discipline through the Delta-built bootstrap compiler and the Omega self-build edge. |
| `beta-lang-rs`, `delta-rs`, `omega-rs` (producers) | **untrusted producer** | **DEFERRABLE** — killed for self-sufficiency, not soundness. `omega-rs` stays untouched as the reference producer. |

**Policy:** no work removes Rust from a *producer* merely for pedigree while Rust
still sits in any meaning/checker or while an upstream artifact that builds those
checkers lacks lower-rooted refinement. Checking the cold-started `bc` output is
therefore trust closure, not a producer-language cleanup. Meaning-route Rust
removal otherwise outranks producer replacement. (This is why the Rust-free
Delta-to-Gamma elaborator outranks native-producer optimization. See D6.)

## D2 — Meaning is realized by ELABORATION to the nearest canonical interpreter — not a fresh native interpreter per rung.

The overview's canonical nesting is realized as: **a Rust-free elaborator
translates each rung's programs down into the
nearest rung that already has a canonical interpreter, and that interpreter runs
them.** Delta's meaning is exposed by a Rust-free Delta-to-Gamma elaborator;
Gamma's canonical `interp.beta` runs the result.

**Rationale.** Same semantic content as a bespoke interpreter (delta's
operational semantics written down in a lower rung), but *staged* as
translate-then-run, which is strictly **smaller**: it reuses Gamma's interpreter,
ADTs, recursion, and pattern matching instead of re-implementing an environment +
store + dispatch loop inside Gamma. The elaborator is a simple structural
desugaring (imperative → SSA-threaded functional), **not** an optimizing compiler,
so it does not violate "don't let a compiler be the definition." Two independent
checks keep it honest: (a) it is auditable Beta on the seed lineage; (b) the
**meaning diamond** cross-checks it against the independent native backend.

**Reconciliation with the overview.** Gamma is the general-purpose interpreter
substrate. Delta elaborates to Gamma; the proof kernel is a separate assurance
service and is not part of the language path. This refines the overview without
changing “meaning = reference interpreter.”

## D3 — Trust flows through PROOFS, not through trusting native binaries. Native-code trust ends at translation validation.

The provable chain seed → Psi/Omega is **proof-carrying**: the artifact verifier
reconstructs the claims required by the exact fingerprinted artifact, and the
producer ships derivations which the Rust-free, independently implemented proof
kernel checks. A backdoored *producer* can at worst emit output that **fails**
verification — it cannot select an easier claim, omit a generated obligation, or
forge a derivation the kernel accepts. The soundness of a proof about the
fingerprinted artifact never depends on the proof-producing compiler. A separate
checked-source-to-artifact or artifact-to-native refinement claim is required
before transferring that result across a compilation boundary. Psi-aware
reconstruction is now placed: a total low-rung semantic-ledger definition
consumes canonical terminal-Psi bytes and emits exhaustive canonical goals plus
validity-scoped local premises. Direct evaluation or a checked derivation of
that definition is authoritative; optimized Rust agreement grants no authority.
Algebraic reduction proves the canonical goals rather than replacing them.
Kernel acceptance alone remains insufficient because it neither constructs that
ledger nor proves the separate semantic soundness bridges.

For **native-code** trust (Cathedral running real binaries, where the compiler is
the isolation boundary), the endpoint is **translation validation**: the backend
emits, *per compilation*, a kernel-checkable certificate that its machine code
**refines** the source's meaning. Then even a backdoored backend is caught — a
backdoored output would fail its own refinement check.

**Policy / staging:**
- **Now:** the convergence gates (a Delta program emits a certificate about its own
  *result*) are the first instance of proof-carrying output. Keep them central.
- **Now:** native backends are checked by the **meaning diamond** (test-based
  agreement with the Rust-free meaning) and are explicitly **outside the soundness
  base** for proofs about source.
- **North star:** per-compile refinement certificates (the backend as a checked
  producer). This is where `omega-rs`'s "certs about real binaries vs a hardware
  model" ambition rejoins the lattice.

## D4 — The soundness bridge is built empirically via SEAMS now; every proof-kernel capability ships a paired seam.

`kernel-accepted ⟹ true-about-execution` (overview honest-edge #1) is the hard
core. It is attacked **empirically today** by the soundness *seams* — kernel
derivation vs operational evaluation (induction, predicates, propositional logic,
the soundness sweep, the convergence routes).

**Standing policy:** every new logical capability the proof kernel gains **must** ship with a
paired seam that cross-checks kernel-provability against operational truth on a
corpus (+ a negative battery that must be rejected). A kernel feature without its seam
is not done. This keeps the bridge honest as the kernel grows, and turns the "deep open
problem" into a continuously-tested invariant. Seams remain evidence, not the
endpoint: universally quantified row theorems plus separate safety/partial-
correctness and progress/termination composition theorems must eventually bind
the exact pinned operational semantics. Logical fuel is not termination
evidence.

**Capstone (deferred):** a kernel-checked proof of the kernel's own soundness w.r.t. the Alpha
small-step semantics — the formal bridge. Not attempted until the metatheory
tooling exists; the seams are the standing substitute.

## D5 — Checked refinement, not DDC, closes compiler provenance.

**Supersedes the 2026-07-02 D5 ruling; ratified 2026-08-22.** Diverse double
compilation is not a trust requirement of this architecture. DDC answers whether
a compiler binary corresponds to its source by comparing builds through another
compiler. The lattice uses the stronger rule from D3: an independently
reconstructed, lower-rooted check establishes that the exact produced artifact
refines the canonical meaning of its exact source. The producer is untrusted, so
its implementation language, ancestry, and agreement with another producer do
not grant authority.

For each compiler edge, the required shape is:

```text
source + produced artifact
          ↓ canonical obligation reconstruction
artifact refines canonical source meaning
          ↓ lower-rooted proof/semantic checking
accept or reject
```

A Thompson payload changes the artifact or its behavior and therefore fails this
check. DDC adds no soundness once that edge is closed. Exact output agreement is
also the wrong long-term contract: two correct compilers may emit different
artifacts, while two incorrect compilers may agree. Requiring byte identity
between implementations unnecessarily creates a second compiler to maintain and
conflates reproducibility with correctness.

The current `bc` cold start still passes through `compiler/beta-lang-rs/`; its
self-host fixed point establishes dependency closure, not source correspondence.
That is an **unfinished lower-rooted refinement edge**, not a standing demand for
DDC. Close it by building the seed Beta compiler through the preceding audited
rung or by validating the complete `bc` artifact against `bc.beta` with authority
rooted below `bc`.

`compiler/beta-lang-py/bc2.py` and its comparison scripts may remain temporarily
as untrusted regression/reference tools. They are not part of the trusted
lineage, do not close an architectural proof obligation, and are not required in
the final lattice. Useful interpreter and symbolic-evaluation code in that
directory should be retained by role when the repository is reorganized.

Independent Alpha realizations and independent proof-kernel implementations are
not DDC. They are conformance and soundness cross-checks against explicit
semantics, useful for finding implementation mistakes while the corresponding
formal bridges mature. Their multiplicity supplies evidence; it is not the rule
that grants an artifact authority.

**Policy:**

- Do not add second or third compilers merely for DDC.
- Do not make cross-implementation byte identity a release or trust requirement.
- Require deterministic reproduction where it serves build identity and audit,
  but never present a fixed point as correctness evidence.
- Close every compiler edge with lower-rooted source-to-artifact refinement.
- Keep differential/reference implementations only where their bug-finding value
  justifies their maintenance cost.

## D6 — The language spine reaches Omega through Delta; Omega then rebuilds itself. The proof kernel is orthogonal.

**Ratified 2026-08-04; clarified 2026-08-22.** The small bootstrap
languages form the audited spine:

```text
Alpha → Beta → Gamma → Delta
```

Delta is the systems/compiler-host on-ramp. Its job is to build a deliberately
simple, spec-compliant Omega compiler from the audited seed. That bootstrap
compiler need not contain the production optimizer or advanced lowering
pipeline; it must compile the language correctly. It then compiles the full
Omega compiler written in Omega:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              Omega (Delta-built, simple)
                           ↓
              Omega (Omega-built, optimized)
```

The repeated Omega is deliberate. These are two compiler artifacts for the same
language, not two additional language rungs. The first is a valid stopping point
for a self-sufficient system, although compilation and generated code may be
slow. The second replaces a historical tower of implementation-language
dependencies with one ordinary self-host edge. As with every self-host, a defect
in the first Omega compiler can reproduce into the second; proof, meaning, and
translation-validation gates remain responsible for detecting that defect.

`compiler/delta-rs/` is Delta's disposable Rust implementation; the self-hosted
`lowermachine.alp` and meaning diamond remain its principal gates.

The certificate checker is renamed the **proof kernel**. Its current
`compiler/proof-kernel/` path is historical; its target owner is
`bootstrap/assurance/proof-kernel/`. It remains a trusted assurance service with
Beta and Gamma implementations. The rename changes neither its authority nor its
validation gates; it removes the false claim that proof checking is a language
stage between Gamma and Delta.

Psi remains the source-semantics and terminal-portable-IR owner inside the Omega
product toolchain. The minimal Psi/Omega path needed to accept and compile Omega
source is first hosted from Delta; the complete production compiler is then
built from Omega source by the Delta-built Omega compiler. Neither Psi nor either
Omega compiler artifact is another Greek bootstrap language.

The Rust implementations remain current producers while this two-stage hosted
path matures.

---

## The chain, end to end

```
α  seed VM ...... small written semantics + audited x64/arm64 realizations               [ROOT: execution]
│                 hand-audited; own small-step semantics
β  assembler .... written in α-asm, run by α; self-hosts                                [derived from α]
   bc ........... Beta compiler in Beta; self-hosts; whole-artifact refinement open      [D5 work]
γ  interpreter .. interp.beta (+ typeck): the canonical MEANING substrate               [Rust-free]
δ  systems ...... compiler-host language; meaning elaborates δ → γ                      [D2; Rust removal active]
                  builds a simple, spec-compliant Omega compiler                          [bootstrap producer]

proof kernel .... check.beta + checker.gamma; cross-cutting derivation checker           [Rust-free, audited]
Omega₀ .......... Delta-built bootstrap compiler; correct but minimally optimized         [valid endpoint]
Omega₁ .......... Omega₀ builds the full Omega-source production compiler                 [self-host edge]
Psi ............. source semantics + terminal IR inside both Omega compiler products      [not a rung]
```

**What "provable" buys at the top:** in the completed architecture, a certificate
the proof kernel accepts is trustworthy back to the seed because the kernel and
its soundness bridges are audited and every compiler artifact in that path has a
lower-rooted refinement check. A false proposition cannot get a certificate past
the kernel merely by controlling the producer. Today the whole-artifact `bc`
cold-start refinement is still an explicit open edge; cross-compiler agreement
does not close it.

## Execution order (binds the /loop)

1. **Close the `bc` cold-start edge by checked refinement** — validate the
   complete `bc` artifact against `bc.beta` with authority rooted below `bc`, or
   build it through the preceding audited rung. The Python comparison path is
   not the closure criterion. *(D3/D5)*
2. **Finish Delta's Rust-free meaning route** — retain the existing
   `omega2gamma.beta` → `interp.beta` coverage for state machines, self fields and
   calls, arrays, byte I/O, and D0 storage; close every construct used by the
   eventual Omega0 source and demote `gamma_emit.rs` to a reference producer.
   *(D1 urgent kill.)*
3. **Grow the proof kernel and its seams in lockstep** — no capability without its paired seam. *(D4)*
4. **Translation-validation backend** — per-compile refinement certs. *(D3 north star, later.)*
5. **Build bootstrap Omega from Delta** — host the minimal spec-compliant
   Psi/Omega path needed to compile Omega source. Optimization is not a gate for
   this artifact. *(D2/D6, later.)*
6. **Use bootstrap Omega to build production Omega** — compile the full
   Omega-source optimizer/lowering pipeline, then apply the normal meaning and
   translation-validation gates to the self-host edge. *(D3/D6, later.)*
