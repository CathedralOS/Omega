/*
Resource-bounded native child execution with owned process-tree cleanup.

We wrap native child processes so every helper invocation runs inside an OS
process container (a Unix process group via `command-group` or a suspended-start
Windows Job Object), closes unretained ambient descriptors across `exec`,
enforces CPU/memory/file/process ceilings, and terminates the entire descendant
container before returning or on drop.

`ToolCommandChild` reuses that native container owner for trusted build tools
with caller-configured file streams and resource policy. It does not apply the
runtime resource caps or descriptor policy; the existing prepared runtime API
continues to require both. Tool wrappers can expose primary exit separately,
then terminate remaining workers and complete the ordinary container wait.

Why separate `preparation`, `process_child`, and `bounded_process`?
- `preparation` (`BoundedProcessPrepared`, `BoundedProcessLimits`) validates
  command argument/environment byte ceilings (4,096 args, 1,024 env entries,
  4 MiB total configuration bytes) and requires explicit `Null` or `Piped`
  disposition on all three standard streams before spawn.
- `process_child` (`BoundedProcessChild`, `BoundedProcessCompletion`) owns
  container creation, descriptor hygiene, optional kernel resource-usage
  collection (`wait4` on Unix, `GetProcessTimes` / `GetProcessMemoryInfo` /
  `QueryProcessCycleTime` on Windows), and container termination/reaping.
- `bounded_process` (`run_bounded_process`, `BoundedCaptureBudget`) drives
  concurrent stdin transfer and bounded stdout/stderr capture under a single
  wall-clock deadline with reserved cleanup time.

These controls bound concrete host resources and descendant cleanup; they do not
claim filesystem, executable, credential, or network sandboxing. The one
filesystem exception is `private_root`, which runs a test executable over an
empty private tmpfs root its runner supplies, so that execution cannot reach the
host's filesystem at all.
*/

#![deny(unsafe_op_in_unsafe_fn)]

mod bounded_process;
mod preparation;
mod private_root;
mod process_child;

pub use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessOutput,
    BoundedProcessRunError, run_bounded_process,
};
pub use preparation::{BoundedProcessLimits, BoundedProcessPrepared};
pub use private_root::{
    PrivateFilesystemFiles, PrivateFilesystemOutputs, PrivateFilesystemPurpose,
};
pub use process_child::{
    BoundedProcessChild, BoundedProcessCompletion, BoundedProcessExitStatus, ProcessLimit,
    ProcessResourceUsage, ToolCommandChild,
};

pub use bounded_process::capture_child as capture_bounded_child;
#[cfg(unix)]
pub use process_child::descriptors::mark_ambient_close_on_exec;
