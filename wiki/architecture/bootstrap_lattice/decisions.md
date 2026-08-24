# Lattice — ratified decisions

[Lattice overview](bootstrap_lattice.md)

The overview decides the *principles* (trust-by-checking; meaning = reference
interpreter; checked refinement across every compiler edge; Rust exits every
required trust and bootstrap role) and deliberately leaves a set of
**open questions**. This document resolves them into standing decisions and an
execution order. It is the executive layer on top of the design: when the
overview says "emergent / to be decided," the calls here are the decision.

Format: each decision is **D#**, states the call, the rationale, and the resulting
policy. Decisions bind the construction; they do not touch language *meaning*
(owned by the language guide) nor prescribe the internals of the current product
implementation under `bootstrap/onramps/omega-rust/psi/` and `bootstrap/onramps/omega-rust/omega/`.

---

## D1 — Rust exits by ROLE, not by rung: meaning/checker first, required producer last.

The overview's "Two roles for Rust" is the ordering law. Made concrete, per artifact:

| Where Rust sits | Role | Status / plan |
| --- | --- | --- |
| `check.beta` / `checker.gamma` (the proof kernel) | **trusted base** | **DEAD** — Beta + Gamma implementations, cross-checked against shared seams. |
| cold-started `bc` artifact vs `bc.beta` | **trusted compilation edge** | **OPEN** — the Alpha-rooted fixed point and persisted artifact exist; complete lower-rooted source-to-artifact refinement does not. |
| `interp.beta` / `typeck.beta` (γ meaning) | **trusted base** | **DEAD** — Beta, on the seed lineage. |
| Delta's **meaning** (`gamma_emit.rs`) | **trusted base** | **MIGRATING** — the Beta-written `omega2gamma` route and Gamma execution cover the admitted D0/O1 compiler canaries. `gamma_emit.rs` is a differential reference there; each bridge capability added toward `Ωself` must bring lower-rung meaning with it. |
| Psi/Omega's **meaning** | **trusted base** | Follows the same elaboration discipline through `omega-bootstrap` and the hosted production compile. |
| `beta-rust`, `delta-rust`, `bootstrap/onramps/omega-rust/` (producers) | **untrusted producer** | **DEFERRABLE** — replaced for self-sufficiency, not soundness. The current Rust compiler remains the executable reference producer during migration. |

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
  producer). This is where the Rust on-ramp backend's "certs about real binaries
  vs a hardware model" ambition rejoins the lattice.

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

## D5 — Direct checked refinement closes compiler provenance.

**Supersedes the 2026-07-02 D5 ruling; ratified 2026-08-22.** Diverse double
compilation (DDC) is not a trust requirement of this architecture. It asks whether
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
check. A second compiler adds no soundness once that edge is closed. Exact
output agreement is also the wrong long-term contract: two correct compilers may emit different
artifacts, while two incorrect compilers may agree. Requiring byte identity
between implementations unnecessarily creates a second compiler to maintain and
conflates reproducibility with correctness.

The current `bc` cold start no longer passes through
`bootstrap/onramps/beta-rust/` (`compiler/beta-lang-rs` is a compatibility
path). An Alpha-written compiler accepts the exact pinned `bc.beta` surface,
reconstructs the persisted fixed-point tape, and runs the complete Beta corpus.
That closes the external-producer dependency and establishes reproducible
lineage; it does not by itself establish source correspondence. The latter is an
**unfinished lower-rooted refinement edge**.
Close it by validating the complete `bc` artifact against `bc.beta` with
authority rooted below `bc`.

The dedicated `compiler/beta-lang-py` comparison gate and `bc2.py` backend have
been removed because they supplied no unique semantic or refinement coverage. Shared
source recognition and executable meaning now live under
`bootstrap/rungs/beta/reference/`; symbolic reconstruction lives under
`bootstrap/assurance/refinement/beta/`. `compiler/beta-lang-py/` is compatibility
plumbing only.

Independent Alpha realizations and independent proof-kernel implementations are
conformance and soundness cross-checks against explicit
semantics, useful for finding implementation mistakes while the corresponding
formal bridges mature. Their multiplicity supplies evidence; it is not the rule
that grants an artifact authority.

**Policy:**

- Do not add second or third compilers merely for implementation diversity.
- Do not make cross-implementation byte identity a release or trust requirement.
- Require deterministic reproduction where it serves build identity and audit,
  but never present a fixed point as correctness evidence.
- Close every compiler edge with lower-rooted source-to-artifact refinement.
- Keep differential/reference implementations only where their bug-finding value
  justifies their maintenance cost.
- In particular, retain the current Rust Psi/Omega compiler as a maintained
  parallel reference while useful, but never make its agreement, availability,
  or output a bootstrap authority or release dependency.

## D6 — Delta builds a profile-limited Omega compiler; that compiler builds the full product. The proof kernel is orthogonal.

**Ratified 2026-08-04; superseded and clarified 2026-08-23.** The small
bootstrap languages form the audited spine:

```text
Alpha → Beta → Gamma → Delta
```

Delta is the systems/compiler-host rung and an independent language. It should
be robust, C-like in systems power, and Omega-shaped where consistency is cheap;
it is not required to be a syntactic or semantic Omega subset.

Delta v1 and `Ωself` are separate and asymmetric contracts. Delta v1 is a
literal independent language discovered from the complete `omega-bootstrap`
source closure. `Ωself` is an incidental subset of ordinary Omega discovered
from the production compiler's own complete source closure. Neither manifest
substitutes for a general language/profile definition.

Delta v1 is not frozen in advance from the current Rust producer or D0 corpus.
The fixed design constraints are deterministic specified behavior, no undefined
behavior or ambient host authority, specified failure, lower-rung meaning for
every admitted construct, and Omega spelling and ordinary meaning whenever
Delta retains the same construct. The exact scalar, arithmetic, aggregate,
control, allocation, and boundary inventory remains provisional until the
bridge source and explicit compiler-host design arguments justify it. A
facility's presence in D0 or acceptance by the Rust producer does not admit it
to v1.

The selection rule is whole-bootstrap cost, not literal feature count. A
facility may be retained because it makes the bridge materially smaller, safer,
more regular, more maintainable, or easier to assure than its replacement.
Thus Delta may use only Exact integer arithmetic if that suffices, or one narrow
modular operation if artifact encoding alone requires it; it need not retain
general arithmetic domains. Conversely, a small companion operation may remain
even if the current bridge could contort around it, when omission would make the
literal language brittle or irregular. A one-purpose compiler-host interface
should remain a sealed byte-input, artifact-output, diagnostic-output, and
process-termination surface unless a concrete compiler-host argument requires
more. General boundary traits, filesystem access, and other host extensibility
are not presumed Delta facilities.
The provisional evidence ledger is
[`bootstrap/rungs/delta/FEATURE_LEDGER.md`](../../../bootstrap/rungs/delta/FEATURE_LEDGER.md).

Delta source builds `omega-bootstrap`. That compiler is intentionally
input-incomplete: it accepts only the Omega product-compiler source profile
`Ωself`, rejects everything else, and gives accepted programs their exact
normal Omega meaning.
The production compiler source and all of its transitive dependencies are
deliberately constrained to `Ωself`, while the resulting production compiler
implements the complete Omega specification:

```text
Alpha → Beta → Gamma → Delta
                           ↓
              omega-bootstrap (Delta-built, accepts Ωself)
                           ↓
              omega (full optimizing compiler; own binary may be conservative)
```

`Ωself` is a source profile, not an Epsilon language or another rung. It inherits
Omega semantics and has no private syntax. The bootstrap compiler is not a
general Omega endpoint merely because it compiles the production source. A
compiler can implement proofs, dependent types, and the rest of full Omega
without using those features in its own implementation.

“Full product compiler” means full Omega source acceptance and artifact meaning,
not the inclusion of every compiler-adjacent executable in the hosted source
closure. Terminal-Psi interpreters, REPLs, proof explorers, viewers, and similar
tools are optional unless the compiler executable imports them.

The profile is a compositional feature-and-resource subset, not an allowlist of
particular source files or hard-coded compiler AST permutations. Its exact source
manifest proves that the product compiler closes under the profile; the manifest
does not replace general parsing, checking, lowering, diagnostics, or negative
coverage for every admitted capability. Thus `omega-bootstrap` is deliberately
incomplete in what Omega programs it accepts, but exact in the meaning of every
program it does accept. The production compiler it builds accepts full Omega
and preserves the full specified artifact meaning.

There is one required hosted production build. `omega-bootstrap` may be slow
and may lower the production compiler conservatively. It must compile the
`Ωself` source that implements the production optimizer and advanced lowering,
but it does not implement or run those product passes during this build. The
resulting compiler has full optimizing functionality even if its own binary is
not yet optimized. This Delta-compiler → Omega-source edge is not, strictly, an
Omega self-rebuild. A later production `omega` → `omega` rebuild may optimize
that binary and provide fixed-point/reproducibility evidence; it is product
work, not a second bootstrap task or architectural dependency. As
with every hosted edge, a defect can reproduce; proof, meaning, and
translation-validation gates remain responsible for detecting it.

`bootstrap/onramps/delta-rust/` is Delta's disposable Rust implementation;
`bootstrap/rungs/delta/` owns the language corpus, the self-hosted
`lowermachine.alp`, and lattice-built artifacts. `compiler/delta-rs` and
`compiler/delta` are compatibility paths. The self-host and meaning diamond
remain principal gates.

The certificate checker is renamed the **proof kernel**. Its canonical owner is
`bootstrap/assurance/proof-kernel/`; `compiler/proof-kernel` is a compatibility
path. It remains a trusted assurance service with
Beta and Gamma implementations. The rename changes neither its authority nor its
validation gates; it removes the false claim that proof checking is a language
stage between Gamma and Delta.

Psi remains the source-semantics and terminal-portable-IR owner inside the Omega
product toolchain. `omega-bootstrap` hosts only the Psi/Omega path required by
the exact `Ωself` source closure. Omega proof syntax may be absent from `Ωself`
even though ordinary `Ωself` code implementing full proof-feature parsing and
checking belongs to the production compiler closure. Standalone Terminal Psi
tools, interpreters, and other product breadth are outside that closure unless
the production compiler imports them. Neither Psi nor either compiler artifact
is another Greek bootstrap language.

The current Rust Psi/Omega compiler remains a maintained reference and
differential producer while its bug-finding value justifies the cost. It grants
no authority; once the hosted path closes, it is neither a bootstrap nor a
release dependency. The Beta compiler's default construction and downstream
use are already Alpha-rooted.

The exact Delta language inventory and `Ωself` source-profile inventory are
governed by [`compiler_source_profile.md`](compiler_source_profile.md).

---

## The chain, end to end

```
α  seed VM ...... small written semantics + audited x64/arm64 realizations               [ROOT: execution]
│                 hand-audited; own small-step semantics
α  assembler .... written in α-asm, run by α; self-hosts                                [derived from α]
β  bc ........... Beta compiler in Beta; self-hosts; whole-artifact refinement open      [D5 work]
γ  interpreter .. interp.beta (+ typeck): the canonical MEANING substrate               [Rust-free]
δ  systems ...... independent compiler-host language; meaning elaborates δ → γ           [D2; Rust removal active]
                  builds omega-bootstrap from Delta source                                [bootstrap producer]

proof kernel .... check.beta + checker.gamma; cross-cutting derivation checker           [Rust-free, audited]
omega-bootstrap . accepts exact Ωself and rejects the rest; may itself run slowly         [hosted bridge]
omega ........... full optimizing compiler; own binary may be conservative          [one hosted production build]
Psi ............. source semantics + terminal IR inside both compiler products            [not a rung]
```

**What "provable" buys at the top:** in the completed architecture, a certificate
the proof kernel accepts is trustworthy back to the seed because the kernel and
its soundness bridges are audited and every compiler artifact in that path has a
lower-rooted refinement check. A false proposition cannot get a certificate past
the kernel merely by controlling the producer. Today the whole-artifact `bc`
cold-start refinement is still an explicit open edge; the fixed point does not
close it.

## Execution order (binds the /loop)

1. **Close the `bc` correspondence edge by checked refinement** — the
   Alpha-rooted construction and fixed-point artifact are complete; now validate
   that exact artifact against `bc.beta` with authority rooted below `bc`. The
   Python comparison path and byte-identical self-build are not the closure
   criterion. *(D3/D5)*
2. **Keep Delta's used profile Rust-free** — retain the existing
   `omega2gamma.beta` → `interp.beta` coverage for D0/O1, and require each newly
   admitted compiler construct to land with lower-rung meaning or fail closed.
   `gamma_emit.rs` remains a differential reference producer. *(D1.)*
3. **Grow the proof kernel and its seams in lockstep** — no capability without its paired seam. *(D4)*
4. **Translation-validation backend** — per-compile refinement certs. *(D3 north star, later.)*
5. **Establish the production Omega source tree and `Ωself`** — publish its
   deterministic dependency closure, derive the lowest-total-cost compositional
   source profile, and mechanically reject excluded features. *(D6.)*
6. **Complete `omega-bootstrap` and freeze Delta v1 around its source closure** —
   implement the exact `Ωself` frontend/semantic path and correct conservative
   lowering needed for the one hosted production build, prune accidental Delta
   experiments, then freeze the coherent retained language. Compile, rather than
   duplicate, the product optimizer and advanced lowering source. Do not require
   unrelated full-Omega source or tool surfaces. *(D2/D6, later.)*
7. **Build production Omega once** — compile the `Ωself`-constrained Omega
   source into the full optimizing compiler, then apply the normal meaning and
   translation-validation gates. Its own binary may be conservative; a further
   self-rebuild that optimizes it is optional. *(D3/D6, later.)*
