//! Prepared scalar control graph shared by graph emission and call-closure assembly.
//!
//! States retain ordered bindings, structural effects and outgoing transfers. This
//! is producer working state, not a portable representation or previous-stage wrapper.

use super::{KnownDirectScalar, cycles};
use crate::psi_lowering::operation_emission::LoweredScalarBinding;
use crate::psi_lowering::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::psi_lowering::operation_emission::buffer::SourceCallCoordinate;
use crate::psi_lowering::operation_emission::expressions::LoweredDirectExpression;
use crate::psi_lowering::{
    LoweredContentIdentityReshuffles, LoweredContentPartitionCompositions, scalar_computations,
    scalar_graph_lowering,
};
use checked_trees::{CheckedBooleanExpression, ClosedScalarValueContractPlan};
use semantic_vocabulary::{
    ClaimId, PlaceId, QualifiedScalarType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    CrashCause as TerminalCrashCause, StructuralArgument, StructuralParameterDeclaration,
    StructuralPathSegment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::psi_lowering) enum LoweredScalarBranchTerminator {
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
        structural_arguments: Vec<StructuralArgument>,
        trivial_affine_discards: Vec<PlaceId>,
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
pub(in crate::psi_lowering) struct LoweredCrashExit {
    pub(in crate::psi_lowering) cause: TerminalCrashCause,
    pub(in crate::psi_lowering) site_guard: Vec<CheckedBooleanExpression>,
    pub(in crate::psi_lowering) frontier_lower_bound: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::psi_lowering) struct LoweredScalarBranchState {
    /// Structural edge bindings introduce independent storage homes before effects.
    pub(in crate::psi_lowering) structural_parameters: Vec<StructuralParameterDeclaration>,
    pub(in crate::psi_lowering) parameter_types: Vec<QualifiedScalarType>,
    pub(in crate::psi_lowering) bindings: Vec<LoweredScalarBinding>,
    /// Effects execute after the scalar prefix, without creating scalar slots.
    pub(in crate::psi_lowering) structural_effects: Vec<LoweredScalarEffect>,
    pub(in crate::psi_lowering) terminator: LoweredScalarBranchTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::psi_lowering) enum LoweredScalarEffect {
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
pub(in crate::psi_lowering) struct LoweredUnitCall {
    pub(in crate::psi_lowering) source_coordinate: SourceCallCoordinate,
    pub(in crate::psi_lowering) target_machine: symbols::SymbolHandle,
    pub(in crate::psi_lowering) target_state: symbols::SymbolHandle,
    pub(in crate::psi_lowering) arguments: Vec<LoweredDirectExpression>,
    pub(in crate::psi_lowering) structural_arguments: Vec<StructuralArgument>,
    pub(in crate::psi_lowering) crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::psi_lowering) struct LoweredScalarArrayConstruction {
    pub(in crate::psi_lowering) place: PlaceId,
    pub(in crate::psi_lowering) structural_type: StructuralTypeId,
    pub(in crate::psi_lowering) elements: Vec<LoweredDirectExpression>,
}

pub(in crate::psi_lowering) struct PreparedScalarMachine {
    pub(in crate::psi_lowering) source_machine: symbols::SymbolHandle,
    pub(in crate::psi_lowering) states: Vec<LoweredScalarBranchState>,
    pub(in crate::psi_lowering) result_type: QualifiedScalarType,
    pub(in crate::psi_lowering) scalar_qualifications: terminal_psi::ScalarQualificationCatalog,
    pub(in crate::psi_lowering) contract: PreparedScalarContract,
    pub(in crate::psi_lowering) crash_routes: Vec<checked_trees::CrashRouteBucket>,
    pub(in crate::psi_lowering) identity_reshuffles: LoweredContentIdentityReshuffles,
    pub(in crate::psi_lowering) partition_compositions: LoweredContentPartitionCompositions,
    pub(in crate::psi_lowering) loop_plan: Option<cycles::ScalarLoopPlan>,
}

pub(in crate::psi_lowering) enum PreparedScalarContract {
    Empty,
    ClosedLiteral(KnownDirectScalar),
    Predicates(ClosedScalarValueContractPlan),
}

impl PreparedScalarContract {
    pub(in crate::psi_lowering) fn requirement_count(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::ClosedLiteral(_) => 1,
            // The predicate plan's source clauses are published as one
            // canonical conjunction, including implicit parameter ranges.
            Self::Predicates(plan) => usize::from(!plan.requires().is_empty()),
        }
    }
}
