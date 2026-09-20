//! What the build program declares and selects: program-entry selection,
//! target-scoped machines, the toolchain vocabulary, authored declarations,
//! wire-protocol compatibility, behavior exclusions, and the concrete build
//! configuration read back from the evaluated `Build` value.
//! `admitted_build_program.rs` is the route that consults them.

pub(crate) mod behavior_exclusions;
pub(crate) mod component_assumptions;
pub(crate) mod configuration;
pub(crate) mod declarations;
pub(crate) mod selection;
pub mod target_machines;
pub(crate) mod vocabulary;
pub(crate) mod wire_protocol;
