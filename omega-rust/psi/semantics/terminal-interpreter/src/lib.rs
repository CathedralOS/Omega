//! Fuel-bounded reference execution of admitted Terminal Psi artifacts.
//!
//! Artifact entrypoints decode and verify canonical semantics and proof bytes
//! before constructing runtime state. Execution retains immutable code and
//! resumable live custody; fuel exhaustion is not a semantic program outcome.

mod terminal_interpreter;

pub use terminal_interpreter::*;
