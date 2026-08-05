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
| `check.beta` / `checker.gamma` (the proof kernel) | **trusted base** | **DEAD** — Beta + Gamma, double-implemented, diamonded. |
| `interp.beta` / `typeck.beta` (γ meaning) | **trusted base** | **DEAD** — Beta, on the seed lineage. |
| Delta's **meaning** (`gamma_emit.rs`) | **trusted base** | **DYING** — replaced by the Rust-free Delta-to-Gamma route, slice by slice. *This is the current urgent kill.* |
| Psi/Omega's **meaning** | **trusted base** | Follows the same elaboration discipline once Delta can host the toolchain. |
| `beta-lang-rs`, `delta-rs`, `omega-rs` (producers) | **untrusted producer** | **DEFERRABLE** — killed for self-sufficiency, not soundness. `omega-rs` stays untouched as the reference producer. |

**Policy:** no work removes Rust from a *producer* while Rust still sits in any
*meaning/checker*. Meaning-route Rust removal outranks producer Rust removal,
always. (This is why the Rust-free Delta-to-Gamma elaborator is prioritized over the
`lowermachine.alp` native-self-host — the former is a trusted-base kill, the
latter a producer nicety. See D6.)

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

The provable chain seed → Psi/Omega is **proof-carrying**: a producer ships a
certificate which the Rust-free, independently implemented proof kernel checks. A
backdoored *producer* can at worst emit output that **fails** the check — it cannot
forge a certificate the kernel accepts. So the soundness of a *proof about source* never
depends on the compiler.

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
problem" into a continuously-tested invariant.

**Capstone (deferred):** a kernel-checked proof of the kernel's own soundness w.r.t. the Alpha
small-step semantics — the formal bridge. Not attempted until the metatheory
tooling exists; the seams are the standing substitute.

## D5 — Diversity lives at the seed. The seed diamond is the Thompson root; the bc bootstrap is the next diversity gap to close.

Real Thompson resistance is the **diverse seed** (two independently-authored Alpha
implementations, x64 + arm64, byte-identical on the conformance suite). Everything
above inherits that resistance **only through Rust-free reproduction** — a diamond
between two paths that share a Rust ancestor catches implementation bugs, not
Thompson (overview honest-edge #2).

**The one real inheritance gap — NOW CLOSED (2026-07-02).** `bc`'s first bootstrap
runs through `beta-lang-rs` (Rust); self-host reproduces `bc` but does not
*diversify* it. Closed by a **second, independent Beta compiler**,
`compiler/beta-lang-py/bc2.py` — a from-scratch Beta→Alpha-asm compiler in Python,
written against the ISA + grammar, NOT ported from the Rust on-ramp. It is
UNTRUSTED and output-checked (like `elab.py`/`prover.py`/`tv-encode.py`): a bug or
Trojan in it makes the diverse-double-compilation check FAIL, never silently pass,
so Python is a verification instrument here, not runtime TCB (the lineage stays
α→β→bc). `diverse-double-compilation.sh` proves it: `bc2.py` compiles `bc.beta`
into `bcA`, and `bcA(bc.beta)` is **byte-identical** to the Rust-lineage
`bc0(bc.beta)` (8716 lines). The compilation of `bc` is therefore independent of
which bootstrap compiler produced it — a Trojan would have to sit, identically, in
*both* independent paths. Wired into `verify-lattice.sh`.

**Policy:**
- **Done:** the diverse second path to `bc` exists and is gated on every lattice
  run. Future hardening: a THIRD path in yet another language, and diversifying the
  *assembler* (β) the same way (its only source is α-asm today, so it is already
  seed-diverse, but a second Beta-asm→tape path would deepen it).
- **Grow seed diversity:** more independent Alpha implementations / ISAs, authored
  as independently as possible (the overview's "diversity plan" open question). The
  count is driven by the threat model, not aesthetics; each new independent seed
  multiplies the cost of a Thompson attack.

## D6 — The bootstrap spine ends in Delta; the proof kernel is orthogonal.

**Ratified 2026-08-04.** Greek names identify languages in the bootstrap spine:

```text
Alpha → Beta → Gamma → Delta
```

Delta is the former systems/compiler-host on-ramp. Its job is to host the real
Psi/Omega toolchain from the audited seed. `compiler/delta-rs/` is its disposable
Rust implementation; the self-hosted `lowermachine.alp` and meaning diamond remain
its principal gates.

The certificate checker is renamed the **proof kernel** and lives at
`compiler/proof-kernel/`. It remains a trusted, independently implemented
assurance service in Beta and Gamma. The rename changes neither its authority nor
its validation gates; it removes the false claim that proof checking is a
language stage between Gamma and Delta.

Psi and Omega are compiler products hosted by Delta, not Greek bootstrap rungs.
The Rust implementations remain current producers while the hosted path matures.

---

## The chain, end to end

```
α  seed VM ...... two independent hand-written implementations (x64, arm64), diamond   [ROOT: diversity]
│                 hand-audited; own small-step semantics
β  assembler .... written in α-asm, run by α; self-hosts                                [derived from α]
   bc ........... Beta compiler in Beta; self-hosts; + bc2.py diverse 2nd path (DDC)    [D5 gap CLOSED]
γ  interpreter .. interp.beta (+ typeck): the canonical MEANING substrate               [Rust-free]
δ  systems ...... compiler-host language; meaning elaborates δ → γ                      [D2; Rust removal active]
                  native producer is diamond-checked; self-hosting path is gated          [D3; producer]

proof kernel .... check.beta AND checker.gamma, diamonded; cross-cutting trust anchor    [Rust-free, audited]
Psi/Omega ....... product compiler pipeline hosted by Delta; Rust implementation current [producer]
```

**What "provable" buys at the top:** a certificate the proof kernel accepts is
trustworthy back to the seed because the kernel is hand-audited,
double-implemented in Beta and Gamma, paired with soundness seams, and compiled by
the Rust-free `bc` from a diverse seed. A false proposition cannot get a
certificate past the kernel merely by controlling the producer.

## Execution order (binds the /loop)

1. **Finish Delta's Rust-free meaning route** — the Delta-to-Gamma slices: state
   machines → self fields → cross-machine calls → arrays → read_byte. *(D1 urgent
   kill; slices 0–1 done.)*
2. **Grow the proof kernel and its seams in lockstep** — no capability without its paired seam. *(D4)*
3. ~~Close the bc diversity gap~~ **DONE** — `bc2.py`, an independent second Beta compiler; DDC of `bc` gated. *(D5)*
4. **Translation-validation backend** — per-compile refinement certs. *(D3 north star, later.)*
5. **Host the Psi/Omega toolchain in Delta** — after Delta's meaning route is complete. *(D2/D6, later.)*
