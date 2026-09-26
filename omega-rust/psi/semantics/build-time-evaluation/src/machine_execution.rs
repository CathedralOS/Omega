//! Executing compile-time machines: admission of the machines the compiler
//! runs, the build-machine execution service, supplied operator semantics,
//! syntax probes and reflection schemas.

pub(crate) mod admission;
pub(crate) mod build_machines;
pub(crate) mod reflection;
pub(crate) mod selected_operators;
pub(crate) mod syntax_probes;
