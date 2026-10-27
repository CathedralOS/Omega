# Cyclic private arithmetic

Every cyclic private arrival carries `depth >= 1`, so `consume` can form
`depth - 1`. This discharges guarded recurring-entry observation under
`NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../TASKS.md).

The [root witness](main.omg) revisits the guard on a terminal backedge. It
observes seven iterations, the zero fallback, completion writes and an untouched
canary. Its [reject twin](invalid/main.omg) permits zero at that guard.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [zero allowed](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [zero allowed](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [zero allowed](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [zero allowed](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [zero allowed](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [zero allowed](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [zero allowed](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [zero allowed](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [zero allowed](target_body/invalid/main.omg) |

The frozen optimized baseline `14a211b80ef82c2d89dfcabbd4b5fdc9d44bb7b6`
refuses the root and all nine positives at the exact subtraction, as well as
the intended reject twins. Repaired producer
`1e9d4d0939bd60731383da4630bc0150dd1d7b4c` runs all ten positives with
native/Terminal exit 0 on macOS ARM64; the ten twins still reject at
`consume`'s assignment. The same sources are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_private_cyclic_*`.

The checker closes the private caller slice, then observes each recurring
entry independently with arbitrary typed inputs under its own contract. It
never promotes a first iteration's value into an invariant. Prefix discovery
stays within that closed slice; ordinary completion still observes unrelated
calls' effects and results.

- **Establishment:** the consuming edge is guarded by `depth >= 1`. No helper
  precondition or caller equality guard supplies the result. The false branch
  returns without consuming; each reject twin changes only the bound to zero.
- **Identity and lifetime:** the guard denotes the current output field.
  [Overwriting it](controls/overwritten_guard/main.omg) or
  [guarding another field](controls/other_field/main.omg) rejects.
  [A disjoint write](controls/disjoint_write/main.omg) preserves the fact and
  runs with exit 0 in both engines.
- **Composition:** explicit arguments carry the same bound through all nine
  places. [An alternate unguarded arrival](controls/alternate_arrival/main.omg),
  [an initial parent guard only](controls/parent_guard_only/main.omg), and
  [an inner cycle bypassing the outer guard](controls/nested_cycle/main.omg)
  all reject. An [unrelated public builtin call](controls/unrelated_builtin/main.omg)
  composes with entry discovery and runs in both engines; this control already
  checked on the baseline and is coverage rather than a necessary repair.
- **Soundness boundaries:** private visibility and escaping-use checks remain
  prerequisites. Runtime entry observation grants no mathematical denotation
  or recursive theorem induction hypothesis. Fresh proof-only cycle roots are
  conservatively declined; ranked erased induction retains its existing owner.
- **Checked-to-Terminal Psi:** the actual guarded transitions, explicit borrows,
  mutations and completion survive Terminal execution. Positive programs check
  the final depth, marker and unchanged canary. A corrupted depth oracle traps
  in both native and Terminal execution instead of silently agreeing at zero.
- **Provider settlement, layout and calling plans:** every positive binds all
  four hosted targets. Executed evidence uses macOS ARM64 record layout and
  explicit mutable-reference calls; other-host native execution is unverified.
- **Instruction selection and activation storage:** native observations exercise
  repeated terminal edges and live borrowed caller storage. This row does not
  claim new long-cycle physical-stack qualification; the separate joint-rank
  witnesses retain that acceptance.
- **Encoding and image emission:** each positive executes the direct native
  image and agrees with Terminal Psi. Branches, field offsets and completion
  writes must be correct for the authored oracle to return zero.

The same closed-slice discovery repair also removes the standard-library
`rds_pop_go` and `rds_push_go` formation refusals. Their enclosing routine is
acyclic: unrelated public bodies had incorrectly made its private-prefix
observation incomplete. This is distinct from the recurring-entry requirement.
The subsequent whole-library admission exposes 14 remaining diagnostics:
five Windows result guarantees, seven directory-buffer index obligations and
two unclassified tail cycles. Full library admission, broader cyclic invariants,
remaining completion contracts and the product-source declaration migration
remain open under the parent task. Actual producer identities and raw outcomes
are retained in the qualification/claim evidence; no cross-host execution is claimed.
