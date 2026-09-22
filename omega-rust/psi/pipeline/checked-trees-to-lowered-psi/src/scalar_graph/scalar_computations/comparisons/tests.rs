use super::super::super::OperationKind;
use super::super::CheckedScalarComputationKind;
use crate::TerminalMachineSelection;
use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionNode;

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
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
            let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
                .expect("selected comparison lowers");
            assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 1);
            let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
                .produce_checked_artifact()
                .expect("selected comparison custody publishes");
            let [published] = produced.boundary_operator_scope().occurrences() else {
                panic!("one exact published comparison occurrence");
            };
            assert_eq!(
                published.terminal_operation(),
                lowered.selected_ieee_float_comparison_occurrences[0].terminal_operation,
            );
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
fn the_operand_mapping_addresses_only_the_exact_emitted_pair() {
    use lowered_psi::LoweredSelectedIntegerComparisonOperandOrder as Order;
    assert_eq!(Order::Authored.terminal_operand_position(0, 2), Some(0));
    assert_eq!(Order::Authored.terminal_operand_position(1, 2), Some(1));
    assert_eq!(Order::Swapped.terminal_operand_position(0, 2), Some(1));
    assert_eq!(Order::Swapped.terminal_operand_position(1, 2), Some(0));
    // A roster the mapping cannot address exactly has no position, so the
    // operation crash contract fails closed instead of publishing its routes
    // over a guessed operand order.
    assert_eq!(Order::Authored.terminal_operand_position(2, 2), None);
    assert_eq!(Order::Swapped.terminal_operand_position(0, 1), None);
    assert_eq!(Order::Swapped.terminal_operand_position(0, 3), None);
}

#[test]
fn ordinary_selected_integer_comparisons_emit_one_exact_operation() {
    // The integer counterpart of the IEEE join above: one Terminal operation,
    // one occurrence naming it and recording how that operation reads the
    // authored operands, carried through checked Terminal production beside
    // the float roster. `>` and `>=` emit the reversed operation and `!=` the
    // negated equality, mirroring the checked stage's own normalization of
    // the builtin comparisons, so no spelling needs an operation of its own.
    use lowered_psi::LoweredSelectedIntegerComparisonOperandOrder as Order;
    for (token, name, expected, order, negated) in [
        ("==", "equal", "IntegerEqual", Order::Authored, false),
        ("!=", "not_equal", "IntegerEqual", Order::Authored, true),
        ("<", "less", "IntegerLessThan", Order::Authored, false),
        (
            "<=",
            "less_or_equal",
            "IntegerLessOrEqual",
            Order::Authored,
            false,
        ),
        (">", "greater", "IntegerLessThan", Order::Swapped, false),
        (
            ">=",
            "greater_or_equal",
            "IntegerLessOrEqual",
            Order::Swapped,
            false,
        ),
    ] {
        for primitive in ["i32", "u64"] {
            let source = format!(
                "boundary operator {token} Comparison::{name}(left: {primitive}, right: {primitive}) -> bool; machine choose(left: {primitive}, right: {primitive}) -> bool {{ left {token} right }}"
            );
            let checked = checked(&source);
            let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
                .expect("selected integer comparison lowers");
            assert!(
                lowered
                    .selected_ieee_float_comparison_occurrences
                    .is_empty()
            );
            let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
                panic!("one selected integer comparison occurrence");
            };
            let machine = lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == occurrence.terminal_machine)
                .expect("the occurrence names the lowered machine");
            let (block, operation) = machine
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .operations
                        .iter()
                        .find(|operation| operation.id == occurrence.terminal_operation)
                        .map(|operation| (block, operation))
                })
                .expect("the occurrence names one emitted operation");
            let (kind, left, right) = match operation.kind {
                OperationKind::IntegerEqual { left, right } => ("IntegerEqual", left, right),
                OperationKind::IntegerLessThan { left, right } => ("IntegerLessThan", left, right),
                OperationKind::IntegerLessOrEqual { left, right } => {
                    ("IntegerLessOrEqual", left, right)
                }
                ref other => panic!("selected integer comparison emitted {other:?}"),
            };
            assert_eq!(kind, expected, "{source}");
            assert_eq!(occurrence.operand_order, order, "{source}");
            assert_eq!(occurrence.negated, negated, "{source}");
            // `left` then `right` complete as the trailing parameters of the
            // comparison's own block, in authored evaluation order; only the
            // emitted operand roster follows the recorded mapping.
            let [.., authored_left, authored_right] = block.parameters.as_slice() else {
                panic!("the comparison block carries both completed operands");
            };
            assert_eq!(
                (left, right),
                match order {
                    Order::Authored => (authored_left.id, authored_right.id),
                    Order::Swapped => (authored_right.id, authored_left.id),
                },
                "{source}"
            );
            assert_eq!(
                semantic_vocabulary::ScalarType::Integer(occurrence.integer_type),
                authored_left.scalar_type
            );
            // A negated spelling completes as one `BooleanNot` over the
            // emitted comparison's own result, and nothing else does.
            let result = operation
                .result
                .scalar()
                .expect("the comparison produces a Boolean")
                .id;
            assert_eq!(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|candidate| matches!(candidate.kind,
                        OperationKind::BooleanNot { operand } if operand == result))
                    .count(),
                usize::from(negated),
                "{source}"
            );
            let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
                .produce_checked_artifact()
                .expect("selected integer comparison custody publishes");
            assert_eq!(produced.selected_integer_comparison_occurrences().len(), 1);
            assert_eq!(
                produced.selected_integer_comparison_occurrences()[0].terminal_operation,
                occurrence.terminal_operation
            );
        }
    }
}

#[test]
fn published_match_custody_rejects_missing_and_duplicate_occurrences() {
    let checked = checked(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool;
        machine identity(value: f32) -> f32 { value }
        machine choose(value: f32, first: f32, second: f32) -> u64 {
            match identity(value) { identity(first) -> 7, identity(second) -> 9, _ -> 11 }
        }",
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).unwrap();
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "choose")
        .produce_checked_artifact()
        .expect("both selected arms publish exact custody");
    assert_eq!(produced.boundary_operator_scope().occurrences().len(), 2);
    for duplicate in [false, true] {
        let mut corrupted = lowered.clone();
        if duplicate {
            corrupted.selected_ieee_float_comparison_occurrences[1] =
                corrupted.selected_ieee_float_comparison_occurrences[0];
        } else {
            corrupted.selected_ieee_float_comparison_occurrences.pop();
        }
        let error = lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            &corrupted,
        )
        .unwrap_err();
        assert_eq!(
            error,
            if duplicate {
                "checked boundary-operator applications do not map one-to-one onto Terminal operations"
            } else {
                "IEEE comparison occurrences do not cover the exact Terminal roster"
            },
        );
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
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
        .expect("saved subject and ordered patterns");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 2);
    let original = lowered.selected_ieee_float_comparison_occurrences[0];
    let selected = *checked.facts.operators.uses.get(original.operator_use);
    checked
        .facts
        .operators
        .uses
        .get_mut(original.operator_use)
        .provider_plan_commitment = Default::default();
    assert!(crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err());
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
        crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
        "arm equality cannot borrow a sibling occurrence"
    );
}

#[test]
fn selected_comparison_replay_rejects_swapped_operands_and_changed_source_operation() {
    let mut checked = checked(
        "boundary operator < Float::less(left: f64, right: f64) -> bool;
        machine choose(left: f64, right: f64) -> bool { left < right }",
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).unwrap();
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
        crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
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
        crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
        "source operator cannot reuse selected less meaning"
    );
}

#[test]
fn selected_comparison_replays_origin_role_even_when_application_is_relabelled() {
    let mut checked = checked(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool;
        machine choose(left: f32, right: f32) -> bool { left == right }",
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).unwrap();
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
        crate::lower_machine(&checked, TerminalMachineSelection::Name("choose")).is_err(),
        "origin role is replayed from the actual statement"
    );
}
