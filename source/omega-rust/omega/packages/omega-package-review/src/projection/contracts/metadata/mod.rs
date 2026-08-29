mod contracts;
mod evidence;
mod operations;
mod parameters;
mod service_reach;
mod source_locations;

pub(crate) use contracts::{
    ContractProjectionContext, project_callable_contracts, project_contracts,
    project_trait_requirement_contracts,
};
pub(crate) use evidence::{
    checked_contract_fact, checked_outcome_specific_guarantee, validate_checked_contract_evidence,
    validate_checked_contract_evidence_components,
};
#[allow(unused_imports)] // Compatibility exports from the former flat metadata module.
pub(crate) use operations::{
    canonical_checked_invocation_targets, project_machine_invocation_source_locations,
    project_machine_operational_source_locations, project_operational_keyword_locations,
    project_signature_invocation_source_locations, project_signature_operational_source_locations,
};
pub(crate) use parameters::{
    collect_callable_parameter_source_locations, collect_type_parameter_source_locations,
};
#[allow(unused_imports)] // Compatibility exports from the former flat metadata module.
pub(crate) use service_reach::{
    authored_service_reach_locations, derive_declared_service_reach,
    exact_authored_service_reach_row, project_machine_service_reach_source_locations,
    project_signature_service_reach_source_locations,
};
#[allow(unused_imports)] // Compatibility exports from the former flat metadata module.
pub(crate) use source_locations::{
    project_contract_source_locations, project_required_proof_fact_source_locations,
    proof_fact_handle,
};
