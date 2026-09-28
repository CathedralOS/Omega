//! Borrowed-byte edges retain source ranges and exact authored endpoint roles.
use super::CheckedScalarExpressionRole;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
};
use typed_trees::expression::ExpressionNode;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8); }
    machine writer(bytes: &[u8], marker: u8) reaches Output {
        transition bytes.len > 0 {
            true -> tail(marker, bytes[1..bytes.len])
            false -> done()
        }
        state tail(marker: u8, bytes: &[u8]) { Output::write(bytes, marker); }
        state done() {}
    }
"#;

fn range_source(
    checked: &checked_trees::CheckedTrees,
) -> typed_trees::expression::ExpressionHandle {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(checked, "writer"))
        .expect("borrowed-byte graph");
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &plan.states[0].terminator
    else {
        panic!("selected edge");
    };
    let CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
        expression,
    } = when_true.transfers[0].source
    else {
        panic!("exact source parameter and retained range");
    };
    expression
}

#[test]
fn byte_tail_plan_rejects_missing_duplicate_or_drifted_endpoint_custody() {
    let checked = checked(SOURCE);
    let machine = machine_named(&checked, "writer");
    let state = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap()
        .states[0]
        .state;
    for role in [
        CheckedScalarExpressionRole::SubsliceStart {
            site: checked_trees::CheckedSubsliceSite::TransitionArgument {
                argument_ordinal: 1,
            },
        },
        CheckedScalarExpressionRole::SubsliceEnd {
            site: checked_trees::CheckedSubsliceSite::TransitionArgument {
                argument_ordinal: 1,
            },
        },
    ] {
        for mutation in 0..5 {
            let mut facts = checked.facts.clone();
            let plans = &mut facts.values.scalar_expressions;
            let handle = plans
                .source_bindings
                .iter()
                .find(|(_, binding)| {
                    binding.state == state && binding.statement_ordinal == 0 && binding.role == role
                })
                .unwrap()
                .0;
            match mutation {
                0 => plans.source_bindings.get_mut(handle).expression = Default::default(),
                1 => plans.source_bindings.get_mut(handle).statement_ordinal = 1,
                2 => {
                    let duplicate = plans.source_bindings.get(handle).clone();
                    plans.source_bindings.append(duplicate);
                }
                3 => {
                    let binding = plans.source_bindings.get_mut(handle);
                    binding.destination = if binding.destination.is_valid() {
                        Default::default()
                    } else {
                        machine
                    };
                }
                _ => {
                    plans.source_bindings.get_mut(handle).role = CheckedScalarExpressionRole::Return
                }
            }
            let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
                &checked.typed,
                &facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns,
                },
            );
            assert!(
                rebuilt.composed_for_machine(machine).is_none(),
                "{role:?}: mutation {mutation}"
            );
        }
    }
}

#[test]
fn byte_tail_plan_rejects_selected_or_wrong_range_meaning_and_unknown_source() {
    let checked = checked(SOURCE);
    let expression = range_source(&checked);
    let machine = machine_named(&checked, "writer");
    let operator = checked
        .facts
        .operators
        .uses
        .iter()
        .find(|(_, selected)| selected.expression == expression)
        .expect("range operator fact")
        .0;
    for mutation in 0..5 {
        let mut facts = checked.facts.clone();
        let selected = facts.operators.uses.get_mut(operator);
        match mutation {
            0 => selected.spelling = language_core::OperatorSpelling::Index,
            1 => selected.selected_operator_symbol = machine,
            2 => selected.candidate_count = 1,
            3 => selected.status = checked_trees::CheckedOperatorResolutionStatus::Resolved,
            _ => selected.status = checked_trees::CheckedOperatorResolutionStatus::DomainPending,
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.composed_for_machine(machine).is_none(),
            "operator mutation {mutation}"
        );
    }
    let mut typed = checked.typed.clone();
    let ExpressionNode::Indexed(indexed) = typed.expression_table.expression(expression) else {
        panic!("range")
    };
    let collection = indexed.collection;
    let ExpressionNode::Name(path) = typed.expression_table.expression_mut(collection) else {
        panic!("parameter")
    };
    path.symbol = machine;
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.composed_for_machine(machine).is_none(),
        "unknown source parameter"
    );
    let mut typed = checked.typed.clone();
    let ExpressionNode::Indexed(indexed) = typed.expression_table.expression(expression) else {
        panic!("range");
    };
    let range_handle = indexed.index;
    let ExpressionNode::Range(range) = typed.expression_table.expression_mut(range_handle) else {
        panic!("endpoints");
    };
    range.end_inclusive = true;
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &typed,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.composed_for_machine(machine).is_none(),
        "authored inclusive range"
    );
}
