use super::{
    TypeReferenceHandle, TypedTrees, evaluate_const_array_lengths, evaluate_selected_array_lengths,
    validate_folded_array_lengths,
};
use crate::SelectedBuildTimeOperators;
use crate::{SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::{
    BuildMachineEvaluationRequest, BuildTimeOperationEvaluation, InterpretOptions,
};

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
    let mut typed = crate::front_end::typed_program(&source);
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
            checked_interpreter::evaluate_build_time_machine(
                &typed,
                BuildMachineEvaluationRequest {
                    product_entry_compatibility: None,
                    operators: &rows,
                    ..BuildMachineEvaluationRequest::symbol(machine, Vec::new())
                }
            )
            .map(BuildTimeOperationEvaluation::into_measured)
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
            checked_interpreter::interpret_entry(
                &checked,
                BuildMachineEntry::Name("length"),
                &[],
                InterpretOptions::default()
            )
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
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &rows,
            provider_bodies: &[],
        },
    )
    .unwrap();
    assert_eq!(folds.len(), 1);
    validate_folded_array_lengths(&typed, &folds, &rows, &[], None).unwrap();
    let mut forged = folds.clone();
    forged[0].value = 5;
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 5);
    assert!(validate_folded_array_lengths(&typed, &forged, &rows, &[], None).is_err());
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
    assert!(validate_folded_array_lengths(&typed, &folds, &rows, &[], None).is_err());
}

#[test]
fn independent_and_selected_lengths_share_full_width_integer_decoding() {
    for (carrier, result) in [("u64", "18446744073709551615"), ("i64", "-1")] {
        let source = format!(
            "machine length() -> {carrier} {{ {result} }} data Main {{ bytes: [u8; length()]; }}"
        );
        let mut selected = crate::front_end::typed_program(&source);
        let mut independent = selected.clone();
        let independent_result = evaluate_const_array_lengths(&mut independent, None);
        let selected_result = evaluate_selected_array_lengths(
            &mut selected,
            None,
            SelectedBuildTimeOperators {
                operators: &[],
                provider_bodies: &[],
            },
        );
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
        validate_folded_array_lengths(&selected, &folds, &[], &[], None)
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

/// A selected boundary-operator use whose settled plan binds an ordinary
/// checked adapter. The provider's body computes `left + right`, which differs
/// from the builtin `%` result, so a fold of 9 instead of 1 is the positive
/// witness that the provider's machine -- not host arithmetic -- ran.
fn provider_fixture() -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left + right }
machine length() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (left % right) } }
data Main { bytes:[u8;length()]; }
"#;
    let typed = crate::front_end::typed_program(source);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| {
            typed.operators().iter().any(|operator| {
                operator.symbol == fact.selected_operator_symbol && operator.is_boundary
            })
        })
        .map(|fact| {
            let provider = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = typed.machine_states(provider).first().unwrap();
            SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: fact.operands(&typed).unwrap(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect();
    (typed, rows)
}

/// The same selected provider-body execution, invoked through a uniquely
/// resolved named call rather than the spelled token. The provider's body
/// still computes `left + right` = 9 where builtin `%` would fold 1.
fn named_provider_fixture() -> (TypedTrees, Vec<SelectedBuildTimeProviderBody>) {
    let source = r#"
data Math {}
boundary operator % Math::remainder(left: u64, right: u64) -> u64;
data Provider {}
machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { left + right }
machine length() -> u64 { let left:u64 = 7; let right:u64 = 2; transition { _ -> (Math::remainder(left, right)) } }
data Main { bytes:[u8;length()]; }
"#;
    let typed = crate::front_end::typed_program(source);
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(&typed);
    let rows = facts
        .named_uses()
        .filter(|fact| {
            typed.operators().iter().any(|operator| {
                operator.symbol == fact.selected_operator_symbol && operator.is_boundary
            })
        })
        .map(|fact| {
            let typed_trees::expression::ExpressionNode::Call(call) =
                typed.expression_table.expression(fact.expression)
            else {
                panic!("a named operator use is an expression call");
            };
            let provider = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Provider::remainder")
                .unwrap();
            let entry = typed.machine_states(provider).first().unwrap();
            SelectedBuildTimeProviderBody {
                expression: fact.expression,
                origin: fact.origin,
                requirement: fact.selected_operator_symbol,
                operands: typed
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec(),
                provider_machine: provider.symbol,
                provider_state: entry.symbol,
                provider_type: provider.attached_data.as_ref().unwrap().as_str().to_owned(),
                provider: checked_trees::CheckedProviderPlanCommitment::from_digest([7; 32]),
            }
        })
        .collect();
    (typed, rows)
}

#[test]
fn named_provider_body_executes_its_ordinary_machine_and_replays() {
    let (mut typed, rows) = named_provider_fixture();
    assert_eq!(rows.len(), 1);
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap();
    assert_eq!(folds.len(), 1);
    assert_eq!(
        folds[0].value, 9,
        "the provider's `left + right` body must run; builtin `%` would fold 1"
    );
    validate_folded_array_lengths(&typed, &folds, &[], &rows, None).unwrap();
}

#[test]
fn named_provider_body_row_rejects_stale_and_substituted_custody() {
    let (typed, rows) = named_provider_fixture();
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();

    let mut substituted = rows.clone();
    let caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "length")
        .unwrap();
    substituted[0].provider_machine = caller.symbol;
    substituted[0].provider_state = typed.machine_states(caller).first().unwrap().symbol;
    assert!(
        crate::validate_selected_provider_bodies(&typed, &substituted).is_err(),
        "a substituted provider machine must never satisfy the named row"
    );

    let mut stale = rows.clone();
    stale[0].expression = stale[0].operands[0];
    assert!(
        crate::validate_selected_provider_bodies(&typed, &stale).is_err(),
        "a stale expression coordinate must never rejoin a current named use"
    );
}

#[test]
fn selected_provider_body_executes_its_ordinary_machine_and_replays() {
    let (mut typed, rows) = provider_fixture();
    assert_eq!(rows.len(), 1);
    crate::validate_selected_provider_bodies(&typed, &rows).unwrap();
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap();
    assert_eq!(folds.len(), 1);
    assert_eq!(
        folds[0].value, 9,
        "the provider's `left + right` body must run; builtin `%` would fold 1"
    );
    validate_folded_array_lengths(&typed, &folds, &[], &rows, None).unwrap();
}

#[test]
fn unselected_boundary_use_cannot_fall_back_to_host_semantics() {
    let (mut typed, _rows) = provider_fixture();
    let diagnostics = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
    .unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("requires exact authored selection")),
        "an unselected boundary use must never reach builtin execution: {diagnostics:?}"
    );
}

#[test]
fn provider_body_fold_rejects_a_forged_builtin_result() {
    let (mut typed, rows) = provider_fixture();
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &rows,
        },
    )
    .unwrap();
    let mut forged = folds.clone();
    forged[0].value = 1;
    typed
        .type_reference_table
        .set_fixed_array_length(forged[0].type_reference, 1);
    assert!(
        validate_folded_array_lengths(&typed, &forged, &[], &rows, None).is_err(),
        "a fold claiming the builtin `%` result must not survive provider replay"
    );
}

#[test]
fn provider_body_row_rejects_stale_and_substituted_custody() {
    let (typed, rows) = provider_fixture();
    let mut invalid_entry = rows.clone();
    invalid_entry[0].provider_state = symbols::SymbolHandle::invalid();
    assert!(
        crate::validate_selected_provider_bodies(&typed, &invalid_entry)
            .unwrap_err()
            .contains("executable entry")
    );

    let caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "length")
        .unwrap();
    let mut substituted = rows.clone();
    substituted[0].provider_machine = caller.symbol;
    substituted[0].provider_state = typed.machine_states(caller).first().unwrap().symbol;
    assert!(
        crate::validate_selected_provider_bodies(&typed, &substituted).is_err(),
        "a substituted provider machine must never satisfy the row"
    );

    let mut anonymous = rows.clone();
    anonymous[0].provider = checked_trees::CheckedProviderPlanCommitment::default();
    assert!(
        crate::validate_selected_provider_bodies(&typed, &anonymous)
            .unwrap_err()
            .contains("unique exact provider custody")
    );

    let mut swapped = rows.clone();
    swapped[0].operands.swap(0, 1);
    assert!(
        crate::validate_selected_provider_bodies(&typed, &swapped)
            .unwrap_err()
            .contains("differ")
    );

    let mut stale = rows.clone();
    stale[0].expression = stale[0].operands[0];
    assert!(
        crate::validate_selected_provider_bodies(&typed, &stale).is_err(),
        "a stale expression coordinate must never rejoin a current use"
    );
}

#[test]
fn folded_payload_retains_its_exact_variant_parent() {
    let (mut typed, rows) = fixture_with_receiver(
        "Float",
        "data Main { case First(bytes:[u8;length()]); case Second; }",
    );
    let folds = evaluate_selected_array_lengths(
        &mut typed,
        None,
        SelectedBuildTimeOperators {
            operators: &rows,
            provider_bodies: &[],
        },
    )
    .unwrap();
    validate_folded_array_lengths(&typed, &folds, &rows, &[], None).unwrap();
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
    assert!(validate_folded_array_lengths(&typed, &folds, &rows, &[], None).is_err());
}
