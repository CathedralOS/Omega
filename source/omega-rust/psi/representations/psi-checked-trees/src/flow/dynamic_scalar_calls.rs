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
/// Direct devirtualization and rebound descriptor/table calls remain distinct
/// lanes without widening `CheckedUnitEffectPlans` again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedDynamicDispatchPlans {
    /// Exact descriptor movements across ordinary calls, independent of
    /// whether a particular Terminal lowering composes or preserves the call.
    pub transfers: Vec<CheckedDynamicDescriptorTransferPlan>,
    pub direct_scalar_calls: Vec<CheckedDynamicScalarCallPlan>,
    pub rebound_scalar_calls: Vec<CheckedReboundDynamicScalarCallPlan>,
}

/// One checked call argument that transfers an already-selected dynamic
/// descriptor into one exact bare dynamic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicDescriptorTransferPlan {
    pub caller_machine: SymbolHandle,
    pub caller_state: SymbolHandle,
    pub coordinate: CheckedUnitCallCoordinate,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
    /// Dense among the target's non-self runtime parameters.
    pub parameter_position: u32,
    pub parameter: SymbolHandle,
    pub target_trait: SymbolHandle,
    pub source_binding: SymbolHandle,
    pub selection: DynamicConformanceBindingFact,
}

/// Shared checked custody for the selected call version of one local named
/// dynamic scalar call. The containing direct or rebound catalog supplies its
/// dispatch semantics; Terminal lowering must consume this row rather than
/// repeat conformance discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicScalarCallPlan {
    /// Exact authored route by which the selected descriptor reaches this
    /// scalar dispatch. A forwarded route is admitted only for one transparent
    /// scalar helper whose dynamic parameter is returned directly; Terminal
    /// lowering may then compose that internal call without inventing
    /// descriptor custody.
    pub origin: CheckedDynamicScalarCallOrigin,
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
    /// Exact optional primitive-field mutation performed by the selected
    /// realization immediately before its scalar return. This is realization
    /// custody, not the independent caller-side pre-selection store below.
    pub realization_structural_scalar_field_store: Option<CheckedStructuralScalarFieldStorePlan>,
    /// Complete closed realization roster for the selected conformance. A
    /// rebound dynamic descriptor is materializable only when every table
    /// slot retains its exact checked callable and scalar body; retaining only
    /// the currently selected row would make the later indirect table a
    /// producer assertion rather than a reconstruction.
    pub realization_callables: Vec<CheckedDynamicRealizationCallablePlan>,
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
    pub unit_continuation: Option<CheckedDynamicUnitContinuationPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedDynamicScalarCallOrigin {
    Local,
    Forwarded {
        machine: SymbolHandle,
        state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        parameter: SymbolHandle,
    },
}

/// One exact checked callable behind a closed dynamic-conformance table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicRealizationCallablePlan {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_identity: String,
    pub result_type: psi_typed_trees::types::PrimitiveType,
    /// Exact optional primitive-field mutation performed before the return.
    /// The v1 body shape admits at most one direct literal store through
    /// mutable `self`; downstream lowering must not rediscover it from source.
    pub structural_scalar_field_store: Option<CheckedStructuralScalarFieldStorePlan>,
    pub return_expression: CheckedScalarExpression,
    pub contract_report_fingerprint: u64,
    pub contract_commitment: MachineContractCommitment,
}

/// Checked custody for one local named-dynamic scalar call after exactly one
/// same-conformance reassignment. This is a separate lane from direct
/// devirtualization: later Terminal lowering must consume both source versions
/// as descriptor/table state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedReboundDynamicScalarCallPlan {
    pub initial: CheckedDynamicSelectionPlan,
    pub latest: CheckedDynamicScalarCallPlan,
}

/// Source-normalized custody for one version of a local named-dynamic
/// selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicSelectionPlan {
    pub fact: DynamicConformanceBindingFact,
    pub field: SymbolHandle,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
}

/// One named-dynamic scalar result consumed by an immediate binary control
/// split whose leaves each perform one checked Unit effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDynamicUnitContinuationPlan {
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
