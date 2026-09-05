use std::collections::BTreeMap;

use optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationPassManifestRecord,
    OptimizationUnitIdentity, OptimizationWorkUsage,
};
use optimization_unit::PsiOptimizationUnit;

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
                abstract_operations::AbstractOperation::ExactIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | abstract_operations::AbstractOperation::BooleanNot { .. }
                    | abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
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
                abstract_operations::AbstractOperation::IntegerConstant { .. }
                    | abstract_operations::AbstractOperation::BooleanConstant { .. }
                    | abstract_operations::AbstractOperation::BooleanNot { .. }
                    | abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
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
                abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerAdd { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
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
    } else {
        integer_evaluation_operation_count(unit)
    }
}
