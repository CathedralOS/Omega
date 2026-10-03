//! Generic cleanup prerequisites follow exact substituted owners and current
//! call operands. Generic syntax alone cannot export a concrete body demand.
use crate::tests::flow::terminal_unit::machine_named;
use crate::tests::front_end::{checked_program, checked_program_result};

const TOKEN: &str = "
    data Token { ready: bool; }
    machine Token::drop(owner: &mut Token) requires owner.ready {}
";

fn rejects_contextual_input(source: &str) {
    let diagnostics = checked_program_result(source)
        .err()
        .expect("unproved cleanup input must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("contextual cleanup prerequisite")
                && diagnostic.message.contains("Token::drop")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn contextual_generic_retire_checks_current_local_and_constructor_actuals() {
    let source = format!(
        "{TOKEN}
        machine retire<T>(value: T) {{}}
        machine run() {{
            let token: Token = Token {{ ready: true }};
            retire(token);
            retire(Token {{ ready: true }});
        }}"
    );
    let checked = checked_program(&source);
    let template = machine_named(&checked, "retire");
    let rows = &checked.facts.contextual_cleanup.entry_requirements.rows;
    assert!(
        rows.iter()
            .any(|(_, row)| row.template_machine == template && row.expected)
    );
    assert!(
        !rows
            .iter()
            .any(|(_, row)| row.machine == machine_named(&checked, "run")),
        "locally established facts do not become caller demands"
    );
    rejects_contextual_input(&source.replace("ready: true", "ready: false"));
}

#[test]
fn contextual_generic_forwarding_transports_only_the_selected_input_requirement() {
    let source = format!(
        "{TOKEN}
        machine retire<T>(value: T) {{}}
        machine forward<T>(marker: bool, value: T) {{ retire(value); }}
        machine run() {{ forward(true, Token {{ ready: true }}); }}"
    );
    let checked = checked_program(&source);
    let forward = machine_named(&checked, "forward");
    let instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|row| row.template == forward)
        .map(|row| row.instance)
        .collect();
    assert!(!instances.is_empty());
    for instance in instances {
        let rows: Vec<_> = checked
            .facts
            .contextual_cleanup
            .entry_requirements
            .rows
            .iter()
            .filter(|(_, row)| row.machine == instance)
            .map(|(_, row)| row)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].parameter_position, 1,
            "the primitive argument retains its source position"
        );
        assert!(rows[0].expected);
    }
    rejects_contextual_input(&source.replace("ready: true", "ready: false"));
}

#[test]
fn contextual_generic_identity_does_not_invent_a_cleanup_demand() {
    let source = format!(
        "{TOKEN}
        machine identity<T>(value: T) -> T {{ value }}
        machine pass(value: Token) -> Token {{ identity(value) }}"
    );
    let checked = checked_program(&source);
    let identity = machine_named(&checked, "identity");
    let instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|row| row.template == identity)
        .map(|row| row.instance)
        .collect();
    assert!(!instances.is_empty());
    assert!(
        !checked
            .facts
            .contextual_cleanup
            .entry_requirements
            .rows
            .iter()
            .any(|(_, row)| instances.contains(&row.machine)),
        "returning custody untouched has no symbolic death prerequisite"
    );
}

#[test]
fn contextual_generic_unrelated_type_parameter_cannot_export_a_concrete_demand() {
    let source = format!(
        "{TOKEN}
        machine unrelated<T>(value: T, token: Token) {{}}
        machine run() {{ unrelated(true, Token {{ ready: true }}); }}"
    );
    let diagnostics = checked_program_result(&source)
        .err()
        .expect("fixed Token remains an ordinary body obligation");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("missing token.ready == true required by Token::drop")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn contextual_generic_call_cannot_reuse_an_entry_fact_after_mutation() {
    let source = format!(
        "{TOKEN}
        machine retire<T>(value: T) {{}}
        machine run(mut value: Token) requires value.ready {{
            value.ready = false;
            retire(value);
        }}"
    );
    rejects_contextual_input(&source);
}

#[test]
fn contextual_generic_instances_and_repeated_actuals_keep_distinct_predicates() {
    let source = format!(
        "{TOKEN}
        data Other {{ ready: bool; }}
        machine Other::drop(owner: &mut Other) requires !owner.ready {{}}
        machine retire<T>(value: T) {{}}
        machine run() {{
            retire(Token {{ ready: true }});
            retire(Other {{ ready: false }});
            retire(Token {{ ready: true }});
        }}"
    );
    let checked = checked_program(&source);
    let retire = machine_named(&checked, "retire");
    let instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|row| row.template == retire)
        .map(|row| row.instance)
        .collect();
    assert_eq!(instances.len(), 2);
    let rows: Vec<_> = checked
        .facts
        .contextual_cleanup
        .entry_requirements
        .rows
        .iter()
        .filter(|(_, row)| instances.contains(&row.machine))
        .map(|(_, row)| row)
        .collect();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].machine, rows[1].machine);
    assert_ne!(rows[0].expected, rows[1].expected);
    assert_ne!(rows[0].hook_machine, rows[1].hook_machine);
    let invalid = source.replacen(
        "retire(Token { ready: true });",
        "retire(Token { ready: false });",
        1,
    );
    rejects_contextual_input(&invalid);
}

#[test]
fn contextual_cleanup_uses_only_the_guarded_edge_that_retains_the_owner() {
    let source = "
        data Token { ready: bool; other: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine choose(value: Token) -> Token {
            transition value.ready {
                true -> Token { ready: true, other: false }
                false -> value
            }
        }
    ";
    let checked = checked_program(source);
    let machine = machine_named(&checked, "choose");
    let selected: Vec<_> = checked
        .facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| {
            row.machine == machine
                && row.source == language_semantics::PermissionEventSource::StateExit
        })
        .map(|(_, row)| row)
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "the returned owner does not also die on the false arm"
    );
    let exit = selected[0].exit;
    assert!(exit.state_symbol.is_valid());
    assert_eq!(
        exit.kind,
        checked_trees::FlowClaimJoinExitKind::TransitionTarget
    );
    let state = checked
        .machine_states(checked.machine_by_symbol(machine).unwrap())
        .iter()
        .find(|state| state.symbol == exit.state_symbol)
        .unwrap();
    let typed_trees::statement::StatementNode::Transition(transition) =
        &checked.statement_table.statements(state.statement_nodes)[exit.statement_index]
    else {
        panic!("selected transition");
    };
    assert!(checked.facts.flow.control.exits.iter().any(|(_, row)| {
        row.state_symbol == exit.state_symbol
            && row.statement_index == exit.statement_index
            && row.transition_target == transition.target
    }));
    let diagnostics =
        checked_program_result(&source.replace("transition value.ready", "transition value.other"))
            .expect_err("a different guarded field cannot prove ready at the death edge");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ready")
                && diagnostic.message.contains("Token::drop")),
        "{diagnostics:#?}"
    );
}

#[test]
fn contextual_replacement_checks_old_storage_after_rhs_calls() {
    let source = format!(
        "{TOKEN}
        machine replacement(owner: &mut Token) -> Token
        ensures !owner.ready
        {{ owner.ready = false; Token {{ ready: true }} }}
        machine replace(mut value: Token) -> Token
        requires value.ready
        {{ value = replacement(&mut value); value }}"
    );
    let diagnostics = checked_program_result(&source)
        .err()
        .expect("RHS invalidates the old owner's cleanup prerequisite");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ready")
                && diagnostic.message.contains("Token::drop")),
        "{diagnostics:#?}"
    );
}

#[test]
fn contextual_replacement_uses_rhs_guarantee_before_new_storage_is_established() {
    let source = format!(
        "{TOKEN}
        machine replacement(owner: &mut Token) -> Token
        ensures owner.ready
        {{ owner.ready = true; Token {{ ready: false }} }}
        machine replace(mut value: Token) -> Token
        requires !value.ready
        {{ value = replacement(&mut value); value }}"
    );
    let checked = checked_program(&source);
    let machine = machine_named(&checked, "replace");
    assert!(
        checked
            .facts
            .contextual_cleanup
            .dispositions
            .iter()
            .any(|(_, row)| {
                row.machine == machine
                    && row.source
                        == language_semantics::PermissionEventSource::Statement {
                            statement_index: 0,
                        }
            }),
        "the old owner is disposed at replacement, while the false replacement is returned"
    );
}
