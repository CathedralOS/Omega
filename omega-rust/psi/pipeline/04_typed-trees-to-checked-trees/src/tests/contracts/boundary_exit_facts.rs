use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::front_end::typed_program_with_core_service;

#[test]
fn output_predicates_survive_read_only_boundary_arguments() {
    for receiver in ["console: Binding<Console>", "console: &mut Console"] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            pub boundary trait Console {{ machine write(text: &[u8]); }}
            machine fill({receiver}, output: &mut [u8; 4]) reaches Console
            ensures output in Utf8 {{
                output = "okay";
                console.write(output);
            }}
        "#
        );
        // The `Binding<Console>` receiver needs the same fused-service
        // erasure authorizations `settle_checked_providers` binds in real
        // builds; without them the carrier parameter stays unshaped.
        let mut typed = typed_program_with_core_service(&source);
        crate::tests::bind_fixture_fused_service_erasures(&mut typed);
        lower_typed_trees(typed, &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{receiver}: {diagnostics:#?}"));
    }
}

#[test]
fn boundary_parameter_methods_do_not_acquire_builtin_empty_frames() {
    use typed_trees::statement::StatementNode;
    let program = parse_typed_trees(
        r#"
        boundary trait Console { machine bytes() -> u64; }
        machine run(console: &mut Console) reaches Console { let count: u64 = console.bytes(); }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run");
    let state = &program.machine_states(machine)[0];
    let statement = &program.statement_table.statements(state.statement_nodes)[0];
    assert!(matches!(statement, StatementNode::LocalData(_)));
    let paths = validation::CallFrameResolver::new(&program)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert_eq!(paths, Some(vec!["console".to_owned()]));
    let StatementNode::LocalData(local) = statement else {
        unreachable!();
    };
    let expression = local.initial_value;
    let mut missing = program.clone();
    let typed_trees::expression::ExpressionNode::Call(call) =
        missing.expression_table.expression_mut(expression)
    else {
        panic!("value call");
    };
    call.target_symbol = symbols::SymbolHandle::invalid();
    let machine = missing
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run");
    let state = &missing.machine_states(machine)[0];
    let statement = &missing.statement_table.statements(state.statement_nodes)[0];
    let paths = validation::CallFrameResolver::new(&missing)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert!(
        paths.is_none_or(|paths| paths.iter().any(|path| path == "console")),
        "a missing method identity must not turn a known boundary receiver into a builtin"
    );
}
