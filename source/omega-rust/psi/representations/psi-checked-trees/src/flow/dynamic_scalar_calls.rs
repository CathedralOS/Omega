use psi_language_semantics::ServiceReachSummary;
use psi_symbols::SymbolHandle;

use crate::{
    CheckedComposedUnitControlStatePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarExpression, CheckedStructuralControlSuccessorPlan, DynamicConformanceBindingFact,
    MachineContractCommitment,
};

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
    /// Exact caller-side store immediately preceding the selected dynamic
    /// binding, when the bounded three-statement structural-field shape was
    /// admitted. Ordinary direct calls retain `None`.
    pub caller_structural_scalar_field_store: Option<CheckedStructuralScalarFieldStorePlan>,
    /// Exact checked control suffix when this result immediately selects two
    /// Unit effect leaves. The dynamic call remains in this plan; this suffix
    /// begins at the authored guard and therefore cannot be lowered as an
    /// independent machine or silently discarded.
    pub unit_continuation: Option<CheckedDirectDynamicUnitContinuationPlan>,
}

/// One direct named-dynamic scalar result consumed by an immediate binary
/// control split whose leaves each perform one checked Unit effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDirectDynamicUnitContinuationPlan {
    pub guard: CheckedScalarExpression,
    pub when_true: CheckedStructuralControlSuccessorPlan,
    pub when_false: CheckedStructuralControlSuccessorPlan,
    pub leaves: Vec<CheckedComposedUnitControlStatePlan>,
    pub provider_attachment_requirements: Vec<CheckedProviderAttachmentRequirementPlan>,
}

/// Checked custody for one literal store into a primitive field below the
/// structural carrier later selected for a direct named-dynamic call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarFieldStorePlan {
    pub statement_index: u32,
    pub destination_parameter_position: u32,
    /// Exact structural path from the destination parameter to the carrier;
    /// the final primitive field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub primitive_type: psi_typed_trees::types::PrimitiveType,
    pub value: CheckedScalarExpression,
}
