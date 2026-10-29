# Ranged invocation ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../TASKS.md).

> A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

Each existing transport place has `ranged_value/main.omg` and its `invalid/`
twin. The input stays seven or zero, the counter starts at 3, and both paths
execute in each positive: seven increments reach 10, then the empty path
preserves 10. The range `0..=7` constrains that same rank. Helpers reorder the
arguments, declare their own entry premises and author no redundant rank.
A twin changes only `pending - 1` to `pending`, removing complete-cycle descent.

[Local](main.omg), [field](../../field/ranged_value/main.omg),
[parameter](../../parameter/ranged_value/main.omg),
[call result](../../call_result/ranged_value/main.omg),
[match join](../../match_join/ranged_value/main.omg),
[const name](../../const_name/ranged_value/main.omg),
[nested machine](../../other_state/ranged_value/main.omg),
[another machine](../../other_machine/ranged_value/main.omg), and
[Build-selected implementation](../../target_body/ranged_value/main.omg)
carry the same operation. Call-result producers publish their actual bound;
forwarding machines declare their actual input requirement.

## Why optional bounds need not stay equal

The canonical [termination contract](../../../../../../../wiki/spec/language/termination.md)
makes optional intervals private constraints on the produced rank. The selected
relation here is fixed Nat₀: each edge proves a nonnegative, nonincreasing rank,
and removing strict edges must leave a DAG. Equality of optional ceilings or
floors adds no termination or public-contract guarantee.

The independent entry theorem proves that declared Requires and domains imply
a formed interval and membership. Ordinary invocation checking proves those
same premises under exact actual substitution, so the theorem applies at every
call. A second arithmetic-only membership solver would unnecessarily reject
calls justified by stronger theorem evidence. No caller guard or later edge
establishes the entry theorem; no callee conclusion establishes its own premise.
A parameter belonging to the selected view, such as a bounded-distance limit,
retains that view's separate obligations.

The former whole-component optional-endpoint conservation pass was removed.
That pass rejected valid changed-bound programs; it was not a language rule.
The old `mixed_call_member_endpoint_foreign` control remains invalid because its
caller fails an ordinary helper requirement, not because an optional ceiling
changed. Its rejection oracle now names that actual obligation.

## Composition and controls

The same seven-step counter operation also runs with a
[carried bound](carried_bound/main.omg), a
[descriptor length](sequence_length/main.omg), a
[partial floor](partial_floor/main.omg), and a
[static declaration bound](static_bound/main.omg).
[Changed ceiling](rebound_ceiling/main.omg) instantiates 9 then 7;
[changed floor](rebound_floor/main.omg) instantiates -1 then 0. Both preserve the
same nonnegative rank relation and satisfy each invocation's premises.
Each production has a fact-sensitive twin: no strict edge, missing floor
formation, or an actual tuple that violates the next invocation's requirements.
The [missing entry bound](../../controls/ranged_missing_entry/main.omg) and
[self-justifying partial operand](../../controls/ranged_self_justifying_floor/main.omg)
remain independent refusals.

- Establishment: ranges form from the machine's own declared premises, before
  body guards; ordinary calls establish those premises on the actual values.
- Identity: immutable declaration indices use exact symbols and declared
  carriers. Runtime arguments remain explicit; parent runtime facts are absent.
- Composition: preserved and strict edges share one relation; optional ranges
  instantiate at each arrival. Calls retain contract and ownership checking.
- Soundness: arithmetic formation, exact source versions, write invalidation,
  view-bound obligations and strict induction premises remain independent.
- Realization: each positive binds all four hosted targets. The shared retained
  scalar/length observations feed ordinary Terminal production and native
  constant-stack component lowering. Other-host execution and portable WCSU
  remain separate requirements.

The frozen local source refuses on exact optimized producer
`e3c13ae2455c62f31486b028dab0fd6705ab6c4e` because the private helper lacks a
supported witness. Removing only the optional range from the changed-ceiling
program makes that baseline run in both engines, demonstrating the unnecessary
restriction. Raw producer identities, baseline comparisons, actual engine
observations and source scopes are retained in `build/ranged-cycle-ranks/`
and the publication evidence.

Fixtures are pinned under `tests/omega/{pass,fail}/facts/` as
`nested_machine_joint_rank_ranged_*`. Projected ranks, declared views,
dependent-distance private coordinates and the rest of the parent capability
remain open. This production does not close the task.
