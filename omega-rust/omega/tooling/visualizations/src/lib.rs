//! JSON observations of retained checked-program and selected-provider evidence.
//!
//! Start at the named manifest for the report being inspected. Each writer
//! owns its reconstruction checks and rendering; the compiler owns emission
//! policy and filesystem output. These reports grant no admission authority.
//!
//! Writers currently panic on inconsistent input instead of emitting partial
//! evidence. Their negative tests retain those rejection conditions.
//!
//! `manifests/` holds one module per manifest and `encoding/` the
//! coordinates and value encodings they share; `test_support.rs` builds the
//! checked programs the manifest tests observe.

pub use manifests::capability_manifest::{
    capability_manifest_json, capability_manifest_json_with_composition,
    capability_manifest_json_with_selection,
};
pub use manifests::carry_manifest::carry_manifest_json;
pub use manifests::claim_outcome_manifest::claim_outcome_manifest_json;
pub use manifests::executable_tcb_manifest::{
    executable_tcb_manifest_json, executable_tcb_manifest_set_json,
    executable_tcb_manifest_value_json,
};
pub use manifests::index_compatibility_manifest::index_compatibility_manifest_json;
pub use manifests::machine_contract_manifest::machine_contract_manifest_json;
pub use manifests::qualification_manifest::qualification_evidence_manifest_json;
pub use manifests::task_activation_manifest::task_activation_manifest_json;

mod encoding;
mod manifests;
#[cfg(test)]
mod test_support;
