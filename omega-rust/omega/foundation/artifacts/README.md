# Host artifacts and measurements

[artifact_writer.rs](src/artifact_writer.rs) owns per-file output replacement:
write bytes to a temporary file, rename into place, and remove stale files.
Callers supply already-produced bytes; this owner neither encodes executable
containers nor decides semantic validity.

[compile_timings](src/compile_timings/mod.rs) records opt-in phase durations and
allocation deltas. [allocations](src/allocations/mod.rs) supplies the optional
allocator instrumentation. The CLI controls measurement presentation.

The only package dependency is Psi's diagnostic vocabulary. Trust report
records and validation live with [realization admission](../../realization/trust-model/README.md).
The writer tests cover byte replacement and failure cleanup directly.
