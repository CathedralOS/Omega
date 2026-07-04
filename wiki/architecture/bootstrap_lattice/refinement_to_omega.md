# Refinement → Omega: the ω-climb roadmap

[Lattice overview](bootstrap_lattice.md) · [Decisions](decisions.md) · [Refinement pillar](../../../compiler/alpha/REFINEMENT.md)

The instruction-level refinement pillar is structurally complete: for a substantial Beta fragment, the
compiled alpha machine code is **kernel-proven** to compute its source meaning, for all inputs, without
running it — two independent symbolic derivations, each differentially pinned to its own reference,
equivalence certificates decided identically by three independent checkers. This document takes the long
view the pillar earned: **which of its techniques carry to the ω rung, and what the climb toward
`omega-rs`'s proof pipeline actually consists of.**

`omega-rs` (untouched reference producer, per D1) already sketches the destination in
`semantics/omega-proof`: `obligations.rs` (proof obligations attached to compilation), `boundary.rs`
(boundary obligations at capability seams), `lemmas.rs`, `checker.rs`. The lattice's job is to reach that
shape with *its own trust story* — obligations discharged by certificates the δ anchor checks, not by a
244k-line Rust codebase we take on faith.

## What the refinement pillar proved transferable

| Technique (proved at α/bc) | ω-rung analogue |
| --- | --- |
| **Two untrusted derivations, kernel-checked equal** — `alpha_symbolic` (what the code does) vs `beta_symbolic` (what the source means), refl at δ | `omega_symbolic` (what elaborated gamma does) vs an Omega-source meaning derivation; the same refl-at-δ shape |
| **Meaning-language constructors instead of kernel surgery** — ℤ pairs `(k 5 ..)`, monus `(k 6 ..)`, stream `(k 7/8 ..)`: plain constructors to the kernel, semantics carried by pinned evaluators | Omega values (structs, cases, strings, capabilities) enter the meaning language the same way — constructor families + pinned evaluators, δ untouched |
| **Obstacles as loop variables** — the read position as a hidden loop var made streams compose with existing machinery | Omega's effect sequencing (I/O order, capability use) can ride the same trick: model the effect *cursor* as summarizable state |
| **Mod-observable congruence** — untruncated symbolic values sound because the observable is mod 256 and ops are ring homs | Omega's observable equivalences (e.g. encoding round-trips) justified by the same congruence pattern |
| **Diversity at every cert class** — 3-checker diamonds incl. perturbed-teeth rejects | non-negotiable at ω: every new obligation class lands with its diamond on day one, not as a follow-up |
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
3. **Obligation generation.** `omega-rs` attaches obligations during compilation. The lattice's analogue is
   the elaboration route (D2): `omega2gamma` should eventually *emit certificates alongside code* the way
   epsilon's convergence certifiers do — the proof-carrying-Omega target. Today's TV gate re-evaluates
   results; the climb is from "results checked" to "compilation obligations discharged."

## Execution order (long view)

1. **Conditional terms** in the meaning language (unblocks: match-arm meanings, buffer segments, zz/monus
   trip guards). Kernel impact: likely a constructor family + evaluator branches; certs stay refl where
   both engines derive the same conditional normal form.
2. **Omega value families** in the meaning language, paired with `omega2gamma` (the Rust-free route) so the
   *gamma elaboration* is the thing symbolically executed — refinement at the γ level, reusing interp.beta
   as the pin.
3. **Obligation-emitting elaboration**: port epsilon's certify-* pattern up to omega2gamma, one obligation
   class at a time, each with its three-checker diamond. *Status: two classes live in `meaning-tv.sh` —
   division safety (iszero(divisor) = 0 at every / and % site) and array bounds (ult(idx, len) = 1 at every
   user-level nth/setl access, difference-pair form in ℤ mode; omega2gamma's Cons-spine array lowering
   returns silent defaults on overrun, the exact silent-OOB shape the obligation forbids). Structural
   results also covered: constructor-tree values get a leaf-computed vs literal-tree claim plus a render
   pin against the interpreter's printed value. Heavy arithmetic rides DIVISION-BY-WITNESS: the encoder
   witnesses quotients/products/differences and the kernel checks the defining property across in-envelope
   certificates (value-pin certs re-compute each operand expression; chunked literal addition certs verify
   the op) — certifying computation, replacing the old quotient wall with the measured reduction envelope
   of the 64 MiB alpha image.*

Everything above obeys the standing decisions: D1 (Rust exits by role), D2 (meaning by elaboration to
gamma), D3 (trust via proofs + TV), D5 (diversity at every seam). The refinement pillar is the proof that
the method scales; the ω climb is the same method against a bigger language.
