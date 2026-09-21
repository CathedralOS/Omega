# C2L conjunct-lowering cliff — hotspot record

Status: measurement record, not a design or implementation task. Distills the
instrumented evidence behind the `checked-trees-to-lowered-psi` member that
never returns, for whatever lane picks up the open asymptotic centre. Every
number below is copied from measurements recorded on the C2L blowup row; the
hotspots are load-bearing facts, not hypotheses.

Subject fixture:
`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`.
Unreduced it produces no verdict — killed at 780s on the terminating-reduction
runs, SIGTERM'd at 1480.871s in the `d936717f` census.

## What it is not

Measured at `de5798c306`: this is **not a proof search and nothing diverges**.
Tracing producer calls over 200ms at the cliff: 192 calls, 99.6s total, all
192 returned a proof. The relaxed fallback was never reached and the kernel's
`StepCeiling` was never hit. There is no non-converging obligation to contain,
so **do not bound the search** — every traced obligation is provable, and
bounding abandons obligations the compiler demonstrably proves. Fail-closed
refusal is design-blocked (OWNER_QUESTIONS.md question 4).

`OMEGA_PROOF_MEASUREMENTS` does not instrument this stage at all: on the
terminating reduction it reports `obligations=1 ... decided_elsewhere=1`, all
other counters zero — the ~20,000 kernel certificate acceptances and 633
evidence rows this program costs are produced outside `check_proof_plan`.
Rows routing this cost question through the retired PROOF-SEARCH-MEASUREMENT
name are pointing at the wrong substrate.

## The cliff

Bisecting the fixture on two axes with a scratch probe timing
`lower_typed_trees` and `lower_machine` separately:

| axis varied | result |
| --- | --- |
| `requires` premises: 80 → 32 (all ten redundant `input` upper bounds dropped) | 3251ms → 2191ms — 1.5x, no cliff, no subset enumeration |
| body top-level `&&` conjuncts (premises held) | 24 → 3.1s; 72 → 37.5s; 73 → 37.3s; **74 → >400s** |

The trigger is a conjunct **count threshold**, not one bad conjunct: omitting
group 74 and taking six later ones (79 total) also exceeds 240s. Below the
cliff the curve is smooth and roughly n^2.5–n^3.

Terminating reduction for future work: the first 73 body conjuncts with all
80 premises — 45s total. Reproduction note: the same split counts as the first
**72** conjuncts when the leading parenthesised triple is treated as one
conjunct; that reading reproduces 7s checking, 38s lowering, 633 evidence
rows.

## Hotspots

Four compounding centres, none in `proof/src/checker`:

1. **Duplicated whole-module obligation reconstruction** — `proofs/scalar_block_invariants.rs:48`
   and `proofs/scalar_block_invariants/cyclic_guarantees.rs:40` each call
   `terminal_verifier::reconstruct_terminal_obligations` on the same unchanged
   module; each reconstruction is itself O(N^2). ~47% of lowering even when
   the candidate roster is empty and neither loop iterates.
   **Closed at `4003c703186`**: the duplicate call in `retain_provable` is
   removed — reconstruction count 2 → 1 (13.4s → 6.7s), lowering 37.8s →
   31.0s on the reduction.
2. **Per-condition-fact equality-roster rebuild** —
   `terminal-verifier/src/verification/reconstruction/path_facts/conditions.rs:71-84`
   clones every `Equal(Value, _)` axiom in the roster into a certificate per
   condition fact, and `condition_fact` runs twice per conditional. A chain of
   N short-circuiting `&&`s gives O(N) conditionals with O(N) rosters — O(N^2)
   kernel work and the source of ~20,000 certificate acceptances. **This is
   the open residual and the only centre that changes the asymptotics**; it
   alters what the kernel is shown, so it needs a design pass (not an owner
   decision).
3. **Per-certificate kernel cost linear in chain length** —
   `mathematical_core::typing::infer_type` recurses past depth 260 on one
   certificate from this program.
4. **Checking-stage scans** — `typed-trees-to-checked-trees/src/authored_selections/operator_targets.rs:56`
   scanned per operator-by-fact pair and `member_targets.rs:369-381` tested
   membership with a `Vec` linear scan (16% of the run).
   **Closed at `4003c703186`**: visited-set + per-operator scan repair took
   checking 7.02s → 3.35s (2.09x). The remaining 3.3s of checking now sits in
   `checks::ranges::indexes::check_expression` and
   `checks::ranges::facts::dependencies::RangeFacts::record_dependencies`,
   each recursing ~25 levels beneath `seed_binary_guard_facts` — a fourth
   centre the original diagnosis did not name; its "worth most of a 7-second
   checking stage" estimate was exactly half right.

Aggregate: 51.6s → 41.1s (1.26x) on the terminating reduction. The same
obligations are discharged either way — 633 evidence rows and 633
reconstructed obligations before and after every run.

Re-verified at `2dbfecd98e49` (z140): the hotspot-1 repair is intact —
`retain_provable` still reconstructs the module once
(`scalar_block_invariants.rs:39`) and hands `&original` to the roster pass,
with `cyclic_guarantees::strengthen` taking the owner's reconstruction or
one of its own only when the module changed (:43-52); the hotspot-2 residual
is unchanged — `path_facts/conditions.rs` still clones every
`Equal(Value, _)` axiom into a per-condition-fact certificate; the subject
fixture still sits at
`nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return`.

## Acceptance for the repair

The unreduced fixture terminates with a verdict under an ordinary test
timeout, with no obligation abandoned — the repair is algorithmic and the
traced producer calls still return their proofs — and a `--no-fail-fast`
`checked-trees-to-lowered-psi` run reports no SIGTERM member.

## Re-verification

Re-checked at `3a82039327` (linux x86-64): no commit has touched either
anchor file since the design doc's `f9efadbb493e` re-verification.
`transport_certified` still clones the whole `Equal(Value, _)` roster into
both per-arm certificates (`conditions.rs` ~:71-84), so hotspot 2 — the only
centre that changes the asymptotics — remains the open residual exactly as
recorded.
