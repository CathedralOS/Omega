#![forbid(unsafe_code)]

//! Psi checked-semantics to terminal-Psi lowering.
//!
//! This producer proves a real source program can cross the terminal boundary
//! without retaining source trees. Its executable surface is deliberately tiny
//! and exact; unsupported source constructs fail closed instead of being
//! dropped. Checked content conservation, identity reshuffles, and direct
//! partition composition lower into the corresponding terminal-Psi evidence.

use std::collections::{BTreeMap, BTreeSet};

use psi_checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind, CheckedPropositionEvidence,
    CheckedScalarBindingValue, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedScalarMachineGraph, CheckedScalarStateTerminator, CheckedScalarSuccessor,
    CheckedStructuralCallReturnMachinePlan, CheckedStructuralReturnMachinePlan,
    CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedTerminalMachineDebugPlan, CheckedTerminalMachineSelection,
    CheckedTerminalSignatureEligibility, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEntryClaimPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ClosedScalarContractValue, ClosedScalarValueContractPlan,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact, types::PrimitiveType,
};
use psi_core::{
    BlockId, BoundaryMachineId, ByteSequenceStructuralField, CanonicalStructuralPathSegment,
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    ContractId, DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PropositionContext, PropositionError, PropositionId,
    ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralCaseSubject, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_fingerprint,
};
use psi_language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use psi_numerics::{
    arithmetic::ArithmeticDomain,
    integer_policy::{IntegerPolicyPrimitive, integer_policy_bridge},
};
use psi_proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimContentProjection, ClaimTransfer,
    CompletionReceipt, ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause,
    CrashCause as TerminalCrashCause, EntryClaim, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, InstallationReachDependency, MachineContract, NominalAffineCleanup,
    Operation, OperationKind, OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
    ProgramLocalRootIntroductionSchema, ProofOutput, ProofOutputCall, ProofOutputRuntimeCall,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderSignatureParameter, ProviderUnitRefinement,
    ProviderUnitSignature, ServiceDeclaration, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralCaseDeclaration, StructuralContentProjection,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker, program_local_root_introduction_identity,
};
use psi_terminal_codec::{
    DebugFileId, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    TerminalDebugMap, source_digest, terminal_psi_identity, validate_debug_map,
};
use psi_terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
};

mod attached_unit;
mod boolean_control;
mod boundary_scalar_return;
mod conformance_applications;
mod content_conservation;
mod crash_routes;
mod debug_map;
mod evidence_lowering;
mod float_meaning_projection;
mod nonzero_divisor_certificate;
mod operation_emission;
mod payloadless_case_return;
mod payloadless_guarded_call_return;
mod quotient_correspondence;
mod scalar_call_closure;
mod scalar_graph_lowering;
mod scalar_graph_module;
mod shared_runtime_parameters;
mod structural_call_return;
mod structural_return;
mod structural_scalar_return;
mod structural_types;
mod structural_unit_control;
mod unit_cleanup;
use attached_unit::lower_root_service_reach;
use attached_unit::{
    checked_unit_boundary_identity, checked_unit_call_closure_including,
    checked_unit_target_reach_matches, collect_contract_services,
    collect_installation_machine_contract_services, collect_service_summary,
    lower_attached_unit_closure, lower_attached_unit_closure_including,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_structural_path, lower_unit_parameters, unique_unit_machine,
    validate_transfer_shape,
};
use boolean_control::{
    bind_boolean_decision, boolean_decision_block_count, boolean_decision_test_count,
    build_scalar_conditional_target, emit_inlined_boolean_guard_blocks,
    emit_inlined_boolean_value_blocks, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_control_decision, lower_boolean_value_decision, scalar_source_block,
};
use boundary_scalar_return::lower_boundary_scalar_return_machine;
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
    substitute_structural_crash_route_roots, substitute_structural_requirement_roots,
};
use debug_map::build_debug_map;
use evidence_lowering::lower_and_install_evidence_artifacts;
pub use float_meaning_projection::{
    FloatMeaningProjectionLoweringError, lower_float_meaning_equality,
    lower_float_meaning_projection,
};
use operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
    emit_staged_scalar_call_binding, finalize_operation_proofs,
};
use payloadless_case_return::lower_payloadless_case_return_machine;
use payloadless_guarded_call_return::lower_payloadless_guarded_call_return_machine;
pub use quotient_correspondence::install_non_executable_quotient_correspondences;
use scalar_call_closure::{checked_scalar_call_closure, lower_scalar_call_closure};
use scalar_graph_lowering::{
    KnownDirectScalar, contains_short_circuit, direct_expression_contains_short_circuit,
    integer_landing_scalar_type, integer_scalar_type, integer_value,
    lower_checked_scalar_expression, lower_checked_scalar_expression_at,
    lower_scalar_graph_machine, prepare_scalar_graph_machine,
    staged_short_circuit_bindings_terminator, terminal_scalar_type,
    validate_boolean_parameter_types, validate_direct_parameter_types,
};
use scalar_graph_module::build_scalar_graph_module;
use shared_runtime_parameters::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};
use structural_call_return::lower_structural_call_return_machine;
use structural_return::lower_structural_return_machine;
use structural_scalar_return::{
    lower_structural_scalar_return_machine, lower_trait_operator_scalar_return_machine,
};
use structural_types::{
    lower_mixed_cases, lower_mixed_fields, lower_structural_type_plans,
    retain_additional_structural_types, terminal_byte_sequence_carrier,
    terminal_structural_field_type,
};
use structural_unit_control::lower_structural_unit_control_machine;
use unit_cleanup::{
    lower_nominal_affine_unit_cleanup_machine, lower_partial_affine_unit_cleanup_machine,
};

/// Semantic module and separate replaceable proof artifact produced by the
/// Psi frontend producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTerminalPsi {
    pub semantic_module: TerminalModule,
    pub proof_bundle: ProofBundle,
    /// Replaceable presentation metadata. The public producer always fills
    /// this after semantic identities are final; private builders leave it
    /// empty until that finalization step.
    pub debug_map: Option<TerminalDebugMap>,
}

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
    target_machine: psi_symbols::SymbolHandle,
    result_type: ScalarType,
    arguments: Vec<LoweredDirectExpression>,
    crash_continuations: Vec<psi_checked_trees::CrashRouteBucket>,
    parameter_relative_crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceCallCoordinate {
    state: psi_symbols::SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
}

struct PreparedScalarMachine {
    source_machine: psi_symbols::SymbolHandle,
    states: Vec<LoweredScalarBranchState>,
    result_type: ScalarType,
    contract_value: Option<KnownDirectScalar>,
    crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
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
    source_calls: Vec<(SourceCallCoordinate, OperationId, psi_symbols::SymbolHandle)>,
}

impl OperationBuffer {
    fn new(identity_base: u64) -> Self {
        Self {
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
            source_calls: Vec::new(),
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
        operation: OperationId,
        target: psi_symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        if self
            .source_calls
            .iter()
            .any(|(existing, _, _)| *existing == coordinate)
        {
            return Err(LoweringError::DuplicateContentPartitionProducerCoordinate);
        }
        self.source_calls.push((coordinate, operation, target));
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
    machine_ids: &'a [(psi_symbols::SymbolHandle, MachineId)],
    requirement_counts: &'a [(psi_symbols::SymbolHandle, usize)],
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
    pub source_fingerprint: u64,
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
    source_callable: psi_symbols::SymbolHandle,
    source_fingerprint: u64,
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
) -> Result<LoweredTerminalPsi, LoweringError> {
    let mut matches = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|machine| machine.name == machine_name);
    let selection = matches
        .next()
        .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
    if matches.next().is_some() {
        return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
    }
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
    let mut lowered = lower_selected_machine(checked, selection)?;
    let source_machines = if let Some(target_machine) = guarded_payloadless_callee {
        vec![selection.machine, target_machine]
    } else if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_for_machine(selection.machine)
    {
        vec![plan.machine, plan.realization_machine]
    } else if selection.signature == CheckedTerminalSignatureEligibility::Eligible {
        checked_scalar_call_closure(checked, selection.machine)?
    } else {
        vec![selection.machine]
    };
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
    lower_closed_conformance_applications(checked, &source_machines, &mut lowered.semantic_module)?;
    lowered.semantic_module.float_meaning_projections = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .copied()
        .map(lower_float_meaning_projection)
        .collect::<Result<Vec<_>, _>>()
        .map_err(LoweringError::InvalidFloatMeaningProjection)?;
    lowered.semantic_module.float_meaning_equalities = checked
        .facts
        .proof
        .float_meaning_equalities
        .iter()
        .copied()
        .map(lower_float_meaning_equality)
        .collect();
    lower_and_install_evidence_artifacts(checked, selection.machine, &mut lowered)?;
    psi_terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible {
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

/// Produce the canonical source-free artifact consumed at the Psi/Omega seam.
///
/// Unsupported checked constructs fail at lowering; this operation never
/// selects the legacy checked-tree backend as a fallback.
pub fn produce_terminal_artifact(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<psi_terminal_codec::CanonicalTerminalArtifact, TerminalArtifactProductionError> {
    let lowered =
        lower_machine(checked, machine_name).map_err(TerminalArtifactProductionError::Lowering)?;
    psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        lowered.debug_map.as_ref(),
    )
    .map_err(TerminalArtifactProductionError::Artifact)
}

fn lower_selected_machine(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("guarded payloadless call return requires an attached signature");
        }
        return lower_payloadless_guarded_call_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_for_machine(selection.machine)
    {
        return lower_trait_operator_scalar_return_machine(checked, plan);
    }
    // A result-bearing structural plan owns both the scalar result and its
    // post-result cleanup.  It must win over the overlapping Unit-only
    // nominal-cleanup eligibility for the same attached machine.
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural scalar return plan requires an attached signature");
        }
        return lower_structural_scalar_return_machine(checked, plan);
    }
    let mut nominal_matches = checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = nominal_matches.next() {
        if nominal_matches.next().is_some() {
            return unsupported("nominal affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("nominal affine Unit cleanup requires an attached signature");
        }
        return lower_nominal_affine_unit_cleanup_machine(checked, plan);
    }
    let mut partial_matches = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .iter()
        .filter(|plan| plan.machine.machine == selection.machine);
    if let Some(plan) = partial_matches.next() {
        if partial_matches.next().is_some() {
            return unsupported("partial affine Unit cleanup plan is duplicated");
        }
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("partial affine Unit cleanup requires an attached signature");
        }
        return lower_partial_affine_unit_cleanup_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("result-bearing boundary custody requires an attached signature");
        }
        return lower_boundary_scalar_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural call result transfer requires an attached signature");
        }
        return lower_structural_call_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported(
                "payloadless structural case return requires an attached signature",
            );
        }
        return lower_payloadless_case_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_returns
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural result transfer requires an attached signature");
        }
        return lower_structural_return_machine(checked, plan);
    }
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .for_machine(selection.machine)
    {
        if selection.signature != CheckedTerminalSignatureEligibility::Attached {
            return unsupported("structural Unit control plan requires an attached signature");
        }
        return lower_structural_unit_control_machine(checked, plan);
    }
    match selection.signature {
        CheckedTerminalSignatureEligibility::Eligible => {}
        CheckedTerminalSignatureEligibility::Attached => {
            return lower_attached_unit_closure(checked, selection.machine);
        }
        CheckedTerminalSignatureEligibility::Unsupported => {
            return unsupported(
                "machine signature is outside the current terminal-Psi source slice",
            );
        }
    }

    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(selection.machine)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked scalar control plan",
        ))?;
    let closure = checked_scalar_call_closure(checked, selection.machine)?;
    if closure.len() == 1 {
        lower_scalar_graph_machine(checked, selection.machine, graph)
    } else {
        lower_scalar_call_closure(checked, &closure)
    }
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
    ids: &[(psi_language_semantics::SemanticDomainId, StructuralDomainId)],
    source: psi_language_semantics::SemanticDomainId,
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
    ids: &[(psi_symbols::SymbolHandle, MachineId)],
    source: psi_symbols::SymbolHandle,
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
    DebugSemanticCodec(psi_terminal_codec::CodecError),
    InvalidDebugMap(psi_terminal_codec::DebugMapError),
    InvalidTerminalModule(psi_terminal_verifier::ModuleError),
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

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    Lowering(LoweringError),
    Artifact(psi_terminal_codec::CanonicalTerminalArtifactError),
}

impl std::fmt::Display for TerminalArtifactProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactProductionError {}

#[cfg(test)]
mod tests;
