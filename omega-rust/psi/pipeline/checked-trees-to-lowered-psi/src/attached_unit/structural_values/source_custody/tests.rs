use super::*;

fn checked() -> CheckedTrees {
    let source = "data Tag { case First; case Second; }
        machine choose(selector: u64) -> Tag {
            match selector { 0 -> Tag::First, _ -> Tag::Second }
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

fn operation(
    checked: &CheckedTrees,
) -> (SymbolHandle, SymbolHandle, CheckedUnitEffectOperationPlan) {
    let (_, root) = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .expect("fresh value root");
    (
        root.machine,
        root.state,
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: root.statement_ordinal,
                binding_ordinal: 0,
                type_identity: checked
                    .normalized_type_identity(root.type_reference)
                    .into_string(),
                multiplicity: checked.type_multiplicity(root.type_reference),
            },
            value: root.root,
            discard_result_on_return: false,
        },
    )
}

#[test]
fn structural_replay_rejects_substituted_integer_pattern_payload() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).unwrap();
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("structural value");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("dispatch");
    };
    let CheckedScalarDispatchPattern::Value(pattern) = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()[0]
        .pattern
    else {
        panic!("value pattern");
    };
    let checked_trees::CheckedScalarComputationKind::Value(
        checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
    ) = &mut checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(pattern)
        .kind
    else {
        panic!("retained integer pattern");
    };
    *literal =
        numerics::literals::IntegerLiteral::from_value(17).with_landing(literal.landing().unwrap());
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn structural_replay_rejects_swapped_case_results_and_missing_coverage() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("original source correspondence");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } = operation else {
        panic!("value operation");
    };
    let CheckedStructuralValueKind::Dispatch { arms, .. } =
        checked.facts.values.structural_values.nodes.get(value).kind
    else {
        panic!("match");
    };
    let original = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span(arms)
        .unwrap()
        .to_vec();
    let retained = checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap();
    retained[0].value = original[1].value;
    retained[1].value = original[0].value;
    assert!(validate(&checked, machine, state, &operation).is_err());
    checked
        .facts
        .values
        .structural_values
        .dispatch_arms
        .span_mut(arms)
        .unwrap()
        .clone_from_slice(&original);
    let CheckedStructuralValueKind::Dispatch { arms, .. } = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(value)
        .kind
    else {
        panic!("match");
    };
    *arms = arena::HandleSpan::from_parts(arms.start(), 1);
    assert!(validate(&checked, machine, state, &operation).is_err());
}

#[test]
fn structural_replay_rejects_case_identity_and_root_owner_substitution() {
    let mut checked = checked();
    let (machine, state, operation) = operation(&checked);
    validate(&checked, machine, state, &operation).expect("original source correspondence");
    let case = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(node.kind, CheckedStructuralValueKind::Case { .. }).then_some(handle)
        })
        .unwrap();
    let original = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(case)
        .clone();
    let CheckedStructuralValueKind::Case { case_symbol, .. } = &mut checked
        .facts
        .values
        .structural_values
        .nodes
        .get_mut(case)
        .kind
    else {
        panic!("case");
    };
    *case_symbol = machine;
    assert!(validate(&checked, machine, state, &operation).is_err());
    *checked.facts.values.structural_values.nodes.get_mut(case) = original;
    let root = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .next()
        .unwrap()
        .0;
    checked
        .facts
        .values
        .structural_values
        .roots
        .get_mut(root)
        .machine = state;
    assert!(validate(&checked, machine, state, &operation).is_err());
}
