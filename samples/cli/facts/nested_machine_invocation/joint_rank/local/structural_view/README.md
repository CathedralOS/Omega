# Structural rank coordinates

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../TASKS.md).
This is another production of the existing joint-rank fact:

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

The authored `Slice::Length` witness belongs to `count`. Its private helper
has no separate rank clause and reorders the counter and descriptor arguments.
The selected descriptor crosses each existing transport place before entering
the same cycle. Seven increments change 3 to 10; the empty descriptor then
performs no increment. Both paths execute in each positive program.

Each place has `structural_view/main.omg` and `structural_view/invalid/main.omg`:
[local](main.omg), [field](../../field/structural_view/main.omg),
[parameter](../../parameter/structural_view/main.omg),
[call result](../../call_result/structural_view/main.omg),
[match join](../../match_join/structural_view/main.omg),
[const name](../../const_name/structural_view/main.omg),
[nested machine](../../other_state/structural_view/main.omg),
[another machine](../../other_machine/structural_view/main.omg), and
[Build-selected implementation](../../target_body/structural_view/main.omg).
The const array is explicitly materialized before borrowing its view.
Every twin changes only `pending[1..]` to `pending`, removing strict descent.

The frozen optimized baseline `9a4ec4a40c352ea4eb51f49f2d23ee03e3e3a78e`
refuses these positives because the helper lacks a supported ranking witness.
Exact baseline/candidate producer identities and observations are retained under
`build/private-cycle-ranks/` and in the publication evidence.

- Establishment: the authored view selects descriptor length, never an unrelated
  integer with the same output carrier. Each edge proves nonnegative,
  nonincreasing rank under its own contracts and completed prefix; removing
  strict edges must leave a DAG. Ranking does not prove its own slice bounds.
- Identity and lifetime: the entry length is captured before replaying writes;
  successor length comes from the actual descriptor's current observation.
  A [restored descriptor](../../controls/structural_restored_descriptor/main.omg)
  rejects after a decreasing local is replaced with its original descriptor.
  A [missing bound](../../controls/structural_missing_bound/main.omg) also rejects.
- Composition: exact declaration symbols and actual parameter positions select
  helper coordinates. Lexical nesting imports no parent facts. The caller's
  mutable counter and backing byte array remain live until ordinary completion.
- Checked-to-Terminal: the same admitted descriptor coordinate produces the
  shared scalar `StructuralSequenceLength` computation for authored and private
  participants. No helper source expression or alternate evaluator is fabricated.
- Realization: the nine positives bind all four hosted targets. Native execution
  agreement is measured on macOS ARM64; other-host execution remains unverified.
  This row uses existing descriptor ABI, tail-component fusion, register/frame
  realization and image emission. It does not establish portable WCSU or a new
  physical stack bound.

Pinned retention is `facts/nested_machine_joint_rank_structural_view_*` under
`tests/omega/{pass,fail}`, plus the two descriptor/formation controls. Ranged
joint coordinates, other structural views and the remaining parent acceptance
stay open; this production does not close the capability.
