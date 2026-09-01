use psi_language_semantics::ServiceReachSummary;
use psi_symbols::SymbolHandle;

use crate::{CheckedScalarExpression, DynamicConformanceBindingFact, MachineContractCommitment};

use super::{
    CheckedUnitCallCoordinate, CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralPathSegment,
};

/// Checked dynamic-dispatch custody published by the Unit-effect planner.
/// Direct devirtualization is only the first lane; rebound descriptor/table
/// rows will extend this catalog without widening `CheckedUnitEffectPlans`
/// again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDynamicDispatchPlans {
    pub direct_scalar_calls: Vec<CheckedDirectDynamicScalarCallPlan>,
}

/// Checked custody for one direct scalar call through a local named dynamic
/// value. Every identity required to select the concrete realization is
/// retained here while typed expression handles are still available; Terminal
/// lowering must consume this row rather than repeat conformance discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDirectDynamicScalarCallPlan {
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub caller_attachment_type_identity: String,
    pub caller_multiplicity: psi_language_semantics::Multiplicity,
    pub caller_parameter_access: super::CheckedStructuralAccess,
    pub caller_contract_report_fingerprint: u64,
    pub caller_contract_commitment: MachineContractCommitment,
    pub caller_service_reach: ServiceReachSummary,
    pub coordinate: CheckedUnitCallCoordinate,
    pub result_binding: SymbolHandle,
    pub result: CheckedUnitScalarResultBindingPlan,
    pub receiver_binding: SymbolHandle,
    /// The exact latest selection preceding `coordinate`. This projection is
    /// source-handle-free and retains the complete selected row roster for
    /// downstream replay.
    pub selection: DynamicConformanceBindingFact,
    /// Exact field below the caller attachment selected as concrete `self`.
    pub source_parameter_position: u32,
    pub source_access: super::CheckedStructuralAccess,
    pub source_field: SymbolHandle,
    pub source_path: Vec<CheckedUnitStructuralPathSegment>,
    pub source_type_identity: String,
    pub source_multiplicity: psi_language_semantics::Multiplicity,
    pub target_trait: SymbolHandle,
    pub selected_conformance: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    /// Exact source-independent body of the selected realization's sole
    /// scalar return. The current structural-scalar lane cannot represent an
    /// unrestricted borrowed `self`, so downstream lowering consumes this
    /// expression directly instead of reopening typed source.
    pub realization_return_expression: CheckedScalarExpression,
    /// Compact report coordinate; authority uses the adjacent commitment.
    pub realization_contract_report_fingerprint: u64,
    pub realization_contract_commitment: MachineContractCommitment,
    pub checked_call_service_reach: ServiceReachSummary,
}
