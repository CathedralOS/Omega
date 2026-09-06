#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi lowering.
//!
//! The output retains proof, debug and source-occurrence companions for the
//! subsequent optimization and Terminal publication stages. Unsupported source
//! constructs fail closed instead of being dropped. This stage does not run
//! optimizations or publish the portable artifact.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryScalarReturnMachinePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind, CheckedPropositionEvidence,
    CheckedScalarBindingValue, CheckedScalarBranchDestination, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
    CheckedStructuralCallReturnMachinePlan, CheckedStructuralReturnMachinePlan,
    CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedTerminalMachineDebugPlan, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, ContentIdentityReshuffleFact, ContentPartitionCompositionFact,
    types::PrimitiveType,
};
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use numerics::{
    arithmetic::ArithmeticDomain,
    integer_policy::{IntegerPolicyPrimitive, integer_policy_bridge},
};
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ByteSequenceStructuralField, CanonicalStructuralPathSegment,
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    ContractId, DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId, IeeeFloatFormat,
    IeeeFloatStructuralField, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, PropositionContext, PropositionError,
    PropositionId, ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralCaseSubject,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_codec::{
    DebugFileId, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    TerminalDebugMap, source_digest, terminal_psi_identity, validate_debug_map,
};
use terminal_psi::{
    Block, BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryStructuralResultDeclaration, ByteSequenceCarrier, ClaimContentProjection,
    ClaimTransfer, CompletionReceipt, ContentConservationGuarantee, ContentEntryClaim,
    ContentIdentityReshuffle, ContentPartitionComposition, ContentPlaceSubstitution,
    ContractClause, CrashCause as TerminalCrashCause, EntryClaim, EvidenceContractLane,
    EvidenceContractLaneKind, EvidenceInterfaceIdentity, EvidenceProjectionIdentity,
    EvidenceRequirementIdentity, EvidenceTermDeclaration, InstallationReachDependency,
    MachineContract, NominalAffineCleanup, Operation, OperationKind, OperationResult,
    OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificCallResultSubstitution, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificEvidenceUse, OutcomeSpecificGuard, ProgramLocalRootIntroductionSchema,
    ProofOutput, ProofOutputCall, ProofOutputRuntimeCall, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderSignatureParameter,
    ProviderUnitRefinement, ProviderUnitSignature, ServiceDeclaration, StaticRequirementDispatch,
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralCaseDeclaration,
    StructuralCaseSuccessorEdge, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalPlacedViewInput,
    TerminalRankedGuard, TerminalRankedScc, TerminalRankedSccEdge, TerminalRankedSuccessorArgument,
    Terminator, ValueDeclaration, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};
#[cfg(test)]
use terminal_verifier::reconstruct_operation_obligations;
use terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence, ProofBundle,
};

mod attached_unit;
mod boolean_control;
mod boundary_scalar_return;
mod call_source_custody;
mod conformance_applications;
mod content_conservation;
mod crash_routes;
mod debug_map;
mod dynamic_composed_unit;
mod evidence_lowering;
mod float_meaning_projection;
mod machine_dispatch;
mod nonzero_divisor_certificate;
pub use nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
mod operation_emission;
mod payloadless_case_return;
mod payloadless_guarded_call_return;
mod proof_recursion;
mod quotient_correspondence;
mod reborrow_restored_call_use;
mod reborrow_root_handoff;
mod retained_borrow_custody;
mod scalar_bindings;
mod scalar_call_closure;
mod scalar_computations;
mod scalar_graph_lowering;
mod scalar_graph_module;
mod scalar_source_custody;
mod shared_runtime_parameters;
mod structural_call_return;
mod structural_return;
mod structural_scalar_return;
mod structural_scalar_store;
mod structural_types;
mod structural_unit_control;
mod suspension_call_plan;
mod unit_cleanup;
use attached_unit::lower_root_service_reach;
use attached_unit::{
    checked_unit_boundary_identity, checked_unit_call_closure_including,
    checked_unit_target_reach_matches, collect_installation_machine_contract_services,
    collect_published_contract_services, collect_service_summary,
    lower_attached_unit_closure_including, lower_installation_machine_service_ceiling,
    lower_published_service_ceiling, lower_structural_arguments, lower_structural_path,
    lower_unit_effect_closure, lower_unit_parameters, unique_unit_machine, validate_transfer_shape,
};
#[cfg(test)]
use attached_unit::{collect_contract_services, lower_contract_service_ceiling};
use boolean_control::{
    bind_boolean_decision, boolean_decision_block_count, boolean_decision_test_count,
    build_scalar_conditional_target, emit_inlined_boolean_guard_blocks,
    emit_inlined_boolean_value_blocks, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_control_decision, lower_boolean_value_decision, scalar_source_block,
};
use conformance_applications::lower_closed_conformance_applications;
use content_conservation::{RESULT_STRUCTURAL_PLACE_ID, merge_content_place_declaration};
pub use content_conservation::{
    lower_boundary_content_guarantees, lower_content_conservation_plan,
    lower_content_identity_reshuffles, lower_content_partition_compositions,
};
#[cfg(test)]
use crash_routes::{checked_boolean_proposition, lower_checked_crash_frontier};
use crash_routes::{
    lower_checked_crash_exit, lower_checked_crash_predicates, lower_checked_crash_route_buckets,
    lower_checked_crash_routes, lower_structural_crash_route_buckets,
    lower_structural_runtime_requirement, structural_crash_route_argument_prefix,
    substitute_structural_crash_route_roots,
};
use debug_map::build_debug_map;
use evidence_lowering::lower_and_install_evidence_artifacts;
use float_meaning_projection::resolve_direct_float_source_binding;
pub use float_meaning_projection::{
    FloatMeaningProjectionLoweringError, lower_float_meaning_equality,
    lower_float_meaning_projection,
};
pub use machine_dispatch::select_terminal_machine;
use machine_dispatch::{LoweredSelectedMachine, SelectedMachineRoute, lower_selected_machine};
use operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
    emit_staged_scalar_call_binding, finalize_operation_proofs,
};
use payloadless_case_return::lower_payloadless_case_return_machine;
use proof_recursion::lower_and_install_proof_recursion;
pub use quotient_correspondence::install_non_executable_quotient_correspondences;
use scalar_call_closure::checked_scalar_call_closure;
use scalar_graph_lowering::{
    KnownDirectScalar, contains_short_circuit, direct_expression_contains_short_circuit,
    integer_landing_scalar_type, integer_scalar_type, integer_value,
    lower_checked_scalar_expression, lower_checked_scalar_expression_at,
    lower_selected_scalar_graph_machine, prepare_embedded_scalar_graph_machine,
    prepare_scalar_graph_machine, staged_short_circuit_bindings_terminator, terminal_scalar_type,
    validate_boolean_parameter_types, validate_direct_parameter_types,
};
use scalar_graph_module::build_scalar_graph_module;
use shared_runtime_parameters::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};
use structural_return::lower_structural_return_machine;
use structural_types::{
    lower_mixed_cases, lower_mixed_fields, lower_structural_type_plans,
    retain_additional_structural_types, terminal_byte_sequence_carrier,
    terminal_structural_field_type,
};
use unit_cleanup::lower_nominal_affine_unit_cleanup_machine;

#[cfg(test)]
use structural_unit_control::lower_structural_unit_control_machine;
#[cfg(test)]
use unit_cleanup::lower_partial_affine_unit_cleanup_machine;

use lowered_psi::{
    CallbackTerminalLoweringReceipt, LoweredCallbackPsi, LoweredPsi,
    LoweredSelectedIeeeFloatFmaOccurrence, LoweredSourceCallOccurrence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredDirectExpression {
    Parameter {
        position: usize,
        scalar_type: ScalarType,
    },
    Local {
        position: usize,
        scalar_type: ScalarType,
    },
    IntegerLiteral {
        value: IntegerValue,
        scalar_type: ScalarType,
    },
    IeeeFloatLiteral {
        value: IeeeFloatValue,
    },
    IntegerBinary {
        kind: LoweredIntegerBinaryKind,
        scalar_type: ScalarType,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    IntegerBitwiseNot {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerWiden {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    IntegerExactCast {
        scalar_type: ScalarType,
        operand: Box<LoweredDirectExpression>,
    },
    Boolean {
        expression: Box<LoweredBooleanReturnExpression>,
    },
}

impl LoweredDirectExpression {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Parameter { scalar_type, .. }
            | Self::Local { scalar_type, .. }
            | Self::IntegerLiteral { scalar_type, .. }
            | Self::IntegerBinary { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerWiden { scalar_type, .. }
            | Self::IntegerExactCast { scalar_type, .. } => *scalar_type,
            Self::IeeeFloatLiteral { value } => ScalarType::IeeeFloat(value.format()),
            Self::Boolean { .. } => ScalarType::Boolean,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanReturnExpression {
    Constant {
        value: bool,
    },
    Parameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    UnresolvedStructuralParameterField {
        parameter_position: u32,
        path: Vec<String>,
    },
    StructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
    Not {
        operand: Box<LoweredBooleanReturnExpression>,
    },
    Equal {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    IntegerComparison {
        kind: LoweredIntegerComparisonKind,
        left: Box<LoweredDirectExpression>,
        right: Box<LoweredDirectExpression>,
    },
    And {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
    Or {
        left: Box<LoweredBooleanReturnExpression>,
        right: Box<LoweredBooleanReturnExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

impl LoweredIntegerComparisonKind {
    const fn operation(self, left: ValueId, right: ValueId) -> OperationKind {
        match self {
            Self::Equal => OperationKind::IntegerEqual { left, right },
            Self::LessThan => OperationKind::IntegerLessThan { left, right },
            Self::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredBooleanDecision {
    Value(LoweredBooleanReturnExpression),
    Test {
        condition: LoweredBooleanReturnExpression,
        when_true: Box<LoweredBooleanDecision>,
        when_false: Box<LoweredBooleanDecision>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredBooleanDecisionExit {
    Return,
    Jump { target: BlockId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoweredIntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredScalarBranchTerminator {
    Jump {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
    },
    Conditional {
        condition: LoweredBooleanReturnExpression,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredDirectExpression>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredDirectExpression>,
    },
    Return {
        expression: LoweredDirectExpression,
    },
    Crash(LoweredCrashExit),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredCrashExit {
    cause: TerminalCrashCause,
    site_guard: Vec<CheckedBooleanExpression>,
    frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredScalarBranchState {
    parameter_types: Vec<ScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    terminator: LoweredScalarBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoweredScalarBinding {
    Expression(LoweredDirectExpression),
    DirectCall(LoweredDirectCallBinding),
}

impl LoweredScalarBinding {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Expression(expression) => expression.scalar_type(),
            Self::DirectCall(call) => call.result_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredDirectCallBinding {
    source_coordinate: SourceCallCoordinate,
    target_machine: symbols::SymbolHandle,
    result_type: ScalarType,
    arguments: Vec<LoweredDirectExpression>,
    crash_continuations: Vec<checked_trees::CrashRouteBucket>,
    parameter_relative_crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarCallCrashScope {
    CallerValues,
    Arguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceCallCoordinate {
    state: symbols::SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
}

struct PreparedScalarMachine {
    source_machine: symbols::SymbolHandle,
    states: Vec<LoweredScalarBranchState>,
    result_type: ScalarType,
    contract: PreparedScalarContract,
    crash_routes: Vec<checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
}

enum PreparedScalarContract {
    Empty,
    ClosedLiteral(KnownDirectScalar),
    Predicates(ClosedScalarValueContractPlan),
}

impl PreparedScalarContract {
    fn requirement_count(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::ClosedLiteral(_) => 1,
            // The predicate plan's source clauses are published as one
            // canonical conjunction, including implicit parameter ranges.
            Self::Predicates(plan) => usize::from(!plan.requires().is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingConditionalBindingBlock {
    id: BlockId,
    parameters: Vec<ValueDeclaration>,
    target: BlockId,
    arguments: Vec<LoweredDirectExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMixedTupleBindingBlocks {
    first_id: BlockId,
    original_parameter_count: usize,
    arguments: Vec<LoweredDirectExpression>,
    stage_parameters: Vec<Vec<ValueDeclaration>>,
    target: BlockId,
}

const TERMINAL_MACHINE_IDENTITY_STRIDE: u64 = 1_u64 << 32;
// Structural Unit call requirements occupy the upper half of the module-wide
// obligation namespace. Existing contract and cleanup producers allocate from
// the lower half, so composing their proof bundles cannot alias a call site.
const TERMINAL_UNIT_CALL_OBLIGATION_BASE: u64 = 1_u64 << 63;

/// Module-wide operation identities for one machine namespace. Machine zero
/// uses the historical one-based range; additional machines receive disjoint
/// ranges when source call-closure production composes them.
struct OperationBuffer {
    next_identity: u64,
    operations: Vec<Operation>,
    source_calls: Vec<LoweredSourceCallOccurrence>,
    selected_ieee_float_fmas: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
}

impl OperationBuffer {
    fn new(identity_base: u64) -> Self {
        Self {
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
            source_calls: Vec::new(),
            selected_ieee_float_fmas: Vec::new(),
        }
    }

    fn allocate(&mut self) -> OperationId {
        let id = operation_id(self.next_identity);
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .expect("terminal operation identities advance");
        id
    }

    fn record_source_call(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        self.record_source_call_with_values(coordinate, source_site, operation, target, &[])
    }

    fn record_source_call_with_values(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
        source_values_before_call: &[ValueDeclaration],
    ) -> Result<(), LoweringError> {
        if self.source_calls.iter().any(|existing| {
            existing.source_state == coordinate.state
                && existing.statement_index == coordinate.statement_index
                && existing.call_ordinal == coordinate.call_ordinal
        }) {
            return Err(LoweringError::DuplicateContentPartitionProducerCoordinate);
        }
        self.source_calls.push(LoweredSourceCallOccurrence {
            source_site,
            source_state: coordinate.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
            terminal_operation: operation,
            source_target: target,
            source_values_before_call: source_values_before_call.to_vec(),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_selected_ieee_float_fma(
        &mut self,
        coordinate: SourceCallCoordinate,
        operation: OperationId,
        requirement_operator: symbols::SymbolHandle,
        provider_plan_report_fingerprint: u64,
        provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
        format: semantic_vocabulary::IeeeFloatFormat,
    ) -> Result<(), LoweringError> {
        if provider_plan_report_fingerprint == 0 || provider_plan_commitment.is_empty() {
            return unsupported(
                "selected IEEE FMA occurrence lacks complete ProviderPlan evidence",
            );
        }
        if self.selected_ieee_float_fmas.iter().any(|existing| {
            existing.source_state == coordinate.state
                && existing.statement_index == coordinate.statement_index
                && existing.call_ordinal == coordinate.call_ordinal
        }) {
            return unsupported("selected IEEE FMA occurrence coordinate is duplicated");
        }
        self.selected_ieee_float_fmas
            .push(LoweredSelectedIeeeFloatFmaOccurrence {
                source_state: coordinate.state,
                statement_index: coordinate.statement_index,
                call_ordinal: coordinate.call_ordinal,
                terminal_operation: operation,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                format,
            });
        Ok(())
    }
}

impl std::ops::Deref for OperationBuffer {
    type Target = Vec<Operation>;

    fn deref(&self) -> &Self::Target {
        &self.operations
    }
}

impl std::ops::DerefMut for OperationBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.operations
    }
}

struct CallEmissionContext<'a> {
    machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    next_obligation_identity: u64,
    obligation_limit: u64,
}

impl CallEmissionContext<'_> {
    fn allocate_requirement(&mut self) -> Result<ObligationId, LoweringError> {
        if self.next_obligation_identity >= self.obligation_limit {
            return unsupported("terminal call obligations exceed their machine identity range");
        }
        let obligation = obligation_id(self.next_obligation_identity);
        self.next_obligation_identity = self
            .next_obligation_identity
            .checked_add(1)
            .expect("terminal call obligation identities advance");
        Ok(obligation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingNestedBlockGroup {
    ConditionalBinding(PendingConditionalBindingBlock),
    TupleBinding(PendingMixedTupleBindingBlocks),
}

impl PendingNestedBlockGroup {
    fn first_id(&self) -> BlockId {
        match self {
            Self::ConditionalBinding(block) => block.id,
            Self::TupleBinding(blocks) => blocks.first_id,
        }
    }
}

impl LoweredIntegerBinaryKind {
    const fn integer_policy_binding(self) -> Option<(IntegerPolicyPrimitive, ArithmeticDomain)> {
        match self {
            Self::WrappingShiftLeft => Some((
                IntegerPolicyPrimitive::ShiftLeft,
                ArithmeticDomain::Wrapping,
            )),
            Self::WrappingShiftRight => Some((
                IntegerPolicyPrimitive::ShiftRight,
                ArithmeticDomain::Wrapping,
            )),
            Self::ExactShiftLeft => {
                Some((IntegerPolicyPrimitive::ShiftLeft, ArithmeticDomain::Exact))
            }
            Self::ExactShiftRight => {
                Some((IntegerPolicyPrimitive::ShiftRight, ArithmeticDomain::Exact))
            }
            Self::ExactAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Exact)),
            Self::ExactSubtract => {
                Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact))
            }
            Self::ExactMultiply => {
                Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Exact))
            }
            Self::ExactDivide => Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Exact)),
            Self::WrappingDivide => {
                Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingDivide => {
                Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating))
            }
            Self::ExactRemainder => {
                Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact))
            }
            Self::WrappingRemainder => Some((
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Wrapping,
            )),
            Self::SaturatingRemainder => Some((
                IntegerPolicyPrimitive::Remainder,
                ArithmeticDomain::Saturating,
            )),
            Self::WrappingAdd => Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Wrapping)),
            Self::SaturatingAdd => {
                Some((IntegerPolicyPrimitive::Add, ArithmeticDomain::Saturating))
            }
            Self::WrappingSubtract => {
                Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingSubtract => Some((
                IntegerPolicyPrimitive::Subtract,
                ArithmeticDomain::Saturating,
            )),
            Self::WrappingMultiply => {
                Some((IntegerPolicyPrimitive::Multiply, ArithmeticDomain::Wrapping))
            }
            Self::SaturatingMultiply => Some((
                IntegerPolicyPrimitive::Multiply,
                ArithmeticDomain::Saturating,
            )),
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => None,
        }
    }

    fn formation_obligation(self, operation: OperationId) -> Option<ObligationId> {
        let catalog_requires_formation =
            self.integer_policy_binding()
                .is_some_and(|(primitive, policy)| {
                    !integer_policy_bridge(primitive, policy)
                        .formation_conditions
                        .is_empty()
                });
        catalog_requires_formation.then(|| {
            obligation_id(
                operation
                    .get()
                    .checked_add(1)
                    .expect("integer formation obligation follows its operation identity"),
            )
        })
    }

    fn operation(self, operation: OperationId, left: ValueId, right: ValueId) -> OperationKind {
        let formation_obligation = self.formation_obligation(operation);
        match self {
            Self::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Self::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Self::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Self::WrappingShiftLeft => OperationKind::WrappingIntegerShiftLeft {
                value: left,
                count: right,
            },
            Self::WrappingShiftRight => OperationKind::WrappingIntegerShiftRight {
                value: left,
                count: right,
            },
            Self::ExactShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: formation_obligation.expect("exact shift has formation conditions"),
            },
            Self::ExactShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: formation_obligation.expect("exact shift has formation conditions"),
            },
            Self::ExactAdd => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: formation_obligation.expect("exact add has formation conditions"),
            },
            Self::ExactSubtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: formation_obligation.expect("exact subtract has formation conditions"),
            },
            Self::ExactMultiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: formation_obligation.expect("exact multiply has formation conditions"),
            },
            Self::ExactDivide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: formation_obligation.expect("exact divide has formation conditions"),
            },
            Self::ExactRemainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: formation_obligation.expect("exact remainder retains its obligation"),
            },
            Self::WrappingDivide => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: formation_obligation.expect("wrapping divide has formation conditions"),
            },
            Self::WrappingRemainder => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: formation_obligation
                    .expect("wrapping remainder retains its obligation"),
            },
            Self::SaturatingDivide => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: formation_obligation
                    .expect("saturating divide has formation conditions"),
            },
            Self::SaturatingRemainder => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: formation_obligation
                    .expect("saturating remainder retains its obligation"),
            },
            Self::WrappingAdd => OperationKind::WrappingIntegerAdd { left, right },
            Self::SaturatingAdd => OperationKind::SaturatingIntegerAdd { left, right },
            Self::WrappingSubtract => OperationKind::WrappingIntegerSubtract { left, right },
            Self::SaturatingSubtract => OperationKind::SaturatingIntegerSubtract { left, right },
            Self::WrappingMultiply => OperationKind::WrappingIntegerMultiply { left, right },
            Self::SaturatingMultiply => OperationKind::SaturatingIntegerMultiply { left, right },
        }
    }
}

/// One checked content equation translated into terminal-Psi identities.
/// Arena-local domain, projection-machine, and field symbols are deliberately
/// absent; only normalized semantic identities and stable spellings survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentConservation {
    /// Non-authoritative compact coordinate beside the exact proposition.
    pub source_report_fingerprint: u64,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub proposition: Proposition,
}

/// Canonical terminal-Psi carrier for checker-derived one-to-one claim
/// reshuffles. Source claim identities are used only to group exact projection
/// facts; the emitted IDs are dense and determined by the semantic rows, so no
/// arena-local symbol identity crosses the terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentIdentityReshuffles {
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<ContentEntryClaim>,
    pub reshuffles: Vec<ContentIdentityReshuffle>,
    /// Source checked identities paired with their dense terminal IDs.
    /// This map never enters terminal Psi; later derived rows consume it while
    /// the producer still owns both representations.
    pub source_claims: Vec<(PermissionClaimIdentity, ClaimId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentPartitionCompositions {
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub compositions: Vec<LoweredContentPartitionComposition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredContentPartitionComposition {
    producer_coordinate: SourceCallCoordinate,
    source_callable: symbols::SymbolHandle,
    /// Non-authoritative compact coordinate beside the exact retained source.
    source_report_fingerprint: u64,
    source_structural_places: Vec<StructuralPlaceDeclaration>,
    source: ContentConservation,
    input_claims: Vec<ClaimId>,
    substitutions: Vec<ContentPlaceSubstitution>,
    derived: ContentConservation,
}

/// Lower one named checked free machine and its reachable checked scalar callees
/// through the current terminal-Psi source slice.
///
/// Accepted shape:
///
/// ```text
/// machine name(p0: bool, ...) -> bool
/// requires B == B
/// ensures B == B
/// {
///     B | pN
/// }
///
/// machine name(p0: bool, ...) -> bool
/// requires B == B
/// ensures B == B
/// {
///     transition { _ -> next(B | pN) }
///     state next(value: bool) -> bool { B | value }
/// }
///
/// machine name(p0: integer, ...) -> integer
/// requires C == C
/// ensures C == C
/// {
///     E
/// }
/// E := pN | L | E (+|-|*) E
///
/// machine name(p0: integer, ...) -> integer
/// requires L == L
/// ensures L == L
/// {
///     transition { _ -> next(E0, E1, ...) }
///     state next(p0: integer, p1: integer, ...) -> integer {
///         transition { _ -> done(E0, E1, ...) }
///     }
///     state done(p0: integer, p1: integer, ...) -> integer { E }
/// }
/// ```
///
/// The first explicit-crash slice also accepts a one-state scalar machine whose
/// sole statement is `crash Cause;` and whose checked site cites a prechecked
/// same-cause route bucket. It emits a distinct terminal-
/// Psi crash terminator; it never reuses ordinary return lowering.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredPsi, LoweringError> {
    attached_unit::validate_direct_unit_parameter_custody(checked)?;
    let selection = select_terminal_machine(checked, machine_name)?;
    let exact_guarded_payloadless = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(selection.machine)
        .is_some();
    let guarded_payloadless_callee = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selection.machine)
        .map(|plan| plan.target_machine);
    if !exact_guarded_payloadless
        && checked
            .facts
            .proof
            .outcome_specific_guarantees
            .iter()
            .any(|(_, guarantee)| guarantee.machine_symbol == selection.machine)
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    let LoweredSelectedMachine {
        terminal: mut lowered,
        route,
        exact_sources,
    } = lower_selected_machine(checked, selection)?;
    let has_exact_source_owners = exact_sources.is_some();
    let source_machines =
        if let Some(sources) = &exact_sources {
            sources.iter().map(|(source, _)| *source).collect()
        } else {
            match route {
                SelectedMachineRoute::GuardedPayloadlessCallReturn { target_machine } => {
                    vec![selection.machine, target_machine]
                }
                SelectedMachineRoute::TraitOperatorScalarReturn {
                    realization_machine,
                } => vec![selection.machine, realization_machine],
                SelectedMachineRoute::SelectedOperatorStructuralScalarReturn {
                    realization_machine,
                } => vec![selection.machine, realization_machine],
                SelectedMachineRoute::DirectDynamicComposedUnit {
                    realization_machine,
                } => vec![selection.machine, realization_machine],
                SelectedMachineRoute::ReboundDynamicComposedUnit {
                    realization_machine,
                } => vec![selection.machine, realization_machine],
                SelectedMachineRoute::StoredDynamicComposedUnit {
                    realization_machine,
                } => vec![selection.machine, realization_machine],
                SelectedMachineRoute::JoinedDynamicComposedUnit {
                    realization_machines,
                } => {
                    let mut machines = vec![selection.machine];
                    machines.extend(realization_machines);
                    machines.sort_by_key(|machine| (machine.arena_index(), machine.generation()));
                    machines.dedup();
                    machines
                }
                SelectedMachineRoute::UnitEffect => {
                    let mut closure =
                        checked_unit_call_closure_including(checked, selection.machine, &[])?;
                    let mut structural_realizations = Vec::new();
                    for source in &closure {
                        let plan = unique_unit_machine(
                            &checked.facts.flow.terminal_unit_effects,
                            *source,
                        )?;
                        for realization in plan.operations.iter().filter_map(|operation| {
                            match operation {
                        CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                            realization_machine,
                            ..
                        } => Some(*realization_machine),
                        _ => None,
                    }
                        }) {
                            if !structural_realizations.contains(&realization) {
                                structural_realizations.push(realization);
                            }
                        }
                    }
                    let requires_complete_unit_closure =
                        !structural_realizations.is_empty()
                            || checked.facts.proof.proof_output_calls.iter().any(
                                |(_, invocation)| {
                                    invocation.caller_machine_symbol == selection.machine
                                        && invocation.static_requirement_dispatch.is_some()
                                },
                            )
                            || (lowered.semantic_module.machines.len() > 1
                                && checked
                                    .machine_specializations
                                    .iter()
                                    .any(|specialization| {
                                        specialization.instance == selection.machine
                                            && !specialization.conformance_applications.is_empty()
                                    }));
                    if requires_complete_unit_closure {
                        closure.extend(structural_realizations);
                        closure
                    } else {
                        vec![selection.machine]
                    }
                }
                SelectedMachineRoute::ScalarGraph => {
                    checked_scalar_call_closure(checked, selection.machine)?
                }
                SelectedMachineRoute::StructuralScalarReturn
                | SelectedMachineRoute::NominalAffineUnitCleanup
                | SelectedMachineRoute::PartialAffineUnitCleanup
                | SelectedMachineRoute::BoundaryScalarReturn
                | SelectedMachineRoute::StructuralCallReturn
                | SelectedMachineRoute::PayloadlessCaseReturn
                | SelectedMachineRoute::StructuralReturn
                | SelectedMachineRoute::ComposedAttachedUnit
                | SelectedMachineRoute::StructuralUnitControl => vec![selection.machine],
            }
        };
    let direct_float_source_machines = if let Some(sources) = exact_sources {
        sources
    } else if route == SelectedMachineRoute::ScalarGraph {
        if source_machines.len() != lowered.semantic_module.machines.len() {
            return unsupported(
                "scalar call closure source and Terminal machine tables must correspond exactly",
            );
        }
        source_machines
            .iter()
            .copied()
            .zip(
                lowered
                    .semantic_module
                    .machines
                    .iter()
                    .map(|machine| machine.id),
            )
            .collect::<Vec<_>>()
    } else {
        vec![(selection.machine, lowered.semantic_module.entry)]
    };
    retain_selected_placed_view_inputs(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &mut lowered.semantic_module,
    )?;
    reborrow_root_handoff::retain_selected_reborrow_root_handoffs(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &mut lowered.semantic_module.reborrow_root_handoffs,
    )?;
    reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &lowered.source_call_occurrences,
        &lowered.semantic_module.machines,
        &mut lowered.semantic_module.reborrow_restored_call_uses,
    )?;
    suspension_call_plan::retain_suspension_call_plans(
        checked,
        &source_machines,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retained_borrow_custody::retain_foreign_borrow_custodies(
        checked,
        &mut lowered.semantic_module,
    )?;
    if checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .any(|(_, guarantee)| {
            source_machines.contains(&guarantee.machine_symbol)
                && !((exact_guarded_payloadless
                    && source_machines.as_slice() == [selection.machine]
                    && guarantee.machine_symbol == selection.machine)
                    || guarded_payloadless_callee == Some(guarantee.machine_symbol))
        })
    {
        return unsupported(
            "outcome-specific guarantees require guarded exit and caller-arm lowering",
        );
    }
    let dynamic_root_application_count = lowered
        .semantic_module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            !has_exact_source_owners || application.owner == lowered.semantic_module.entry
        })
        .count();
    if matches!(
        route,
        SelectedMachineRoute::DirectDynamicComposedUnit { .. }
            | SelectedMachineRoute::StoredDynamicComposedUnit { .. }
    ) {
        if dynamic_root_application_count != 1 {
            return unsupported(
                "direct dynamic dispatch must publish exactly one closed conformance application",
            );
        }
    } else if matches!(
        route,
        SelectedMachineRoute::ReboundDynamicComposedUnit { .. }
    ) {
        if !(1..=2).contains(&dynamic_root_application_count) {
            return unsupported(
                "rebound dynamic dispatch must publish one or two closed conformance applications",
            );
        }
    } else if matches!(
        route,
        SelectedMachineRoute::JoinedDynamicComposedUnit { .. }
    ) {
        if !(1..=2).contains(
            &lowered
                .semantic_module
                .closed_conformance_applications
                .len(),
        ) {
            return unsupported(
                "joined dynamic dispatch must publish one or two closed conformance applications",
            );
        }
    } else {
        lower_closed_conformance_applications(
            checked,
            &source_machines,
            &mut lowered.semantic_module,
        )?;
    }
    if has_exact_source_owners
        && matches!(
            route,
            SelectedMachineRoute::DirectDynamicComposedUnit { .. }
                | SelectedMachineRoute::ReboundDynamicComposedUnit { .. }
                | SelectedMachineRoute::StoredDynamicComposedUnit { .. }
        )
    {
        conformance_applications::append_closed_conformance_applications_excluding(
            checked,
            &direct_float_source_machines,
            selection.machine,
            &mut lowered.semantic_module,
        )?;
    }
    lowered.semantic_module.float_meaning_projections = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .cloned()
        .map(|projection| {
            let direct_source = resolve_direct_float_source_binding(
                checked,
                &direct_float_source_machines,
                &lowered.semantic_module.machines,
                &lowered.semantic_module.structural_types,
                projection.clone(),
            )?;
            lower_float_meaning_projection(projection, direct_source)
                .map_err(LoweringError::InvalidFloatMeaningProjection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.semantic_module.float_meaning_equalities = checked
        .facts
        .proof
        .float_meaning_equalities
        .iter()
        .copied()
        .map(lower_float_meaning_equality)
        .collect();
    lower_and_install_evidence_artifacts(checked, selection.machine, &mut lowered)?;
    lower_and_install_proof_recursion(
        checked,
        &source_machines,
        &mut lowered.semantic_module,
        &mut lowered.proof_bundle,
    )?;
    // Unit closures can be provisional inputs to cleanup/borrow assembly.
    // Discharge operand obligations only after the selected module is complete.
    if matches!(
        route,
        SelectedMachineRoute::UnitEffect
            | SelectedMachineRoute::NominalAffineUnitCleanup
            | SelectedMachineRoute::PartialAffineUnitCleanup
    ) {
        finalize_operation_proofs(&mut lowered)?;
    } else if lowered
        .semantic_module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    } else {
        terminal_verifier::validate_module(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    }
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && route != SelectedMachineRoute::UnitEffect
    {
        checked
            .facts
            .flow
            .terminal_debug
            .for_machine(selection.machine)
            .map(|plan| build_debug_map(plan, &lowered.semantic_module))
            .transpose()?
    } else {
        None
    };
    Ok(lowered)
}

/// Lower the exact first callback-body cohort without treating a nominally
/// attached static machine as an attached Unit program root.
///
/// This entry point is deliberately narrower than general machine lowering:
/// one checked `u64 -> u64` scalar graph, one selected entry, no local
/// operations, and an identity return. The callback placement separately owns
/// its satisfaction and ABI evidence; this producer owns only executable body
/// semantics and the checked-to-Terminal coordinate join.
pub fn lower_bounded_callback_identity_machine(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    source_entry: symbols::SymbolHandle,
) -> Result<LoweredCallbackPsi, LoweringError> {
    let matching_selection_count = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == source_machine)
        .count();
    if matching_selection_count != 1 {
        return unsupported(
            "bounded callback body must name one exact checked Terminal machine selection",
        );
    }
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "bounded callback body has no checked scalar graph",
        ))?;
    let [state] = graph.states.as_slice() else {
        return unsupported("bounded callback body must contain exactly one checked state");
    };
    if state.state != source_entry
        || state.parameter_types.as_slice() != [PrimitiveType::U64]
        || state.result_type != PrimitiveType::U64
        || !state.bindings.is_empty()
        || !matches!(
            state.terminator,
            CheckedScalarStateTerminator::Return {
                statement_ordinal: 0
            }
        )
    {
        return unsupported(
            "bounded callback body must be the exact one-state u64 identity-return cohort",
        );
    }
    let lowered = lower_selected_scalar_graph_machine(checked, source_machine, graph)?;
    terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        return unsupported("bounded callback lowering did not produce one Terminal machine");
    };
    Ok(LoweredCallbackPsi {
        receipt: CallbackTerminalLoweringReceipt {
            source_machine,
            source_entry,
            terminal_machine: machine.id,
            terminal_entry: machine.entry,
        },
        terminal: lowered,
    })
}

fn retain_selected_placed_view_inputs(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    for input in checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == source_machine)
    {
        let lowered = lower_placed_view_input(checked, input, terminal_machine)?;
        if let Some(existing) = module.placed_view_inputs.iter().find(|existing| {
            (
                existing.machine,
                &existing.source_state_identity,
                existing.position,
            ) == (
                lowered.machine,
                &lowered.source_state_identity,
                lowered.position,
            )
        }) {
            if existing != &lowered {
                return unsupported("placed-view input custody disagrees across Terminal lowering");
            }
        } else {
            module.placed_view_inputs.push(lowered);
        }
    }
    module.placed_view_inputs.sort();
    Ok(())
}

fn lower_placed_view_input(
    checked: &CheckedTrees,
    input: &checked_trees::CheckedPlacedViewInput,
    machine: MachineId,
) -> Result<TerminalPlacedViewInput, LoweringError> {
    let identity = |symbol, missing| {
        checked
            .typed
            .normalized_hermetic_symbol_identity(symbol)
            .map_err(|_| LoweringError::Unsupported(missing))
    };
    let access = match input.reference_access {
        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    };
    let policy_identity = identity(
        input.policy,
        "placed-view policy has no hermetic declaration identity",
    )?;
    let schema_identity = identity(
        input.schema,
        "placed-view schema has no hermetic declaration identity",
    )?;
    let view_identity =
        terminal_psi::canonical_placed_view_identity(&policy_identity, &schema_identity);
    Ok(TerminalPlacedViewInput {
        machine,
        position: input.position,
        source_machine_identity: identity(
            input.machine,
            "placed-view consumer has no hermetic declaration identity",
        )?,
        source_state_identity: identity(
            input.state,
            "placed-view state has no hermetic declaration identity",
        )?,
        source_parameter_identity: identity(
            input.parameter,
            "placed-view parameter has no hermetic declaration identity",
        )?,
        access,
        binding_is_const: input.binding_is_const,
        binding_is_mutable: input.binding_is_mutable,
        view_identity,
        policy_identity,
        policy_plan_machine_identity: identity(
            input.policy_plan_machine,
            "placed-view plan machine has no hermetic declaration identity",
        )?,
        schema_identity,
        placement_report_fingerprint: input.placement.identity().compatibility_fingerprint(),
        placement_commitment: input.placement.content_interpretation().commitment(),
    })
}

fn lookup_type_id(
    ids: &[(String, StructuralTypeId)],
    identity: &str,
) -> Result<StructuralTypeId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (candidate == identity).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural type",
        ))
}

fn lookup_domain_id(
    ids: &[(language_semantics::SemanticDomainId, StructuralDomainId)],
    source: language_semantics::SemanticDomainId,
) -> Result<StructuralDomainId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural domain",
        ))
}

fn lookup_service_id(
    ids: &[(ServiceReachId, ServiceId)],
    source: ServiceReachId,
) -> Result<ServiceId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered checked service",
        ))
}

fn lookup_machine_id(
    ids: &[(symbols::SymbolHandle, MachineId)],
    source: symbols::SymbolHandle,
) -> Result<MachineId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit call target is absent from the lowered closure",
        ))
}

fn lookup_claim_id(
    ids: &[(PermissionClaimIdentity, ClaimId)],
    source: PermissionClaimIdentity,
) -> Result<ClaimId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit claim transfer references a non-entry caller claim",
        ))
}

fn dense_identity(index: usize) -> Result<u64, LoweringError> {
    u64::try_from(index)
        .map_err(|_| LoweringError::Unsupported("terminal Unit identity count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "terminal Unit identity count exceeds u64",
        ))
}

fn allocate_dense(next: &mut u64) -> Result<u64, LoweringError> {
    let current = *next;
    *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
        "terminal Unit identity count exceeds u64",
    ))?;
    Ok(current)
}

fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    DebugSourceFileCountOverflow,
    DebugSourceLengthOverflow,
    MissingDebugSourceFile(usize),
    DebugSemanticCodec(terminal_codec::CodecError),
    InvalidDebugMap(terminal_codec::DebugMapError),
    InvalidTerminalModule(terminal_verifier::ModuleError),
    InvalidFloatMeaningProjection(FloatMeaningProjectionLoweringError),
    InvalidQuotientCorrespondence(Vec<String>),
    OperationProofUnavailable(ObligationId),
    Unsupported(&'static str),
    InvalidPsiIntegerType,
    UnlandedIntegerLiteral,
    IntegerLandingMismatch,
    IntegerLiteralOutsideSupportedMagnitude,
    IntegerLiteralOutsidePsiType,
    ContentConservationFingerprintMismatch {
        expected: u64,
        actual: u64,
    },
    ContentIdentityFactOwnerMismatch,
    ContentPartitionFactOwnerMismatch,
    ContentPartitionNotConservation,
    ContentPartitionInputClaimNotLowered,
    ContentPartitionInputClaimBindingMismatch,
    ContentEntryClaimRequiresEntryPlace,
    ContentEntryClaimHasNoProjection,
    ContentEntryClaimMapsMultiplePlaces,
    DuplicateContentEntryClaimInput,
    OverlappingContentEntryClaimInput,
    DuplicateContentPartitionSubstitution,
    DuplicateContentPartitionComposition,
    DuplicateContentPartitionProducerCoordinate,
    ContentPartitionProducerOperationMissing,
    ContentPartitionProducerTargetMismatch,
    ContentPartitionResultRewriteUnsupported,
    ContentPartitionDerivedSourceUnsupported,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    UnknownContentClaimIdentity,
    ContentIdentityInputParameterMismatch,
    ContentIdentityNotDirectEquality,
    ContentIdentityProjectionMismatch,
    ContentIdentityDirectionMismatch,
    ContentIdentityRootMismatch,
    ContentIdentityClaimMapsMultiplePlaces,
    DuplicateContentIdentityProjection,
    DuplicateContentIdentityInput,
    DuplicateContentIdentityOutput,
    OverlappingContentIdentityInput,
    OverlappingContentIdentityOutput,
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    CrashFrontierClaimNotLowered(PermissionClaimIdentity),
    InvalidContentDomainIdentity,
    ZeroContentProjectionFingerprint,
    ContentTermNestingTooDeep,
    ConflictingContentPlaceRoot {
        id: PlaceId,
        first: StructuralPlaceKind,
        second: StructuralPlaceKind,
    },
    InvalidContentProposition(PropositionError),
    InvalidCrashPredicate(PropositionError),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests;
