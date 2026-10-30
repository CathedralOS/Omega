//! Optimizer module role: executable entrance. Exact countdown constant-placement coordination.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, ValueUse,
};

use crate::validation::ValidatedOptimizerCycleComponents;
use optimization_unit::{
    CycleComponentEdge, CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use super::{
    AnalysisRoot, AnalysisRootError, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantRole, CountdownInvariantIntegerConstant, CountedLoopAnalysisError,
    UnsignedCountdownLoopSummary, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountedLoopAnalysis, expected_constant, invariant_constant_row, sole_provenance_node,
    validate_analysis_root, validate_moved_constant_locations,
};

mod compute;
mod replay;
mod validate;

pub(crate) fn analyze_countdown_invariant_constant_placement(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    let candidate = compute::propose(unit, custody, counted, invariants)?;
    validate::accept(unit, custody, counted, invariants, &candidate)
}

/// The exact insertion coordinate immediately before the unique preheader jump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantDestination {
    pub before: NodeLocation,
    pub entry_edge: CycleComponentEdge,
}

/// The exact certificate operation that consumes one invariant constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantConsumer {
    pub location: NodeLocation,
    pub psi_operation: OperationId,
    pub value_use: ValueUse,
}

/// One analysis-only placement fact. This is not a rewrite plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantPlacement {
    pub constant: CountdownInvariantIntegerConstant,
    pub destination: CountdownInvariantConstantDestination,
    pub consumer: CountdownInvariantConstantConsumer,
}

/// Exact zero/one placement facts keyed by their authenticated component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsignedCountdownInvariantConstantPlacements {
    pub component: CycleComponentId,
    pub counted_loop: UnsignedCountdownLoopSummary,
    pub placements: Vec<CountdownInvariantConstantPlacement>,
}

/// Replayable placement facts without mutation or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantPlacementAnalysisSnapshot {
    pub root: AnalysisRoot,
    pub loops: Vec<UnsignedCountdownInvariantConstantPlacements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedCountdownInvariantConstantPlacementAnalysis {
    snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantPlacementAnalysis {
    const fn new(snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot) -> Self {
        Self { snapshot }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountdownInvariantConstantPlacementAnalysisError {
    CountedLoop(CountedLoopAnalysisError),
    InvariantConstant(CountdownInvariantConstantAnalysisError),
    Root(AnalysisRootError),
    ComponentRosterMismatch,
    UnsupportedPlacement {
        machine: MachineId,
        operation: OperationId,
    },
    SnapshotMismatch,
}

failed_display!(
    CountdownInvariantConstantPlacementAnalysisError,
    "countdown invariant-constant placement analysis failure"
);

/// The certificate operation that consumes one invariant constant: the guard
/// comparison reading the zero, or the backedge decrement reading the one.
pub(crate) fn consumer_operation(
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> OperationId {
    match role {
        CountdownInvariantConstantRole::PositiveGuardZero => certificate.guard.comparison_operation,
        CountdownInvariantConstantRole::BackedgeDecrementOne => {
            certificate.descent.subtract_operation
        }
    }
}

/// Whether `node` is exactly `role`'s consumer of `constant`: the guard
/// `rank < 0` comparison carrying the certificate's operation, condition,
/// constant and rank parameter, or the backedge `rank - 1` exact subtraction
/// carrying its operation, descent obligation, argument, rank type, source
/// parameter and constant. Any other operation, and a decrement under another
/// overflow policy or operator, is not the consumer the certificate names.
pub(crate) fn consumer_matches(
    node: &OptimizationNode,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
    operation: OperationId,
    constant: ValueId,
) -> bool {
    match (role, &node.operation) {
        (
            CountdownInvariantConstantRole::PositiveGuardZero,
            O::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            },
        ) => {
            (*psi_operation, *result, *left, *right)
                == (
                    operation,
                    certificate.guard.condition,
                    constant,
                    certificate.rank_parameter,
                )
        }
        (
            CountdownInvariantConstantRole::BackedgeDecrementOne,
            O::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            },
        ) if (arithmetic.policy(), arithmetic.operator())
            == (
                IntegerOverflowPolicy::Exact,
                IntegerArithmeticOperator::Subtract,
            ) =>
        {
            (
                *psi_operation,
                arithmetic.obligation(),
                *result,
                *scalar_type,
                *left,
                *right,
            ) == (
                operation,
                Some(certificate.descent.subtract_obligation),
                certificate.descent.argument,
                certificate.rank_type,
                certificate.descent.source_parameter,
                constant,
            )
        }
        _ => false,
    }
}
