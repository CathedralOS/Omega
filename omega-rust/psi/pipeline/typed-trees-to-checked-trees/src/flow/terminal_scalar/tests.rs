use super::*;

fn crash_source(cause: &str, guard: &str, prefix: &str) -> checked_trees::CheckedTrees {
    let source = format!(
        "machine identity(flag: bool) -> bool {{ flag }}
         machine value(flag: bool) -> u8 crashes {cause} {{
             {prefix}
             transition {{ {guard} -> 7u8 }}
             crash {cause};
         }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

#[test]
fn scalar_crash_destinations_retain_exact_source_site_after_bindings() {
    for (cause, expected_cause) in [
        ("Trap", checked_trees::CrashCause::Trap),
        ("Abort", checked_trees::CrashCause::Abort),
    ] {
        for (guard, prefix, guard_ordinal) in [
            ("flag", "", 0),
            ("flag && identity(flag)", "", 0),
            ("current", "let mut current: bool = flag;", 1),
        ] {
            let checked = crash_source(cause, guard, prefix);
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "value")
                .unwrap();
            let graph = checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .expect("scalar control graph includes the standalone crash fallback");
            let state = &graph.states[0];
            assert_eq!(
                state.terminator,
                CheckedScalarStateTerminator::Conditional {
                    guard_statement_ordinal: guard_ordinal,
                    when_true: CheckedScalarBranchDestination::Return {
                        statement_ordinal: guard_ordinal,
                        is_continuation: false,
                    },
                    when_false: CheckedScalarBranchDestination::Crash {
                        statement_ordinal: guard_ordinal + 1,
                    },
                }
            );
            let contract = checked
                .facts
                .contract_plans
                .for_machine(machine.symbol)
                .unwrap();
            let [site] = contract.crash.checked_sites() else {
                panic!("one exact authored crash site");
            };
            assert_eq!(site.location().state(), state.state);
            assert_eq!(site.location().statement_ordinal(), guard_ordinal + 1);
            assert_eq!(site.cause(), expected_cause);
        }
    }
}

#[test]
fn scalar_crash_destinations_reject_combined_or_nonterminal_exits() {
    let checked = crash_source("Trap", "flag", "");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.machine_states(machine);
    let [
        StatementNode::Transition(ordinary),
        StatementNode::Transition(crash),
    ] = checked
        .statement_table
        .statements(states[0].statement_nodes)
    else {
        panic!("guarded return followed by standalone crash");
    };
    for mutation in 0..7 {
        let mut changed = *crash;
        let is_continuation = match mutation {
            0 => true,
            1 => {
                changed.continuation = ordinary.target;
                false
            }
            2 => {
                changed.target = ordinary.target;
                false
            }
            3 => {
                changed.exit = TransitionExit::Ordinary;
                false
            }
            4 => {
                changed.target = typed_trees::statement::TransitionTargetHandle::invalid();
                false
            }
            5 => {
                changed.target = typed_trees::statement::TransitionTargetHandle::from_parts(
                    changed.target.arena_index(),
                    changed.target.generation() + 1,
                );
                false
            }
            _ => {
                changed.target =
                    typed_trees::statement::TransitionTargetHandle::from_arena_index(u32::MAX);
                false
            }
        };
        assert_eq!(
            checked_branch_destination(&checked, states, 1, &changed, is_continuation),
            None,
            "mutation {mutation}"
        );
    }
}
