# Distinct stored observations

`omega --check tests/omega/fail/calls/constructor_child_local_epochs_not_equal/main.omg`
rejects the observer's equality requirement. Its immutable record contains two
reads separated by a write, with no premise relating the incoming and replacement
values. Immutability cannot merge their original evaluation epochs.

The source is unchanged from the pending reproduction. The positive control is
`pass/calls/runtime_stored_equal_observations_exit`: two reads before the write
remain equal after storage.
