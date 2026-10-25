#![forbid(unsafe_code)]

//! Hosted execution of an already-produced Terminal artifact.
//!
//! Start at `execution.rs`: admit the exact retained provisions, establish a
//! private host root, install bindings, then execute through the Psi interpreter.
//! This application owner supplies target-aware host integration. It neither
//! compiles source nor implements language execution or native lowering.

mod execution;

#[cfg(test)]
mod runtime_boundary;

pub use execution::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeError,
    TerminalRuntimeObservation, TerminalRuntimeRequest, capture_host_x86_flags, execute,
    execute_sliced,
};
pub use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
