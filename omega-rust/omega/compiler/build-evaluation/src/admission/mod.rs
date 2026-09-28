//! What the build program declares and selects: program-entry selection,
//! the toolchain vocabulary and authored grants,
//! wire-protocol demands, behavior exclusions, and the concrete build
//! configuration read back from the evaluated `Build` value.
//! `admitted_build_program.rs` is the route that consults them.

pub(crate) mod behavior_exclusions;
pub(crate) mod component_assumptions;
pub(crate) mod configuration;
pub(crate) mod declarations;
pub(crate) mod selection;
pub(crate) mod vocabulary;
