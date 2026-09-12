use super::typed_source;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn selected_provider_clone_refreshes_result_local_without_new_ordinary_specialization() {
    check_selected_provider_result_local(false, false);
}

#[test]
fn selected_provider_clone_retains_exact_calls_among_ordinary_specializations() {
    check_selected_provider_result_local(true, false);
}

#[test]
fn selected_provider_replays_calls_after_an_ordinary_provider_instantiation() {
    check_selected_provider_result_local(false, true);
    check_selected_provider_result_local(true, true);
}

fn check_selected_provider_result_local(additional_endpoint: bool, direct_provider: bool) {
    let source = r#"
        pub data GenericMath {}
        pub boundary operator GenericMath::measure<Element>(value: Element) -> u64;

        machine endpoint<const N: u64>() -> u64[0..=N] { N }

        pub data GenericProvider {}
        pub machine GenericProvider::measure<Value>(value: Value) -> u64
        satisfies GenericMath::measure
        { endpoint<2>() }

        machine exercise(value: i32) -> u64 {
            GenericMath::measure(value)
        }
        "#;
    let source = if additional_endpoint {
        source.replace("{ endpoint<2>() }", "{ _ = endpoint<5>(); endpoint<2>() }")
    } else {
        source.to_owned()
    };
    let source = if direct_provider {
        source.replace("machine exercise(value: i32) -> u64", "machine direct(value: i32) -> u64 { GenericProvider::measure(value) } machine exercise(value: bool) -> u64")
    } else {
        source
    };
    let typed = typed_source(&source).expect("type the provider's inferred tail-call local");
    let requirement_operator = typed
        .operators()
        .iter()
        .find(|operator| {
            typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["GenericMath", "measure"])
        })
        .expect("generic measure requirement")
        .symbol;
    let realization_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "GenericProvider::measure")
        .expect("generic measure provider")
        .symbol;
    let endpoint_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "endpoint")
        .expect("const-generic endpoint")
        .symbol;

    // Ordinary specialization closes endpoint<2> before the provider is cloned
    // from the saved generic snapshot. The following ordinary pass has no new
    // tuple to apply, but must repair the clone's copied endpoint result type.
    let checked = crate::lower_typed_trees_with_selected_generic_operator_providers(
        typed,
        &[crate::SelectedGenericOperatorProviderSpecialization {
            requirement_operator,
            realization_machine,
        }],
        &[],
    )
    .expect("a selected provider clone must refresh its already-closed callee result");

    assert_eq!(
        checked.machine_specializations.len(),
        2 + usize::from(additional_endpoint) + usize::from(direct_provider)
    );
    let endpoint_specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.template == endpoint_machine && specialization.const_arguments == ["2"]
        })
        .expect("the tail endpoint specializes exactly once");
    assert_ne!(endpoint_specialization.instance, endpoint_machine);
    assert_eq!(endpoint_specialization.const_arguments, ["2"]);
    let provider_specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.template == realization_machine
                && specialization.instance != realization_machine
        })
        .expect("one selected provider clone");
    assert_ne!(provider_specialization.instance, realization_machine);
    assert!(
        provider_specialization
            .operator_realizations
            .iter()
            .any(|realization| realization.requirement_symbol == requirement_operator)
    );
    let provider = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == provider_specialization.instance)
        .expect("the concrete provider clone");
    assert!(checked.machine_type_parameters(provider).is_empty());
    let endpoint = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == endpoint_specialization.instance)
        .expect("the already-specialized endpoint");
    assert!(checked.machine_type_parameters(endpoint).is_empty());
    let selected_state = checked
        .machine_states(endpoint)
        .first()
        .expect("endpoint entry");

    let mut inferred_result_count = 0;
    let mut additional_call_count = 0;
    for state in checked.machine_states(provider) {
        for statement in checked.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Call(call) = statement {
                let other_endpoint = checked
                    .machine_specializations
                    .iter()
                    .find(|specialization| {
                        specialization.template == endpoint_machine
                            && specialization.const_arguments == ["5"]
                    })
                    .expect("the additional endpoint has its own selection");
                let other_machine = checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == other_endpoint.instance)
                    .expect("the additional endpoint instance");
                assert_eq!(
                    call.target_symbol,
                    checked.machine_states(other_machine)[0].symbol
                );
                assert_ne!(call.target_symbol, selected_state.symbol);
                assert!(call.machine_arguments.is_empty());
                additional_call_count += 1;
            }
            let StatementNode::LocalData(local) = statement else {
                continue;
            };
            if !local.type_is_inferred {
                continue;
            }
            let ExpressionNode::Call(call) =
                checked.expression_table.expression(local.initial_value)
            else {
                continue;
            };
            assert_eq!(call.target_symbol, selected_state.symbol);
            assert!(call.machine_arguments.is_empty());
            assert_eq!(local.type_reference, selected_state.return_type);
            inferred_result_count += 1;
        }
    }
    assert_eq!(
        inferred_result_count, 1,
        "the clone retains one tail-call local"
    );
    assert_eq!(additional_call_count, usize::from(additional_endpoint));
}
