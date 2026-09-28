# Host artifacts and measurements

[artifact_writer.rs](src/artifact_writer.rs) owns per-file output replacement:
write bytes to a temporary file, rename into place, and remove stale files.
Callers supply already-produced bytes; this owner neither encodes executable
containers nor decides semantic validity.

[compile_timings](src/compile_timings/mod.rs) records opt-in phase durations and
allocation deltas. [allocations](src/allocations/mod.rs) supplies the optional
allocator instrumentation. The CLI controls measurement presentation.

[Activity counters](src/compile_timings/counters.rs) retain process-wide checked
requests, source assemblies, frontend reuse, and preliminary/settled check totals.
Compilation and Build evaluation record events; package review reads this owner
directly. These diagnostic counters are not per-request custody, successful-check
evidence, or atomic snapshots. Psi's repeated-check owner separately counts actual
check executions rather than requests served by its memo.

The only package dependency is Psi's diagnostic vocabulary. Trust report
records and validation live with [realization admission](../../compilation/realization/trust-model/README.md).
The writer tests cover byte replacement and failure cleanup directly.
