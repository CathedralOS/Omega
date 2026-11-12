/*
Resource-bounded native child execution with owned process-tree cleanup and
authenticated private observation lanes.

We wrap native child processes so every helper invocation runs inside an OS
process container (a Unix process group via `command-group` or a suspended-start
Windows Job Object), closes unretained ambient descriptors across `exec`,
enforces CPU/memory/file/process ceilings, and terminates the entire descendant
container before returning or on drop.

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
- `private_observation` provides an authenticated out-of-band byte channel
  (FD 3 Unix socketpair or PID-verified Windows named pipe) for runtime
  observation rosters separate from guest stdout/stderr.

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

pub use private_root::{PrivateRootUnavailable, run_in_private_root};

pub use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessOutput,
    BoundedProcessRunError, run_bounded_process,
};
pub use preparation::{BoundedProcessLimits, BoundedProcessPrepared};
pub use process_child::{
    BoundedProcessChild, BoundedProcessCompletion, BoundedProcessExitStatus, ProcessResourceUsage,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod private_observation;
#[cfg(windows)]
#[path = "private_observation/windows.rs"]
mod private_observation;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub use private_observation::{
    PrivateObservationCapture, capture_private_observation, prepare_private_observation,
};
