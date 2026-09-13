# Compiler observation artifacts

Start at [artifact_writer.rs](src/artifact_writer.rs) for directory creation,
text/byte publication, and stale-file removal. Text and binary output use the
same temporary-file write followed by rename. This is a per-file publication
operation, not a transaction over a complete report directory.

The compiler decides which observations to request under its
[product and observation contract](../../compiler/compiler/README.md#product-boundaries-and-observations).
The report owners are:

- [Timing](src/timing_report.rs): phase measurements, allocation deltas, and their presentation.
- [Trust](src/trust_report.rs): retained evidence, target consistency checks, and presentation.
- [Wire](src/wire_report.rs): schema/compatibility records assembled by the compiler.
- [HTML](src/html_report.rs): page framing, navigation, and escaping.

Suppression of output does not waive semantic or trust validation. These
observations grant no admission or executable-publication authority. The
`external-root-report` feature adds optional external-root rendering; executable
container packaging remains a test-only adapter.
