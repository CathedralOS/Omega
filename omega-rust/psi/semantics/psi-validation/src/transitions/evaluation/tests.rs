mod returns;

fn parse(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn check(source: &str, accepted: bool) {
    check_typed(&parse(source), source, accepted);
}

fn check_typed(typed: &psi_typed_trees::TypedTrees, source: &str, accepted: bool) {
    let result = crate::validate_program(typed);
    if accepted {
        assert!(result.is_ok(), "{source}: {result:#?}");
    } else {
        let diagnostics = result.expect_err("invalidated Exact range must reject");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("overflow") || diagnostic.message.contains("range")
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn combined_transition_siblings_keep_separate_argument_ranges() {
    use psi_typed_trees::statement::StatementNode;

    for (source, accepted) in [
        (
            "machine replace(target: &mut u8) -> u8 { target = 255; 0 } machine run(flag: bool) -> u8 { let mut current: u8 = 3; transition flag { true -> finish(replace(&mut current)) false -> finish(current + 1) } state finish(value: u8) -> u8 { value } }",
            true,
        ),
        (
            "machine run(current: u8) -> u8 { transition current >= 10 { true -> finish(0) false -> finish(current + 1) } state finish(value: u8) -> u8 { value } }",
            true,
        ),
        (
            "machine run(current: u8) -> u8 { transition current < 10 { true -> finish(0) false -> finish(current + 1) } state finish(value: u8) -> u8 { value } }",
            false,
        ),
        (
            "machine replace(target: &mut u8) -> bool { target = 255; false } machine run() -> u8 { let mut current: u8 = 3; transition replace(&mut current) { true -> finish(0) false -> finish(current + 1) } state finish(value: u8) -> u8 { value } }",
            false,
        ),
        (
            "machine replace(target: &mut u8) -> u8 { target = 255; 0 } machine run(flag: bool) -> u8 { let mut current: u8 = 3; transition flag { true -> finish(0, 0) false -> finish(replace(&mut current), current + 1) } state finish(first: u8, second: u8) -> u8 { second } }",
            false,
        ),
    ] {
        let mut typed = parse(source);
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "run")
            .unwrap()
            .clone();
        let nodes = typed.machine_states(&machine)[0].statement_nodes;
        let statements = typed.statement_table.statements(nodes);
        let first = statements.len() - 2;
        let StatementNode::Transition(sibling) = &statements[first + 1] else {
            panic!("last statement is the false transition")
        };
        let continuation = sibling.target;
        let StatementNode::Transition(primary) =
            &mut typed.statement_table.statements_mut(nodes)[first]
        else {
            panic!("preceding statement is the true transition")
        };
        assert!(!primary.continuation.is_valid());
        primary.continuation = continuation;
        typed.machine_states_mut(&machine)[0].statement_nodes =
            psi_arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
        check_typed(&typed, source, accepted);
    }
}

#[test]
fn transition_argument_ranges_cross_only_own_and_earlier_calls() {
    for (arguments, accepted) in [
        ("current + 1, replace(&mut current)", true),
        ("replace(&mut current), current + 1", false),
        ("current + replace(&mut current), 0", false),
    ] {
        check(
            &format!(
                "machine replace(target: &mut u8) -> u8 {{ target = 255; 0 }} machine run() -> u8 {{ let mut current: u8 = 3; transition {{ _ -> finish({arguments}) }} state finish(first: u8, second: u8) -> u8 {{ first }} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn transition_guard_writes_retire_pre_guard_ranges_and_guard_bounds() {
    for guard in [
        "replace(&mut current)",
        "current < 10 && replace(&mut current)",
    ] {
        check(
            &format!(
                "machine replace(target: &mut u8) -> bool {{ target = 255; true }} machine run() -> u8 {{ let mut current: u8 = 3; transition {guard} {{ true -> finish(current + 1) false -> finish(0) }} state finish(value: u8) -> u8 {{ value }} }}"
            ),
            false,
        );
    }
    check(
        "machine run(current: u8) -> u8 { transition current < 10 { true -> finish(current + 1) false -> finish(0) } state finish(value: u8) -> u8 { value } }",
        true,
    );
}

#[test]
fn transition_argument_narrowing_uses_its_own_evaluation_point() {
    for (arguments, parameters, accepted) in [
        (
            "current, replace(&mut current)",
            "first: u8, second: u16",
            true,
        ),
        (
            "current as u8, replace(&mut current)",
            "first: u8, second: u16",
            true,
        ),
        (
            "replace(&mut current), current",
            "first: u16, second: u8",
            false,
        ),
        (
            "replace(&mut current), current as u8",
            "first: u16, second: u8",
            false,
        ),
    ] {
        check(
            &format!(
                "machine replace(target: &mut u16) -> u16 {{ target = 65535; 0 }} machine run() {{ let mut current: u16 = 3; transition {{ _ -> finish({arguments}) }} state finish({parameters}) {{}} }}"
            ),
            accepted,
        );
    }
}
