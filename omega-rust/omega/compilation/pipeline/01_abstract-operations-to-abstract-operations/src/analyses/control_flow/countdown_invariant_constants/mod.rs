//! Optimizer module role: executable entrance. Exact countdown invariant-constant coordination.

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    EffectLink, FuelSettlement, NodeLocation, OptimizationBlock, OptimizationNode,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, ValueDefinition,
    ValueDefinitionSite,
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
    // Every refusal below names the same certificate coordinate pair.
    let rejected = || unsupported(function.machine, certificate.guard.zero_operation);
    let [entry] = component.entries.as_slice() else {
        return Err(rejected());
    };
    if component.members.contains(&entry.source) || entry.target != certificate.header {
        return Err(rejected());
    }
    let preheader = function.block(entry.source).ok_or_else(rejected)?;
    let jump_index = preheader.nodes.len().checked_sub(1).ok_or_else(rejected)?;
    let O::Jump {
        psi_edge, target, ..
    } = preheader.nodes[jump_index].operation
    else {
        return Err(rejected());
    };
    if psi_edge != entry.edge || target != certificate.header {
        return Err(rejected());
    }
    let moved = constants
        .iter()
        .zip(original_blocks)
        .filter_map(|(constant, original)| {
            (constant.location.block != original).then_some(constant)
        })
        .collect::<Vec<_>>();
    let suffix_start = jump_index.checked_sub(moved.len()).ok_or_else(rejected)?;
    if moved.iter().enumerate().any(|(offset, constant)| {
        constant.location.machine != function.machine
            || constant.location.block != preheader.id
            || usize::try_from(constant.location.node).ok() != Some(suffix_start + offset)
    }) {
        return Err(rejected());
    }
    Ok(())
}

/// The certificate-owned expectation one closed constant role names: the
/// operation, result, scalar type and literal value the certified occurrence
/// must spell.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExpectedInvariantConstant {
    pub operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
}

/// The expectation a role names on its ranking certificate. Every leaf that
/// resolves a role — the constants analysis on both sides, and the placement
/// replay re-checking the rows it retains — reads the same mapping.
pub(crate) fn expected_constant(
    certificate: &OptimizerUnsignedCountdownRankingCertificate,
    role: CountdownInvariantConstantRole,
) -> ExpectedInvariantConstant {
    let (operation, result, value) = match role {
        CountdownInvariantConstantRole::PositiveGuardZero => (
            certificate.guard.zero_operation,
            certificate.guard.zero,
            IntegerValue::Unsigned(0),
        ),
        CountdownInvariantConstantRole::BackedgeDecrementOne => (
            certificate.descent.one_operation,
            certificate.descent.one,
            IntegerValue::Unsigned(1),
        ),
    };
    ExpectedInvariantConstant {
        operation,
        result,
        scalar_type: certificate.rank_type,
        value,
    }
}

/// The unique node `blocks` holds whose provenance names `operation` first:
/// `None` when none or several carry it. The keyed discovery both replay
/// leaves use; the proposal leaf reaches the same row through the operation
/// shape instead, so the acceptance replay keeps its own path to it.
pub(crate) fn sole_provenance_node<'function>(
    blocks: impl Iterator<Item = &'function OptimizationBlock>,
    operation: OperationId,
) -> Option<(BlockId, u32, &'function OptimizationNode)> {
    let mut matches = blocks
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .map(move |(node, value)| (block.id, node, value))
        })
        .filter(|(_, _, node)| {
            node.provenance
                .first()
                .is_some_and(|source| *source == PsiProvenance::Operation(operation))
        });
    let (block, node_index, node) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some((block, u32::try_from(node_index).ok()?, node))
}

/// The exact invariant-constant row a certified occurrence must be: the
/// `IntegerConstant` the role expects — its operation, result, scalar type
/// and literal value — carrying exactly one definition of that result at its
/// own site, and no uses, successors or ownership rows. One owner states the
/// obligation; each leaf supplies the occurrence its own discovery admitted
/// and maps `None` to its own rejection.
pub(crate) fn invariant_constant_row(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node_index: u32,
    node: &OptimizationNode,
    role: CountdownInvariantConstantRole,
    expected: ExpectedInvariantConstant,
) -> Option<CountdownInvariantIntegerConstant> {
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type: ScalarType::Integer(scalar_type),
        value,
    } = node.operation
    else {
        return None;
    };
    let [definition] = node.definitions.as_slice() else {
        return None;
    };
    if psi_operation != expected.operation
        || result != expected.result
        || scalar_type != expected.scalar_type
        || value != expected.value
        || definition.value != result
        || definition.scalar_type != ScalarType::Integer(scalar_type)
        || definition.site
            != (ValueDefinitionSite::Node {
                block,
                node: node_index,
            })
        || !node.uses.is_empty()
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return None;
    }
    Some(CountdownInvariantIntegerConstant {
        role,
        location: NodeLocation {
            machine: function.machine,
            block,
            node: node_index,
        },
        psi_operation,
        result,
        scalar_type,
        value,
        definition: *definition,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
    })
}

fn unsupported(
    machine: MachineId,
    operation: OperationId,
) -> CountdownInvariantConstantAnalysisError {
    CountdownInvariantConstantAnalysisError::UnsupportedInvariantConstant { machine, operation }
}
