# Bounded process execution

Start at [bounded_process.rs](src/bounded_process.rs). `run_bounded_process`
checks the capture limits, launches a prepared child, starts threads that write
its input and read its output, collects them under one deadline, and closes
the process container before returning output. On failure it closes and reaps
the child within the remaining cleanup time.

Two files handle the child:

- [preparation.rs](src/preparation.rs) builds the command, resource limits,
  and null or piped streams.
- [process_child.rs](src/process_child.rs) launches, terminates, reaps and
  completes the child. Its submodules hold native limits, descriptor handling,
  completion records and Windows Job Object support.

[capture_contract.rs](src/capture_contract.rs) defines the run loop's input,
output, limits and errors; [budget.rs](src/budget.rs) counts total retained
output. `lib.rs` only wires and exports these. Callers can use the prepared
child without capture.

`BoundedProcessPrepared::observe_resources()` turns on CPU, peak resident
memory, fault and cycle measurement, kept in `BoundedProcessCompletion` and
`BoundedProcessOutput`; without it nothing changes. On Unix it reaps the exact
child with `wait4` and keeps its status; process-group cleanup still runs.
Windows queries the primary process handle before closing it. Each
`ProcessResourceUsage` names its source and scope; an unavailable metric is
`None`, including cycles on Unix. Windows peak working set is resident memory,
and its fault total does not separate minor from major faults. See the
[benchmark guide](../../../../tools/benchmark/README.md#cpu-resident-memory-and-faults) for units and uses.

Windows uses a kill-on-close Job Object that caps processes, memory and CPU.
Unix applies the tighter of each inherited and requested limit, launches a
process group, and kills it during cleanup. A Unix descendant can leave its
group on purpose. Neither platform isolates the filesystem, executables,
credentials or network; a caller that needs a sandbox must provide and verify
it separately.
