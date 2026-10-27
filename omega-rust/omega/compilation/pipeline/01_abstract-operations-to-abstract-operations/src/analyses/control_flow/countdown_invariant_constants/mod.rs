//! Optimizer module role: executable entrance. Exact countdown invariant-constant coordination.

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    EffectLink, FuelSettlement, NodeLocation, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ValueDefinition, ValueDefinitionSite,
};

use crate::validation::ValidatedOptimizerCycleComponents;
use optimization_unit::OptimizerUnsignedCountdownRankingCertificate;
use semantic_vocabulary::{
    BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::{
    AnalysisRoot, AnalysisRootError, CountedLoopAnalysisError, UnsignedCountdownLoopSummary,
    ValidatedCountedLoopAnalysis, validate_analysis_root,
};

mod compute;
mod replay;
mod validate;

pub(crate) fn analyze_countdown_invariant_constants(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError> {
    let candidate = compute::propose(unit, custody, counted)?;
    validate::accept(unit, custody, counted, &candidate)
}

/// Closed semantic role of an input-free integer constant retained by the
/// exact unsigned-countdown ranking relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CountdownInvariantConstantRole {
    PositiveGuardZero,
    BackedgeDecrementOne,
}

/// One exact source node that is invariant across every iteration of its
/// authenticated countdown component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantIntegerConstant {
    pub role: CountdownInvariantConstantRole,
    pub location: NodeLocation,
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub definition: ValueDefinition,
    pub provenance: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
}

/// Exact invariant rows for one independently authenticated counted loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsignedCountdownInvariantConstants {
    pub counted_loop: UnsignedCountdownLoopSummary,
    /// The only destination a future hoist may consider. This analysis does
    /// not authorize insertion there or mutation of the component.
    pub prospective_preheader: BlockId,
    pub constants: Vec<CountdownInvariantIntegerConstant>,
}

/// Replayable analysis facts without cyclic-rewrite or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantAnalysisSnapshot {
    pub root: AnalysisRoot,
    pub loops: Vec<UnsignedCountdownInvariantConstants>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedCountdownInvariantConstantAnalysis {
    snapshot: CountdownInvariantConstantAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantAnalysis {
    const fn new(snapshot: CountdownInvariantConstantAnalysisSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn loops(&self) -> &[UnsignedCountdownInvariantConstants] {
        &self.snapshot.loops
    }

    pub const fn snapshot(&self) -> &CountdownInvariantConstantAnalysisSnapshot {
        &self.snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountdownInvariantConstantAnalysisError {
    CountedLoop(CountedLoopAnalysisError),
    Root(AnalysisRootError),
    ComponentRosterMismatch,
    UnsupportedInvariantConstant {
        machine: MachineId,
        operation: OperationId,
    },
    SnapshotMismatch,
}

failed_display!(
    CountdownInvariantConstantAnalysisError,
    "countdown invariant-constant analysis failure"
);

impl std::error::Error for CountdownInvariantConstantAnalysisError {}
fn validate_locations(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constants: &[CountdownInvariantIntegerConstant; 2],
) -> Result<(), CountdownInvariantConstantAnalysisError> {
    validate_moved_constant_locations(function, component, certificate, constants, unsupported)
}

/// Both countdown analyses verify that any constant which moved out of its
/// original block now sits in the single preheader's trailing instruction
/// suffix; `unsupported` builds each analysis's own error variant.
pub(crate) fn validate_moved_constant_locations<E>(
    function: &PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    constants: &[CountdownInvariantIntegerConstant; 2],
    unsupported: impl Fn(MachineId, OperationId) -> E,
) -> Result<(), E> {
    let original_blocks = [certificate.header, certificate.descent.backedge.source];
    if constants
        .iter()
        .zip(original_blocks)
        .all(|(constant, original)| constant.location.block == original)
    {
        return Ok(());
    }
    let [entry] = component.entries.as_slice() else {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    };
    if component.members.contains(&entry.source) || entry.target != certificate.header {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    let preheader = function
        .block(entry.source)
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    let jump_index = preheader
        .nodes
        .len()
        .checked_sub(1)
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    let O::Jump {
        psi_edge, target, ..
    } = preheader.nodes[jump_index].operation
    else {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    };
    if psi_edge != entry.edge || target != certificate.header {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    let moved = constants
        .iter()
        .zip(original_blocks)
        .filter_map(|(constant, original)| {
            (constant.location.block != original).then_some(constant)
        })
        .collect::<Vec<_>>();
    let suffix_start = jump_index
        .checked_sub(moved.len())
        .ok_or_else(|| unsupported(function.machine, certificate.guard.zero_operation))?;
    if moved.iter().enumerate().any(|(offset, constant)| {
        constant.location.machine != function.machine
            || constant.location.block != preheader.id
            || usize::try_from(constant.location.node).ok() != Some(suffix_start + offset)
    }) {
        return Err(unsupported(
            function.machine,
            certificate.guard.zero_operation,
        ));
    }
    Ok(())
}

fn unsupported(
    machine: MachineId,
    operation: OperationId,
) -> CountdownInvariantConstantAnalysisError {
    CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant { machine, operation }
}
