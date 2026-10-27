# Private arithmetic through borrowed contents

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../TASKS.md).
This extends the [private arithmetic fact](../README.md) through another
production of the same captured value:

> Every private arrival carries a positive value, so the helper can form value - 1.

An arrival copies the guarded scalar into `output.value`, then invokes `checked`
with an explicit mutable reference. That helper writes the unrelated `touched`
field, reads the still-positive `value` field, and performs the same subtraction.
The caller observes the final stored value, both returned values and tags, the
write, its neighboring canary, and the zero-input fallback. The invalid twin
changes only the scalar actual to zero before it is stored.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target body | [main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The retained optimized baseline producer `d871d8aaf7a98bfb2f0bb27de86aac844c5c8a3a`
has compiler inputs identical to the frozen starting source
`286c3a66623fcdda18830cd7d81ea8bc8d78d506`. It refuses all nine positives and all
nine invalid twins specifically at `checked`'s subtraction formation.
Repaired producer `56aa116c415` executes the nine positives with native/Terminal
exit-zero agreement on macOS ARM64, while the nine twins still reject for that
obligation. Sources and controls are pinned under `tests/omega/{pass,fail}/facts/`
with prefix `nested_machine_private_arithmetic_borrowed_`. The overlapping-write
controls reuse their existing `nested_machine_private_arithmetic_mutable_` pins.
Every positive has all four hosted target entries; other-host native execution
is not claimed.

The [disjoint control](controls/disjoint/main.omg) starts with an arbitrary
borrowed record and establishes its count bound through a guard. Its callee
writes another field and subtracts from the count. It now executes in both
engines. Changing that write to the count, writing through a local alias,
replacing the count in an intermediate invocation, or admitting an unproved
second arrival must still reject:
[overlap](controls/overlap/main.omg),
[alias](controls/alias_overlap/main.omg),
[prior write](controls/prior_write/main.omg), and
[unproved arrival](controls/unproved_arrival/main.omg).

Specification obligations established by this production:

- Establishment: actual arrivals carry the positive scalar into current storage.
  The helper does not assume its own arithmetic goal or an enclosing contract.
- Identity and lifetime: entry binding rebases only current observed cells to
  exact formal coordinates, retaining frozen selectors and detached value
  identities. A callee entry snapshot is its current arrival, not an earlier
  caller entry epoch. Related writes retire facts; unrelated writes preserve them.
- Composition: ordinary and terminal body traversal share one binding operation.
  The same value crosses all nine places and multiple invocation boundaries;
  callers do not re-establish the static fact with a result equality guard.
- Soundness: exact frame origins supply storage correspondence, never loans or
  predicate truth. Entry origin capture is independent of future preservation;
  normal completion still owns return facts. Unknown origins remain conservative,
  and theorem/case-only traversal keeps denotational admission.
- Checked-to-Terminal Psi: both-engine execution preserves explicit references,
  ordered stores and reads, selected helpers, record results and caller checks.
  This row does not introduce affine cleanup or proof-term constructs.
- Provider/layout/calling plans: the authored hosted entry, record fields,
  scalar arguments and explicit reference operands use the existing plans.
  No new ABI placement lane is claimed.
- Instructions and activation frames: native execution checks the storage value,
  write and canary as well as both result/tag paths across ordinary and terminal
  invocations. No recursive cycle or new stack-bound acceptance is claimed.
- Encoding and emission: direct macOS ARM64 execution agrees with Terminal Psi
  for every place. Each authored runtime falsifier traps on a changed result.

Remaining parent acceptance includes product compiler declaration/contract
migration, broader restoration/custody and recursive invocation obligations, and
other-host native qualification. This row does not close the whole migration.
