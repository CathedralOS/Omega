//! Unit machine lowering.
//!
//! `lower_unit_plan_machine` lowers the one checked Unit plan a selected
//! machine carries: it dispatches on the plan's kind to the body that owns it
//! — nominal and partial affine cleanup, composed control, structural control
//! — and publishes what each kind completes. The module also owns attached
//! closure assembly and the entry requirements every Unit body must satisfy.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTerminalSignatureEligibility,
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape,
};
use language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, DomainSemanticId, IntegerType, MachineId, ObligationId,
    PlaceId, Proposition, QualifiedScalarType, ScalarTerm, ScalarType, ServiceId, StructuralCaseId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    ClaimTransfer, ContractClause, EntryClaim, InstallationReachDependency, MachineContract,
    NominalAffineCleanup, Operation, OperationKind, OperationResult,
    ProgramLocalRootIntroductionSchema, ProviderCandidateConformance, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalRankedScc, Terminator, ValueDeclaration,
    VocabularyMarker, program_local_root_introduction_compatibility_report_identity,
};

use crate::emission::boolean_control::{
    boolean_decision_block_count, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_value_decision,
};
use crate::emission::expression_validation::{
    contains_short_circuit, direct_expression_contains_short_circuit,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_scalar_type, integer_value, terminal_scalar_type,
};
use crate::lowering_error::{LoweringError, unsupported};
use crate::producer_result::{DebugPublication, LoweredSelectedMachine, LoweringCompletion};
use crate::proofs::content_conservation;
use crate::proofs::content_conservation::lower_boundary_content_guarantees;
use crate::proofs::crash_routes::{
    lower_boundary_crash_routes, lower_checked_crash_route_buckets,
    lower_structural_crash_route_buckets, structural_crash_route_argument_prefix,
    substitute_structural_crash_route_roots,
};
use crate::retention::placed_view_inputs::lower_placed_view_input;
use crate::scalar_lowering::scalar_carriers;
use crate::structural_types::{
    lower_mixed_cases, lower_mixed_fields, terminal_byte_sequence_carrier,
};
use crate::terminal_identities::{
    TERMINAL_UNIT_CALL_OBLIGATION_BASE, allocate_dense, block_id, boundary_machine_id, claim_id,
    contract_id, dense_identity, edge_id, lookup_claim_id, lookup_domain_id, lookup_machine_id,
    lookup_service_id, lookup_type_id, machine_id, obligation_id, place_id, service_id,
    structural_domain_id, structural_field_id, structural_type_id, value_id,
};
use crate::unit::attached_unit::{lower_unit_closure, lower_unit_effect_closure};
use crate::unit::unit_cleanup::lower_nominal_affine_unit_cleanup_machine;

pub(crate) mod attached_unit;
pub(crate) mod crash_interface;
pub(crate) mod dynamic_dispatch;
pub(crate) use crash_interface::effective_crash_routes;
pub(crate) mod runtime_requirements;
pub(crate) mod unit_cleanup;

/// Lower the one checked Unit plan a selected machine carries: the body that
/// owns its kind, then the publication that kind completes. The cleanup kinds
/// finalize their operand proofs; the partial cleanup and composed control
/// bodies publish an exact machine catalog. No kind publishes the checked
/// debug map, which describes a scalar entry result.
pub(crate) fn lower_unit_plan_machine(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: CheckedUnitPlan<'_>,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<LoweredSelectedMachine, LoweringError> {
    Ok(match plan {
        CheckedUnitPlan::NominalAffineCleanup(plan) => LoweredSelectedMachine::source_mapped(
            lower_nominal_affine_unit_cleanup_machine(
                identities,
                checked,
                plan,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
        // The partial-affine plan is an ordinary body whose residual return
        // edge the closure installs for every emitted member.
        CheckedUnitPlan::PartialAffineCleanup(plan) => LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(
                identities,
                checked,
                plan.machine.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
        CheckedUnitPlan::ComposedControl(plan) => LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(
                identities,
                checked,
                plan.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
    })
}
