//! The canonical sections of a terminal-Psi artifact, one module each: the
//! manifest, the semantic module wires, the debug map, the obligation
//! ledger, the optimization execution record, the program-local root
//! catalog, the proof bundle and sidecar, the trust graph and the trace
//! profile.

pub(crate) mod artifact_manifest;
pub(crate) mod debug_map;
pub(crate) mod obligation_ledger;
pub(crate) mod optimization_execution;
pub(crate) mod program_local_root_catalog;
pub(crate) mod proof_bundle;
pub(crate) mod proof_sidecar;
pub(crate) mod semantic_module;
pub(crate) mod terminal_trace_v1_profile;
pub(crate) mod trust_graph;
