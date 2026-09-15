//! One JSON manifest per retained subject: capabilities, carry policies,
//! claim outcomes, the trusted computing base, indexed-domain
//! compatibility, machine contracts, qualification evidence and task
//! activation.

pub(crate) mod capability_manifest;
pub(crate) mod carry_manifest;
pub(crate) mod claim_outcome_manifest;
pub(crate) mod executable_tcb_manifest;
pub(crate) mod index_compatibility_manifest;
pub(crate) mod machine_contract_manifest;
pub(crate) mod qualification_manifest;
pub(crate) mod task_activation_manifest;
