use super::*;
use checked_trees::expression::ExpressionNode;

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    // This unit boundary tests source-to-Terminal custody. Omega separately
    // rejoins these opaque commitments to actual selected ProviderPlans.
    let handles = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let selected = checked.facts.operators.uses.get_mut(handle);
        selected.provider_plan_report_fingerprint = 7;
        selected.provider_plan_commitment =
            checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]);
    }
    checked
}

#[test]
fn ordinary_selected_float_comparisons_emit_one_exact_operation() {
    for (token, name) in [
        ("==", "equal"),
        ("!=", "not_equal"),
        ("<", "less"),
        ("<=", "less_or_equal"),
        (">", "greater"),
        (">=", "greater_or_equal"),
    ] {
        for format in ["f32", "f64"] {
            let source = format!(
                "boundary operator {token} Float::{name}(left: {format}, right: {format}) -> bool; machine choose(left: {format}, right: {format}) -> bool {{ left {token} right }}"
            );
            let checked = checked(&source);
            let lowered =
                crate::lower_machine(&checked, "choose").expect("selected comparison lowers");
            assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 1);
            assert_eq!(
                lowered
                    .semantic_module
                    .machines
                    .iter()
                    .flat_map(|machine| &machine.blocks)
                    .flat_map(|block| &block.operations)
                    .filter(|operation| matches!(
                        operation.kind,
                        OperationKind::IeeeFloatCompare { .. }
                    ))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn match_comparison_custody_rejects_arm_and_provider_substitution() {
    let mut checked = checked(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool;
        machine identity(value: f32) -> f32 { value }
        machine choose(value: f32, first: f32, second: f32) -> u64 {
            match identity(value) { identity(first) -> 7, identity(second) -> 9, _ -> 11 }
        }",
    );
    let lowered =
        crate::lower_machine(&checked, "choose").expect("saved subject and ordered patterns");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 2);
    let original = lowered.selected_ieee_float_comparison_occurrences[0];
    let selected = *checked.facts.operators.uses.get(original.operator_use);
    checked
        .facts
        .operators
        .uses
        .get_mut(original.operator_use)
        .provider_plan_commitment = Default::default();
    assert!(crate::lower_machine(&checked, "choose").is_err());
    *checked.facts.operators.uses.get_mut(original.operator_use) = selected;
    let arms = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| {
            if let CheckedScalarComputationKind::Dispatch { arms, .. } = node.kind {
                Some(arms)
            } else {
                None
            }
        })
        .expect("retained dispatch");
    let retained = checked
        .facts
        .values
        .scalar_computations
        .dispatch_arms
        .span_mut(arms)
        .unwrap();
    let first_use = retained[0].equality_use;
    retained[0].equality_use = retained[1].equality_use;
    retained[1].equality_use = first_use;
    assert!(
        crate::lower_machine(&checked, "choose").is_err(),
        "arm equality cannot borrow a sibling occurrence"
    );
}

#[test]
fn selected_comparison_replay_rejects_swapped_operands_and_changed_source_operation() {
    let mut checked = checked(
        "boundary operator < Float::less(left: f64, right: f64) -> bool;
        machine choose(left: f64, right: f64) -> bool { left < right }",
    );
    let lowered = crate::lower_machine(&checked, "choose").unwrap();
    let use_handle = lowered.selected_ieee_float_comparison_occurrences[0].operator_use;
    let expression = checked.facts.operators.uses.get(use_handle).expression;
    let computation = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::SelectedComparison { .. }
            )
            .then_some(handle)
        })
        .unwrap();
    let original = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(computation)
        .kind
        .clone();
    if let CheckedScalarComputationKind::SelectedComparison { left, right, .. } = &mut checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(computation)
        .kind
    {
        std::mem::swap(left, right);
    }
    assert!(
        crate::lower_machine(&checked, "choose").is_err(),
        "same-typed operands retain authored order"
    );
    checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(computation)
        .kind = original;
    if let ExpressionNode::Binary(binary) =
        checked.typed.expression_table.expression_mut(expression)
    {
        binary.operator = checked_trees::expression::BinaryOperator::Greater;
    }
    assert!(
        crate::lower_machine(&checked, "choose").is_err(),
        "source operator cannot reuse selected less meaning"
    );
}

#[test]
fn selected_comparison_replays_origin_role_even_when_application_is_relabelled() {
    let mut checked = checked(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool;
        machine choose(left: f32, right: f32) -> bool { left == right }",
    );
    let lowered = crate::lower_machine(&checked, "choose").unwrap();
    let handle = lowered.selected_ieee_float_comparison_occurrences[0].operator_use;
    let selected = checked.facts.operators.uses.get_mut(handle);
    let original_site = selected.application_site();
    let checked_trees::CheckedValueOrigin::StateStatement { role, .. } = &mut selected.origin
    else {
        panic!("statement origin");
    };
    *role = checked_trees::CheckedValueStatementRole::LocalInitializer;
    let substituted_site = selected.application_site();
    for application in &mut checked.facts.operators.boundary_applications {
        if application.site == original_site {
            application.site = substituted_site;
        }
    }
    assert!(
        crate::lower_machine(&checked, "choose").is_err(),
        "origin role is replayed from the actual statement"
    );
}
