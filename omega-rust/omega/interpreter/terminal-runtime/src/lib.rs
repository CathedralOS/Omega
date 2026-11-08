/*
We drive hosted reference execution of an already-produced Terminal Psi
artifact (`execute`, `execute_sliced`).

Start at `execution.rs`: we admit the exact retained provisions from the native
producer without rebuilding source or re-running Build, establish a private
isolated host root, install host bindings and component providers, and invoke
the Psi interpreter (`terminal_interpreter`). We supply target-aware host
integration only; we neither compile source nor implement language execution or
native lowering.
*/

/*
Why separate hosted Terminal runtime integration from `terminal-interpreter` and
`compiler`?

The Psi interpreter (`terminal-interpreter`) lives in `psi/` and must remain
target-neutral, while `compiler` must never depend on runtime execution. Our
crate bridges target-specific retained provisions (`TerminalRuntimeProvisions`),
isolated native provider processes (`native-provider-runtime`), and explicit
host capability delegations (`TerminalDomainExitDelegation`,
`TerminalFilesystemDelegation`, `TerminalNativeServiceDelegation`) into the
target-neutral interpreter.
*/

#![forbid(unsafe_code)]

mod execution;

#[cfg(test)]
mod runtime_boundary;

pub use execution::bind_admitted_intrinsics;
pub use execution::{
    TerminalClockEnvironment, TerminalClockSnapshot, TerminalDomainExitDelegation,
    TerminalFilesystemDelegation, TerminalNativeServiceDelegation, TerminalRuntimeError,
    TerminalRuntimeObservation, TerminalRuntimeRequest, capture_host_x86_flags, execute,
    execute_sliced,
};
pub use native_provider_runtime::run_worker_if_requested as run_native_provider_worker_if_requested;
pub use terminal_interpreter::TerminalMemoryOrderingEnvironment;
pub use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
