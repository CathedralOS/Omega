# worker-10 handoff: LOWERED-FORM-CHECKER Step 3, calls and callee summaries

Slot: C:\Projects\Omega\.agents\worktree\worker-10 (base origin/main 380d1c53763ec).

## Measured

Local re-measurement of all 880 calls/contracts fixtures is not feasible on this
workstation: `route_one_comparison` spends 220-420 s per fixture in the current
checker (dev build, idle priority), against 1-2 s on the Linux VMs. I used the
whole-corpus table from open PR #2745 (`wiki/drafts/audits/route_coverage_2026-11-12.tsv`,
pin 0835b3aaf, 11 route commits behind main) for ranking and probed 6 + 12 + 14
fixtures locally at main.

First refusal of the 491 pass+run fixtures under `calls/` and `contracts/` that the
route hands back (audit pin):

| First refusal | Programs | Lane it belongs to |
| --- | ---: | --- |
| machine service reach (`reaches Console`, entry scaffolding) | 223 | boundary services and providers; stacked with attached data, `Build` selection and native leaves |
| parameter mutable parameter | 67 | ownership (#2765 in flight) |
| machine attached data | 40 | entry machines taking their own state |
| type reference | 35 | references |
| type data with custody | 27 | ownership |
| no entry bound | 38 | not a construct |
| machine generic parameters | 15 | generics |
| contract result binding (outcome-specific `ensures`) | 7 | calls |
| every other call or contract refusal | under 6 each | |

Call and contract refusals corpus-wide (first refusal, all tiers): contract term other
than a formal or literal 121 (arithmetic, casts and calls inside a clause; needs
formation obligations, contracts.md "Total arithmetic"), clause that is not a comparison
or conjunction 95 (predicate calls, mostly proofs lane), ranking witness 52, clause that
projects a formal or result 46, compiler operation call 44, result binding 29,
disjunction 26, receiver/requirement/coercion call 21, private machine of another
package 12 (correct hand-back), late-bound selection 11.

None of those is a missing callee summary. The missing summary shows up as a
rejection, not a fallback: the route judged the program and refused a caller's
obligation that only the callee's body establishes. Because the route's rejection
governs the compile since #2655, those valid programs do not compile on main.

Probes at main 380d1c537 (`current=` is the default compile, router included):
- pass/calls/converted_call_value_private_arrival_exit: current=rejected, route=rejected
  (`cannot prove requires contract for call verify_wide from step`). Missing result summary.
- pass/contracts/boolean_result_all_normal_returns_exit: route=incomplete (falls back).
- pass/facts/nine_arg_stack_calling_plan_through_local_exit: current=rejected, route=rejected
  (`cannot prove ensures ... result <= 92 && result == a9`): a prover gap in sums of
  eight equalities, numbers lane, not a summary.
- pass/calls/private_arrival_guarded_helper_range_exit,
  pass/calls/private_arrival_identity_return_range_exit,
  pass/facts/signed_shift_left_bound_through_local_exit: route=refused-module,
  `ForeignIdentity: a planned private arrival is not a call operation`.

## Landed

PR #2785, merge commit d351e60be82881f20892bf5287f18ea815038a9e: result summaries for private
helpers of the subject package (`lowered-discharge/src/result_summaries.rs`,
`Elaboration::local_results`), plus the `entry_inputs::require` repair for `CallUnit`,
`CallStructuralScalar` and `CallStructuralWithScalarArguments`. The slot is clean at origin/main.
Not run: `mbx xtask corpus`, `cargo xtask stage-coverage`, `lowered_route_*` compiler tests
(220 to 420 s per fixture on this host). 20 fixtures were probed with `route_one_comparison`.

## Next steps, exact

1. Run `cargo xtask stage-coverage` and the `lowered_route_*` compiler tests on a fast host for the
   newly governed programs: `pass/calls/converted_call_value_private_arrival_exit`,
   `pass/calls/private_arrival_{guarded_helper_range,identity_return_range}_exit`,
   `pass/contracts/boolean_result_all_normal_returns_exit`,
   `pass/facts/signed_shift_left_bound_through_*_exit`,
   `pass/facts/{narrow_to_wide_*,scalar_sign_zero_extension}_through_call_result_exit`.
2. `fail/contracts/boolean_result_conflicting_normal_returns` is `route=incomplete` (Boolean
   equality goal has no prover rule). With the fall-back removed it must become a rejection with
   its pinned diagnostic; that is a prover rule for `b == false` over Booleans, not a summary.
3. Summaries for public machines' same-package callers need a side table instead of a contract
   clause (a public contract is published). Summaries with one alternative per return need the
   prover to re-establish a disjunction of conjunctions at a private arrival.
4. Next call/contract refusals by first-refusal count: arithmetic terms in clauses (121, needs
   formation obligations per contracts.md "Total arithmetic"), outcome-specific `ensures` (29),
   machine-contract disjunction (26; the canonical form and substitution already carry it).
5. A unit test for `require` on a Unit helper needs the test `Machine` helper in
   `lowered-discharge/src/tests.rs` to build Unit machines.
