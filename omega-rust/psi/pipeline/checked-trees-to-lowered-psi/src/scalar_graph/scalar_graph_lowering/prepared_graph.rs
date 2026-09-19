//! Prepared scalar control graph shared by graph emission and call-closure assembly.
//!
//! States retain ordered bindings, structural effects and outgoing transfers. This
//! is producer working state, not a portable representation or previous-stage wrapper.

use super::{KnownDirectScalar, cycles};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::proofs::content_conservation::LoweredContentIdentityReshuffles;
use crate::proofs::content_conservation::LoweredContentPartitionCompositions;
use crate::scalar_graph::scalar_computations;
use crate::scalar_graph::scalar_graph_lowering;
use checked_trees::{
    CheckedBooleanExpression, ClosedScalarContractValue, ClosedScalarValueContractPlan,
};
use semantic_vocabulary::{
    ClaimId, PlaceId, QualifiedScalarType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    CrashCause as TerminalCrashCause, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarBranchTerminator {
    /// A representation-identical transfer establishing the final argument's
    /// explicit qualification on a fresh successor parameter, not its source.
    Qualify {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
        structural_arguments: Vec<StructuralArgument>,
    },
    Jump {
        target: usize,
        arguments: Vec<LoweredDirectExpression>,
        /// Proof-only erased actuals for the successor state's erased roster.
        erased_arguments: Vec<LoweredDirectExpression>,
        structural_arguments: Vec<StructuralArgument>,
        trivial_affine_discards: Vec<PlaceId>,
    },
    Conditional {
        condition: LoweredBooleanReturnExpression,
        when_true_target: usize,
        when_true_arguments: Vec<LoweredDirectExpression>,
        when_true_erased_arguments: Vec<LoweredDirectExpression>,
        when_false_target: usize,
        when_false_arguments: Vec<LoweredDirectExpression>,
        when_false_erased_arguments: Vec<LoweredDirectExpression>,
    },
    Return {
        expression: LoweredDirectExpression,
    },
    Crash(LoweredCrashExit),
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
    pub(crate) bindings: Vec<LoweredScalarBinding>,
    /// Effects execute after the scalar prefix, without creating scalar slots.
    pub(crate) structural_effects: Vec<LoweredScalarEffect>,
    pub(crate) terminator: LoweredScalarBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarEffect {
    StoreScalarField {
        destination: PlaceId,
        path: Vec<StructuralPathSegment>,
        field: StructuralFieldId,
        value_position: usize,
        scalar_type: ScalarType,
    },
    EstablishRecord(scalar_graph_lowering::structural_values::Construction),
    EstablishScalarArray(LoweredScalarArrayConstruction),
    EstablishScalarCase(scalar_computations::cases::Construction),
    CallUnit(LoweredUnitCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredUnitCall {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) target_state: symbols::SymbolHandle,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredScalarArrayConstruction {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) elements: Vec<LoweredDirectExpression>,
}

pub(crate) struct PreparedScalarMachine {
    pub(crate) source_machine: symbols::SymbolHandle,
    pub(crate) states: Vec<LoweredScalarBranchState>,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) scalar_qualifications: terminal_psi::ScalarQualificationCatalog,
    pub(crate) contract: PreparedScalarContract,
    pub(crate) crash_routes: Vec<checked_trees::CrashRouteBucket>,
    pub(crate) identity_reshuffles: LoweredContentIdentityReshuffles,
    pub(crate) partition_compositions: LoweredContentPartitionCompositions,
    pub(crate) loop_plan: Option<cycles::ScalarLoopPlan>,
}

pub(crate) enum PreparedScalarContract {
    Empty,
    ClosedLiteral(KnownDirectScalar),
    Predicates(ClosedScalarValueContractPlan),
}

impl PreparedScalarContract {
    pub(crate) fn requirement_count(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::ClosedLiteral(_) => 1,
            // The predicate plan's source clauses are published as one
            // canonical conjunction, including implicit integer parameter
            // ranges. Floating entry range clauses publish no proposition:
            // they discharge through the retained catalog rows at call
            // delivery, so a range-only requires tail contributes no
            // obligation.
            Self::Predicates(plan) => usize::from(plan.requires().iter().any(|clause| {
                matches!(
                    clause,
                    Some(
                        ClosedScalarContractValue::Predicate(_)
                            | ClosedScalarContractValue::Boolean(_)
                            | ClosedScalarContractValue::Integer(_)
                    )
                )
            })),
        }
    }
}
