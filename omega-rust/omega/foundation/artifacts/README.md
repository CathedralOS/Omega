# Compilation measurements

[compile_timings](src/compile_timings/mod.rs) records opt-in phase durations:
one row per stage through `record_result`/`add_completed`, accumulated into the
ladder the CLI prints under `--timings`. A disabled accumulator runs the same
work without retaining rows. The CLI controls measurement presentation.

[Activity counters](src/compile_timings/counters.rs) retain process-wide checked
requests, source assemblies, frontend reuse, and preliminary/settled check totals.
Compilation and Build evaluation record events; package review reads this owner
directly. These diagnostic counters are not per-request custody, successful-check
evidence, or atomic snapshots. Psi's repeated-check owner separately counts actual
check executions rather than requests served by its memo.

Trust report records and validation live with
[realization admission](../../compilation/realization/trust-model/README.md).
