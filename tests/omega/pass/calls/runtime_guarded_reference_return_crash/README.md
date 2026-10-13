The false guard reaches the authored crash after a guarded reference return.
Native execution must trap; an ordinary return cannot satisfy the oracle. The
caller ends normally if the call returns, so a wrong reference value cannot
satisfy the trap oracle by sending the caller into another crash.

`expected_terminal_targets.txt` checks that the same retained Terminal artifact
traps when installed for each declared hosted target. Terminal reports the typed
trap with empty stdout and stderr; its outcome is independent of the host signal
or Windows exception status used by native execution. The sibling
`runtime_guarded_reference_return_before_crash_exit` covers the selected normal
return before the same fallback, including scalar and Unit completions.

Run the two fixtures through the ordinary corpus native/Terminal gate:

```sh
python tools/corpus_gate.py --both --filter calls/runtime_guarded_reference_return
```

A Terminal installation for a foreign target exercises portable target selection.
Matching-host native execution still supplies that target's physical trap oracle.
