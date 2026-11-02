# Computed distance endpoint

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../TASKS.md).

The existing joint-rank fact also uses `stop + 1` as its authored upper endpoint.
The scalar input starts at 6, so the interval remains `[0, 7]`. Three cursor
advances and four upper-input decrements leave `Counts { value: 10, climbs: 3,
shrinks: 4 }`; an idle invocation preserves all three caller-owned fields.
Every twin changes only `ceiling - 1` to `ceiling - 0`. Skipping a branch or
confusing the two kinds of progress breaks the oracle even if the total stays seven.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](../../../../local/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../local/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| field | [source](../../../../field/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../field/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| parameter | [source](../../../../parameter/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../parameter/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| call_result | [source](../../../../call_result/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../call_result/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| match_join | [source](../../../../match_join/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../match_join/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| const_name | [source](../../../../const_name/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../const_name/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| other_state | [source](../../../../other_state/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../other_state/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| other_machine | [source](../../../../other_machine/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../other_machine/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |
| target_body | [source](../../../../target_body/bounded_distance/moving_upper/computed_upper/main.omg) | [source](../../../../target_body/bounded_distance/moving_upper/computed_upper/invalid/main.omg) |

The fixed clean base `6487b04d9a6c346970ac69bc503aaeb2a0423e38` admits all
nine positives but cannot produce their executable rank observations. Recursive
integer lowering did not resolve the same coordinate-bound inputs as whole
expressions. Both now share one declaration-input resolver; the separate Name
handling in the whole-expression builder is removed.

- Establishment: each invocation independently forms `stop + 1`, bounds the
  cursor and supplies its helper's requirements. [Missing formation](controls/missing_formation/main.omg)
  removes only the prerequisite bounding `stop`; its own partial addition rejects.
- Identity and lifetime: completed input captures take priority over coordinate
  observations. A name resolves its exact formal identity, not spelling; the
  source formal supplies both carrier and arithmetic policy. The
  [narrow input](controls/narrow_inputs/main.omg) computes in `u8` before the
  distance widens, and its [twin](controls/narrow_inputs/invalid/main.omg) rejects.
- Composition: the [indexed upper](controls/indexed_upper/main.omg) computes
  from a mutable cell and its captured selector; the caller checks cell `6 -> 2`
  and neighboring canaries `641`/`919`. Its
  [twin](controls/indexed_upper/invalid/main.omg) removes only the cell decrement.
  Both whole expressions and recursive arithmetic operands read
  the same admitted destination. No enclosing entry facts are imported. Existing
  structured, indexed and call-bearing declared-measure captures remain separate
  complete values and retain their own entry/call/cast receipts.
- Soundness: missing, ambiguous or incompatible bound inputs cannot select an
  ordinary parameter as a fallback. Geometry supplies no dynamic-index receipt.
- Psi 06–08: the ordinary scalar graph produces the actual computed upper and
  its exact difference, preserving the retained rank inputs through Terminal.
- Omega 00–02: every positive binds the four hosted entries and follows the
  existing explicit-argument calling plans; this repair changes no ABI schema.
- Omega 03–06: the same caller-owned counter remains live across the private
  cycle; this is execution evidence, not portable WCSU qualification.
- Omega 07–09: macOS ARM64 native execution agrees with Terminal Psi and the
  authored exit-0 oracle. Other-host physical execution remains unverified.

The original `stop + 0` [probe](probe/main.omg) is preserved unchanged. Pinned
fixtures are `facts/nested_machine_joint_rank_computed_upper_*` and
`termination/nested_machine_computed_distance_*`. Raw baseline, producer and
engine observations are in this workbench's `build/computed-upper/`.
The broader parent acceptance, other declared body forms, storage observations,
source migration and Omega-written compiler remain open.
