# A completed normal call retains its exact type application

`copied` captures the result of `choose<u32>(value)` in an ordinary local and
returns it. Its ensures names that same application. The runtime oracle exercises
both Boolean values and independently checks that `choose<u64>(true)` is false.
The invalid twin changes only the ensured application to `choose<u64>(value)`.

Frozen R23 source `00116985bb9ea1554b2199b8d3818f0211af6aae` refuses the positive
ensures at statement 1. R24 source `95c3d68f3755c7c35d133cc1a8dac592d4d100eb`
updates successfully and actually executes native0/Terminal0. Both reject the
wrong-type contract. Source hashes, binary identities, raw diagnostics, accepted
locks and engine results are retained in
`build/core-float-20261028/specialized-normal-call-replay-r23-r24/observations.json`.
Main/build sources are byte-identical to those replay inputs.

The original terminal-forward positive remains separately frozen at
`build/core-float-20261028/specialized-call-contract-positive`. R24 still refuses
its ensures at statement 0, as recorded by `r24-specialized-call-trace.stderr`.
This fixture proves ordinary completed-call capture only; it does not claim a fix
for that terminal-forward proof gap or rewrite its oracle.
