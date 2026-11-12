A local array lends its original storage to a write-only forwarding helper.
The helper selects a record at runtime and calls its replacement machine.
A second call selects directly from the local using readable caller metadata.
The caller checks both modified values, the untouched element and every padding
field; Native and Terminal must both exit 0.

Run `omega update --project tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit`,
complete any reported package decisions, then run
`omega run --both tests/omega/pass/borrow/runtime_indexed_write_only_local_receiver_exit/main.omg`
with stdin closed (`</dev/null` on macOS/Linux).

The corpus command is
`python3 tools/corpus_gate.py --both --filter runtime_indexed_write_only_local_receiver_exit`.
The fail controls `write_only_index_missing_bounds` and
`write_only_index_selector_observation` retain range and non-observation checks.
