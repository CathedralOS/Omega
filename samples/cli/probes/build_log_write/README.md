# build_log_write

`BuildLog::write_line(&mut builder.log, text)` — the compiler-owned build log facet takes an explicit operand; the line is retained in the build observation summary.

## Gaps found while authoring (filed)
- Named build inputs (`BuildInputs::get(&builder.inputs, "template")`) are assigned corpus-side via `BuildSnapshotRequest::with_inputs`; no CLI/`update` path assigns them — corpus-only surface (like TaskRuntime/slice writes).

Compile-checked only: run a check target, not native.
