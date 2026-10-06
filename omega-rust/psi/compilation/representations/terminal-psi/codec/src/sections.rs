//! The canonical sections of a terminal-Psi artifact, one module each: the
//! manifest, the semantic module wires, the debug map and the optimization
//! execution record.

pub(crate) mod artifact_manifest;
pub(crate) mod debug_map;
pub(crate) mod optimization_execution;
pub(crate) mod semantic_module;
