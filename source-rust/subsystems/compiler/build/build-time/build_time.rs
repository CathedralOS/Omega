//! Run admitted Psi code at compile time for Build and the compilers: project
//! Build values into an interpreter entry and read its result, execute one
//! admitted call, and evaluate the scalar observations a published program
//! requests. These modules depend on the interpreter, which depends on
//! build-model, so they cannot live in build-model.

#![forbid(unsafe_code)]

pub mod invocations;
pub mod projection;
pub mod scalars;
