use super::*;

#[test]
fn output_predicates_survive_read_only_boundary_arguments() {
    for receiver in ["console: Console", "console: &mut Console"] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            boundary trait Console {{ machine write(text: &[u8]); }}
            machine fill({receiver}, output: &mut [u8; 4])
            ensures output in Utf8 {{
                output = "okay";
                console.write(output);
            }}
        "#
        );
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{receiver}: {diagnostics:#?}"));
    }
}

#[test]
fn output_predicates_do_not_survive_writable_boundary_arguments() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine read(output: &mut [u8]); }
        machine fill(device: &mut Device, output: &mut [u8; 4])
        ensures output in Utf8 {
            output = "okay";
            device.read(output);
        }
    "#;
    assert!(lower_typed_trees(parse_typed_trees(source)).is_err());
}

#[test]
fn output_predicates_survive_read_only_boundary_expression_arguments() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Console { machine write(text: &[u8]) -> u64; }
        machine fill(console: &mut Console, output: &mut [u8; 4])
        ensures output in Utf8 {
            output = "okay";
            let count: u64 = console.write(output);
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("expression calls use the same selected readonly formal frame");
}

#[test]
fn boundary_parameter_frames_require_exact_receiver_scope_and_signature() {
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::statement::StatementNode;
    let original = parse_typed_trees(
        r#"
        boundary trait Console { machine write(text: &[u8]); }
        boundary trait Device { machine write(text: &mut [u8]); }
        machine fill(console: &mut Console, output: &mut [u8; 4]) {
            console.write(output);
            state other(console: &mut Device, output: &mut [u8; 4]) {
                console.write(output);
            }
        }
    "#,
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .expect("fill");
    let entry = &original.machine_states(machine)[0];
    let other = &original.machine_states(machine)[1];
    let statements = entry.statement_nodes;
    let StatementNode::Call(call) = &original.statement_table.statements(statements)[0] else {
        panic!("entry call");
    };
    let StatementNode::Call(other_call) =
        &original.statement_table.statements(other.statement_nodes)[0]
    else {
        panic!("other call");
    };
    for (receiver, target, exact) in [
        (call.receiver_symbol, call.target_symbol, true),
        (SymbolHandle::invalid(), call.target_symbol, false),
        (
            SymbolHandle::from_parts(
                call.receiver_symbol.arena_index(),
                call.receiver_symbol.generation() + 1,
            ),
            call.target_symbol,
            false,
        ),
        (call.receiver_symbol, SymbolHandle::invalid(), false),
        (
            call.receiver_symbol,
            SymbolHandle::from_parts(
                call.target_symbol.arena_index(),
                call.target_symbol.generation() + 1,
            ),
            false,
        ),
        (call.receiver_symbol, other_call.target_symbol, false),
        (other_call.receiver_symbol, other_call.target_symbol, false),
    ] {
        let mut program = original.clone();
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statements)[0]
        else {
            panic!("entry call");
        };
        call.receiver_symbol = receiver;
        call.target_symbol = target;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "fill")
            .expect("fill");
        let StatementNode::Call(call) = &program.statement_table.statements(statements)[0] else {
            panic!("entry call");
        };
        let paths = psi_validation::CallFrameResolver::new(&program)
            .expect("resolver")
            .may_write_frame(machine, call)
            .into_complete_paths();
        if exact {
            assert_eq!(paths, Some(vec!["console".to_owned()]));
        } else {
            assert!(
                paths.is_none_or(|paths| paths.iter().any(|path| path == "output")),
                "invalid receiver/signature identity cannot manufacture a readonly frame"
            );
        }
    }
}

#[test]
fn boundary_parameter_methods_do_not_acquire_builtin_empty_frames() {
    use psi_typed_trees::statement::StatementNode;
    let program = parse_typed_trees(
        r#"
        boundary trait Console { machine bytes() -> u64; }
        machine run(console: &mut Console) { let count: u64 = console.bytes(); }
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
    let paths = psi_validation::CallFrameResolver::new(&program)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert_eq!(paths, Some(vec!["console".to_owned()]));
    let StatementNode::LocalData(local) = statement else {
        unreachable!();
    };
    let expression = local.initial_value;
    let mut missing = program.clone();
    let psi_typed_trees::expression::ExpressionNode::Call(call) =
        missing.expression_table.expression_mut(expression)
    else {
        panic!("value call");
    };
    call.target_symbol = psi_symbols::SymbolHandle::invalid();
    let machine = missing
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run");
    let state = &missing.machine_states(machine)[0];
    let statement = &missing.statement_table.statements(state.statement_nodes)[0];
    let paths = psi_validation::CallFrameResolver::new(&missing)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert!(
        paths.is_none_or(|paths| paths.iter().any(|path| path == "console")),
        "a missing method identity must not turn a known boundary receiver into a builtin"
    );
}
