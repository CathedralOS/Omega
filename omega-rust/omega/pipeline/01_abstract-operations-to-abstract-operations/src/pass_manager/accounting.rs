use std::collections::BTreeMap;

use optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationPassManifestRecord,
    OptimizationUnitIdentity, OptimizationWorkUsage,
};
use terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit;

use crate::OrderedRuleRegistry;

use super::{OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit};

pub(super) fn register_revision(
    seen: &mut BTreeMap<OptimizationUnitIdentity, u64>,
    identity: OptimizationUnitIdentity,
    iteration: u64,
) -> Result<(), OptimizationRunError> {
    if let Some(first_seen_iteration) = seen.get(&identity).copied() {
        return Err(OptimizationRunError::OscillatingRevision {
            identity,
            first_seen_iteration,
            repeated_at_iteration: iteration,
        });
    }
    seen.insert(identity, iteration);
    Ok(())
}

pub(super) fn build_pass_manifest(
    registry: &OrderedRuleRegistry,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    commits: &[PsiOptimizationCommit],
    decisions: &[OptimizationDecisionRecord],
    usage: OptimizationRunUsage,
) -> Result<Option<OptimizationPassManifestRecord>, OptimizationRunError> {
    let Some(pass) = registry.pass() else {
        return Ok(None);
    };
    let contracts = registry.contracts().collect::<Vec<_>>();
    let ordered_rules = contracts
        .iter()
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    for commit in commits {
        assert!(
            decisions.iter().any(|decision| {
                decision.candidate() == commit.candidate
                    && decision.verdict() == OptimizationCandidateVerdict::Applied
            }),
            "every committed candidate has an applied manifest decision"
        );
    }
    OptimizationPassManifestRecord::new(
        pass,
        input,
        output,
        registry.identity(),
        ordered_rules,
        decisions.to_vec(),
        OptimizationWorkUsage {
            rule_evaluations: usage.rule_evaluations,
            candidates: usage.candidates,
            validation_steps: usage.validation_steps,
            commits: usage.commits,
            iterations: usage.iterations,
        },
    )
    .map(Some)
    .map_err(OptimizationRunError::InvalidManifest)
}

pub(super) fn charge(
    counter: &mut u64,
    limit: u64,
    axis: &'static str,
) -> Result<(), OptimizationRunError> {
    if *counter == limit {
        return Err(OptimizationRunError::WorkBudgetExhausted(axis));
    }
    *counter += 1;
    Ok(())
}

pub(super) fn add_usage(
    left: OptimizationRunUsage,
    right: OptimizationRunUsage,
) -> Result<OptimizationRunUsage, OptimizationRunError> {
    Ok(OptimizationRunUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
    })
}

fn integer_evaluation_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanNot { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
            )
        })
        .count()
        .try_into()
        .expect("operation count fits u64")
}

fn block_parameter_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| u64::try_from(block.parameters.len()).expect("parameter count fits u64"))
        .sum()
}

fn dead_total_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerConstant { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanConstant { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanNot { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
            )
        })
        .count()
        .try_into()
        .expect("dead-total scalar operation count fits u64")
}

fn proof_certified_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerAdd { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
            )
        })
        .count()
        .try_into()
        .expect("proof-certified scalar operation count fits u64")
}

fn control_flow_structure_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .map(|function| {
            1 + u64::try_from(function.blocks.len()).expect("block count fits u64")
                + function
                    .blocks
                    .iter()
                    .map(|block| {
                        u64::try_from(block.nodes.len()).expect("node count fits u64")
                            + block
                                .nodes
                                .iter()
                                .map(|node| {
                                    u64::try_from(node.successors.len())
                                        .expect("successor count fits u64")
                                })
                                .sum::<u64>()
                    })
                    .sum::<u64>()
        })
        .sum()
}

pub(super) fn convergence_measure(
    unit: &PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
) -> u64 {
    let copy_pass = optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.copy-propagation.v1",
    );
    let cfg_pass = optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.control-flow-cleanup.v13",
    );
    let dead_scalar_pass = optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.dead-pure-scalar-elimination.v2",
    );
    let proof_elision_pass = optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.proof-check-elision.v12",
    );
    let global_value_numbering_pass =
        optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v14",
        );
    let state_specialization_pass =
        optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.state-specialization.v1",
        );
    let representation_specialization_pass =
        optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.representation-specialization.v1",
        );
    if registry.pass() == Some(cfg_pass) {
        control_flow_structure_count(unit)
    } else if registry.pass() == Some(copy_pass) {
        block_parameter_count(unit)
    } else if registry.pass() == Some(dead_scalar_pass) {
        dead_total_scalar_operation_count(unit)
    } else if registry.pass() == Some(proof_elision_pass) {
        proof_certified_scalar_operation_count(unit)
    } else if registry.pass() == Some(global_value_numbering_pass) {
        unit.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .map(|block| block.nodes.len() as u64)
            .sum()
    } else if registry.pass() == Some(state_specialization_pass) {
        dispatch_chain_depth_measure(unit)
    } else if registry.pass() == Some(representation_specialization_pass) {
        representation_observation_count(unit)
    } else {
        integer_evaluation_operation_count(unit)
    }
}

/// Non-increasing convergence measure for the representation-specialization
/// pass: the count of structural observations left to fold —
/// `StructuralCaseMembership`, `BooleanStructuralField`, and
/// `IntegerStructuralField`. Every committed candidate rewrites each admitted
/// observation into a constant, so a commit strictly lowers the measure and a
/// fixed point is reached when no proven observation remains.
fn representation_observation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::StructuralCaseMembership { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::BooleanStructuralField { .. }
                    | terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation::IntegerStructuralField { .. }
            )
        })
        .count() as u64
}

/// Non-decreasing convergence measure for the state-specialization pass: the
/// sum, over every edge, of the dispatch-nesting depth of its target — where a
/// `Conditional`-terminated block (a direct parameter dispatch, or an integer
/// state argument's `parameter CMP literal` dispatch trailing its pure
/// scalar-computation prefix) contributes `1 + max` of its arm targets'
/// depths and any other block contributes `0`. Fusing one constant-supplied
/// incoming edge retargets it from a dispatch (depth `>= 1`) to a resolved
/// arm target (depth `< dispatch depth`), so every committed rewrite strictly
/// lowers the measure even when the resolved target is itself a dispatch.
/// Eligible machines are acyclic; an on-stack revisit contributes `0` so the
/// traversal is total on arbitrary input.
fn dispatch_chain_depth_measure(unit: &PsiOptimizationUnit) -> u64 {
    use std::collections::BTreeSet;

    use semantic_vocabulary::BlockId;
    use terminal_psi_to_abstract_operations::abstract_operations::AbstractOperation;
    use terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationFunction;

    fn depth(
        function: &PsiOptimizationFunction,
        block: BlockId,
        memo: &mut BTreeMap<BlockId, u64>,
        visiting: &mut BTreeSet<BlockId>,
    ) -> u64 {
        if let Some(cached) = memo.get(&block) {
            return *cached;
        }
        if !visiting.insert(block) {
            return 0;
        }
        let resolved = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .and_then(|owner| {
                let node = owner.nodes.last()?;
                match &node.operation {
                    AbstractOperation::Conditional { .. } => Some(
                        node.successors
                            .iter()
                            .map(|edge| depth(function, edge.target, memo, visiting))
                            .max()
                            .unwrap_or(0)
                            + 1,
                    ),
                    _ => None,
                }
            })
            .unwrap_or(0);
        visiting.remove(&block);
        memo.insert(block, resolved);
        resolved
    }

    unit.functions
        .iter()
        .map(|function| {
            let mut memo = BTreeMap::new();
            let mut visiting = BTreeSet::new();
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.successors)
                .map(|edge| depth(function, edge.target, &mut memo, &mut visiting))
                .sum::<u64>()
        })
        .sum()
}
