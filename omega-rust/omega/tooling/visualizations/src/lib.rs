//! JSON observations of retained checked-program and selected-provider evidence.
//!
//! Start at the named manifest for the report being inspected. Each writer
//! owns its reconstruction checks and rendering; the compiler owns emission
//! policy and filesystem output. These reports grant no admission authority.
//!
//! Writers currently panic on inconsistent input instead of emitting partial
//! evidence. Their negative tests retain those rejection conditions.

mod capability_manifest;
mod carry_manifest;
mod claim_outcome_manifest;
mod executable_tcb_manifest;
mod index_compatibility_manifest;
mod machine_contract_manifest;
mod manifest_coordinates;
mod manifest_values;
mod qualification_manifest;
mod task_activation_manifest;

pub use capability_manifest::{
    capability_manifest_json, capability_manifest_json_with_composition,
    capability_manifest_json_with_selection,
};
pub use carry_manifest::carry_manifest_json;
pub use claim_outcome_manifest::claim_outcome_manifest_json;
pub use executable_tcb_manifest::{
    executable_tcb_manifest_json, executable_tcb_manifest_set_json,
    executable_tcb_manifest_value_json,
};
pub use index_compatibility_manifest::index_compatibility_manifest_json;
pub use machine_contract_manifest::machine_contract_manifest_json;
pub use qualification_manifest::qualification_evidence_manifest_json;
pub use task_activation_manifest::task_activation_manifest_json;

#[cfg(test)]
mod test_support;
