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

| Technique (proved at α/bc) | ω-rung analogue |
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

## Execution order (long view)

1. **Conditional terms** in the meaning language (unblocks: match-arm meanings, buffer segments, zz/monus
   trip guards). Kernel impact: likely a constructor family + evaluator branches; certs stay refl where
   both engines derive the same conditional normal form.
2. **Omega value families** in the meaning language, paired with `omega2gamma` (the Rust-free route) so the
   *gamma elaboration* is the thing symbolically executed — refinement at the γ level, reusing interp.beta
   as the pin. *First summit camp reached: BINARY NUMERALS — a second value representation (bit-spine
   constructors + carry-passing badd / shift-and-add bmul user funs), engaged per sample when unary
   magnitudes overflow, O(bits) kernel reductions. `meaning-tv.sh` gates the
   current Omega meaning corpus.*
3. **Obligation-emitting elaboration**: port delta's certify-* pattern up to omega2gamma, one obligation
   class at a time, each with canonical reconstruction, operational seams, and
   negative controls. Cross-checking available checker implementations remains a
   useful regression test. *Status: four classes live in `meaning-tv.sh` —
   division safety, array bounds, arithmetic witnesses (pins + chunked literal certificates), and DOMAIN
   ERASURE (the translator drops `in Saturating`/`Wrapping` annotations; every subtraction site carries a
   kernel-checked no-underflow witness proving the erasure changed nothing). Earlier status detail: —
   division safety (iszero(divisor) = 0 at every / and % site) and array bounds (ult(idx, len) = 1 at every
   user-level nth/setl access, difference-pair form in ℤ mode; omega2gamma's Cons-spine array lowering
   returns silent defaults on overrun, the exact silent-OOB shape the obligation forbids). Structural
   results also covered: constructor-tree values get a leaf-computed vs literal-tree claim plus a render
   pin against the interpreter's printed value. Heavy arithmetic rides DIVISION-BY-WITNESS: the encoder
   witnesses quotients/products/differences and the kernel checks the defining property across in-envelope
   certificates (value-pin certs re-compute each operand expression; chunked literal addition certs verify
   the op) — certifying computation, replacing the old quotient wall with the measured reduction envelope
   of the 64 MiB alpha image.*

Everything above obeys the standing decisions: D1 (Rust exits by role), D2
(meaning by elaboration to Gamma), D3 (trust via proofs + translation
validation), D5 (direct checked refinement), and D6 (Delta-built
`omega-bootstrap`, then the hosted production compiler). The bridge binary and
its product output may initially be conservatively lowered; the product still
implements the full optimizer and language. The same method validates that hosted result
against canonical meaning rather than trusting the bootstrap compiler's
pedigree.

## The ∀-input climb (plan of record, 2026-07-05)

Input-taking samples are proven per input vector (`input-tv.sh`, step 52: substitution closes the
program). Making those claims universal hits a precise kernel wall: **user functions are primitive
recursion with a FIXED extra argument**, but every translated input loop threads an UPDATING accumulator
(`loop(Cons b t, n) = loop(t, n+1)`). The ∀-input theorems cannot even be *defined* kernel-side today.

The capability that fixes it is small and sound: **`(recx i E)`** — a rule-body form recursing on field
`i` with the extra argument replaced by the instantiated `E`. The scrutinee still decreases structurally
(only the extra changes), so termination and consistency arguments carry over unchanged. Climb order,
one seam per tick (D4):

1. `check_ref.py` spike — DONE (inert: nothing emits recx; ground accumulator certs accept, wrong counts
   reject; the anchor rejects the form until it learns it).
2. `check.beta` learns recx (parser tag + instantiate branch) + a seam gate (recx defeq vs operational
   eval over ground accumulator loops) + the checker diamonds re-run.
3. `checker.gamma` (finst2 `Recx` node) + refcert_to_gamma translation, restoring the three-checker
   diamond over recx certs.
4. Proof assembly: the first ∀-input theorem (`∀xs ∀n: loop(xs, n) = uadd(len(xs), n)` for
   stdin_checksum's count loop) via prover.py induction + the uadd-suc helper lemma.
5. Automation: gamma2claim (or a sibling) emits the fun rules + spec + proof skeleton from the
   translated gamma — grids become theorems.
