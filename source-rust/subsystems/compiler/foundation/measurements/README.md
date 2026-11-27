# Compilation measurements

[compile_timings](src/compile_timings/mod.rs) records opt-in phase durations:
one row per stage through `record_result`/`add_completed`, collected into the
ladder the CLI prints under `--timings`. A disabled accumulator does the
same work without keeping rows. The CLI decides presentation.

[Activity counters](src/compile_timings/counters.rs) count process-wide Build
activations by purpose: the original program's Build and separate
provider-default companions. Build evaluation records each activation; tests
read the counters to prove a Build ran once. These diagnostic counters are not
per-request custody, successful-check evidence, or atomic snapshots.

Trust report records and validation are in
[realization admission](../../compilation/realization/trust-model/README.md).
