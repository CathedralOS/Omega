# Exact selected terminal completion and cleanup

A selected terminal Unit call preserves its prefix and call effects, then cleans
its owned local exactly once. This is a focused regression of the existing
[terminal Unit local cleanup fixture](../runtime_terminal_unit_local_cleanup_exit/README.md),
not a new mechanism row. The mutable owned local is lent into a nested terminal
call. Its unconditional cleanup increments an external counter, without an extra
cleanup precondition. The caller checks prefix count, call count, selected value
and cleanup count after completion.

The source exit selected before destination planning must reach both result
custody and cleanup. Canonical source formation and flow retain every arm;
unknown runtime guards keep both alternatives. The
[linear twin](../../../fail/control_flow/static_terminal_unit_linear_cleanup/main.omg)
removes automatic disposal authority and must still refuse the undischarged owner.

Frozen R23 producer source `00116985bb9ea1554b2199b8d3818f0211af6aae`,
binary SHA256 `6a5573298b71d13cd727c8bdebb88f6a516f744c9062532897dd293fddf81e23`,
accepts all four positives at update. Literal and immutable-local Boolean dispatch
then refuse native lowering with `structural result has no selected disposition`.
The type-predicate and unknown-runtime-guard controls already execute with native
and Terminal exits 0. R24 source `95c3d68f3755c7c35d133cc1a8dac592d4d100eb`,
binary SHA256 `d9dc6df67ecf93a36366ade3c129f2914f117934db767f9a387c2826cea0d43f`,
executes all four positives with native0/Terminal0 and rejects the linear twin.
These are observed engine outcomes, not update-only claims.

Exact authored input hashes, commands, producer profiles, accepted replay locks,
diagnostics and engine results are retained in
`build/core-float-20261028/native-admitted-cleanup-replay-r23-r24/observations.json`
and summarized in `build/core-float-20261028/peirce-native-r24-observations.json`.
Tracked main/build sources are byte-identical to those frozen inputs.

The runtime guard control invokes both authored destinations. It does **not**
claim runtime coverage of a nonzero internal `TableTransition.continuation`:
current source parsing emits separate guarded transitions. That internal case
remains conservatively declined by static selection and source-reviewed only.

The earlier probe with `release_receipt requires receipt.code == 7` and its
invalid twin remain preserved under `native-static-selection-controls`; the
positive refuses during checking before native lowering. They are too broad for
this repair and do not establish a discriminating cleanup-precondition control.
The original generic floating representation witness is unchanged.
