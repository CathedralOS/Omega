# Compiler observation artifacts

Start at [artifact_writer.rs](src/artifact_writer.rs) for directory creation,
text/byte publication, and stale-file removal. Text and binary output use the
same temporary-file write followed by rename. This is a per-file publication
operation, not a transaction over a complete report directory.

The compiler decides which observations to request under its
[product and observation contract](../../compiler/compiler/README.md#product-boundaries-and-observations).
The report owners are:

- [Timing](src/reports/timing_report.rs): phase measurements and allocation deltas
	written as plain text in `00_timings.txt`.
- [Trust](src/trust_report.rs): retained evidence, target consistency checks, and presentation.
- [Wire](src/wire_report.rs): schema/compatibility records assembled by the compiler.

There is no HTML renderer or graph-viewer navigation. JSON manifests and text
reports remain independent of any browser presentation.

Suppression of output does not waive semantic or trust validation. These
observations grant no admission or executable-publication authority. The
`external-root-report` feature adds optional external-root rendering; executable
container packaging remains a test-only adapter.
