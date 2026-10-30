# Bounded process execution

Start at [bounded_process.rs](src/bounded_process.rs).
`run_bounded_process` validates the capture limits, launches a prepared child,
starts duplex I/O workers, collects their results under one deadline, and closes
the process container before returning output. Failure paths close and reap the
child within the remaining cleanup budget.

Follow its two lifecycle inputs:

- [preparation.rs](src/preparation.rs) owns the structured command, resource
  limits, and explicit null/piped stream setup.
- [process_child.rs](src/process_child.rs) owns launch, termination, reaping,
  and completion. Its children contain native limits, descriptor handling,
  completion records, and Windows Job Object support.

The run loop's subordinate [capture contract](src/bounded_process/capture_contract.rs)
defines input, output, limits, and errors; [budget.rs](src/bounded_process/budget.rs)
accounts for aggregate retained output. `lib.rs` only wires and exports these
owners. Callers can also use the prepared-child lifecycle directly without
requesting duplex capture.

`BoundedProcessPrepared::observe_resources()` opts into native CPU, peak resident
memory, fault and cycle observations retained in `BoundedProcessCompletion` and
`BoundedProcessOutput`. Defaults keep the existing lifecycle. On Unix the opted-in
owner reaps the exact child with `wait4` and retains its status; process-group
cleanup still runs. Windows queries the owned primary handle before closing it.
Each `ProcessResourceUsage` names its provider and accounting scope; unavailable
metrics are `None`, including cycles on Unix. Windows peak working set is resident
memory, and its total faults do not distinguish minor/major faults. See the
[benchmark guide](../../../../../tools/benchmark/comparison.md) for units and uses.

Windows uses a kill-on-close Job Object with process, memory, and CPU ceilings.
Unix applies inherited resource-limit intersections, launches a process group,
and kills that group during cleanup. A Unix descendant can deliberately detach
from its process group. Neither platform path claims filesystem, executable,
credential, or network isolation. Callers requiring a security sandbox must
provide and verify that separate boundary.
