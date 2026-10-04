A nested write-only array loan lends one record element to two ordinary helper
calls. The selected element ends at 9 and its neighbor remains 85. Neither
helper observes the loan; both native and checked interpretation must exit 0.

Run `omega update --project tests/omega/pass/borrow/runtime_write_only_record_array_argument_exit`,
complete any reported package decisions, then run
`omega run --both tests/omega/pass/borrow/runtime_write_only_record_array_argument_exit/main.omg`
with stdin closed (`</dev/null` on macOS/Linux).

The corpus command is `python tools/corpus_gate.py --both --filter runtime_write_only_record_array_argument_exit`.
