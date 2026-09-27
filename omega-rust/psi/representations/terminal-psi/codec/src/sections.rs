//! The canonical sections of a terminal-Psi artifact, one module each: the
//! manifest, the semantic module wires, the debug map, the obligation
//! ledger, the optimization execution record, the proof bundle and sidecar,
//! and the trace profile. Receiver admission, implementation trust, and verified
//! semantic projections have separate owners outside this section roster.

pub(crate) mod artifact_manifest;
pub(crate) mod debug_map;
pub(crate) mod obligation_ledger;
pub(crate) mod optimization_execution;
pub(crate) mod proof_bundle;
pub(crate) mod proof_sidecar;
pub(crate) mod semantic_module;
pub(crate) mod terminal_trace_v1_profile;
