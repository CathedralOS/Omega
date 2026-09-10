use super::*;
use crate::CallFrameResolver;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("types")
}

// The former demand route deliberately recovered the prefix separately for
// the direct target and closure. Keep it as a result/work comparison, not a
// second production query path.
fn replayed_assignment_paths(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<Vec<String>> {
    let StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    let site = CallerWriteSite::Statement(statement);
    let (aliases, stored) = caller_aliases_at_site(program, machine, symbols, site)?;
    let target = if aliases.is_empty()
        && stored.is_empty()
        && let Some(path) = super::super::coarse_place_path(program, assignment.target)
    {
        AssignmentWriteTarget::Storage { paths: vec![path] }
    } else {
        let state = program.machine_states(machine).iter().find(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|candidate| std::ptr::eq(statement, candidate))
        })?;
        walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            &mut FrameInference::default(),
            &mut Vec::new(),
            Some(StateWriteQuery::Assignment(statement)),
        )?
        .assignment?
    };
    match target {
        AssignmentWriteTarget::LocalBindingReplacement { path } => Some(vec![path]),
        AssignmentWriteTarget::Storage { paths } => {
            close_caller_aliases(program, machine, symbols, site, paths)
        }
    }
}

fn compare_assignments(source: &str) {
    let program = typed(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("exercise");
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    let state = &program.machine_states(machine)[0];
    let assignments: Vec<_> = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter(|statement| matches!(statement, StatementNode::Assignment(_)))
        .collect();
    assert!(!assignments.is_empty());
    for statement in assignments.iter().rev().chain(assignments.iter()) {
        let expected = replayed_assignment_paths(&program, machine, &symbols, statement);
        assert_eq!(
            resolver
                .assignment_write_frame(machine, statement)
                .into_complete_paths(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn assignment_query_reuses_one_prefix_for_target_and_closure() {
    let program = typed(
        "data Pair { left: u64; right: u64; } machine exercise(pair: &mut Pair) { let selected: &mut u64 = &mut pair.left; selected = 7; }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let statement = &program.statement_table.statements(state.statement_nodes)[1];
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());
    super::super::PREFIX_WALKS.with(|walks| walks.set(0));
    let expected = replayed_assignment_paths(&program, machine, &symbols, statement);
    let previous = super::super::PREFIX_WALKS.with(|walks| walks.replace(0));
    let actual = CallFrameResolver::new(&program)
        .expect("resolver")
        .assignment_write_frame(machine, statement)
        .into_complete_paths();
    let current = super::super::PREFIX_WALKS.with(|walks| walks.get());
    assert_eq!(actual, expected);
    assert_eq!(
        actual,
        Some(vec!["pair.left".to_owned(), "selected".to_owned()])
    );
    assert_eq!(previous, 3);
    assert_eq!(current, 1);
}

#[test]
fn assignment_query_preserves_prefix_modes_and_opaque_barriers() {
    for body in [
        "let mut selected: &mut u64 = &mut pair.left; let prior: &mut u64 = selected; selected = &mut pair.right; prior = 7; selected = 9;",
        "let view: &u64 = &pair.right; pair.left = 7;",
        "unknown(); pair.left = 7;",
        "let selected: &mut u64 = &mut pair.left; unknown(); selected = 7;",
        "let selected: &mut u64 = &mut pair.left; selected = unknown_value();",
        "let selected: &mut u64 = helper(&mut pair.left); selected = 7;",
        "let carrier: Carrier = Carrier { value: &mut pair.left }; carrier.value = 7;",
        "let mut carrier: Carrier = Carrier { value: &mut pair.left }; expose(&mut carrier); pair.right = 7;",
    ] {
        compare_assignments(&format!(
            "data Pair {{ left: u64; right: u64; }} data Carrier {{ value: &mut u64; }} machine helper(value: &mut u64) -> &mut u64 {{ value }} machine expose(value: &mut Carrier) {{}} machine exercise(pair: &mut Pair) {{ {body} }}"
        ));
    }
}
