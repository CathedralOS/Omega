# Lattice — ratified decisions

[Lattice overview](bootstrap_lattice.md)

The overview decides the *principles* (trust-by-checking; meaning follows each
rung's canonical written or evaluator route; checked refinement crosses every
compiler edge; Rust exits every required trust and bootstrap role) and
deliberately leaves a set of
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

| Rust-dependent role | Architectural role | Status / plan |
| --- | --- | --- |
| proof checking, now supplied by `check.beta` / `checker.gamma` | **trusted base** | **RUST CLOSED** — the surviving Beta and Gamma implementations are on the seed lineage and cross-checked against shared seams. The proof kernel itself is live cross-cutting assurance. |
| cold-started `bc` artifact vs `bc.beta` | **trusted compilation edge** | **CLOSED for `B_bc1`** — an independently reconstructed ROOT proposition binds the exact persisted tape to the complete maximal source observable, including exhaustion and divergence. |
| Gamma meaning, now supplied by `interp.beta` / `typeck.beta` | **trusted base** | **RUST CLOSED** — the surviving meaning implementation is Beta on the seed lineage. |
| Delta's **meaning** (`gamma_emit.rs`) | **trusted base** | **MIGRATING** — the Beta-written `omega2gamma` route and Gamma execution cover the admitted D0/O1 compiler canaries. `gamma_emit.rs` is a differential reference there; each bridge capability added toward `Ωself` must bring lower-rung meaning with it. |
| Psi/Omega meaning and artifact-aware verification | **trusted base** | **OPEN** — the current Rust interpreter/verifier remains an explicit migration dependency until the lower-rung semantic-ledger and hosted meaning route close. |
| `beta-rust`, `delta-rust`, `bootstrap/onramps/omega-rust/` | **untrusted producers** | **DEFERRABLE** — replaced as required dependencies for self-sufficiency, not for soundness pedigree. The current Rust Omega compiler remains a maintained reference producer while useful. |

**Policy:** no work removes Rust from a *producer* merely for pedigree while Rust
still sits in any meaning/checker or while an upstream artifact that builds those
checkers lacks lower-rooted refinement. Checking the cold-started `bc` output is
therefore trust closure, not a producer-language cleanup. Meaning-route Rust
removal otherwise outranks producer replacement. (This is why the Rust-free
Delta-to-Gamma elaborator outranks native-producer optimization. See D6.)

## D2 — Delta meaning is realized by nonoptimizing elaboration to Gamma.

Alpha and Beta retain their own written small-step semantics. Gamma's canonical
evaluator is `interp.beta`. Delta therefore uses a Rust-free, nonoptimizing
elaboration into Gamma rather than adding a fresh native interpreter. Gamma's
canonical interpreter runs the result.

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
service and is not part of the language path. This does not replace the written
Alpha or Beta meanings with executable reference tools.

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

Kernel acceptance is authoritative for one exact subject-qualified judgment.
The terminal artifact claim is canonical operational refinement under a
verifier-reconstructed observation profile. Intended-model mathematics and
global theory consequence are supporting subjects only; each reaches that root
through an explicit identity-bearing checked bridge. `satisfies`, `embed`/`as`,
and `terminates by` mark common bridge applications but prove nothing by their
spelling. Subject, model/theory and semantics versions, observation profile,
target capsule, bridge graph, and admissions enter certificate identity.

The observation profile is reconstructed from canonical semantics and the
consumer-selected deployment policy. The producer may neither select nor
weaken it. Exact identity is the first conservative replay gate; reuse across
profiles eventually requires a checked canonical forgetting projection because
profiles can be incomparable. Verification reports always name the profile,
semantics versions, and admissions rather than presenting a profile-free
`verified` verdict.

Connecting each accepted judgment through those bridges to execution is the
hard core. It is attacked **empirically today** by the soundness *seams* —
kernel derivation vs operational evaluation (induction, predicates,
propositional logic, the soundness sweep, the convergence routes).

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
source + produced artifact + consumer deployment policy
          ↓ canonical obligation reconstruction
artifact refines canonical source operational meaning
under verifier-derived observation profile O
          ↓ lower-rooted proof/semantic checking
accept or reject
```

A Thompson payload changes the artifact or its behavior and therefore fails this
check. A second compiler adds no soundness once that edge is closed. Exact
output agreement is also the wrong long-term contract: two correct compilers may emit different
artifacts, while two incorrect compilers may agree. Requiring byte identity
between implementations unnecessarily creates a second compiler to maintain and
conflates reproducibility with correctness.

Applied across the full Alpha-to-production-Omega chain, this directly answers
the source-correspondence question DDC would otherwise be introduced to ask,
and answers it independently at every edge. DDC therefore has no residual
bootstrap or release role. A maintained Rust Omega compiler can still provide
useful differential tests, but that is ordinary bug finding rather than a
second provenance construction.

The current `bc` cold start no longer passes through
`bootstrap/onramps/beta-rust/`. An Alpha-written compiler accepts the exact
pinned `bc.beta` surface,
reconstructs the persisted fixed-point tape, and runs the complete Beta corpus.
That closes the external-producer dependency and establishes reproducible
lineage. The separate lower-rooted refinement gate now establishes source
correspondence: its independently reconstructed ROOT proposition proves exact
maximal-observation equality for the persisted artifact over every finite source
and supported `B_bc1` resource profile. This does not turn fixed-point identity
or corpus agreement into correctness evidence; they remain separate regression
and dependency-closure checks.

The dedicated `compiler/beta-lang-py` comparison gate and `bc2.py` backend have
been removed because they supplied no unique semantic or refinement coverage. Shared
source recognition and executable meaning now live under
`bootstrap/rungs/beta/reference/`; symbolic reconstruction lives under
`bootstrap/assurance/refinement/beta/`. The forwarding facade under
`compiler/beta-lang-py/` has been retired.

Independent Alpha realizations and independent proof-kernel implementations are
conformance and soundness cross-checks against explicit
semantics, useful for finding implementation mistakes while the corresponding
formal bridges mature. Their multiplicity supplies evidence; it is not the rule
that grants an artifact authority.

**Policy:**

- Do not add second or third compilers merely for implementation diversity.
- Do not create a DDC work lane or acceptance gate; direct checked refinement
  is the compiler-provenance obligation.
- Do not make cross-implementation byte identity a release or trust requirement.
- Require deterministic reproduction where it serves build identity and audit,
  but never present a fixed point as correctness evidence.
- Close every compiler edge with lower-rooted source-to-artifact refinement.
- Keep differential/reference implementations only where their bug-finding value
  justifies their maintenance cost.
- In particular, retain the current Rust Psi/Omega compiler as a maintained
  parallel reference while useful, but never make its agreement, availability,
  or output a bootstrap authority or release dependency.

## D6 — Delta builds a profile-limited bridge; that bridge builds the full-spec product compiler. The proof kernel is orthogonal.

**Ratified 2026-08-04; superseded and clarified 2026-08-23.** The small
bootstrap languages form the audited spine:

```text
Alpha → Beta → Gamma → Delta
```

Delta is the systems/compiler-host rung and an independent language. It should
be robust, C-like in systems power, and Omega-shaped where consistency is cheap;
it is not required to be a syntactic or semantic Omega subset.

Delta v1 and `Ωself` are separate and asymmetric contracts. Delta v1 is a
literal independent language selected from the complete `omega-bootstrap`
source closure plus explicit compiler-host coherence, robustness, safety, and
maintainability arguments. `Ωself` is an incidental subset of ordinary Omega
selected from the production compiler's own complete source closure and the
measured retain/refactor tradeoff. Neither manifest substitutes for a general
language/profile definition.

These are the only two feature inventories being selected. The source used to
write `omega-bootstrap` is governed by Delta v1; the Omega source it accepts is
governed by `Ωself`. The resulting product compiler's user-facing feature set is
not a third choice: it implements the already-authoritative full Omega
specification. Generated-code quality is likewise not a language inventory.
The frozen D0/O0/O1 envelopes are regression contracts for existing vertical
slices, not additional inventories or numbered ancestors of these two
contracts.

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
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
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
resulting compiler contains the full production optimizer and advanced lowering
even if its own binary is not yet optimized. This Delta-compiler → Omega-source edge is not, strictly, an
Omega self-rebuild. A later production `omega` → `omega` rebuild may optimize
that binary and provide fixed-point/reproducibility evidence; it is product
work, not a second bootstrap task or architectural dependency. As
with every hosted edge, a defect can reproduce; proof, meaning, and
translation-validation gates remain responsible for detecting it.

`bootstrap/onramps/delta-rust/` is Delta's disposable Rust implementation;
`bootstrap/rungs/delta/` owns the language corpus, the self-hosted
`lowermachine.alp`, and lattice-built artifacts. The former `compiler/delta-rs`
and `compiler/delta` compatibility entries are retired. The self-host and
meaning diamond remain principal gates.

The certificate checker is renamed the **proof kernel**. Its canonical owner is
`bootstrap/assurance/proof-kernel/`. It remains a trusted assurance service with
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

The two contracts settle at one explicit join. Product checkpoints yield a
provisional `Ωself` while the general Delta-written bridge supplies the
implementation and assurance costs used to settle its retained features. At
the completed join, `Ωself` freezes from the final product closure and those
measured costs, while Delta v1 freezes from the complete bridge closure plus
explicit compiler-host coherence, safety, and maintainability arguments. The
publications remain separately scoped and versioned; neither is a language rung
that must be frozen before the other. This co-evolution is a design discovery
loop, not a runtime or build cycle.

Terminal Psi follows ordinary source-closure rules. Representation and lowering
modules linked into the product compiler belong to its manifest; standalone
interpreters, verifiers, viewers, and debugging tools do not unless imported.
The bridge may lower directly and is not required to use Terminal Psi as its own
internal IR merely because it compiles product modules that implement it.

---

## The chain, end to end

```
α  seed VM ...... small written semantics + audited x64/arm64 realizations               [ROOT: execution]
│                 hand-audited; own small-step semantics
α  assembler .... written in α-asm, run by α; self-hosts                                [derived from α]
β  bc ........... Beta compiler in Beta; self-hosts; B_bc1 whole-artifact refinement closed [D5]
γ  interpreter .. interp.beta (+ typeck): the canonical MEANING substrate               [Rust-free]
δ  systems ...... independent compiler-host language; meaning elaborates δ → γ           [D2; Rust removal active]
                  builds omega-bootstrap from Delta source                                [bootstrap producer]

proof kernel .... check.beta + checker.gamma; cross-cutting derivation checker           [Rust-free, audited]
omega-bootstrap . accepts exact Ωself and rejects the rest; may itself run slowly         [hosted bridge]
omega ........... full-spec compiler with optimizer; own binary may be conservative  [one hosted production build]
Psi ............. source semantics + terminal IR inside both compiler products            [not a rung]
```

**What "provable" buys at the top:** in the completed architecture, a certificate
the proof kernel accepts is trustworthy back to the seed because the kernel and
its soundness bridges are audited and every compiler artifact in that path has a
lower-rooted refinement check. A false proposition cannot get a certificate past
the kernel merely by controlling the producer. The whole-artifact `bc`
cold-start refinement now closes that edge; its fixed point remains dependency-
closure and reproducibility evidence rather than the reason the artifact is
authoritative.

## Dependency order

This page fixes architectural dependencies; it is not a second work queue.
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md) is the sole live bootstrap
execution order.

1. Product work publishes deterministic checkpoints of the Omega-written
   compiler source while bootstrap work derives and enforces provisional
   `Ωself` rules from them.
2. The Delta-written `omega-bootstrap` grows against those checkpoints. Every
   admitted capability lands with lower-rung meaning, direct artifact
   refinement, resource behavior, and a negative boundary; reference-producer
   agreement remains diagnostic evidence.
3. The complete product source and complete general bridge form one settlement
   join. It publishes `Ωself` from the product closure plus measured bridge
   cost, and Delta v1 from the bridge closure plus explicit language-coherence,
   robustness, safety, and maintainability arguments.
4. The lattice-built Delta compiler builds the exact validated
   `omega-bootstrap` artifact.
5. That bridge performs the one required hosted build of the full-spec
   production compiler, including its optimizer and advanced lowering. An
   Omega-to-Omega rebuild of the same source is optional.

The proof kernel, meaning routes, semantic seams, and translation-validation
machinery are cross-cutting obligations on the relevant edges, not additional
language stages inserted into this sequence. Closed lower edges reopen only for
a concrete defect, a changed artifact, or a widened claim.
