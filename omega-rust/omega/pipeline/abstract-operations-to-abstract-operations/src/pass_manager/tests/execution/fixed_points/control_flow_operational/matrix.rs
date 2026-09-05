//! Exact operational roster for the seven control-flow cleanup transformations.

use super::custody::{Case, assert_operational_custody};
use super::fixtures::{
    constant_merge_barrier_unit, isolated_shared_terminal_unit, linear_shared_target_unit,
    path_qualified_direct_edges_unit, terminal_non_adjacent_merge_unit,
    unreachable_private_machine_unit,
};
use crate::rules::tests::adjacent_conditional_merge_unit;
use crate::rules::{PathQualifiedEmptyBlockThreadRule, UnreachablePrivateMachinePruneRule};
use crate::{
    AdjacentBlockMergeRule, ConstantConditionalFoldRule, LinearEmptyBlockThreadRule,
    NonAdjacentBlockMergeRule, SharedJumpFusionRule,
};

fn validator(domain: &[u8]) -> optimization_core::OptimizationValidatorIdentity {
    optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(domain)
}

#[test]
fn every_control_flow_cleanup_rule_has_whole_engine_operational_custody() {
    assert_operational_custody(vec![
        Case {
            roster_position: 0,
            unit: constant_merge_barrier_unit(),
            rule: ConstantConditionalFoldRule::contract().identity(),
            validator: validator(b"omega.validator.constant-conditional-fold.v4"),
            predicted_cost_delta: -1,
            consumed_fact_count: 1,
        },
        Case {
            roster_position: 1,
            unit: linear_shared_target_unit(),
            rule: LinearEmptyBlockThreadRule::contract().identity(),
            validator: validator(b"omega.validator.linear-empty-block-thread.v2"),
            predicted_cost_delta: -3,
            consumed_fact_count: 0,
        },
        Case {
            roster_position: 2,
            unit: path_qualified_direct_edges_unit(),
            rule: PathQualifiedEmptyBlockThreadRule::contract().identity(),
            validator: validator(b"omega.validator.path-qualified-empty-block-thread.v1"),
            predicted_cost_delta: -3,
            consumed_fact_count: 0,
        },
        Case {
            roster_position: 3,
            unit: adjacent_conditional_merge_unit(),
            rule: AdjacentBlockMergeRule::contract().identity(),
            validator: validator(b"omega.validator.adjacent-single-predecessor-block-merge.v5"),
            predicted_cost_delta: -2,
            consumed_fact_count: 0,
        },
        Case {
            roster_position: 4,
            unit: isolated_shared_terminal_unit(),
            rule: SharedJumpFusionRule::contract().identity(),
            validator: validator(b"omega.validator.shared-terminal-jump-fusion.v2"),
            predicted_cost_delta: -1,
            consumed_fact_count: 0,
        },
        Case {
            roster_position: 5,
            unit: unreachable_private_machine_unit(),
            rule: UnreachablePrivateMachinePruneRule::contract().identity(),
            validator: validator(b"omega.validator.unreachable-private-machine-pruning.v1"),
            predicted_cost_delta: -1,
            consumed_fact_count: 0,
        },
        Case {
            roster_position: 6,
            unit: terminal_non_adjacent_merge_unit(),
            rule: NonAdjacentBlockMergeRule::contract().identity(),
            validator: validator(b"omega.validator.non-adjacent-unique-predecessor-block-merge.v1"),
            predicted_cost_delta: -2,
            consumed_fact_count: 0,
        },
    ]);
}
