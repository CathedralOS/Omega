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
    CheckedStructuralReturnMachinePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedTerminalMachineDebugPlan,
    CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ClosedScalarContractValue, ClosedScalarValueContractPlan,
    ContentIdentityReshuffleFact, ContentPartitionCompositionFact, types::PrimitiveType,
};
use psi_core::{
    BlockId, BoundaryMachineId, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceIdentity, EvidenceTermId, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionContext,
    PropositionError, PropositionId, ScalarTerm, ScalarType, ServiceId, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_fingerprint,
};
use psi_language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
use psi_proof_kernel::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause as TerminalCrashCause, EntryClaim,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidencePackageInvocation, EvidencePackageOutputBinding, EvidencePackageRuntimeCall,
    EvidenceProjectionIdentity, EvidenceRequirementIdentity, EvidenceTermDeclaration,
    MachineContract, NominalAffineCleanup, Operation, OperationKind,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ProviderCandidateConformance,
    ProviderParameterRefinement, ProviderSignatureParameter, ProviderUnitRefinement,
    ProviderUnitSignature, ServiceDeclaration, StructuralAffineDiscard, StructuralArgument,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
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
mod boundary_scalar_return;
mod content_conservation;
mod crash_routes;
mod evidence_lowering;
mod operation_emission;
mod scalar_graph_module;
mod shared_runtime_parameters;
mod structural_return;
mod structural_scalar_return;
mod structural_types;
mod structural_unit_control;
mod unit_cleanup;
use attached_unit::{
    checked_unit_boundary_identity, checked_unit_call_closure_including,
    checked_unit_target_reach_matches, collect_contract_services, collect_service_summary,
    lower_attached_unit_closure, lower_attached_unit_closure_including,
    lower_published_service_ceiling, lower_structural_arguments, lower_structural_path,
    lower_unit_parameters, unique_unit_machine, validate_transfer_shape,
};
use boundary_scalar_return::lower_boundary_scalar_return_machine;
use content_conservation::{RESULT_STRUCTURAL_PLACE_ID, merge_content_place_declaration};
pub use content_conservation::{
    lower_content_conservation_plan, lower_content_identity_reshuffles,
    lower_content_partition_compositions,
};
#[cfg(test)]
use crash_routes::checked_boolean_proposition;
use crash_routes::{
    lower_checked_crash_predicates, lower_checked_crash_route_buckets,
    lower_structural_crash_route_buckets, lower_structural_runtime_requirement,
    structural_crash_route_argument_prefix, substitute_structural_crash_route_roots,
    substitute_structural_requirement_roots,
};
use evidence_lowering::lower_and_install_evidence_artifacts;
use operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
    emit_staged_scalar_call_binding, finalize_operation_proofs,
};
use scalar_graph_module::build_scalar_graph_module;
use shared_runtime_parameters::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};
use structural_return::lower_structural_return_machine;
use structural_scalar_return::lower_structural_scalar_return_machine;
use structural_types::{lower_structural_type_plans, retain_additional_structural_types};
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
    target_machine: psi_symbols::SymbolHandle,
    result_type: ScalarType,
    arguments: Vec<LoweredDirectExpression>,
    crash_continuations: Vec<psi_checked_trees::CrashRouteBucket>,
    parameter_relative_crash_routes: Vec<psi_checked_trees::CrashRouteBucket>,
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
}

impl OperationBuffer {
    fn new(identity_base: u64) -> Self {
        Self {
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
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
    fn operation(self, operation: OperationId, left: ValueId, right: ValueId) -> OperationKind {
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
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-shift obligation follows its operation identity"),
                ),
            },
            Self::ExactShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-shift obligation follows its operation identity"),
                ),
            },
            Self::ExactAdd => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-add obligation follows its operation identity"),
                ),
            },
            Self::ExactSubtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-subtract obligation follows its operation identity"),
                ),
            },
            Self::ExactMultiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-multiply obligation follows its operation identity"),
                ),
            },
            Self::ExactDivide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-divide obligation follows its operation identity"),
                ),
            },
            Self::ExactRemainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("exact-remainder obligation follows its operation identity"),
                ),
            },
            Self::WrappingDivide => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("wrapping-divide obligation follows its operation identity"),
                ),
            },
            Self::WrappingRemainder => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("wrapping-remainder obligation follows its operation identity"),
                ),
            },
            Self::SaturatingDivide => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("saturating-divide obligation follows its operation identity"),
                ),
            },
            Self::SaturatingRemainder => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: obligation_id(
                    operation
                        .get()
                        .checked_add(1)
                        .expect("saturating-remainder obligation follows its operation identity"),
                ),
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
    pub compositions: Vec<ContentPartitionComposition>,
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
    let mut lowered = lower_selected_machine(checked, selection)?;
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

fn lower_selected_machine(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
) -> Result<LoweredTerminalPsi, LoweringError> {
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

fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    entry: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_symbols::SymbolHandle>, LoweringError> {
    let mut closure = vec![entry];
    let mut next = 0usize;
    while let Some(machine) = closure.get(next).copied() {
        next += 1;
        let selection = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked terminal selection",
            ))?;
        if selection.signature != CheckedTerminalSignatureEligibility::Eligible {
            return unsupported("direct scalar call target has an unsupported terminal signature");
        }
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct scalar call target has no checked scalar graph",
            ))?;
        for target in graph.states.iter().flat_map(|state| {
            state.bindings.iter().filter_map(|binding| {
                let CheckedScalarBindingValue::DirectCall { target_machine, .. } = &binding.value
                else {
                    return None;
                };
                Some(*target_machine)
            })
        }) {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

fn lower_scalar_call_closure(
    checked: &CheckedTrees,
    closure: &[psi_symbols::SymbolHandle],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let prepared = closure
        .iter()
        .map(|machine| {
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(*machine)
                .ok_or(LoweringError::Unsupported(
                    "terminal call-closure machine has no checked scalar graph",
                ))?;
            prepare_scalar_graph_machine(checked, *machine, graph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if prepared.iter().any(|machine| {
        !machine.identity_reshuffles.structural_places.is_empty()
            || !machine.identity_reshuffles.entry_claims.is_empty()
            || !machine.identity_reshuffles.reshuffles.is_empty()
            || !machine.partition_compositions.structural_places.is_empty()
            || !machine.partition_compositions.compositions.is_empty()
    }) {
        return unsupported(
            "structural/content call effects require the terminal content-call slice",
        );
    }
    let machine_ids = prepared
        .iter()
        .enumerate()
        .map(|(index, machine)| {
            Ok((
                machine.source_machine,
                machine_id(
                    u64::try_from(index)
                        .map_err(|_| {
                            LoweringError::Unsupported("terminal call closure exceeds u64")
                        })?
                        .checked_add(1)
                        .expect("terminal machine identities are one-based"),
                ),
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let requirement_counts = prepared
        .iter()
        .map(|machine| {
            (
                machine.source_machine,
                usize::from(machine.contract_value.is_some()),
            )
        })
        .collect::<Vec<_>>();
    let mut machines = Vec::with_capacity(prepared.len());
    let mut evidence = Vec::new();
    for (index, machine) in prepared.into_iter().enumerate() {
        let terminal_machine = machine_ids[index].1;
        let identity_base = u64::try_from(index)
            .map_err(|_| LoweringError::Unsupported("terminal call closure exceeds u64"))?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "terminal call closure identity range overflows",
            ))?;
        let mut lowered = build_scalar_graph_module(
            &machine.states,
            machine.result_type,
            machine.contract_value,
            machine.crash_routes,
            machine.identity_reshuffles,
            machine.partition_compositions,
            terminal_machine,
            identity_base,
            &machine_ids,
            &requirement_counts,
        )?;
        let [terminal_machine] = lowered.semantic_module.machines.as_slice() else {
            unreachable!("one prepared scalar graph emits one terminal machine")
        };
        machines.push(terminal_machine.clone());
        evidence.append(&mut lowered.proof_bundle.evidence);
    }
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine_id(1),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            evidence_package_invocations: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn lower_checked_crash_frontier(
    frontier: &[PermissionClaimIdentity],
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ClaimId>, LoweringError> {
    let mut lowered = frontier
        .iter()
        .map(|identity| {
            source_claims
                .iter()
                .find_map(|(source, claim)| (source == identity).then_some(*claim))
                .ok_or(LoweringError::CrashFrontierClaimNotLowered(*identity))
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

fn lower_checked_crash_routes(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<Vec<psi_checked_trees::CrashRouteBucket>, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| {
            contract
                .crash
                .published()
                .iter()
                .map(|bucket| {
                    if bucket.alternative_guards().iter().any(|guard| {
                        matches!(guard, psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                            if predicate.scalar_expression().is_none())
                    }) {
                        return unsupported(
                            "guarded crash route is outside structured scalar predicate lowering",
                        );
                    }
                    Ok(bucket.clone())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn lower_checked_crash_exit(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<LoweredCrashExit, LoweringError> {
    let Some(crash_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| &contract.crash)
    else {
        return unsupported("explicit crash has no checked machine-contract plan");
    };
    let Some(checked_site) = crash_plan.checked_site_at(state, statement_ordinal) else {
        return unsupported("explicit crash has no body-derived checked crash-site row");
    };
    let matching_contracts = crash_plan
        .covering_buckets_for_site(checked_site)
        .map(|(_, bucket)| bucket)
        .collect::<Vec<_>>();
    let [covering_bucket] = matching_contracts.as_slice() else {
        return unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket",
        );
    };
    let site_identities = checked_site
        .path_guard_conjuncts()
        .iter()
        .chain(checked_site.path_guard_consequences())
        .collect::<BTreeSet<_>>();
    let site_guard = covering_bucket
        .alternative_guards()
        .iter()
        .filter_map(|guard| match guard {
            psi_checked_trees::CrashRouteGuard::Truth => None,
            psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                if site_identities.contains(predicate) =>
            {
                Some(
                    predicate
                        .scalar_expression()
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "guarded crash site is outside structured scalar predicate lowering",
                        )),
                )
            }
            psi_checked_trees::CrashRouteGuard::Predicate(_) => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !covering_bucket
        .alternative_guards()
        .contains(&psi_checked_trees::CrashRouteGuard::Truth)
        && site_guard.is_empty()
    {
        return unsupported("guarded crash site has no structured covering predicate");
    }
    Ok(LoweredCrashExit {
        cause: match checked_site.cause() {
            psi_checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            psi_checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        },
        site_guard,
        frontier_lower_bound: lower_checked_crash_frontier(
            checked_site.frontier_lower_bound(),
            source_claims,
        )?,
    })
}

fn merge_known_parameters<T: Copy + Eq>(
    current: &mut Option<Vec<Option<T>>>,
    incoming: Vec<Option<T>>,
) {
    if let Some(current) = current {
        assert_eq!(current.len(), incoming.len());
        for (current, incoming) in current.iter_mut().zip(incoming) {
            if *current != incoming {
                *current = None;
            }
        }
    } else {
        *current = Some(incoming);
    }
}

fn acyclic_topological_order(successors: &[Vec<usize>]) -> Vec<usize> {
    let mut indegree = vec![0_usize; successors.len()];
    for targets in successors {
        for target in targets {
            indegree[*target] = indegree[*target]
                .checked_add(1)
                .expect("source state count fits usize");
        }
    }
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(successors.len());
    while let Some(state) = ready.iter().next().copied() {
        ready.remove(&state);
        order.push(state);
        for target in &successors[state] {
            indegree[*target] = indegree[*target]
                .checked_sub(1)
                .expect("graph indegree is positive before traversal");
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    assert_eq!(order.len(), successors.len());
    order
}

fn evaluate_known_scalar_graph(states: &[LoweredScalarBranchState]) -> Option<KnownDirectScalar> {
    let successors = states
        .iter()
        .map(|state| match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            LoweredScalarBranchTerminator::Return { .. }
            | LoweredScalarBranchTerminator::Crash(_) => Vec::new(),
        })
        .collect::<Vec<_>>();
    let topological_order = acyclic_topological_order(&successors);
    let mut known_parameters = vec![None; states.len()];
    known_parameters[0] = Some(vec![None; states[0].parameter_types.len()]);
    let mut return_values = Vec::new();
    let mut reachable_crash = false;
    for state_index in topological_order {
        let Some(mut values) = known_parameters[state_index].clone() else {
            continue;
        };
        for binding in &states[state_index].bindings {
            let value = match binding {
                LoweredScalarBinding::Expression(expression) => {
                    evaluate_direct_expression(expression, &values)
                }
                LoweredScalarBinding::DirectCall(_) => None,
            };
            values.push(value);
        }
        let evaluate_arguments = |arguments: &[LoweredDirectExpression]| {
            arguments
                .iter()
                .map(|argument| evaluate_direct_expression(argument, &values))
                .collect::<Vec<_>>()
        };
        match &states[state_index].terminator {
            LoweredScalarBranchTerminator::Jump { target, arguments } => {
                merge_known_parameters(
                    &mut known_parameters[*target],
                    evaluate_arguments(arguments),
                );
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => match evaluate_compile_known_boolean_expression(condition, &values) {
                Some(true) => merge_known_parameters(
                    &mut known_parameters[*when_true_target],
                    evaluate_arguments(when_true_arguments),
                ),
                Some(false) => merge_known_parameters(
                    &mut known_parameters[*when_false_target],
                    evaluate_arguments(when_false_arguments),
                ),
                None => {
                    merge_known_parameters(
                        &mut known_parameters[*when_true_target],
                        evaluate_arguments(when_true_arguments),
                    );
                    merge_known_parameters(
                        &mut known_parameters[*when_false_target],
                        evaluate_arguments(when_false_arguments),
                    );
                }
            },
            LoweredScalarBranchTerminator::Return { expression } => {
                return_values.push(evaluate_direct_expression(expression, &values));
            }
            LoweredScalarBranchTerminator::Crash(_) => reachable_crash = true,
        }
    }

    if reachable_crash {
        return None;
    }
    let expected = return_values.first().copied().flatten()?;
    return_values
        .into_iter()
        .all(|value| value == Some(expected))
        .then_some(expected)
}

fn lower_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let prepared = prepare_scalar_graph_machine(checked, machine, graph)?;
    let machine_ids = [(machine, machine_id(1))];
    let requirement_counts = [(machine, usize::from(prepared.contract_value.is_some()))];
    let mut lowered = build_scalar_graph_module(
        &prepared.states,
        prepared.result_type,
        prepared.contract_value,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &machine_ids,
        &requirement_counts,
    )?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

fn prepare_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    let states = &graph.states;
    let entry_state = states.first().ok_or(LoweringError::Unsupported(
        "checked scalar control plan must contain an entry state",
    ))?;
    let result_type = terminal_scalar_type(entry_state.result_type)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state.state)?;
    let mut lowered_states = Vec::with_capacity(states.len());
    let mut successors = vec![Vec::new(); states.len()];
    let mut indegree = vec![0usize; states.len()];

    for (state_index, state) in states.iter().enumerate() {
        if terminal_scalar_type(state.result_type)? != result_type {
            return unsupported("scalar graph state result types must match exactly");
        }
        let parameter_types = state
            .parameter_types
            .iter()
            .copied()
            .map(terminal_scalar_type)
            .collect::<Result<Vec<_>, _>>()?;
        let mut value_types = parameter_types.clone();
        let mut bindings = Vec::with_capacity(state.bindings.len());
        for (binding_index, binding) in state.bindings.iter().enumerate() {
            let binding_ordinal = u32::try_from(binding_index)
                .map_err(|_| LoweringError::Unsupported("scalar local count exceeds u32"))?;
            let binding_type = terminal_scalar_type(binding.primitive_type)?;
            let lowered = match &binding.value {
                CheckedScalarBindingValue::Expression => {
                    let expression = lower_checked_scalar_expression_at(
                        checked,
                        state.state,
                        binding.statement_ordinal,
                        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
                    )?;
                    if expression.scalar_type() != binding_type {
                        return unsupported(
                            "checked scalar local initializer type must match its binding",
                        );
                    }
                    validate_direct_parameter_types(&expression, &value_types)?;
                    LoweredScalarBinding::Expression(expression)
                }
                CheckedScalarBindingValue::DirectCall {
                    target_machine,
                    target_state,
                    call_ordinal,
                    argument_count,
                } => LoweredScalarBinding::DirectCall(lower_checked_direct_call_binding(
                    checked,
                    machine,
                    state.state,
                    binding.statement_ordinal,
                    binding_ordinal,
                    *target_machine,
                    *target_state,
                    *call_ordinal,
                    *argument_count,
                    binding_type,
                    &value_types,
                )?),
            };
            bindings.push(lowered);
            value_types.push(binding_type);
        }
        let terminator = match &state.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let expression = lower_checked_scalar_expression_at(
                    checked,
                    state.state,
                    *statement_ordinal,
                    CheckedScalarExpressionRole::Return,
                )?;
                if expression.scalar_type() != result_type {
                    return unsupported("checked scalar return type must match the machine result");
                }
                validate_direct_parameter_types(&expression, &value_types)?;
                LoweredScalarBranchTerminator::Return { expression }
            }
            CheckedScalarStateTerminator::Crash { statement_ordinal } => {
                LoweredScalarBranchTerminator::Crash(lower_checked_crash_exit(
                    checked,
                    machine,
                    state.state,
                    *statement_ordinal,
                    &identity_reshuffles.source_claims,
                )?)
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let LoweredDirectExpression::Boolean {
                    expression: condition,
                } = lower_checked_scalar_expression_at(
                    checked,
                    state.state,
                    *guard_statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                )?
                else {
                    return unsupported("checked scalar graph guard must be Boolean");
                };
                let condition = *condition;
                validate_short_circuit_expression(&condition)?;
                validate_boolean_parameter_types(&condition, &value_types)?;

                let (when_true_target, when_true_arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    when_true,
                )?;
                let (when_false_target, when_false_arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    when_false,
                )?;
                successors[state_index] = vec![when_true_target, when_false_target];
                indegree[when_true_target] = indegree[when_true_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                indegree[when_false_target] = indegree[when_false_target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredScalarBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                }
            }
            CheckedScalarStateTerminator::Jump(successor) => {
                let (target, arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    &value_types,
                    successor,
                )?;
                successors[state_index] = vec![target];
                indegree[target] = indegree[target]
                    .checked_add(1)
                    .expect("source state count fits usize");
                LoweredScalarBranchTerminator::Jump { target, arguments }
            }
        };
        lowered_states.push(LoweredScalarBranchState {
            parameter_types,
            bindings,
            terminator,
        });
    }

    if indegree[0] != 0 || indegree[1..].contains(&0) {
        return unsupported(
            "scalar graph control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; states.len()];
    let mut active = vec![false; states.len()];
    validate_scalar_graph(0, &successors, &mut visited, &mut active)?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("scalar graph control contains an unreachable state");
    }

    let has_crash = lowered_states.iter().any(|state| {
        matches!(&state.terminator, LoweredScalarBranchTerminator::Crash(_))
            || state.bindings.iter().any(|binding| {
                matches!(binding, LoweredScalarBinding::DirectCall(call)
                        if !call.crash_continuations.is_empty())
            })
    });
    let has_return = lowered_states.iter().any(|state| {
        matches!(
            &state.terminator,
            LoweredScalarBranchTerminator::Return { .. }
        )
    });
    let expected_value = evaluate_known_scalar_graph(&lowered_states);
    let contract_value = if has_return {
        Some(validate_closed_scalar_contract(
            checked,
            machine,
            result_type,
            expected_value,
            has_crash,
        )?)
    } else {
        let contract = closed_scalar_contract_plan(checked, machine)?;
        if contract.has_other_clauses()
            || !contract.requires().is_empty()
            || !contract.ensures().is_empty()
        {
            return unsupported("an all-crash scalar graph cannot declare a value contract");
        }
        None
    };
    Ok(PreparedScalarMachine {
        source_machine: machine,
        states: lowered_states,
        result_type,
        contract_value,
        crash_routes: lower_checked_crash_routes(checked, machine)?,
        identity_reshuffles,
        partition_compositions,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_checked_direct_call_binding(
    checked: &CheckedTrees,
    caller_machine: psi_symbols::SymbolHandle,
    caller_state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    target_machine: psi_symbols::SymbolHandle,
    target_state: psi_symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: ScalarType,
    caller_value_types: &[ScalarType],
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let target_graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no source-independent checked graph",
        ))?;
    let target_entry = target_graph
        .states
        .first()
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked entry state",
        ))?;
    if target_entry.state != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if terminal_scalar_type(target_entry.result_type)? != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if usize::try_from(argument_count).ok() != Some(target_entry.parameter_types.len()) {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    let arguments = target_entry
        .parameter_types
        .iter()
        .enumerate()
        .map(|(argument_index, target_type)| {
            let argument_ordinal = u32::try_from(argument_index).map_err(|_| {
                LoweringError::Unsupported("scalar call argument count exceeds u32")
            })?;
            let expression = lower_checked_scalar_expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                },
            )?;
            let target_type = terminal_scalar_type(*target_type)?;
            if expression.scalar_type() != target_type {
                return unsupported(
                    "checked scalar call argument type must match its callee parameter",
                );
            }
            validate_direct_parameter_types(&expression, caller_value_types)?;
            Ok(expression)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller_machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(caller_state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "direct scalar call has no matching checked crash-refinement row",
        ))?;
    if checked_call.target_machine() != target_machine
        || checked_call.target_state() != target_state
    {
        return unsupported("checked scalar call target disagrees with crash refinement");
    }
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_call.target_contract_fingerprint() != target_contract.fingerprint {
        return unsupported("checked scalar call target contract fingerprint disagrees");
    }
    if checked_call.surviving_buckets().iter().any(|bucket| {
        bucket.alternative_guards().iter().any(|guard| {
            matches!(guard, psi_checked_trees::CrashRouteGuard::Predicate(predicate)
                if predicate.scalar_expression().is_none())
        })
    }) {
        return unsupported("direct scalar call crash continuation lacks a checked scalar term");
    }
    Ok(LoweredDirectCallBinding {
        target_machine,
        result_type,
        arguments,
        crash_continuations: checked_call.surviving_buckets().to_vec(),
        parameter_relative_crash_routes: target_contract.crash.published().to_vec(),
    })
}

fn lower_scalar_graph_successor(
    checked: &CheckedTrees,
    states: &[psi_checked_trees::CheckedScalarStateGraph],
    source_state: psi_symbols::SymbolHandle,
    source_value_types: &[ScalarType],
    successor: &CheckedScalarSuccessor,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    let target = states
        .iter()
        .position(|candidate| candidate.state == successor.target)
        .ok_or(LoweringError::Unsupported(
            "scalar graph successor must belong to the selected machine",
        ))?;
    let target_parameter_types = &states[target].parameter_types;
    if usize::try_from(successor.argument_count).ok() != Some(target_parameter_types.len()) {
        return unsupported(
            "scalar graph successor bindings must match the target parameter count",
        );
    }
    let arguments = (0..successor.argument_count)
        .zip(target_parameter_types)
        .map(|(argument_ordinal, target_type)| {
            let target_type = terminal_scalar_type(*target_type)?;
            let expression = lower_checked_scalar_expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                CheckedScalarExpressionRole::TransitionArgument { argument_ordinal },
            )?;
            validate_direct_parameter_types(&expression, source_value_types)?;
            (expression.scalar_type() == target_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

fn lower_checked_scalar_expression_at(
    checked: &CheckedTrees,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
) -> Result<LoweredDirectExpression, LoweringError> {
    let expression = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no source-independent checked value plan",
        ))?;
    lower_checked_scalar_expression(expression)
}

fn lower_checked_scalar_expression(
    expression: &CheckedScalarExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Parameter {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Local {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::StructuralParameterField { .. } => unsupported(
            "structural parameter fields are retained only inside structural crash predicates",
        ),
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let scalar_type = integer_landing_scalar_type(literal)?;
            Ok(LoweredDirectExpression::IntegerLiteral {
                value: integer_value(literal, scalar_type)?,
                scalar_type,
            })
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => Ok(LoweredDirectExpression::IntegerBinary {
            kind: match kind {
                CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                CheckedIntegerBinaryKind::ExactSubtract => LoweredIntegerBinaryKind::ExactSubtract,
                CheckedIntegerBinaryKind::ExactMultiply => LoweredIntegerBinaryKind::ExactMultiply,
                CheckedIntegerBinaryKind::ExactDivide => LoweredIntegerBinaryKind::ExactDivide,
                CheckedIntegerBinaryKind::ExactRemainder => {
                    LoweredIntegerBinaryKind::ExactRemainder
                }
                CheckedIntegerBinaryKind::WrappingDivide => {
                    LoweredIntegerBinaryKind::WrappingDivide
                }
                CheckedIntegerBinaryKind::WrappingRemainder => {
                    LoweredIntegerBinaryKind::WrappingRemainder
                }
                CheckedIntegerBinaryKind::SaturatingDivide => {
                    LoweredIntegerBinaryKind::SaturatingDivide
                }
                CheckedIntegerBinaryKind::SaturatingRemainder => {
                    LoweredIntegerBinaryKind::SaturatingRemainder
                }
                CheckedIntegerBinaryKind::WrappingAdd => LoweredIntegerBinaryKind::WrappingAdd,
                CheckedIntegerBinaryKind::SaturatingAdd => LoweredIntegerBinaryKind::SaturatingAdd,
                CheckedIntegerBinaryKind::WrappingSubtract => {
                    LoweredIntegerBinaryKind::WrappingSubtract
                }
                CheckedIntegerBinaryKind::SaturatingSubtract => {
                    LoweredIntegerBinaryKind::SaturatingSubtract
                }
                CheckedIntegerBinaryKind::WrappingMultiply => {
                    LoweredIntegerBinaryKind::WrappingMultiply
                }
                CheckedIntegerBinaryKind::SaturatingMultiply => {
                    LoweredIntegerBinaryKind::SaturatingMultiply
                }
                CheckedIntegerBinaryKind::BitwiseAnd => LoweredIntegerBinaryKind::BitwiseAnd,
                CheckedIntegerBinaryKind::BitwiseOr => LoweredIntegerBinaryKind::BitwiseOr,
                CheckedIntegerBinaryKind::BitwiseXor => LoweredIntegerBinaryKind::BitwiseXor,
                CheckedIntegerBinaryKind::WrappingShiftLeft => {
                    LoweredIntegerBinaryKind::WrappingShiftLeft
                }
                CheckedIntegerBinaryKind::WrappingShiftRight => {
                    LoweredIntegerBinaryKind::WrappingShiftRight
                }
                CheckedIntegerBinaryKind::ExactShiftLeft => {
                    LoweredIntegerBinaryKind::ExactShiftLeft
                }
                CheckedIntegerBinaryKind::ExactShiftRight => {
                    LoweredIntegerBinaryKind::ExactShiftRight
                }
            },
            scalar_type: terminal_scalar_type(*primitive_type)?,
            left: Box::new(lower_checked_scalar_expression(left)?),
            right: Box::new(lower_checked_scalar_expression(right)?),
        }),
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } => Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
            expression: Box::new(lower_checked_boolean_expression(expression)?),
        }),
    }
}

fn lower_checked_boolean_expression(
    expression: &CheckedBooleanExpression,
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => {
            LoweredBooleanReturnExpression::Constant { value: *value }
        }
        CheckedBooleanExpression::Parameter { position } => {
            LoweredBooleanReturnExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::Local { position } => LoweredBooleanReturnExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
            parameter_position: *parameter_position,
            path: path.clone(),
        },
        CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
            operand: Box::new(lower_checked_boolean_expression(operand)?),
        },
        CheckedBooleanExpression::Equal { left, right } => LoweredBooleanReturnExpression::Equal {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            LoweredBooleanReturnExpression::IntegerComparison {
                kind: match kind {
                    CheckedIntegerComparisonKind::Equal => LoweredIntegerComparisonKind::Equal,
                    CheckedIntegerComparisonKind::LessThan => {
                        LoweredIntegerComparisonKind::LessThan
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        LoweredIntegerComparisonKind::LessOrEqual
                    }
                },
                left: Box::new(lower_checked_scalar_expression(left)?),
                right: Box::new(lower_checked_scalar_expression(right)?),
            }
        }
        CheckedBooleanExpression::And { left, right } => LoweredBooleanReturnExpression::And {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
    })
}

fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::StructuralField { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean type validation")
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("scalar graph guard parameters must be Boolean")
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_boolean_parameter_types(operand, parameter_types)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_boolean_parameter_types(left, parameter_types)?;
            validate_boolean_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
    }
}

fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("scalar graph integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. } => Ok(()),
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

fn validate_scalar_graph(
    state: usize,
    successors: &[Vec<usize>],
    visited: &mut [bool],
    active: &mut [bool],
) -> Result<(), LoweringError> {
    if active[state] {
        return unsupported("scalar graph control must be acyclic");
    }
    if visited[state] {
        return Ok(());
    }
    active[state] = true;
    for successor in &successors[state] {
        validate_scalar_graph(*successor, successors, visited, active)?;
    }
    active[state] = false;
    visited[state] = true;
    Ok(())
}

fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
        LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
        LoweredBooleanReturnExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            true
        }
    }
}

fn direct_expression_contains_short_circuit(expression: &LoweredDirectExpression) -> bool {
    matches!(
        expression,
        LoweredDirectExpression::Boolean { expression }
            if contains_short_circuit(expression)
    )
}

fn scalar_binding_contains_short_circuit(binding: &LoweredScalarBinding) -> bool {
    match binding {
        LoweredScalarBinding::Expression(expression) => {
            direct_expression_contains_short_circuit(expression)
        }
        LoweredScalarBinding::DirectCall(call) => call
            .arguments
            .iter()
            .any(direct_expression_contains_short_circuit),
    }
}

fn staged_short_circuit_bindings_terminator(
    bindings: &[LoweredScalarBinding],
    terminator: &LoweredScalarBranchTerminator,
) -> Option<(Vec<LoweredScalarBinding>, LoweredScalarBranchTerminator)> {
    if !bindings.iter().any(scalar_binding_contains_short_circuit) {
        return None;
    }
    Some((bindings.to_vec(), terminator.clone()))
}

fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean validation")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownDirectScalar {
    Boolean(bool),
    Integer(IntegerValue),
}

fn evaluate_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<KnownDirectScalar> {
    match expression {
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredDirectExpression::IntegerLiteral { value, .. } => {
            Some(KnownDirectScalar::Integer(*value))
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let count_type = right.scalar_type();
            let KnownDirectScalar::Integer(left) = evaluate_direct_expression(left, parameters)?
            else {
                return None;
            };
            let KnownDirectScalar::Integer(right) = evaluate_direct_expression(right, parameters)?
            else {
                return None;
            };
            evaluate_lowered_integer_binary(*kind, *scalar_type, count_type, left, right)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(operand) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            integer_type
                .bitwise_not(operand)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .widen_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .exact_cast_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::Boolean { expression } => {
            evaluate_compile_known_boolean_expression(expression, parameters)
                .map(KnownDirectScalar::Boolean)
        }
    }
}

fn evaluate_integer_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<IntegerValue> {
    let KnownDirectScalar::Integer(value) = evaluate_direct_expression(expression, parameters)?
    else {
        return None;
    };
    Some(value)
}

fn evaluate_lowered_integer_binary(
    kind: LoweredIntegerBinaryKind,
    scalar_type: ScalarType,
    count_type: ScalarType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return None;
    };
    match kind {
        LoweredIntegerBinaryKind::BitwiseAnd => integer_type.bitwise_and(left, right),
        LoweredIntegerBinaryKind::BitwiseOr => integer_type.bitwise_or(left, right),
        LoweredIntegerBinaryKind::BitwiseXor => integer_type.bitwise_xor(left, right),
        LoweredIntegerBinaryKind::WrappingShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::WrappingShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactAdd => integer_type.exact_add(left, right),
        LoweredIntegerBinaryKind::ExactSubtract => integer_type.exact_sub(left, right),
        LoweredIntegerBinaryKind::ExactMultiply => integer_type.exact_mul(left, right),
        LoweredIntegerBinaryKind::ExactDivide => integer_type.exact_div(left, right),
        LoweredIntegerBinaryKind::ExactRemainder => integer_type.exact_rem(left, right),
        LoweredIntegerBinaryKind::WrappingDivide => integer_type.wrapping_div(left, right),
        LoweredIntegerBinaryKind::WrappingRemainder => integer_type.wrapping_rem(left, right),
        LoweredIntegerBinaryKind::SaturatingDivide => integer_type.saturating_div(left, right),
        LoweredIntegerBinaryKind::SaturatingRemainder => integer_type.saturating_rem(left, right),
        LoweredIntegerBinaryKind::WrappingAdd => integer_type.wrapping_add(left, right),
        LoweredIntegerBinaryKind::SaturatingAdd => integer_type.saturating_add(left, right),
        LoweredIntegerBinaryKind::WrappingSubtract => integer_type.wrapping_sub(left, right),
        LoweredIntegerBinaryKind::SaturatingSubtract => integer_type.saturating_sub(left, right),
        LoweredIntegerBinaryKind::WrappingMultiply => integer_type.wrapping_mul(left, right),
        LoweredIntegerBinaryKind::SaturatingMultiply => integer_type.saturating_mul(left, right),
    }
}

fn evaluate_compile_known_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<bool> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Some(*value),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let KnownDirectScalar::Boolean(value) = parameters.get(*position).copied().flatten()?
            else {
                return None;
            };
            Some(value)
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. } => None,
        LoweredBooleanReturnExpression::Not { operand } => Some(
            !evaluate_compile_known_boolean_expression(operand, parameters)?,
        ),
        LoweredBooleanReturnExpression::Equal { left, right } => Some(
            evaluate_compile_known_boolean_expression(left, parameters)?
                == evaluate_compile_known_boolean_expression(right, parameters)?,
        ),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return None;
            };
            let left = evaluate_integer_direct_expression(left, parameters)?;
            let right = evaluate_integer_direct_expression(right, parameters)?;
            match kind {
                LoweredIntegerComparisonKind::Equal => Some(left == right),
                LoweredIntegerComparisonKind::LessThan => {
                    Some(integer_type.compare(left, right)?.is_lt())
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    Some(!integer_type.compare(left, right)?.is_gt())
                }
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                evaluate_compile_known_boolean_expression(right, parameters)
            } else {
                Some(false)
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                Some(true)
            } else {
                evaluate_compile_known_boolean_expression(right, parameters)
            }
        }
    }
}

fn lower_content_evidence(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    state: psi_symbols::SymbolHandle,
) -> Result<
    (
        LoweredContentIdentityReshuffles,
        LoweredContentPartitionCompositions,
    ),
    LoweringError,
> {
    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &mut identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

fn closed_scalar_contract_plan(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<&ClosedScalarValueContractPlan, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked contract plan",
        ))
}

fn validate_closed_scalar_contract(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    result_type: ScalarType,
    expected_value: Option<KnownDirectScalar>,
    allow_crash_contracts: bool,
) -> Result<KnownDirectScalar, LoweringError> {
    let contract = closed_scalar_contract_plan(checked, machine)?;
    let ([Some(requires)], [Some(ensures)]) = (contract.requires(), contract.ensures()) else {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    if contract.has_other_clauses() || (!allow_crash_contracts && contract.has_crash_clauses()) {
        return unsupported("machine must have exactly one requires and one ensures clause");
    }
    let (requires, ensures) = match (result_type, requires, ensures) {
        (
            ScalarType::Boolean,
            ClosedScalarContractValue::Boolean(requires),
            ClosedScalarContractValue::Boolean(ensures),
        ) => (
            KnownDirectScalar::Boolean(*requires),
            KnownDirectScalar::Boolean(*ensures),
        ),
        (
            ScalarType::Integer(_),
            ClosedScalarContractValue::Integer(requires),
            ClosedScalarContractValue::Integer(ensures),
        ) => (
            KnownDirectScalar::Integer(integer_value(requires, result_type)?),
            KnownDirectScalar::Integer(integer_value(ensures, result_type)?),
        ),
        _ => return unsupported("contract scalar type must match the machine result type"),
    };
    if requires != ensures {
        return unsupported("requires and ensures must carry the same closed equality");
    }
    if expected_value.is_some_and(|expected| expected != requires) {
        return match result_type {
            ScalarType::Boolean => {
                unsupported("Boolean contract literal must match the compile-known result")
            }
            ScalarType::Integer(_) => {
                unsupported("contract literals must equal the executed literal")
            }
        };
    }
    Ok(requires)
}

fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    if primitive == PrimitiveType::Addr {
        return IntegerType::address(64)
            .map(ScalarType::Integer)
            .map_err(|_| LoweringError::InvalidPsiIntegerType);
    }
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr => unreachable!("address carrier handled above"),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return unsupported("only primitive integers are supported");
        }
    };
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| LoweringError::InvalidPsiIntegerType)
}

fn terminal_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ScalarType::Boolean),
        primitive => integer_scalar_type(primitive),
    }
}

fn integer_landing_scalar_type(
    literal: &psi_numerics::literals::IntegerLiteral,
) -> Result<ScalarType, LoweringError> {
    use psi_numerics::literals::LandedIntegerType;

    let primitive = match literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?
        .landed_type
    {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    };
    integer_scalar_type(primitive)
}

fn integer_value(
    literal: &psi_numerics::literals::IntegerLiteral,
    scalar_type: ScalarType,
) -> Result<IntegerValue, LoweringError> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return Err(LoweringError::InvalidPsiIntegerType);
    };
    let landing = literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?;
    if landing.landed_type.bit_width() != u32::from(integer_type.bits())
        || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
    {
        return Err(LoweringError::IntegerLandingMismatch);
    }
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(
            literal
                .value_i64()
                .map(i128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            literal
                .value_u64()
                .map(u128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
    };
    if !integer_type.admits(value) {
        return Err(LoweringError::IntegerLiteralOutsidePsiType);
    }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn bind_boolean_decision<F>(
    decision: LoweredBooleanDecision,
    continuation: &F,
) -> LoweredBooleanDecision
where
    F: Fn(&LoweredBooleanReturnExpression) -> LoweredBooleanDecision,
{
    match decision {
        LoweredBooleanDecision::Value(expression) => continuation(&expression),
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(bind_boolean_decision(*when_true, continuation)),
            when_false: Box::new(bind_boolean_decision(*when_false, continuation)),
        },
    }
}

fn branch_boolean_decision(
    decision: LoweredBooleanDecision,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value }) => {
            if value {
                when_true
            } else {
                when_false
            }
        }
        LoweredBooleanDecision::Value(condition) => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
        LoweredBooleanDecision::Test {
            condition,
            when_true: nested_true,
            when_false: nested_false,
        } => LoweredBooleanDecision::Test {
            condition,
            when_true: Box::new(branch_boolean_decision(
                *nested_true,
                when_true.clone(),
                when_false.clone(),
            )),
            when_false: Box::new(branch_boolean_decision(
                *nested_false,
                when_true,
                when_false,
            )),
        },
    }
}

fn lower_boolean_control_decision(
    expression: &LoweredBooleanReturnExpression,
    when_true: LoweredBooleanDecision,
    when_false: LoweredBooleanDecision,
) -> LoweredBooleanDecision {
    match expression {
        LoweredBooleanReturnExpression::And { left, right } => {
            let right = lower_boolean_control_decision(right, when_true, when_false.clone());
            lower_boolean_control_decision(left, right, when_false)
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let right = lower_boolean_control_decision(right, when_true.clone(), when_false);
            lower_boolean_control_decision(left, when_true, right)
        }
        LoweredBooleanReturnExpression::Not { operand } if contains_short_circuit(operand) => {
            lower_boolean_control_decision(operand, when_false, when_true)
        }
        expression if contains_short_circuit(expression) => branch_boolean_decision(
            lower_boolean_value_decision(expression),
            when_true,
            when_false,
        ),
        expression => LoweredBooleanDecision::Test {
            condition: expression.clone(),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
    }
}

fn lower_boolean_value_decision(
    expression: &LoweredBooleanReturnExpression,
) -> LoweredBooleanDecision {
    if !contains_short_circuit(expression) {
        return LoweredBooleanDecision::Value(expression.clone());
    }
    match expression {
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            lower_boolean_control_decision(
                expression,
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: true,
                }),
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: false,
                }),
            )
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            bind_boolean_decision(lower_boolean_value_decision(operand), &|operand| {
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Not {
                    operand: Box::new(operand.clone()),
                })
            })
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            bind_boolean_decision(lower_boolean_value_decision(left), &|left| {
                bind_boolean_decision(lower_boolean_value_decision(right), &|right| {
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Equal {
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    })
                })
            })
        }
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => {
            unreachable!("non-short-circuit expressions return above")
        }
    }
}

fn boolean_decision_block_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 1,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_block_count(when_true) + boolean_decision_block_count(when_false),
    }
}

fn boolean_decision_test_count(decision: &LoweredBooleanDecision) -> usize {
    match decision {
        LoweredBooleanDecision::Value(_) => 0,
        LoweredBooleanDecision::Test {
            when_true,
            when_false,
            ..
        } => 1 + boolean_decision_test_count(when_true) + boolean_decision_test_count(when_false),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoweredBooleanDecisionTarget {
    block: BlockId,
    arguments: Vec<ValueId>,
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_guard_decision_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> LoweredBooleanDecisionTarget {
    match decision {
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: true }) => {
            when_true_target.clone()
        }
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
            value: false,
        }) => when_false_target.clone(),
        LoweredBooleanDecision::Value(_) => {
            unreachable!("guard control decisions end in canonical Boolean choices")
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let block_index = blocks.len();
            let block = block_id(
                first_block_identity
                    .checked_add(
                        u64::try_from(block_index)
                            .expect("reserved guard block count fits a semantic identity"),
                    )
                    .expect("reserved guard block identity advances"),
            );
            blocks.push(None);
            let operation_start = all_operations.len();
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved guard false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved guard decision edge identities advance");
            let when_true = emit_reserved_boolean_guard_decision_blocks(
                when_true,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_guard_decision_blocks(
                when_false,
                parameters,
                Vec::new(),
                when_true_target,
                when_false_target,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            blocks[block_index] = Some(Block {
                id: block,
                parameters: block_parameters,
                operations: all_operations[operation_start..operation_end].to_vec(),
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true.block,
                        arguments: when_true.arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                },
            });
            LoweredBooleanDecisionTarget {
                block,
                arguments: Vec::new(),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_value_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    exit: LoweredBooleanDecisionExit,
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved Boolean return block count fits a semantic identity"),
            )
            .expect("reserved Boolean return block identity advances"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let value = emit_boolean_expression(
                expression,
                parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("reserved Boolean return edge identity advances");
            let terminator = match exit {
                LoweredBooleanDecisionExit::Return => Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge,
                    value,
                },
                LoweredBooleanDecisionExit::Jump { target } => Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![value],
                    trivial_affine_discards: Vec::new(),
                },
            };
            (terminator, all_operations.len())
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved Boolean return false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved Boolean return decision edges advance");
            let when_true = emit_reserved_boolean_value_blocks(
                when_true,
                parameters,
                Vec::new(),
                exit,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_value_blocks(
                when_false,
                parameters,
                Vec::new(),
                exit,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            (
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                operation_end,
            )
        }
    };
    blocks[block_index] = Some(Block {
        id: block,
        parameters: block_parameters,
        operations: all_operations[operation_start..operation_end].to_vec(),
        terminator,
    });
    block
}

#[allow(clippy::too_many_arguments)]
fn emit_inlined_boolean_value_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    exit: LoweredBooleanDecisionExit,
    source_block: BlockId,
    first_synthetic_block: BlockId,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
) -> (Block, Vec<Block>) {
    let first_reserved_identity = first_synthetic_block
        .get()
        .checked_sub(1)
        .expect("synthetic Boolean blocks follow source blocks");
    let mut reserved = Vec::new();
    let entry = emit_reserved_boolean_value_blocks(
        decision,
        parameters,
        block_parameters,
        exit,
        first_reserved_identity,
        next_value_identity,
        next_edge_identity,
        all_operations,
        &mut reserved,
    );
    assert_eq!(entry.get(), first_reserved_identity);
    let mut reserved = reserved
        .into_iter()
        .map(|block| block.expect("every inlined Boolean value block is finalized"));
    let mut root = reserved
        .next()
        .expect("short-circuit Boolean value has a decision root");
    root.id = source_block;
    (root, reserved.collect())
}

#[allow(clippy::too_many_arguments)]
fn emit_inlined_boolean_guard_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    when_true_target: &LoweredBooleanDecisionTarget,
    when_false_target: &LoweredBooleanDecisionTarget,
    source_block: BlockId,
    first_synthetic_block: BlockId,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
) -> (Block, Vec<Block>) {
    let first_reserved_identity = first_synthetic_block
        .get()
        .checked_sub(1)
        .expect("synthetic Boolean blocks follow source blocks");
    let mut reserved = Vec::new();
    let entry = emit_reserved_boolean_guard_decision_blocks(
        decision,
        parameters,
        block_parameters,
        when_true_target,
        when_false_target,
        first_reserved_identity,
        next_value_identity,
        next_edge_identity,
        all_operations,
        &mut reserved,
    );
    assert_eq!(entry.block.get(), first_reserved_identity);
    assert!(entry.arguments.is_empty());
    let mut reserved = reserved
        .into_iter()
        .map(|block| block.expect("every inlined Boolean guard block is finalized"));
    let mut root = reserved
        .next()
        .expect("short-circuit Boolean guard has a decision root");
    root.id = source_block;
    (root, reserved.collect())
}

#[allow(clippy::too_many_arguments)]
fn emit_reserved_boolean_tuple_stage_blocks(
    decision: &LoweredBooleanDecision,
    parameters: &[ValueDeclaration],
    block_parameters: Vec<ValueDeclaration>,
    next_stage: BlockId,
    carried_arguments: &[ValueId],
    first_block_identity: u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    all_operations: &mut OperationBuffer,
    blocks: &mut Vec<Option<Block>>,
) -> BlockId {
    let block_index = blocks.len();
    let block = block_id(
        first_block_identity
            .checked_add(
                u64::try_from(block_index)
                    .expect("reserved Boolean tuple block count fits a semantic identity"),
            )
            .expect("reserved Boolean tuple block identity advances"),
    );
    blocks.push(None);
    let operation_start = all_operations.len();
    let (terminator, operation_end) = match decision {
        LoweredBooleanDecision::Value(expression) => {
            let value = emit_boolean_expression(
                expression,
                parameters,
                next_value_identity,
                all_operations,
            );
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("reserved Boolean tuple value edge identity advances");
            let mut arguments = carried_arguments.to_vec();
            arguments.push(value);
            (
                Terminator::Jump {
                    edge,
                    target: next_stage,
                    arguments,
                    trivial_affine_discards: Vec::new(),
                },
                all_operations.len(),
            )
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            let condition =
                emit_boolean_expression(condition, parameters, next_value_identity, all_operations);
            let operation_end = all_operations.len();
            let true_edge = edge_id(*next_edge_identity);
            let false_edge = edge_id(
                next_edge_identity
                    .checked_add(1)
                    .expect("reserved Boolean tuple false edge identity advances"),
            );
            *next_edge_identity = next_edge_identity
                .checked_add(2)
                .expect("reserved Boolean tuple decision edges advance");
            let when_true = emit_reserved_boolean_tuple_stage_blocks(
                when_true,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            let when_false = emit_reserved_boolean_tuple_stage_blocks(
                when_false,
                parameters,
                Vec::new(),
                next_stage,
                carried_arguments,
                first_block_identity,
                next_value_identity,
                next_edge_identity,
                all_operations,
                blocks,
            );
            (
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: true_edge,
                        target: when_true,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: false_edge,
                        target: when_false,
                        arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                operation_end,
            )
        }
    };
    blocks[block_index] = Some(Block {
        id: block,
        parameters: block_parameters,
        operations: all_operations[operation_start..operation_end].to_vec(),
        terminator,
    });
    block
}

#[allow(clippy::too_many_arguments)]
fn build_scalar_conditional_target(
    target: usize,
    arguments: &[LoweredDirectExpression],
    current_parameters: &[ValueDeclaration],
    current_parameter_types: &[ScalarType],
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    pending_blocks: &mut Vec<PendingNestedBlockGroup>,
    identity_base: u64,
) -> LoweredBooleanDecisionTarget {
    let direct_arguments = arguments
        .iter()
        .map(|argument| match argument {
            LoweredDirectExpression::Parameter { position, .. }
            | LoweredDirectExpression::Local { position, .. } => {
                Some(current_parameters[*position].id)
            }
            LoweredDirectExpression::Boolean { expression } => match expression.as_ref() {
                LoweredBooleanReturnExpression::Parameter { position }
                | LoweredBooleanReturnExpression::Local { position } => {
                    Some(current_parameters[*position].id)
                }
                _ => None,
            },
            LoweredDirectExpression::IntegerLiteral { .. }
            | LoweredDirectExpression::IntegerBinary { .. }
            | LoweredDirectExpression::IntegerBitwiseNot { .. }
            | LoweredDirectExpression::IntegerWiden { .. }
            | LoweredDirectExpression::IntegerExactCast { .. } => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(arguments) = direct_arguments {
        return LoweredBooleanDecisionTarget {
            block: scalar_source_block(identity_base, target),
            arguments,
        };
    }

    if arguments
        .iter()
        .any(direct_expression_contains_short_circuit)
    {
        let first_id = block_id(*next_block_identity);
        let reserved_block_count = arguments
            .iter()
            .map(|argument| match argument {
                LoweredDirectExpression::Boolean { expression }
                    if contains_short_circuit(expression) =>
                {
                    boolean_decision_block_count(&lower_boolean_value_decision(expression))
                }
                _ => 1,
            })
            .sum::<usize>()
            .checked_add(1)
            .expect("mixed tuple convergence block count advances");
        *next_block_identity = next_block_identity
            .checked_add(
                u64::try_from(reserved_block_count)
                    .expect("mixed tuple block count fits a semantic identity"),
            )
            .expect("mixed tuple block identities advance");
        let stage_parameters = (0..=arguments.len())
            .map(|completed_argument_count| {
                let mut scalar_types = current_parameter_types.to_vec();
                scalar_types.extend(
                    arguments[..completed_argument_count]
                        .iter()
                        .map(LoweredDirectExpression::scalar_type),
                );
                scalar_types
                    .into_iter()
                    .map(|scalar_type| {
                        let parameter = ValueDeclaration {
                            id: value_id(*next_value_identity),
                            scalar_type,
                        };
                        *next_value_identity = next_value_identity
                            .checked_add(1)
                            .expect("mixed tuple parameter identities advance");
                        parameter
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        pending_blocks.push(PendingNestedBlockGroup::TupleBinding(
            PendingMixedTupleBindingBlocks {
                first_id,
                original_parameter_count: current_parameters.len(),
                arguments: arguments.to_vec(),
                stage_parameters,
                target: scalar_source_block(identity_base, target),
            },
        ));
        return LoweredBooleanDecisionTarget {
            block: first_id,
            arguments: current_parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect(),
        };
    }

    let id = block_id(*next_block_identity);
    *next_block_identity = next_block_identity
        .checked_add(1)
        .expect("conditional binding block identities advance");
    let parameters = current_parameter_types
        .iter()
        .map(|scalar_type| {
            let parameter = ValueDeclaration {
                id: value_id(*next_value_identity),
                scalar_type: *scalar_type,
            };
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("conditional binding parameter identities advance");
            parameter
        })
        .collect::<Vec<_>>();
    pending_blocks.push(PendingNestedBlockGroup::ConditionalBinding(
        PendingConditionalBindingBlock {
            id,
            parameters,
            target: scalar_source_block(identity_base, target),
            arguments: arguments.to_vec(),
        },
    ));
    LoweredBooleanDecisionTarget {
        block: id,
        arguments: current_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
    }
}

fn scalar_source_block(identity_base: u64, state: usize) -> BlockId {
    block_id(
        identity_base
            .checked_add(u64::try_from(state).expect("state index fits a semantic identity"))
            .and_then(|identity| identity.checked_add(1))
            .expect("state block identity fits the machine namespace"),
    )
}

fn build_debug_map(
    plan: &CheckedTerminalMachineDebugPlan,
    module: &TerminalModule,
) -> Result<TerminalDebugMap, LoweringError> {
    let terminal_machine = module
        .machines
        .first()
        .expect("the selected entry machine is first in its terminal call closure");
    let source_states = &plan.states;
    let has_source_file = |span: psi_source::SourceSpan| {
        plan.source_files
            .iter()
            .any(|file| file.source_id == span.source_id)
    };
    let mut subjects = Vec::<(DebugSubject, psi_source::SourceSpan)>::new();
    let mut push = |subject, span| {
        if let Some(span) = span {
            subjects.push((subject, span));
        }
    };

    push(
        DebugSubject::Machine(terminal_machine.id),
        plan.machine_span,
    );
    let contract_span = plan.contract_span;
    push(
        DebugSubject::Contract(terminal_machine.contract.id),
        contract_span,
    );
    for clause in &terminal_machine.contract.ensures {
        push(DebugSubject::Obligation(clause.obligation), contract_span);
    }

    for (index, block) in terminal_machine.blocks.iter().enumerate() {
        let source_state = source_states
            .get(index)
            .or_else(|| source_states.last())
            .expect("an accepted source machine has at least one state");
        push(DebugSubject::Block(block.id), source_state.state_span);
        for (edge_index, edge) in block.terminator.edges().enumerate() {
            let transition_span = source_state
                .transition_spans
                .get(edge_index)
                .or_else(|| {
                    (source_state.transition_spans.len() == 1)
                        .then(|| &source_state.transition_spans[0])
                })
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            push(
                DebugSubject::Edge(edge),
                transition_span.or(source_state.state_span),
            );
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let source_span = source_state
                .operation_spans
                .get(operation_index)
                .copied()
                .filter(|span| *span != psi_source::SourceSpan::default())
                .filter(|span| has_source_file(*span));
            if let Some(source_span) = source_span {
                push(DebugSubject::Operation(operation.id), Some(source_span));
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    Some(source_span),
                );
            } else {
                push(
                    DebugSubject::Operation(operation.id),
                    source_state.state_span,
                );
                push(
                    DebugSubject::Value(operation.result.expect_scalar().id),
                    source_state.state_span,
                );
            }
        }
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if let Some(source_span) = source_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }

    if let Some(entry_state) = source_states.first() {
        for (parameter_index, parameter) in terminal_machine.parameters.iter().enumerate() {
            if let Some(source_span) = entry_state
                .parameter_spans
                .get(parameter_index)
                .copied()
                .flatten()
            {
                push(DebugSubject::Value(parameter.id), Some(source_span));
            }
        }
    }
    push(
        DebugSubject::Value(
            terminal_machine
                .result
                .scalar()
                .expect("the checked scalar producer emits a scalar result")
                .id,
        ),
        plan.machine_span,
    );

    subjects.sort_by_key(|(subject, _)| *subject);
    subjects.dedup_by_key(|(subject, _)| *subject);
    let mut source_ids = subjects
        .iter()
        .map(|(_, span)| span.source_id.0)
        .collect::<Vec<_>>();
    source_ids.sort_unstable();
    source_ids.dedup();

    let mut files = Vec::with_capacity(source_ids.len());
    for (index, source_id) in source_ids.iter().copied().enumerate() {
        let source_file = plan
            .source_files
            .iter()
            .find(|file| file.source_id == psi_source::SourceId(source_id))
            .ok_or(LoweringError::MissingDebugSourceFile(source_id))?;
        let id = DebugFileId::new(
            u32::try_from(index)
                .map_err(|_| LoweringError::DebugSourceFileCountOverflow)?
                .checked_add(1)
                .ok_or(LoweringError::DebugSourceFileCountOverflow)?,
        )
        .expect("one-based debug file identity is nonzero");
        files.push(DebugSourceFile {
            id,
            origin: match source_file.origin {
                psi_source::SourceOrigin::User => DebugSourceOrigin::User,
                psi_source::SourceOrigin::Toolchain => DebugSourceOrigin::Toolchain,
            },
            byte_len: u64::try_from(source_file.source.len())
                .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
            digest: source_digest(source_file.source.as_bytes()),
            path: source_file.path.to_string_lossy().into_owned(),
        });
    }

    let sites = subjects
        .into_iter()
        .map(|(subject, span)| {
            let file_index = source_ids
                .binary_search(&span.source_id.0)
                .expect("source identity was collected above");
            let file = DebugFileId::new(
                u32::try_from(file_index)
                    .expect("validated debug file count fits u32")
                    .checked_add(1)
                    .expect("one-based debug file identity fits u32"),
            )
            .expect("one-based debug file identity is nonzero");
            Ok(DebugSite {
                subject,
                span: DebugSourceSpan {
                    file,
                    start: u64::try_from(span.span.start)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                    end: u64::try_from(span.span.end)
                        .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let debug_map = TerminalDebugMap {
        semantic: terminal_psi_identity(module).map_err(LoweringError::DebugSemanticCodec)?,
        files,
        sites,
    };
    validate_debug_map(module, &debug_map).map_err(LoweringError::InvalidDebugMap)?;
    Ok(debug_map)
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
    ExactIntegerCastProofUnavailable(ObligationId),
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
