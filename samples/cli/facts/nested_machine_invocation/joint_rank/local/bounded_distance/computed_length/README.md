# Computed descriptor endpoint through a local cursor

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).
This is the local place of the
[computed-endpoint witness](../../../parameter/bounded_distance/computed_length/README.md).
[main.omg](main.omg) carries the same sentence: a private helper preserves the
invocation rank on a tail edge, and every complete cycle strictly descends
toward the computed descriptor endpoint `items.len - 1` while advancing the
explicit counter once.

The cursor arrives through a local. `perform` binds `cursor` to `0` or `7`
and passes that local into `count`. Over eight elements the advancing path
runs seven times and raises the counter from 3 to 10. The fallback path starts
at the endpoint `7` (`items.len - 1`), takes `done`, and leaves the counter at
10. Any other count traps.

## Reject twin

[invalid/main.omg](invalid/main.omg) differs only in the helper's call:
`count(counter, view, position)` does not advance, so no complete cycle
descends. The diagnostic is the missing private-coordinate assignment.

## Requirements

- Establishment: `count`'s requires form the endpoint `items.len - 1`. The
  local match supplies the cursor; no caller equality guard re-proves the rank
  after `count` returns.
- Identity and lifetime: the helper reads the endpoint from its entry
  parameters, which it does not reassign. The counter stays caller-owned.
- Composition: the local join admits only cursors that meet `cursor < items.len`.
  Both edges are proved against the transported endpoint, and only
  `forward -> count` is strict.
- Runtime falsifier: the advancing arm changes the counter and the endpoint
  arm does not. A mis-lowered branch that takes the other arm traps.
- Checked-to-Terminal Psi and native execution: on linux_x86_64, optimized
  producer opt-level 1 from base `83e3d72bad8`, `omega run --both` reports
  native exit 0 and Terminal exit 0. The nonadvancing twin rejects with
  `no exact private-coordinate assignment proves every edge and complete cycle`.
  All four hosted entries are declared; Windows, macOS, and linux_arm64 were
  not executed here.

## What remains

A helper that supplies an endpoint formal through a field projection or a
computed argument still has no transported bound. The nine places that carry
this whole-parameter endpoint are parameter, local, field, call result,
match join, const name, nested machine, other machine, and target body.

## Pinned fixtures

- [pass](../../../../../../../../tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_local_exit/main.omg)
- [fail](../../../../../../../../tests/omega/fail/facts/nested_machine_joint_rank_bounded_computed_length_local_rejected/main.omg)
