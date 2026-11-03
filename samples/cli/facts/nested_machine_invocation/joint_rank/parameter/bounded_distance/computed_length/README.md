# Computed descriptor endpoint for an unranked helper

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).
This is an open probe, not a filled row: [main.omg](main.omg) is expected to
exit 0 and is currently refused.

A cycle may end one element before the end of a slice, so its rank is the
distance from `cursor` to `items.len - 1`. The authored machine `count` states
that endpoint; its private helper `forward` states no ranking and only passes the
slice and `position + 1` back. The compiler must give `forward` an equivalent
rank on its own, as it already does when the endpoint is `items.len` or `stop + 1`.

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends toward the computed descriptor endpoint
`items.len - 1` while advancing the explicit counter once.

Baseline `90f3fe11ae75eb63246767979fe483cfe0c312f8` (optimized,
`omega run --both main.omg`) refuses with `a rank upper expression lacks
independent formation`. The same program passes when `forward` also authors
`terminates by (position, view.len - 1) -> Nat::BoundedDistance`, when the
cycle is a single self-recursive machine, and when the endpoint is `items.len + 1`
under `requires items.len < 100`; so the gap is the helper, not the arithmetic.

## What this change does

`items.len - 1` could not be formed because the engine knew a slice length only
as an unsigned unknown with no upper limit, so it could not show the subtraction
fits `u64` even with `requires items.len > 0`. A length is a `u64` count, so
`current_scalar_values.rs` now records the `0..=u64::MAX` interval with the
length it hands to scalar expressions. Endpoint formation now succeeds, and the
refusal moves to `no exact private-coordinate assignment proves every edge and
complete cycle`. This alone changes no accepted program.

## What remains

`forward` can only rank itself against the coordinates it owns: `view.len`,
`position`, or a constant carried over from `count`'s frame. Its distance
`view.len - position` is one larger than `count`'s `items.len - 1 - cursor`, so
the edge `count -> forward` would increase the rank. The helper needs the
authored endpoint expressed over its own parameters. The retained rank plan
(`runtime_ranking/inputs.rs`, `rank_observations/operands.rs`) requires an
expression operand's owner to be the state that evaluates it, so evaluating
`count`'s expression from `forward` needs that plan format to carry a formal
coordinate mapping from the authored machine to the helper. That is the next
step; no recognizer for this program shape was added.

## Evidence

- Candidate build: `current_scalar_values.rs` only, optimized `opt-level = 1`.
- 104 pinned fixtures named `joint_rank_bounded*`, `joint_rank_computed*` and
  `private_bounds*` (48 pass, 56 fail) keep their recorded outcomes with the
  change; 15 other fixtures using `terminates by ... .len` give the same
  results on the baseline and the candidate (13 of them already differ from
  their recorded rows on the baseline, for unrelated reasons).
- Without `requires items.len > 0` the helper's `view.len - 1` is still refused
  by ordinary exact-arithmetic checking.
- Native/Terminal execution of this probe is unverified because it is refused.
