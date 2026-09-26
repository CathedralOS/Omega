# Compiler observation artifacts

Start at [artifact_writer.rs](src/artifact_writer.rs) for directory creation,
text/byte publication, and stale-file removal. Text and binary output use the
same temporary-file write followed by rename. This is a per-file publication
operation, not a transaction over a complete report directory.

The compiler emits no debug report files. The retained owners are:

- [Timings](src/compile_timings/mod.rs): disabled-by-default measurement storage.
	The CLI opts in for command-stage timings and prints them on stderr.
- [Trust](src/reports/trust_report/mod.rs): retained evidence and mandatory
	target consistency checks, without a Markdown writer.
- [Wire](src/reports/wire_report.rs): schema/compatibility records used by checking.

JSON/HTML renderers and the external-root report feature are removed. The normal
CLI uses the standard allocator, not the allocation-counting wrapper. Required
product bytes still use the atomic writer; executable-container packaging
remains a test-only adapter. These utilities do not grant publication authority.
