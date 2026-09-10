use super::*;
use crate::SelectedBuildTimeBinaryOperator;

fn fixture(namespace: &str) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    fixture_with_receiver(namespace, "data Main { bytes:[u8;length()]; }")
}

fn fixture_with_receiver(
    namespace: &str,
    receiver: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    let source = format!(
        r#"
boundary operator * {namespace}::multiply(left:f64, right:f64) -> f64;
boundary operator == {namespace}::equal(left:f64, right:f64) -> bool;
machine length() -> u64 {{
    let left:f64 = 2.0;
    let right:f64 = 3.0;
    transition left * right == 6.0 {{ true -> 4 _ -> 5 }}
}}
{receiver}
"#
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    validation::land_float_literal_destinations(&mut typed);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter_map(|fact| {
            let typed_trees::expression::ExpressionNode::Binary(binary) =
                typed.expression_table.expression(fact.expression)
            else {
                return None;
            };
            Some(SelectedBuildTimeBinaryOperator {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operation: binary.operator,
                operands: [binary.left, binary.right],
                format: numerics::literals::FloatFormat::F64,
                policy: fact.policy_adapter,
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([1; 32]),
            })
        })
        .collect();
    (typed, rows)
}

#[test]
fn folded_result_replay_rejects_paired_literal_forgery_and_lost_owner() {
    let (mut typed, rows) = fixture("Float");
    let folds = evaluate_with_selected_operators(&mut typed, None, &rows).unwrap();
    assert_eq!(folds.len(), 1);
    validate_folded_array_lengths(&typed, &folds, &rows, None).unwrap();
    let mut forged = folds.clone();
    forged[0].value = 5;
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 5);
    assert!(validate_folded_array_lengths(&typed, &forged, &rows, None).is_err());
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 4);
    let owner = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap()
        .clone();
    let typed_trees::data::DataMember::Field(field) =
        typed.tables.data_members.get_mut(owner.members.start())
    else {
        panic!("bytes");
    };
    field.type_reference = TypeReferenceHandle::invalid();
    assert!(validate_folded_array_lengths(&typed, &folds, &rows, None).is_err());
}

#[test]
fn equal_signature_authored_boundary_cannot_claim_float_semantics() {
    let (typed, rows) = fixture("Math");
    assert!(!rows.is_empty());
    assert!(
        crate::validate_selected_operators(&typed, &rows)
            .unwrap_err()
            .contains("sealed Float meaning")
    );
}

#[test]
fn single_supplied_row_cannot_authorize_another_live_expression_origin() {
    let (mut typed, rows) = fixture("Float");
    crate::validate_selected_operators(&typed, &rows).unwrap();
    let row = rows[0];
    let machine = typed
        .machines()
        .iter()
        .find(|machine| Some(machine.symbol) == row.origin.machine_symbol())
        .unwrap()
        .clone();
    let mut statements = typed.machine_states(&machine)[0].statement_nodes;
    typed.statement_table.push_statement(
        &mut statements,
        typed_trees::statement::StatementNode::Expression(row.expression),
    );
    typed.machine_states_mut(&machine)[0].statement_nodes = statements;
    assert!(
        crate::validate_selected_operators(&typed, &rows)
            .unwrap_err()
            .contains("distinct current origins")
    );
}

#[test]
fn folded_payload_retains_its_exact_variant_parent() {
    let (mut typed, rows) = fixture_with_receiver(
        "Float",
        "data Main { case First(bytes:[u8;length()]); case Second; }",
    );
    let folds = evaluate_with_selected_operators(&mut typed, None, &rows).unwrap();
    validate_folded_array_lengths(&typed, &folds, &rows, None).unwrap();
    let owner = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .unwrap()
        .clone();
    let members = typed.tables.data_members.span_mut(owner.members).unwrap();
    let [
        typed_trees::data::DataMember::Variant(first),
        typed_trees::data::DataMember::Variant(second),
    ] = members
    else {
        panic!("two cases");
    };
    std::mem::swap(&mut first.payload, &mut second.payload);
    assert!(validate_folded_array_lengths(&typed, &folds, &rows, None).is_err());
}
