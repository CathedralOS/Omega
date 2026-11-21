# build_log_observability

`BuildLog::write_line(@mutable builder.log, "...")` — the `@mutable` is required;
an immutable lend rejects with "the caller lends only immutable access".

Each build activation's log lines surface in `--check` output in
evaluation order: the requesting activation's line prints first, then
each dep activation's. This is the only authored observability channel
into a pooled evaluate-once check.
