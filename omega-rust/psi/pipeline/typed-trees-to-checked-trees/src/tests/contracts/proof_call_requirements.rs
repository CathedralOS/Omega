use super::parse_typed_trees;

const RESTRICTED: &str = r#"
    data Nat { case Zero; case Succ(previous: Nat); }
    machine restricted(value: Nat) -> Nat
        requires value == Nat::Zero;
        terminates;
    { value }
"#;

#[test]
fn mathematical_value_call_requires_established_premises() {
    let source = format!(
        "{RESTRICTED}
        machine caller(value: Nat) -> Nat terminates; {{ restricted(value) }}"
    );
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
        .map(|_| ())
        .expect_err("an unknown precondition prevents mathematical call formation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")
                || diagnostic.message.contains("required fact")),
        "{diagnostics:?}"
    );
}

#[test]
fn mathematical_value_call_accepts_the_exact_caller_premise() {
    let source = format!(
        "{RESTRICTED}
        machine caller(value: Nat) -> Nat
        requires value == Nat::Zero;
        terminates;
        {{ restricted(value) }}"
    );
    crate::lower_typed_trees(parse_typed_trees(&source))
        .expect("exact caller premise establishes the call");
}

#[test]
fn recursive_call_establishes_the_premise_at_the_smaller_arguments() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine add(left: Nat, right: Nat) -> Nat
        terminates by left;
        {
            transition left {
                Nat::Zero -> right
                Nat::Succ { previous } -> Nat::Succ { previous: add(previous, right) }
            }
        }
        machine cancel(prefix: Nat, left: Nat, right: Nat) -> Nat
        requires add(prefix, left) == add(prefix, right);
        ensures left == right;
        terminates by prefix;
        {
            transition prefix {
                Nat::Zero -> base(left, right)
                Nat::Succ { previous } -> step(previous, left, right)
            }
            state base(left: Nat, right: Nat) -> Nat { transition { _ -> left } }
            state step(previous: Nat, left: Nat, right: Nat) -> Nat {
                transition { _ -> (cancel(previous, left, right)) }
            }
        }
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .expect("constructor injectivity establishes the exact recursive premise");
}

#[test]
fn state_forwarding_preserves_the_actual_premise_subject() {
    for (argument, accepted) in [("known", true), ("unknown", false)] {
        let source = format!(
            r#"{RESTRICTED}
            machine caller(value: Nat, other: Nat) -> Nat
            requires value == Nat::Zero;
            terminates;
            {{
                transition {{ _ -> next(other, value) }}
                state next(unknown: Nat, known: Nat) -> Nat {{
                    restricted({argument})
                }}
            }}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{argument}: {:?}", result.err());
    }
}

#[test]
fn branch_premise_covers_only_its_reaching_path() {
    for (other_arm, accepted) in [("Nat::Zero", true), ("other", false)] {
        let source = format!(
            r#"{RESTRICTED}
            machine caller(value: Nat, other: Nat) -> Nat terminates;
            {{
                transition value {{
                    Nat::Zero -> next(value)
                    Nat::Succ {{ previous }} -> next({other_arm})
                }}
                state next(argument: Nat) -> Nat {{ restricted(argument) }}
            }}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{other_arm}: {:?}", result.err());
    }
}

#[test]
fn later_case_refinement_cannot_establish_an_earlier_call() {
    let source = format!(
        r#"{RESTRICTED}
        machine caller(value: Nat) -> Nat terminates;
        {{
            let unused: Nat = restricted(value);
            transition value {{
                Nat::Zero -> value
                Nat::Succ {{ previous }} -> previous
            }}
        }}
    "#
    );
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
        .map(|_| ())
        .expect_err("later case refinement is unavailable at call entry");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires"))
    );
}

#[test]
fn discarded_mathematical_call_still_owes_its_premise() {
    let source = format!(
        r#"{RESTRICTED}
        machine caller(value: Nat) terminates;
        {{ _ = restricted(value); }}
    "#
    );
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
        .map(|_| ())
        .expect_err("discarding a result cannot waive the selected contract");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires"))
    );
}

#[test]
fn satisfied_premise_does_not_replace_recursive_descent() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine forever(value: Nat) -> Nat
        requires value == Nat::Zero;
        terminates by value;
        { transition { _ -> (forever(value)) } }
    "#;
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("exact premises cannot justify an unchanged recursive argument");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("decrease"))
    );
}

#[test]
fn descending_call_still_establishes_its_own_premise() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine descend(value: Nat) -> Nat
        requires value != Nat::Zero;
        terminates by value;
        {
            transition value {
                Nat::Zero -> Nat::Zero
                Nat::Succ { previous } -> (descend(previous))
            }
        }
    "#;
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("the predecessor may be Zero despite structural descent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires"))
    );
}

#[test]
fn earlier_citation_supplies_only_its_established_guarantees() {
    let source = format!(
        r#"{RESTRICTED}
        machine constant() -> Nat
        ensures result == Nat::Zero;
        terminates;
        {{ transition {{ _ -> Nat::Zero }} }}
        machine caller() -> Nat terminates;
        {{
            let value: Nat = constant();
            transition {{ _ -> (restricted(value)) }}
        }}
    "#
    );
    crate::lower_typed_trees(parse_typed_trees(&source))
        .expect("the completed constant call establishes its result's premise");
}

#[test]
fn later_state_reentry_cannot_reuse_the_first_arrivals_premise() {
    let source = format!(
        r#"{RESTRICTED}
        machine caller(value: Nat, other: Nat)
        requires value == Nat::Zero;
        {{
            transition {{ _ -> repeat(value, other) }}
            state repeat(current: Nat, replacement: Nat) {{
                _ = restricted(current);
                transition {{ _ -> repeat(replacement, replacement) }}
            }}
        }}
    "#
    );
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
        .map(|_| ())
        .expect_err("later arrivals do not preserve the first Zero argument");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}

#[test]
fn guard_call_is_checked_before_its_result_refinement() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine test_zero(value: Nat) -> bool
        requires value == Nat::Zero;
        terminates;
        { transition { _ -> true } }
        machine caller(value: Nat) -> Nat terminates;
        {
            transition test_zero(value) {
                true -> Nat::Zero
                false -> Nat::Zero
            }
        }
    "#;
    let typed = parse_typed_trees(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("caller");
    let state = &typed.machine_states(machine)[0];
    let mut guards = 0;
    for (statement_index, _) in typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let Some(target) = crate::semantic_calls::transition_call_target(
            &typed,
            machine,
            state,
            statement_index,
            0,
        ) && !target.is_valid()
        {
            guards += 1;
        }
    }
    assert!(
        guards > 0,
        "the call is evaluated in the guard, not its selected target"
    );
    let diagnostics = crate::lower_typed_trees(typed)
        .map(|_| ())
        .expect_err("guard requires is unestablished");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}
