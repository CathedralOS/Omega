# Compilation measurements

[compile_timings](src/compile_timings/mod.rs) records opt-in phase durations:
one row per stage through `record_result`/`add_completed`, collected into the
ladder the CLI prints under `--timings`. A disabled accumulator does the
same work without keeping rows. The CLI decides presentation.

[Activity counters](src/compile_timings/counters.rs) count process-wide checked
requests, source assemblies, frontend reuse, and settled check totals.
Compilation and Build evaluation record events; package review reads the
counters directly. These diagnostic counters are not per-request custody,
successful-check evidence, or atomic snapshots. Psi's repeated-check owner
separately counts checks that actually ran, not requests its memo answered.

Trust report records and validation are in
[realization admission](../../compilation/realization/trust-model/README.md).
