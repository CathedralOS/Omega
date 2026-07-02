# Lattice — ratified decisions

[Lattice overview](bootstrap_lattice.md)

The overview decides the *principles* (trust-by-checking; meaning = reference
interpreter; diversity = security; Rust dies) and deliberately leaves a set of
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
| `check.beta` / `checker.gamma` (the checker δ) | **trusted base** | **DEAD** — Beta + Gamma, double-implemented, diamonded. |
| `interp.beta` / `typeck.beta` (γ meaning) | **trusted base** | **DEAD** — Beta, on the seed lineage. |
| epsilon's **meaning** (`gamma_emit.rs`) | **trusted base** | **DYING** — replaced by `epsilon/eps2gamma.beta` (Rust-free), slice by slice. *This is the current urgent kill.* |
| omega's **meaning** | **trusted base** | Follows the same elaboration route (D2), once epsilon's is complete. |
| `beta-lang-rs`, `epsilon-rs`, `omega-rs` (producers) | **untrusted producer** | **DEFERRABLE** — killed for self-sufficiency, not soundness. `omega-rs` stays untouched as the reference producer. |

**Policy:** no work removes Rust from a *producer* while Rust still sits in any
*meaning/checker*. Meaning-route Rust removal outranks producer Rust removal,
always. (This is why `epsilon/eps2gamma.beta` is prioritized over the
`lowermachine.alp` native-self-host — the former is a trusted-base kill, the
latter a producer nicety. See D6.)

## D2 — Meaning is realized by ELABORATION to the nearest canonical interpreter — not a fresh native interpreter per rung.

The overview's canonical nesting ("Epsilon interpreter written in Delta") is
realized as: **a Rust-free elaborator translates the rung's programs down into the
nearest rung that already has a canonical interpreter, and that interpreter runs
them.** Epsilon's meaning = `eps2gamma.beta` (Beta, Rust-free) elaborates epsilon →
Gamma; Gamma's canonical `interp.beta` runs it.

**Rationale.** Same semantic content as a bespoke interpreter (epsilon's
operational semantics written down in a lower rung), but *staged* as
translate-then-run, which is strictly **smaller**: it reuses Gamma's interpreter,
ADTs, recursion, and pattern matching instead of re-implementing an environment +
store + dispatch loop inside Gamma. The elaborator is a simple structural
desugaring (imperative → SSA-threaded functional), **not** an optimizing compiler,
so it does not violate "don't let a compiler be the definition." Two independent
checks keep it honest: (a) it is auditable Beta on the seed lineage; (b) the
**meaning diamond** cross-checks it against the independent native backend.

**Reconciliation with the overview.** The nesting named Delta as epsilon's
interpreter host; the correct substrate is **Gamma** (the general-purpose
interpreter rung). Delta is the *checker*, a different job. Epsilon and Omega both
elaborate to Gamma (or to the rung below them that is closest to Gamma), whose
interpreter is the canonical meaning. This refines the overview; it does not
contradict "meaning = reference interpreter."

## D3 — Trust flows through PROOFS, not through trusting native binaries. Native-code trust ends at translation validation.

The provable chain seed → Omega is **proof-carrying**: an ε/ω program ships a
δ-certificate; δ (Rust-free, double-implemented, hand-audited) checks it. A
backdoored *producer* can at worst emit output that **fails** the check — it cannot
forge a certificate δ accepts. So the soundness of a *proof about source* never
depends on the compiler.

For **native-code** trust (Cathedral running real binaries, where the compiler is
the isolation boundary), the endpoint is **translation validation**: the backend
emits, *per compilation*, a δ-checkable certificate that its machine code
**refines** the source's meaning. Then even a backdoored backend is caught — a
backdoored output would fail its own refinement check.

**Policy / staging:**
- **Now:** the convergence gates (an epsilon program emits a δ-cert about its own
  *result*) are the first instance of proof-carrying output. Keep them central.
- **Now:** native backends are checked by the **meaning diamond** (test-based
  agreement with the Rust-free meaning) and are explicitly **outside the soundness
  base** for proofs about source.
- **North star:** per-compile refinement certificates (the backend as a checked
  producer). This is where `omega-rs`'s "certs about real binaries vs a hardware
  model" ambition rejoins the lattice.

## D4 — The soundness bridge is built empirically via SEAMS now; every δ capability ships a paired seam.

`provable-in-Delta ⟹ true-about-execution` (overview honest-edge #1) is the hard
core. It is attacked **empirically today** by the soundness *seams* — kernel
derivation vs operational evaluation (induction, predicates, propositional logic,
the soundness sweep, the convergence routes).

**Standing policy:** every new logical capability δ gains **must** ship with a
paired seam that cross-checks kernel-provability against operational truth on a
corpus (+ a negative battery that must be rejected). A δ feature without its seam
is not done. This keeps the bridge honest as δ grows, and turns the "deep open
problem" into a continuously-tested invariant.

**Capstone (deferred):** a δ-checked proof of δ's own soundness w.r.t. the Alpha
small-step semantics — the formal bridge. Not attempted until the metatheory
tooling exists; the seams are the standing substitute.

## D5 — Diversity lives at the seed. The seed diamond is the Thompson root; the bc bootstrap is the next diversity gap to close.

Real Thompson resistance is the **diverse seed** (two independently-authored Alpha
implementations, x64 + arm64, byte-identical on the conformance suite). Everything
above inherits that resistance **only through Rust-free reproduction** — a diamond
between two paths that share a Rust ancestor catches implementation bugs, not
Thompson (overview honest-edge #2).

**The one real inheritance gap:** `bc`'s first bootstrap runs through
`beta-lang-rs` (Rust). Self-host reproduces `bc` but does not *diversify* it.

**Policy:**
- **Standing goal:** a second, Rust-free path to `bc` (diverse double compilation)
  — e.g. a Beta front-end derivable directly from the seed lineage, or a proof
  that `bc.beta`'s self-host fixpoint is reachable without the Rust on-ramp. Until
  then the audited on-ramp + fixpoint stands, **logged here as a known gap.**
- **Grow seed diversity:** more independent Alpha implementations / ISAs, authored
  as independently as possible (the overview's "diversity plan" open question). The
  count is driven by the threat model, not aesthetics; each new independent seed
  multiplies the cost of a Thompson attack.

## D6 — Epsilon and Omega are KEPT and justified; the epsilon native self-host is a producer, so its growth is de-prioritized.

- **Epsilon earns its rung**: it emits δ-checkable proofs (convergence) and its
  meaning is Rust-free (D2). Its value is not "another systems language" — it is
  "a systems language whose programs prove themselves to the anchor."
- **The self-hosted native backend (`lowermachine.alp`) is a PRODUCER** (D3), an
  accelerator whose output is diamond-checked. Therefore its *self-host growth* is
  **de-prioritized**: maintain the fixpoint, do not over-invest in extending it
  (min/max, nested calls, …) unless a concrete need arises. Effort goes to the
  meaning route (D1).
- **Omega = Epsilon + the proof surface** (contracts, refinement/dependent types,
  automation-as-untrusted-front-line). Same meaning-elaboration + convergence
  discipline. Self-hosting Omega-in-Omega is permitted only as an accelerator
  (zero trust added). `omega-rs` stays the untouched reference producer.

- **Rung count stays emergent** (overview). Epsilon/Omega are labels for "systems
  layer" and "proof-surface layer"; the DAG, not this doc, fixes the true count.

## D7 — Epsilon is ABSORBED into Omega: one machine-surface language, the "Omega kernel subset".

**Ratified 2026-07-02 (user-directed).** Epsilon was created as omega's on-ramp; the meaning-route work
proved the two surfaces are one language (the shared translator runs real Omega samples with only
surface deltas). Maintaining two named systems languages, two corpora, and two gate families for one
converging surface is redundancy without a trust payoff. Therefore:

- **The rung dissolves.** The ladder is α → β/bc → γ → δ → **ω**. What was "epsilon" is now the
  **Omega kernel subset** — the machine-surface fragment of Omega that the lattice can already give
  Rust-free meaning to. Programs, certifiers, and gates survive unchanged in substance.
- **The translator is Omega's.** `eps2gamma.beta` → `omega/omega2gamma.beta` — the Rust-free
  Omega→gamma elaborator (D2), covering the kernel subset and growing toward the full language.
- **The kept rung home merges**: `compiler/epsilon/` → `compiler/omega/` (the 42-case triple diamond
  becomes the *kernel diamond*; the Rust-free proof-carrying convergence gate moves as-is).
- **`epsilon-rs/` keeps its (historical) name and its role**: the disposable Rust producer for the
  kernel subset, with its native gates, certifier corpus, and self-hosted lowermachine — still
  outside the trust base, still killed eventually for self-sufficiency only (D6 unchanged).
- Rung docs: `rungs/epsilon.md` is marked absorbed; its "Adds" (safe systems programming) is now a
  stage of omega's roadmap rather than a separate rung.

---

## The chain, end to end

```
α  seed VM ...... two independent hand-written implementations (x64, arm64), diamond   [ROOT: diversity]
│                 hand-audited; own small-step semantics
β  assembler .... written in α-asm, run by α; self-hosts                                [derived from α]
   bc ........... Beta compiler in Beta; self-hosts (Rust on-ramp disposable)           [D5 diversity gap]
γ  interpreter .. interp.beta (+ typeck): the canonical MEANING substrate               [Rust-free]
δ  checker ...... check.beta AND checker.gamma, diamonded; the trust anchor             [Rust-free, audited]
                  every capability paired with a soundness seam                          [D4]
ε  systems ...... MEANING: eps2gamma.beta elaborates ε → γ, interp.beta runs it         [D2; Rust DYING]
                  PRODUCER: native backend, diamond-checked, emits δ-certs (convergence) [D3; producer]
ω  full lang .... ε + proof surface; meaning elaborates like ε; omega-rs = reference     [D3]
```

**What "provable" buys at the top:** a certificate δ accepts is trustworthy back to
the seed, because δ is (hand-audited) + (double-implemented in β and γ) + (paired
with soundness seams, D4) + (compiled by the Rust-free bc from a diverse seed, D5).
A false proposition cannot get a certificate past δ, whoever produced it (D3).

## Execution order (binds the /loop)

1. **Finish epsilon's Rust-free meaning route** — `eps2gamma.beta` slices: state
   machines → self fields → cross-machine calls → arrays → read_byte. *(D1 urgent
   kill; slices 0–1 done.)*
2. **Grow δ and its seams in lockstep** — no capability without its paired seam. *(D4)*
3. **Close the bc diversity gap** — a Rust-free second path to `bc`. *(D5)*
4. **Translation-validation backend** — per-compile refinement certs. *(D3 north star, later.)*
5. **Omega meaning route + dependent-type δ** — after epsilon's meaning is complete. *(D2/D6, later.)*
