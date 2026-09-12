use super::*;

const BORROWED_SOURCE: &str = r#"
    boundary trait Output {
        machine emit(value: i32);
        machine echo(value: i32) -> i32;
    }
    boundary trait Other {
        machine emit(value: i32);
        machine echo(value: i32) -> i32;
    }
    data OutputProvider {}
    machine OutputProvider::emit(value: i32) satisfies Output::emit {}
    machine OutputProvider::echo(value: i32) -> i32 satisfies Output::echo { value }
    data OtherProvider {}
    machine OtherProvider::emit(value: i32) satisfies Other::emit {}
    machine OtherProvider::echo(value: i32) -> i32 satisfies Other::echo { value }

    machine output(service: &mut Output, value: i32) reaches Output {
        transition { _ -> send(service, value) }
        state send(service: &mut Output, value: i32) { service.emit(value); }
    }
    machine query(service: &Output) -> i32 reaches Output { service.echo(35) }
    machine other(service: &mut Other) reaches Other { service.emit(2); }
"#;

fn borrowed_fixture() -> (CheckedTrees, effects::SelectedProviderPlanFacts) {
    let mut sources = source::SourceMap::default();
    sources.add(
        "selected-dispatch/borrowed.omg".into(),
        BORROWED_SOURCE.into(),
    );
    let tokens = source_files_to_tokens::Lexer::new(BORROWED_SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        Arc::new(sources),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let plans = provider_planning::plans::derive_satisfies_plans(&typed, None);
    let selected = selected_plan(&plans, "Output");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    (checked, selected)
}

fn send_state(program: &CheckedTrees) -> typed_trees::state::State {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "output")
        .unwrap();
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "send")
        .unwrap()
        .clone()
}

#[test]
fn borrowed_boundary_parameters_keep_source_and_exact_selection() {
    let (checked, selected) = borrowed_fixture();
    let send = send_state(&checked);
    let typed_trees::statement::StatementNode::Call(call) =
        &checked.statement_table.statements(send.statement_nodes)[0]
    else {
        panic!("source call")
    };
    let receiver = call.receiver_symbol;
    let requirement = call.target_symbol;
    let target = adapter_entry_symbol(&checked, "OutputProvider::emit");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(settled.typed, original.typed);
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .any(|row| row.receiver == receiver
                && row.requirement == requirement
                && row.realization_state == target)
    );
}

#[test]
fn borrowed_boundary_dispatch_rejects_symbol_and_method_drift_atomically() {
    for mutation in 0..3 {
        let (mut checked, selected) = borrowed_fixture();
        let send = send_state(&checked);
        match mutation {
            0 => {
                checked
                    .typed
                    .state_parameters
                    .span_mut_or_empty(send.parameters)[0]
                    .symbol = symbols::SymbolHandle::invalid()
            }
            1 => {
                let other_target = checked
                    .traits()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Other")
                    .and_then(|definition| {
                        checked
                            .trait_machine_signatures(definition)
                            .iter()
                            .find(|signature| signature.name.as_str() == "emit")
                    })
                    .unwrap()
                    .symbol;
                let typed_trees::statement::StatementNode::Call(call) = &mut checked
                    .typed
                    .statement_table
                    .statements_mut(send.statement_nodes)[0]
                else {
                    panic!("source statement call");
                };
                call.target_symbol = other_target;
            }
            2 => {
                let typed_trees::statement::StatementNode::Call(call) = &mut checked
                    .typed
                    .statement_table
                    .statements_mut(send.statement_nodes)[0]
                else {
                    panic!("source statement call");
                };
                call.target = typed_trees::name::Identifier::generated("echo");
            }
            _ => unreachable!(),
        }
        let original = Arc::new(checked);
        let mut rejected = Arc::clone(&original);
        assert!(
            settle_selected_boundary_adapter_dispatch(&mut rejected, &selected).is_err(),
            "mutation {mutation}"
        );
        assert!(Arc::ptr_eq(&original, &rejected), "mutation {mutation}");
    }
}
