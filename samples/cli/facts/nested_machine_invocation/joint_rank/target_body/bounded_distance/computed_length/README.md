# Computed descriptor endpoint through target body

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).
This place copies the [computed-endpoint sentence](../../../parameter/bounded_distance/computed_length/README.md):
a private helper preserves the invocation rank on a tail edge, and every complete
cycle strictly descends toward `items.len - 1` while advancing the explicit
counter once.

`HostedDistanceBody` is the ordinary implementation Build selects for every hosted target. Over eight elements the advancing path runs seven times and raises
the counter from 3 to 10. The fallback path starts at `7` and leaves the counter
at 10.

[invalid/main.omg](invalid/main.omg) differs only by calling `count` with
`position` instead of `position + 1`.

## Pinned fixtures

- [pass](../../../../../../../../tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_target_body_exit/main.omg)
- [fail](../../../../../../../../tests/omega/fail/facts/nested_machine_joint_rank_bounded_computed_length_target_body_rejected/main.omg)
