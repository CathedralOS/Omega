# Field projection supplies a computed endpoint formal

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../../TASKS.md).
This witness stays inside the parameter place of the
[computed-length row](../README.md). The sentence, the endpoint
`items.len - 1`, and the one-step counter advance are the same.

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends toward the computed descriptor endpoint
`items.len - 1` while advancing the explicit counter once.

`count` still ranks `cursor` against `items.len - 1`. `forward` states no
ranking. It receives the slice inside `Packet.view` and tail-calls
`count(counter, held.view, position + 1)`. The endpoint formal `items` arrives
as that field, not as a whole parameter. Starting at cursor 0 over eight
elements, the cycle runs seven times and raises the counter from 3 to 10.
Starting at the endpoint 7 leaves the counter at 10. A mis-lowered arm traps.

The reject twin changes only `position + 1` to `position`.

## Requirements

- Establishment: `count` authors `(cursor, items.len - 1) -> Nat::BoundedDistance`.
  `forward` contributes no separate rank; the field projection is the actual
  that supplies `items`.
- Identity and lifetime: `held` is an immutable parameter `forward` does not
  reassign, and `held.view` is the descriptor the endpoint reads.
- Composition: both edges use that transported endpoint. Only
  `forward -> count` is strict, because only that edge advances the cursor.
- Runtime falsifier: the advancing arm changes the counter and the endpoint
  arm does not.
- Checked-to-Terminal Psi and native execution: on linux_x86_64,
  `omega run --both` reports exit 0. The nonadvancing twin rejects with
  `no exact private-coordinate assignment proves every edge and complete cycle`.
  All four hosted entries are declared; Windows, macOS, and linux_arm64 were
  not executed here.

## What remains

A helper that supplies an endpoint formal through a computed argument, rather
than a whole parameter or a field projection, still has no transported bound.
The nine places that carry the whole-parameter endpoint stay parameter, local,
field, call result, match join, const name, nested machine, other machine, and
target body.

## Pinned fixtures

- [pass](../../../../../../../../../tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_field_formal_exit/main.omg)
- [fail](../../../../../../../../../tests/omega/fail/facts/nested_machine_joint_rank_bounded_computed_length_field_formal_rejected/main.omg)
