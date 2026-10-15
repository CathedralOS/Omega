//! Prepared scalar branch states the Unit scalar-completion emitter lowers.
//!
//! States retain ordered bindings, structural effects and outgoing transfers. This
//! is producer working state, not a portable representation or previous-stage wrapper.

use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::lowering_error::LoweringError;
use crate::scalar_lowering::branch_states;
use crate::scalar_lowering::computations;
use crate::scalar_lowering::contracts::LoweredProofTerm;
use checked_trees::CheckedBooleanExpression;
use semantic_vocabulary::{
    ClaimId, PlaceId, QualifiedScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    CrashCause as TerminalCrashCause, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarBranchTerminator {
    /// A representation-identical transfer establishing the final argument's
    /// explicit qualification on a fresh successor parameter, not its source.
    Qualify {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
        erased_arguments: Vec<LoweredDirectExpression>,
        erased_proof_arguments: Vec<LoweredProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
    },
    Jump {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
        /// Proof-only erased actuals for the successor state's erased roster.
        erased_arguments: Vec<LoweredDirectExpression>,
        /// Erased proof-only actuals for the successor state's proof roster.
        /// Scalar leaves still carry checked expressions; emission resolves
        /// them against the source state's completed value namespace.
        erased_proof_arguments: Vec<LoweredProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        /// Exact ordered edge work, including executable nominal hooks.
        cleanup_actions: Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>,
    },
    Conditional {
        condition: LoweredBooleanReturnExpression,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredDirectExpression>,
        when_true_erased_arguments: Vec<LoweredDirectExpression>,
        when_true_erased_proof_arguments: Vec<LoweredProofTerm>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredDirectExpression>,
        when_false_erased_arguments: Vec<LoweredDirectExpression>,
        when_false_erased_proof_arguments: Vec<LoweredProofTerm>,
    },
    /// A case-membership dispatch on a structural operand: the selected
    /// case's scalar payload fields bind as the selected edge's block
    /// parameters, matching `Terminator::StructuralCase` in the emitted
    /// module. Both outcomes stage their arguments through parameter-only
    /// continuations because case edges forward no values.
    CaseDispatch {
        source: PlaceId,
        selected: StructuralCaseId,
        /// Every declared case of the source's shape, in declaration order.
        cases: Vec<StructuralCaseId>,
        /// Scalar payload fields of `selected`, in field order; these bind
        /// positionally as the selected branch block's parameters.
        payloads: Vec<(StructuralFieldId, QualifiedScalarType)>,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredDirectExpression>,
        when_true_erased_arguments: Vec<LoweredDirectExpression>,
        when_true_erased_proof_arguments: Vec<LoweredProofTerm>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredDirectExpression>,
        when_false_erased_arguments: Vec<LoweredDirectExpression>,
        when_false_erased_proof_arguments: Vec<LoweredProofTerm>,
    },
    /// A total case split on a structural operand: every armed case's scalar
    /// payload fields bind on its own edge into that case's continuation, and
    /// every remaining case takes the fallback continuation. Reaching an armed
    /// edge makes that arm's case-membership guard provably true, so armed
    /// continuations skip evaluating their own membership test.
    CaseDispatchSplit {
        source: PlaceId,
        /// Every declared case of the source's shape, in declaration order.
        cases: Vec<StructuralCaseId>,
        /// Armed cases in declaration order: the payloads each edge binds and
        /// the continuation those payloads feed.
        armed: Vec<CaseDispatchArm>,
        fallback_target: usize,
        fallback_arguments: Vec<LoweredDirectExpression>,
        fallback_erased_arguments: Vec<LoweredDirectExpression>,
        fallback_erased_proof_arguments: Vec<LoweredProofTerm>,
    },
}

impl LoweredScalarBranchTerminator {
    /// A value-only edge: no erased, proof-only or structural actuals and no
    /// ordered edge work — the shape nearly every plain continuation takes.
    pub(crate) fn jump(target: usize, arguments: Vec<LoweredDirectExpression>) -> Self {
        Self::jump_with_cleanup(target, arguments, Vec::new())
    }

    /// A jump carrying exact ordered edge work (cleanup actions) while the
    /// erased, proof-only and structural rosters stay empty.
    pub(crate) fn jump_with_cleanup(
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
        cleanup_actions: Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>,
    ) -> Self {
        Self::Jump {
            target,
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions,
        }
    }
}

/// One armed case of a `CaseDispatchSplit`: `payloads` binds positionally as
/// the case edge's block parameters, which the block then forwards as
/// `arguments` to the arm's continuation `target`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaseDispatchArm {
    pub(crate) case: StructuralCaseId,
    pub(crate) payloads: Vec<(StructuralFieldId, QualifiedScalarType)>,
    pub(crate) target: usize,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) erased_arguments: Vec<LoweredDirectExpression>,
    pub(crate) erased_proof_arguments: Vec<LoweredProofTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredCrashExit {
    pub(crate) cause: TerminalCrashCause,
    pub(crate) site_guard: Vec<CheckedBooleanExpression>,
    pub(crate) frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredScalarBranchState {
    /// Structural edge bindings introduce independent storage homes before effects.
    pub(crate) structural_parameters: Vec<StructuralParameterDeclaration>,
    pub(crate) parameter_types: Vec<QualifiedScalarType>,
    /// Proof-only erased formals in authored order — the block's erased roster.
    pub(crate) erased_formal_types: Vec<QualifiedScalarType>,
    /// Erased proof-only formals the emitted block declares: the source
    /// state's roster for authored states, the enclosing caller roster for
    /// expansion states — `ProofTerm::Formal` positions index it.
    pub(crate) erased_proof_formals: Vec<terminal_psi::ErasedProofFormal>,
    pub(crate) bindings: Vec<LoweredScalarBinding>,
    /// Effects execute after the scalar prefix, without creating scalar slots.
    pub(crate) structural_effects: Vec<LoweredScalarEffect>,
    pub(crate) terminator: LoweredScalarBranchTerminator,
}

impl LoweredScalarBranchState {
    /// A jump-only expansion state continuing to `target`, binding the
    /// parameters produced from `argument_types`.
    pub(crate) fn jumping(
        target: usize,
        structural_parameters: Vec<StructuralParameterDeclaration>,
        parameter_types: Vec<QualifiedScalarType>,
        argument_types: &[QualifiedScalarType],
        structural_effects: Vec<LoweredScalarEffect>,
    ) -> Self {
        Self {
            structural_parameters,
            parameter_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            structural_effects,
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: crate::scalar_lowering::computations::parameters(argument_types),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        }
    }
}

/// An expression expansion reserves target zero for its caller's completion,
/// then addresses state n at target n + 1.
fn state_position(target: usize, state_count: usize) -> Result<Option<usize>, LoweringError> {
    let position = target.checked_sub(1);
    if position.is_some_and(|position| position >= state_count) {
        return Err(LoweringError::Unsupported(
            "scalar graph target has no prepared state",
        ));
    }
    Ok(position)
}

/// Allocate producer identities in control-flow order, not backward
/// continuation-construction order. Reverse-postorder keeps dominating owners
/// before their later consumers and gives independent cleanup reverse production
/// order for calls and constructors alike. State identities do not move.
/// Unreachable components remain retained in deterministic state order because
/// their declaration rosters still belong to the prepared graph.
pub(crate) fn scalar_state_emission_order(
    states: &[LoweredScalarBranchState],
    entry: usize,
) -> Result<Vec<usize>, LoweringError> {
    let entry = state_position(entry, states.len())?;
    let mut visited = vec![false; states.len()];
    let mut pending = Vec::new();
    let mut component = Vec::new();
    let mut order = Vec::with_capacity(states.len());
    for start in entry.into_iter().chain(0..states.len()) {
        if visited[start] {
            continue;
        }
        pending.push((start, false));
        while let Some((position, completing)) = pending.pop() {
            if completing {
                component.push(position);
                continue;
            }
            if visited[position] {
                continue;
            }
            visited[position] = true;
            pending.push((position, true));
            let mut push_target = |target| -> Result<(), LoweringError> {
                if let Some(position) = state_position(target, states.len())? {
                    pending.push((position, false));
                }
                Ok(())
            };
            match &states[position].terminator {
                LoweredScalarBranchTerminator::Jump { target, .. }
                | LoweredScalarBranchTerminator::Qualify { target, .. } => push_target(*target)?,
                LoweredScalarBranchTerminator::Conditional {
                    when_true_target,
                    when_false_target,
                    ..
                }
                | LoweredScalarBranchTerminator::CaseDispatch {
                    when_true_target,
                    when_false_target,
                    ..
                } => {
                    push_target(*when_false_target)?;
                    push_target(*when_true_target)?;
                }
                LoweredScalarBranchTerminator::CaseDispatchSplit {
                    armed,
                    fallback_target,
                    ..
                } => {
                    push_target(*fallback_target)?;
                    for arm in armed {
                        push_target(arm.target)?;
                    }
                }
            }
        }
        order.extend(component.drain(..).rev());
    }
    Ok(order)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarEffect {
    QuotientCall {
        value: checked_trees::CheckedStructuralValueHandle,
        arguments: Vec<(terminal_psi::StructuralArgument, StructuralTypeId)>,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    QuotientAdaptation {
        value: checked_trees::CheckedStructuralValueHandle,
        source: PlaceId,
        source_type: StructuralTypeId,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    EstablishReference {
        storage: checked_trees::CheckedReferenceStorage,
        backing_type: typed_trees::types::TypeReferenceHandle,
        qualification_decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        state: symbols::SymbolHandle,
        statement: u32,
        expression: checked_trees::expression::ExpressionHandle,
        place: PlaceId,
        structural_type: StructuralTypeId,
        source: terminal_psi::StructuralArgument,
    },
    /// Snapshot a copyable value while retaining its checked carrier custody.
    CopyStructuralValue {
        source: crate::expression_preparation::bindings::PreparedStructuralCopy,
        place: PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    },
    EstablishAggregate(branch_states::structural_values::Construction),
    /// A declared-only local's zeroed whole-shape establishment: a fresh home
    /// with no field operands, realized by the dedicated zeroed operation.
    EstablishZeroedStructural {
        place: PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        claims: Vec<crate::emission::claim_bindings::PreparedClaim>,
    },
    EstablishScalarArray(LoweredScalarArrayConstruction),
    EstablishScalarHome {
        place: PlaceId,
        structural_type: StructuralTypeId,
        value: LoweredDirectExpression,
    },
    EstablishElementView(crate::unit::attached_unit::view_ranges::CollectionView),
    EstablishScalarCase(computations::cases::Construction),
    /// Establish a literal's reserved image descriptor or fresh owned backing
    /// at its authored operand position, before later arguments evaluate.
    EstablishByteSequenceLiteral {
        literal: crate::emission::structural_byte_sequence_store::ByteLiteral,
        memberships: Vec<language_semantics::SemanticDomainId>,
    },
    /// Narrow either descriptor family through the same captured operand schedule.
    ViewSubslice {
        source: crate::unit::attached_unit::view_ranges::ViewRangeSource,
        start: LoweredDirectExpression,
        end: Option<LoweredDirectExpression>,
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Call(LoweredEffectCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredEffectCall {
    pub(crate) result: Option<crate::unit::attached_unit::call_results::Prepared>,
    pub(crate) endpoint: EffectCallTarget,
    pub(crate) claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
    pub(crate) checked_coordinate: checked_trees::CheckedUnitCallCoordinate,
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) source_site: Option<checked_trees::NominalMachineUseSite>,
    pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) target_state: symbols::SymbolHandle,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments:
        Vec<crate::emission::operation_emission::calls::PreparedStructuralArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EffectCallTarget {
    Module {
        erased_proof_arguments: Vec<LoweredProofTerm>,
        crash_routes: Vec<checked_trees::CrashRouteBucket>,
    },
    Boundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredScalarArrayConstruction {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) elements: Vec<LoweredDirectExpression>,
}
