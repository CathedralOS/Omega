//! Admitted provider and callback facts consumed by target lowering.
//! These carry execution evidence, not another program representation.

use calling_conventions::NativePlace;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use target_operations::BoundarySettlementRealization;

/// Owned target-side input for one compiler-private callback argument.
///
/// The exact Terminal operation is the join to the unchanged abstract
/// `BoundaryCall`. The application and complete registrar plan/context remain
/// data until this stage independently validates their one-slot relation. The
/// application commitment is retained compiler provenance, not authority this
/// reduced tuple can recompute; callers must supply it from the exact retained
/// placement owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackArgument {
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    /// The private materialization destination this callback binds. For a
    /// field destination, `application` is the root record parameter's
    /// application — the parameter slot the private-slot write flows through
    /// — and `field_byte_offset` names the slot inside that record.
    pub destination: NativePlace,
    pub field_byte_offset: Option<usize>,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    /// Nonempty compiler-origin application-v3 commitment projection.
    pub registrar_application_commitment: [u8; 32],
}

/// One boundary realization sourced from a validated, admitted provider
/// execution or the consuming lowerer's closed compiler-builtin catalog.
#[derive(Debug, Clone)]
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub execution: AdmittedBoundaryExecution<'execution>,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, Copy)]
pub enum AdmittedBoundaryExecution<'execution> {
    Provider(&'execution dyn installation_evidence::ProviderExecutionEvidence),
    CompilerBuiltin(target_operations::CompilerBuiltinExecution),
}
