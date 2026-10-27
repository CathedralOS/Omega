# Effectful completion

Every completed helper carries `result != -1`, so `accept` can consume its result.
This discharges the effectful normal-completion part of
`NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md), under
[invocation contracts](../../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use).

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [target body](target_body/main.omg). Each adjacent `invalid/main.omg` changes
only the helper's filter to exclude `-2` instead of `-1`, retaining the caller's
original postcondition. Both arms and multiple successful inputs are observable.

- **Establishment:** the executed helper checks its written value before
  returning it; its other path returns zero. The caller adds no equality guard
  to justify `accept`'s requirement.
- **Identity and lifetime:** the caller observes all four result snapshots,
  stored values, completion writes, and an untouched neighbor canary. The
  [stale initializer](controls/stale_initializer/main.omg),
  [overwritten guard](controls/overwritten_guard/main.omg), and
  [stale caller storage](controls/stale_caller_storage/main.omg) must reject.
  [Renamed formals](controls/renamed_formals/main.omg) preserve exact reference
  origins. [Caller storage](controls/caller_storage/main.omg) establishes a Unit
  postcondition through a helper with a different formal name.
- **Composition:** completed ordinary calls, hoisted calls, nested terminal
  calls, joins, and target-scoped bodies carry the same fact. Completion restores
  caller coordinates after invalidating may-written storage; it never treats
  callee entry contents as the caller's earlier entry snapshot.
- **Soundness boundaries:** executed-body observation supplies neither pure,
  repeatable denotation nor a recursive induction hypothesis. Existing purity,
  stale-reference, and formation controls remain applicable.
- **Psi 05–07:** `omega run --both` exercises Terminal production and actual
  interpreter execution, including mutable borrowed storage and Unit completion.
- **Omega 00–02:** all positives bind the four hosted entries. Native checks
  observe scalar results and caller-owned record writeback through their calls.
- **Omega 03–06:** the result, storage snapshots and canary make register/frame
  corruption observable. This row does not establish joint SCC ranking or
  constant-stack acceptance.
- **Omega 07–09 and emission:** actual macOS ARM64 images must exit zero and
  agree with Terminal execution. Other hosts remain unverified. A deliberately
  corrupted result oracle must trap in both engines.

The row and controls are pinned under `tests/omega/{pass,fail}/facts/` with the
`nested_machine_effectful_completion_` prefix. Producer-bound observations and
remaining library admission failures are retained with the landing evidence;
this row does not close the Omega-written frontend or downstream migration.
