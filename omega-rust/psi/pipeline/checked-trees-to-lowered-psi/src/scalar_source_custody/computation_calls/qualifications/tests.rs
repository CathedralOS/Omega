use super::*;

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typing");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("checking")
}

fn root(checked: &CheckedTrees) -> checked_trees::CheckedScalarComputationRoot {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("choose");
    checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            root.machine == machine.symbol && root.role == CheckedScalarExpressionRole::Return
        })
        .expect("qualified return graph")
        .clone()
}

fn replay(checked: &CheckedTrees) -> Result<(), LoweringError> {
    let root = root(checked);
    let source = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(root.root)
        .authored_root;
    validate_computation_calls(
        checked,
        root.machine,
        root.state,
        root.statement_ordinal,
        root.root,
        source,
    )
}

fn qualifications(checked: &CheckedTrees) -> Vec<CheckedScalarComputationHandle> {
    crate::scalar_computations::reachable_nodes(checked, &[root(checked).root])
        .expect("closure")
        .into_iter()
        .filter(|handle| {
            matches!(
                checked
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(*handle)
                    .kind,
                CheckedScalarComputationKind::Qualification { .. }
            )
        })
        .collect()
}

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/expressions/match_domain_results/main.omg"
));

#[test]
fn qualification_match_replays_checked_custody_and_publishes_exact_terminal_membership() {
    let checked = checked(SOURCE);
    assert_eq!(qualifications(&checked).len(), 2);
    replay(&checked).expect("exact source, membership, and operand custody");
    let lowered = crate::lower_machine(&checked, "choose").expect("qualified Terminal graph");
    let catalog = &lowered.semantic_module.scalar_qualifications;
    assert_eq!(catalog.domains.len(), 1);
    assert_eq!(catalog.sets.len(), 1);
    assert_eq!(catalog.coercions.len(), 2);
    let result = lowered.semantic_module.machines[0]
        .result
        .scalar_ref()
        .expect("scalar result");
    assert_eq!(result.qualifications, catalog.sets[0].id);
}

#[test]
fn qualification_replay_preserves_alias_and_closed_instance_identity() {
    for (declarations, domain) in [
        ("domain i64::Km; domain i64::Length = Km;", "Length"),
        ("domain<const Axis: u64> i64::Coordinate;", "Coordinate<7>"),
    ] {
        let source = format!("{declarations}
            machine choose(select_left: bool, left: i64, right: i64) -> i64 in {domain} {{
                match select_left {{ true -> left as i64 in {domain}, false -> right as i64 in {domain} }}
            }}");
        let checked = checked(&source);
        replay(&checked).expect("normalized alias or indexed instance");
    }
}

#[test]
fn qualification_replay_rejects_missing_duplicate_or_redirected_evidence() {
    for mutation in 0..5 {
        let mut checked = checked(SOURCE);
        let first = checked.facts.qualifications.vacuous_uses[0];
        match mutation {
            0 => {
                checked.facts.qualifications.vacuous_uses.clear();
            }
            1 => checked.facts.qualifications.vacuous_uses.push(first),
            2 => checked.facts.qualifications.vacuous_uses[0].statement_index += 1,
            3 => {
                checked.facts.qualifications.vacuous_uses[0].domain =
                    symbols::SymbolHandle::invalid()
            }
            4 => {
                checked.facts.qualifications.vacuous_uses[0].semantic_domain =
                    SemanticDomainId::NULL
            }
            _ => unreachable!(),
        }
        assert!(replay(&checked).is_err(), "mutation {mutation}");
    }
}

#[test]
fn qualification_replay_rejects_relabelled_cast_operand_and_result() {
    for mutation in 0..4 {
        let mut checked = checked(SOURCE);
        let handles = qualifications(&checked);
        let CheckedScalarComputationKind::Qualification {
            source_expression: other_source,
            operand: other_operand,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(handles[1])
            .kind
        else {
            panic!("qualification");
        };
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handles[0]);
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            result_type,
        } = &mut node.kind
        else {
            panic!("qualification");
        };
        match mutation {
            0 => *source_expression = other_source,
            1 => *operand = other_operand,
            2 => *result_type = TypeReferenceHandle::invalid(),
            3 => node.primitive_type = PrimitiveType::U64,
            _ => unreachable!(),
        }
        assert!(replay(&checked).is_err(), "mutation {mutation}");
    }
}

#[test]
fn qualification_replay_rejects_erasing_node_even_with_cast_source_on_pure_value() {
    for retain_cast_source in [false, true] {
        let mut checked = checked(SOURCE);
        let handle = qualifications(&checked)[0];
        let CheckedScalarComputationKind::Qualification {
            source_expression,
            operand,
            ..
        } = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(handle)
            .kind
        else {
            panic!("qualification");
        };
        let mut replacement = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(operand)
            .clone();
        if retain_cast_source {
            replacement.value_source = source_expression;
        }
        *checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle) = replacement;
        assert!(
            replay(&checked).is_err(),
            "retain_cast_source={retain_cast_source}"
        );
    }
}

#[test]
fn qualification_replay_rejects_changed_instance() {
    let source = "domain<const Axis: u64> i64::Coordinate;
        machine choose(value: i64) -> i64 in Coordinate<7> { value as i64 in Coordinate<7> }
        machine other(value: i64) -> i64 in Coordinate<8> { value as i64 in Coordinate<8> }";
    let mut checked = checked(source);
    replay(&checked).expect("original instance");
    let other = checked
        .facts
        .qualifications
        .vacuous_uses
        .iter()
        .map(|usage| usage.semantic_domain)
        .find(|identity| *identity != checked.facts.qualifications.vacuous_uses[0].semantic_domain)
        .expect("distinct index");
    checked.facts.qualifications.vacuous_uses[0].semantic_domain = other;
    assert!(
        replay(&checked).is_err(),
        "different canonical index is not evidence"
    );
}

#[test]
fn qualification_replay_reconstructs_indices_when_retained_records_agree_on_a_substitution() {
    let source = "domain<const Axis: u64> i64::Coordinate;
        machine choose(value: i64) -> i64 in Coordinate<7> { value as i64 in Coordinate<7> }
        machine other(value: i64) -> i64 in Coordinate<8> { value as i64 in Coordinate<8> }";
    for change_result in [false, true] {
        let mut checked = checked(source);
        let uses = &checked.facts.qualifications.vacuous_uses;
        let first_source = uses[0].expression;
        let ExpressionNode::Cast(other) = checked.expression_table.expression(uses[1].expression)
        else {
            panic!("other cast");
        };
        let other = *other;
        let handle = qualifications(&checked)[0];
        if change_result {
            let ExpressionNode::Cast(first) =
                checked.typed.expression_table.expression_mut(first_source)
            else {
                panic!("first cast");
            };
            first.result_type = other.result_type;
            let CheckedScalarComputationKind::Qualification { result_type, .. } = &mut checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle)
                .kind
            else {
                panic!("qualification");
            };
            *result_type = other.result_type;
        } else {
            let ExpressionNode::Cast(first) =
                checked.typed.expression_table.expression_mut(first_source)
            else {
                panic!("first cast");
            };
            first.semantic_domain_id = other.semantic_domain_id;
            checked.facts.qualifications.vacuous_uses[0].semantic_domain = other.semantic_domain_id;
        }
        assert!(replay(&checked).is_err(), "change_result={change_result}");
    }
}

#[test]
fn qualification_replay_uses_the_same_custody_for_boolean_and_float_carriers() {
    for carrier in ["bool", "f32", "f64"] {
        let source = format!("domain {carrier}::Tagged;
            machine choose(value: {carrier}) -> {carrier} in Tagged {{ value as {carrier} in Tagged }}");
        replay(&checked(&source)).expect("noninteger scalar qualification");
    }
}

#[test]
fn qualification_keeps_nested_selection_and_call_operands_in_their_source_scopes() {
    for body in [
        "match select_left { true -> identity(left) as i64 in Km, false -> identity(right) as i64 in Km }",
        "(match select_left { true -> identity(left), false -> identity(right) }) as i64 in Km",
    ] {
        let source = format!(
            "domain i64::Km;
            machine identity(value: i64) -> i64 {{ value }}
            machine choose(select_left: bool, left: i64, right: i64) -> i64 in Km {{ {body} }}"
        );
        let checked = checked(&source);
        replay(&checked).expect("selection and call scopes beneath qualification");
        let targets = crate::scalar_computations::call_targets(&checked, root(&checked).machine)
            .expect("qualified operand call discovery");
        assert_eq!(targets.len(), 2, "each arm retains its call");
    }
}

#[test]
fn qualification_replay_rechecks_declaration_instead_of_trusting_vacuous_use() {
    let mut checked = checked(SOURCE);
    let definitions = checked.typed.roots.domain_definitions;
    checked
        .typed
        .tables
        .domain_definitions
        .span_mut(definitions)
        .expect("declarations")[0]
        .predicate_body = language_semantics::DomainPredicateBody::Present;
    assert!(
        replay(&checked).is_err(),
        "an unchanged vacuous-use row cannot hide a predicate"
    );
}
