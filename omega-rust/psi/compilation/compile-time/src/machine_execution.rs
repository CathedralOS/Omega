//! Executing compile-time machines: admission of the machines the compiler
//! runs, the build-machine execution service, supplied operator semantics,
//! selection custody.

pub(crate) mod admission;
pub(crate) mod build_activation;
pub(crate) mod build_machines;
pub(crate) mod selected_operators;
pub(crate) mod selected_target_bodies;
