use super::*;
use crate::SelectedBuildTimeBinaryOperator;

fn fixture(namespace: &str) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    fixture_with_receiver(namespace, "data Main { bytes:[u8;length()]; }")
}

fn fixture_with_receiver(
    namespace: &str,
    receiver: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    fixture_with_operator_contract(namespace, receiver, "")
}

fn fixture_with_operator_contract(
    namespace: &str,
    receiver: &str,
    crash_contract: &str,
) -> (TypedTrees, Vec<SelectedBuildTimeBinaryOperator>) {
    let source = format!(
        r#"
boundary operator * {namespace}::multiply(left:f64, right:f64) -> f64 {crash_contract};
boundary operator * {namespace}::multiply(left:f32, right:f32) -> f32 crashes Trap;
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
fn selected_operator_crash_fences_cover_admission_and_direct_execution() {
    for contract in ["crashes Trap", "crashes Abort", "crashes Trap false"] {
        let (typed, rows) = fixture_with_operator_contract("Float", "", contract);
        assert!(!rows.is_empty());
        let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
        assert!(facts.has_crash_qualified_uses(&typed));
        assert!(
            crate::validate_selected_operators(&typed, &rows)
                .unwrap_err()
                .contains("no build-time execution support")
        );
        let machine = rows[0].origin.machine_symbol().unwrap();
        assert!(
            checked_interpreter::evaluate_build_time_machine_symbol_with_selected_operators(
                &typed,
                machine,
                Vec::new(),
                &rows,
            )
            .unwrap_err()
            .contains("no build-time execution support")
        );
        let checked = checked_trees::CheckedTrees::with_roots(
            typed,
            checked_trees::CheckFacts {
                operators: facts,
                ..Default::default()
            },
        );
        assert!(
            checked_interpreter::interpret_entry(&checked, "length", &[])
                .error
                .unwrap()
                .contains("no checked execution support")
        );
    }
}

#[test]
fn operator_crash_fence_ignores_unselected_overload_and_survives_expression_rewrite() {
    let (typed, rows) = fixture("Float");
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    assert!(!facts.has_crash_qualified_uses(&typed));
    assert!(rows.iter().all(|row| !row.has_crash_contract(&typed)));
    crate::validate_selected_operators(&typed, &rows).unwrap();

    let (mut typed, rows) = fixture_with_operator_contract("Float", "", "crashes Trap");
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    // Adapter settlement may replace the source node; the selected contracts
    // remain the authority for this fence.
    for row in rows {
        *typed.expression_table.expression_mut(row.expression) =
            typed_trees::expression::ExpressionNode::Boolean(false);
    }
    assert!(facts.has_crash_qualified_uses(&typed));
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
fn independent_and_selected_lengths_share_full_width_integer_decoding() {
    for (carrier, result) in [("u64", "18446744073709551615"), ("i64", "-1")] {
        let source = format!(
            "machine length() -> {carrier} {{ {result} }} data Main {{ bytes: [u8; length()]; }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens)
                .unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let mut selected =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut independent = selected.clone();
        let independent_result = evaluate_const_array_lengths(&mut independent);
        let selected_result = evaluate_with_selected_operators(&mut selected, None, &[]);
        if carrier == "i64" || usize::BITS < 64 {
            assert!(independent_result.is_err());
            assert!(selected_result.is_err());
            continue;
        }
        // This checks length substitution only, not permission to allocate an
        // enormous array. Placement and resource supply remain later checks.
        independent_result.expect("unsigned length is not a negative host integer");
        let folds = selected_result.expect("selected fold uses the same integer value");
        assert_eq!(folds.len(), 1);
        assert_eq!(folds[0].value, usize::MAX);
        assert_eq!(
            independent
                .type_reference_table
                .fixed_array_lengths()
                .collect::<Vec<_>>(),
            selected
                .type_reference_table
                .fixed_array_lengths()
                .collect::<Vec<_>>()
        );
        validate_folded_array_lengths(&selected, &folds, &[], None)
            .expect("independent replay retains the unsigned result");
    }
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
