use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match crate::lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved contract accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("requires")
                        || diagnostic.message.contains("ensures")
                        || diagnostic.message.contains("violates required fact")),
                "{diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn nested_argument_mutation_precedes_outer_call_requires() {
    check(
        r#"
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine consume(ignored: bool, flag: bool)
        requires flag == true
        {}
        machine run() {
            let mut flag: bool = true;
            consume(clear(&mut flag), flag);
        }
    "#,
        false,
    );
}

#[test]
fn later_argument_guarantees_do_not_rebind_an_earlier_copied_argument() {
    check(
        r#"
        machine establish(flag: &mut bool) -> bool
        ensures flag == true
        { flag = true; true }
        machine consume(first: bool, ignored: bool)
        requires first == true
        {}
        machine run() {
            let mut flag: bool = false;
            consume(flag, establish(&mut flag));
        }
    "#,
        false,
    );
}

#[test]
fn later_storage_write_retires_instantiated_boolean_call_guarantees() {
    for (replacement, accepted) in [("", true), ("clear(&mut flag);", false)] {
        check(
            &format!(
                r#"
            machine establish(flag: &mut bool)
            ensures flag == true
            {{ flag = true; }}
            machine clear(flag: &mut bool) {{ flag = false; }}
            machine consume(flag: bool)
            requires flag == true
            {{}}
            machine run() {{
                let mut flag: bool = false;
                establish(&mut flag);
                {replacement}
                consume(flag);
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn storage_dependencies_never_assert_their_boolean_operands() {
    check(
        r#"
        machine establish(first: &mut bool, second: &mut bool)
        ensures first == second
        { first = false; second = false; }
        machine consume(flag: bool) requires flag {}
        machine run() {
            let mut first: bool = false;
            let mut second: bool = false;
            establish(&mut first, &mut second);
            consume(first);
        }
        "#,
        false,
    );
}

#[test]
fn skipped_invocations_do_not_require_unexecuted_preconditions() {
    for (gate, accepted) in [("false", true), ("true", false)] {
        check(
            &format!(
                r#"
            machine need(flag: bool) -> bool
            requires flag == true
            {{ true }}
            machine run() {{
                let ignored: bool = {gate} && need(false);
            }}
        "#
            ),
            accepted,
        );
    }
    check(
        r#"
        machine need(flag: bool) -> bool
        requires flag == true
        { true }
        machine run(gate: bool) {
            let ignored: bool = gate && need(false);
        }
    "#,
        false,
    );
}

#[test]
fn selected_right_operand_receives_its_boolean_guard() {
    for (operator, requirement) in [("&&", "flag"), ("||", "!flag")] {
        check(
            &format!(
                r#"
            machine demand(flag: bool) -> bool
            requires {requirement}
            {{ true }}
            machine run(flag: bool) -> bool {{ flag {operator} demand(flag) }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn selected_operand_guards_follow_exact_arguments_and_live_storage() {
    for (argument, accepted) in [("flag", true), ("other", false)] {
        check(
            &format!(
                r#"
            machine demand(renamed: bool) -> bool requires renamed {{ true }}
            machine run(flag: bool, other: bool) -> bool {{
                flag && demand({argument})
            }}
        "#
            ),
            accepted,
        );
    }
    check(
        r#"
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine demand(ignored: bool, current: bool) -> bool requires current { true }
        machine run(flag: &mut bool) -> bool {
            flag && demand(clear(&mut flag), flag)
        }
    "#,
        false,
    );
}

#[test]
fn known_short_circuit_skips_unexecuted_effects() {
    for (operator, initial) in [("&&", "false"), ("||", "true")] {
        check(
            &format!(
                r#"
            machine clear(flag: &mut bool) -> bool {{ flag = false; true }}
            machine run() -> bool
            ensures result == true
            {{
                let mut flag: bool = true;
                let gate: bool = {initial};
                let ignored: bool = gate {operator} clear(&mut flag);
                flag
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn unknown_short_circuit_does_not_publish_conditional_guarantees() {
    check(
        r#"
        machine establish(flag: &mut bool) -> bool
        ensures flag == true
        { flag = true; true }
        machine consume(flag: bool)
        requires flag == true
        {}
        machine run(gate: bool, flag: &mut bool) {
            let ignored: bool = gate && establish(&mut flag);
            consume(flag);
        }
    "#,
        false,
    );
}

#[test]
fn unknown_short_circuit_invalidates_only_conditional_write_subject() {
    for (returned, accepted) in [("flag", false), ("other", true)] {
        check(
            &format!(
                r#"
            machine clear(flag: &mut bool) -> bool {{ flag = false; true }}
            machine run(gate: bool) -> bool
            ensures result == true
            {{
                let mut flag: bool = true;
                let other: bool = true;
                let ignored: bool = gate && clear(&mut flag);
                {returned}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn transition_jump_calls_evaluate_arguments_before_requires() {
    check(
        r#"
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine run() {
            let mut flag: bool = true;
            transition { _ -> done(clear(&mut flag), flag) }
            state done(ignored: bool, current: bool) requires current {}
        }
        "#,
        false,
    );
}

#[test]
fn transition_value_calls_preserve_short_circuit_guards_and_effects() {
    for operand in [
        "false && clear(&mut self.flag)",
        "true || clear(&mut self.flag)",
    ] {
        check(
            &format!(
                r#"
                machine clear(flag: &mut bool) -> bool {{ flag = false; true }}
                data Main {{ flag: bool; }}
                machine Main::run(&mut self) -> bool
                requires self.flag
                ensures self.flag
                {{
                    transition {{ _ -> ({operand}) }}
                }}
                "#
            ),
            true,
        );
    }
    check(
        r#"
        machine demand(flag: bool) -> bool requires flag { true }
        machine run(flag: bool) -> bool {
            transition { _ -> (flag && demand(flag)) }
        }
        "#,
        true,
    );
}

#[test]
fn transition_jump_inputs_do_not_run_skipped_mutations() {
    check(
        r#"
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine run() {
            let mut flag: bool = true;
            transition { _ -> done(false && clear(&mut flag), flag) }
            state done(ignored: bool, current: bool) requires current {}
        }
        "#,
        true,
    );
}

#[test]
fn jump_requirements_use_live_values_instead_of_local_initializers() {
    for (initial, update, accepted) in [
        ("true", "", true),
        ("false", "flag = true;", true),
        ("true", "flag = false;", false),
        ("false", "", false),
    ] {
        check(
            &format!(
                r#"
                machine run() {{
                    let mut flag: bool = {initial};
                    {update}
                    transition {{ _ -> done(flag) }}
                    state done(current: bool) requires current {{}}
                }}
            "#
            ),
            accepted,
        );
    }
}

#[test]
fn transition_sibling_writes_do_not_invalidate_other_jump_inputs() {
    check(
        r#"
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine run(gate: bool) {
            let mut flag: bool = true;
            transition gate {
                true -> discarded(clear(&mut flag))
                false -> checked(flag)
            }
            state discarded(ignored: bool) {}
            state checked(current: bool) requires current {}
        }
        "#,
        true,
    );
}

#[test]
fn jump_operand_mutation_cannot_replay_the_taken_guard() {
    check(
        r#"
        data Main { flag: bool; }
        machine clear(flag: &mut bool) -> bool { flag = false; true }
        machine Main::run(&mut self) {
            transition self.flag {
                true -> done(clear(&mut self.flag), self.flag)
                false -> {}
            }
            state done(ignored: bool, current: bool) requires current {}
        }
        "#,
        false,
    );
}
