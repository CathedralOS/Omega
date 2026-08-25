# Refinement → omega-bootstrap → production Omega

[Lattice overview](bootstrap_lattice.md) · [Decisions](decisions.md) · [Refinement pillar](../../../bootstrap/assurance/refinement/beta/REFINEMENT.md)

The instruction-level refinement pillar is structurally complete: for a substantial Beta fragment, the
compiled alpha machine code is **kernel-proven** to compute its source meaning, for all inputs, without
running it—two separately constructed symbolic derivations, each differentially pinned to its own reference,
with equivalence certificates cross-checked by the available checker implementations. This document takes the long
view the pillar earned: **which of its techniques carry first to the
Delta-built `omega-bootstrap` compiler and then across the hosted production
compile.**

The current Rust reference producer under
`bootstrap/onramps/omega-rust/{psi,omega}/` already sketches the destination in
`bootstrap/onramps/omega-rust/psi/semantics/psi-proof/src/`: `obligations.rs` (proof obligations
attached to compilation), `boundary.rs` (boundary obligations at capability
seams), `lemmas.rs`, and `checker.rs`. The lattice's job is to reach that shape
with *its own trust story* — obligations discharged by certificates the
independent low-rung proof kernel checks, not by trusting the Rust producer.

## What the refinement pillar proved transferable

| Technique (proved at α/bc) | Hosted-Omega analogue |
| --- | --- |
| **Two untrusted derivations, kernel-checked equal** — `alpha_symbolic` (what the code does) vs `beta_symbolic` (what the source means), checked by the low kernel | `omega_symbolic` (what elaborated Gamma does) vs an Omega-source meaning derivation; the same kernel-checked equality shape |
| **Meaning-language constructors instead of kernel surgery** — ℤ pairs `(k 5 ..)`, monus `(k 6 ..)`, stream `(k 7/8 ..)`: plain constructors to the kernel, semantics carried by pinned evaluators | Omega values (structs, cases, strings, capabilities) enter the meaning language the same way — constructor families + pinned evaluators, δ untouched |
| **Obstacles as loop variables** — the read position as a hidden loop var made streams compose with existing machinery | Omega's effect sequencing (I/O order, capability use) can ride the same trick: model the effect *cursor* as summarizable state |
| **Mod-observable congruence** — untruncated symbolic values sound because the observable is mod 256 and ops are ring homs | Omega's observable equivalences (e.g. encoding round-trips) justified by the same congruence pattern |
| **Artifact-bound claims with negative teeth** — reconstructed obligations, accepted derivations, and perturbed controls that must reject | non-negotiable at ω: every obligation class lands with canonical reconstruction and failure controls, not merely producer-selected certificates |
| **Refuse-loudly discipline** — every unsupported shape refuses; wrong summaries are impossible to certify (dual pins + refl) | the ω pipeline inherits this: an obligation the elaborator can't discharge is a loud FAIL, never a silent skip |

## What does NOT transfer (the genuinely new work)

1. **Branching on data.** The refinement fragment is concrete-control: symbolic branches refuse. Omega
   programs branch on data constantly (match arms, guards). The meaning language needs *conditional terms*
   (or per-arm obligations with reachability side-conditions) — the single biggest theory gap. The blocked
   buffer-writes satellite (`byte[base+i] = ..` with post-loop reads needing `addr − base < trip`
   decidability) is the same gap in miniature: **conditional/segment reasoning is the next mountain after
   which several stalled satellites unblock at once.**
2. **Compound values.** Peano nats + pairs + streams got us here; Omega's structs/cases/strings need a
   principled value encoding in the meaning language (likely constructor families again — but the *evaluator*
   work is substantial).
3. **Obligation generation.** `bootstrap/onramps/omega-rust/psi/semantics/psi-proof/` defines and
   checks compilation obligations. The lattice's analogue is
   the elaboration route (D2): `omega2gamma` should eventually *emit certificates alongside code* the way
   delta's convergence certifiers do — the proof-carrying-Omega target. Today's TV gate re-evaluates
   results; the climb is from "results checked" to "compilation obligations discharged."

## Capability progression (not a second work queue)

These are reusable assurance dependencies, not language rungs or an alternate
bootstrap schedule. The live order and concrete acceptance conditions remain in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md); a capability below becomes
active only when a named compiler-edge task needs it.

1. **Conditional terms** in the meaning language (unblocks: match-arm meanings, buffer segments, zz/monus
   trip guards). Kernel impact: likely a constructor family + evaluator branches; certs stay refl where
   both engines derive the same conditional normal form.
2. **Omega value families** in the meaning language, paired with `omega2gamma` (the Rust-free route) so the
   *gamma elaboration* is the thing symbolically executed — refinement at the γ level, reusing interp.beta
   as the pin. Binary numerals and structural constructor values demonstrate
   the pattern; exact live coverage belongs to `meaning-tv.sh` and its local
   refinement README.
3. **Obligation-emitting elaboration**: port delta's certify-* pattern up to omega2gamma, one obligation
   class at a time, each with canonical reconstruction, operational seams, and
   negative controls. Cross-checking available checker implementations remains a
   useful regression test. The current meaning gate covers multiple arithmetic,
   bounds, domain-erasure, boundary-range, and structural-result classes. Those
   exact inventories and resource envelopes are executable gate facts, not
   durable architecture claims.

Everything above obeys the standing decisions: D1 (Rust exits by role), D2
(meaning by elaboration to Gamma), D3 (trust via proofs + translation
validation), D5 (direct checked refinement), and D6 (Delta-built
`omega-bootstrap`, then the hosted production compiler). The bridge binary and
its product output may initially be conservatively lowered; the product still
implements the full optimizer and language. The same method validates that hosted result
against canonical meaning rather than trusting the bootstrap compiler's
pedigree.

## Universal-input refinement

The historical universal-input blocker is closed. The kernel calculus now has
`recx`, structurally decreasing recursion whose threaded extra argument may be
updated on the recursive call. The Beta checker, independent Gamma checker,
reference checker, certificate translation, and theorem generator implement the
form. `recx-soundness.sh`, `forall-input.sh`, and `forall-sample.sh` carry the
operational seam, universal fold theorems, real-sample connections, and matched
perturbation controls.

That capability is reusable assurance infrastructure; it is not another
language rung or an open bootstrap task by itself. New compiler-edge work should
name the exact source/artifact relation it still cannot establish in
`TASKS_BOOTSTRAP.md`, rather than resurrecting the completed climb here.
