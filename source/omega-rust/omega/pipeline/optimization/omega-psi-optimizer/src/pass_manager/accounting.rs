use std::collections::BTreeMap;

use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationPassManifestRecord,
    OptimizationUnitIdentity, OptimizationWorkUsage,
};
use omega_optimization_unit::PsiOptimizationUnit;

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

fn integer_evaluation_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                omega_abstract_operations::AbstractOperation::ExactIntegerAdd {
                    ..
                } | omega_abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | omega_abstract_operations::AbstractOperation::BooleanNot { .. }
                    | omega_abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
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
                omega_abstract_operations::AbstractOperation::IntegerConstant { .. }
                    | omega_abstract_operations::AbstractOperation::BooleanConstant { .. }
                    | omega_abstract_operations::AbstractOperation::BooleanNot { .. }
                    | omega_abstract_operations::AbstractOperation::BooleanEqual { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerEqual { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerLessThan { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerLessOrEqual { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseNot { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseAnd { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseOr { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerBitwiseXor { .. }
                    | omega_abstract_operations::AbstractOperation::IntegerWiden { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerShiftLeft { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerShiftRight { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerAdd { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerMultiply { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerAdd { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerMultiply { .. }
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
                omega_abstract_operations::AbstractOperation::IntegerExactCast { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerShiftLeft { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerShiftRight { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerAdd { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerSubtract { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerMultiply { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::ExactIntegerRemainder { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::WrappingIntegerRemainder { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerDivide { .. }
                    | omega_abstract_operations::AbstractOperation::SaturatingIntegerRemainder { .. }
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
    let copy_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.copy-propagation.v1",
    );
    let cfg_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.control-flow-cleanup.v13",
    );
    let dead_scalar_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.dead-pure-scalar-elimination.v2",
    );
    let proof_elision_pass =
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.proof-check-elision.v12",
        );
    let global_value_numbering_pass =
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v10",
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
