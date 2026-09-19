use super::parse_typed_trees;

const RESTRICTED: &str = r#"
    data Nat { case Zero; case Succ(previous: Nat); }
    machine restricted(value: Nat) -> Nat
        requires value == Nat::Zero;
        terminates;
    { value }
"#;

#[test]
fn contract_only_application_establishes_its_selected_premise() {
    for (premise, accepted) in [("", false), ("requires value == Nat::Zero;", true)] {
        let source = format!(
            "{RESTRICTED}
            machine contract_only(value: Nat)
            {premise}
            ensures restricted(value) == restricted(value);
            {{}}"
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn contract_call_cannot_use_its_own_fact_or_a_later_fact() {
    for contracts in [
        "requires restricted(value) == restricted(value); requires value == Nat::Zero;",
        "requires value == Nat::Zero && restricted(value) == restricted(value);",
        "ensures value == Nat::Zero; ensures restricted(value) == restricted(value);",
    ] {
        let source = format!("{RESTRICTED} machine caller(value: Nat) {contracts} {{}}");
        let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
            .map(|_| ())
            .expect_err("formation cannot assume the current or a future fact");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("specification call")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn nested_contract_call_checks_the_inner_application_first() {
    let source = format!(
        r#"{RESTRICTED}
        machine identity(value: Nat) -> Nat terminates; {{ value }}
        machine caller(value: Nat)
        ensures identity(restricted(value)) == identity(restricted(value));
        {{}}
    "#
    );
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(&source))
        .map(|_| ())
        .expect_err("a total outer call does not make its partial operand total");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("specification call `restricted`")),
        "{diagnostics:?}"
    );
}

#[test]
fn contract_call_substitutes_the_selected_argument_position() {
    for (arguments, accepted) in [("other, value", true), ("value, other", false)] {
        let source = format!(
            r#"
            data Nat {{ case Zero; case Succ(previous: Nat); }}
            machine selected(left: Nat, right: Nat) -> Nat
            requires right == Nat::Zero;
            terminates;
            {{ left }}
            machine caller(value: Nat, other: Nat)
            requires value == Nat::Zero;
            ensures selected({arguments}) == selected({arguments});
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{arguments}: {:?}", result.err());
    }
}

#[test]
fn scalar_contract_call_requires_positive_arithmetic_evidence() {
    for (premise, accepted) in [("", false), ("requires value > 0;", true)] {
        let source = format!(
            r#"
            machine restricted(value: u64) -> u64
            requires value > 0;
            terminates;
            {{ value }}
            machine caller(value: u64)
            {premise}
            requires restricted(value) == restricted(value);
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn recursive_contract_cannot_license_its_own_application() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine restricted(value: Nat) -> Nat
        requires restricted(value) == restricted(value);
        terminates;
        { value }
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("a cyclic formation dependency is not a proof of totality");
}

#[test]
fn mutually_dependent_contracts_do_not_establish_formation() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine first(value: Nat) -> Nat
        requires second(value) == second(value);
        terminates;
        { value }
        machine second(value: Nat) -> Nat
        requires first(value) == first(value);
        terminates;
        { value }
    "#;
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("mutual formation assumptions are circular");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cyclic requires formation")),
        "{diagnostics:?}"
    );
}

#[test]
fn qualified_contract_calls_keep_their_actual_argument_roster() {
    for (premise, accepted) in [("", false), ("requires value > 0;", true)] {
        let source = format!(
            r#"
            data Numbers {{}}
            machine Numbers::restricted(value: u64) -> u64
            requires value > 0;
            terminates;
            {{ value }}
            machine caller(value: u64)
            {premise}
            requires Numbers::restricted(value) == Numbers::restricted(value);
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn state_contracts_do_not_confuse_shadowed_parameters() {
    for (premise, accepted) in [("", false), ("requires value == Nat::Zero;", true)] {
        let argument = if accepted { "Nat::Zero" } else { "other" };
        let source = format!(
            r#"{RESTRICTED}
            machine caller(value: Nat, other: Nat)
            requires value == Nat::Zero;
            {{
                transition {{ _ -> next({argument}) }}
                state next(value: Nat)
                {premise}
                requires restricted(value) == restricted(value);
                {{}}
            }}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn contract_call_on_a_closed_satisfying_argument_needs_no_hypothesis() {
    let source = format!(
        r#"{RESTRICTED}
        machine caller()
        ensures restricted(Nat::Zero) == restricted(Nat::Zero);
        {{}}
    "#
    );
    crate::lower_typed_trees(parse_typed_trees(&source))
        .expect("closed arguments can establish their own selected premises");
}

#[test]
fn internal_state_specification_call_does_not_inherit_entry_requires() {
    let source = r#"
        data Nat { case Zero; case Succ(previous: Nat); }
        machine caller(value: Nat) -> Nat
        requires value == Nat::Zero;
        requires independent(Nat::Succ { previous: value }) == independent(Nat::Succ { previous: value });
        {
            transition { _ -> independent(value) }
            state independent(value: Nat) -> Nat { value }
        }
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .expect("selected state entry owes its own requirements, not its machine's initial entry");
}

#[test]
fn opaque_receiver_call_cannot_hide_a_changed_requirement_argument() {
    let source = r#"
        data Reader {}
        machine Reader::read(&self, index: u64) -> u64 terminates; { index }
        machine selected(object: &Reader, index: u64, expected: u64) -> u64
        requires expected == object.read(index);
        terminates;
        { expected }
        machine caller(object: &Reader, index: u64, other: u64, evidence: u64)
        requires evidence == object.read(index);
        requires selected(object, other, evidence) == selected(object, other, evidence);
        {}
    "#;
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("opaque display text cannot implement parameter substitution");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("specification call `selected`")),
        "{diagnostics:?}"
    );
}

#[test]
fn application_unfolding_cannot_erase_different_arguments() {
    let source = r#"
        machine observe(value: bool) -> bool
        terminates;
        { transition { _ -> (value == true) } }
        machine restricted(left: bool, right: bool) -> bool
        requires observe(left) == observe(right);
        terminates;
        { left }
        machine caller()
        requires restricted(false, true) == restricted(false, true);
        {}
    "#;
    let diagnostics = crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("normalizing distinct applications to identical display text is not evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("specification call `restricted`")),
        "{diagnostics:?}"
    );
}

#[test]
fn omitted_constructor_fields_do_not_prove_equality_to_nonzero_fields() {
    let source = r#"
        data Tree { case Empty; case Node(child: Tree, flag: bool); }
        machine restricted(left: Tree, right: Tree) -> Tree
        requires left == right;
        terminates;
        { left }
        machine caller()
        requires restricted(Tree::Node { child: Tree::Empty }, Tree::Node { child: Tree::Empty, flag: true })
            == restricted(Tree::Node { child: Tree::Empty }, Tree::Node { child: Tree::Empty, flag: true });
        {}
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("omitted flag is zero, not equal to an explicitly true flag");
    for (flag, accepted) in [("false", false), ("true", true)] {
        let complete = source.replace(
            "Tree::Node { child: Tree::Empty }",
            &format!("Tree::Node {{ child: Tree::Empty, flag: {flag} }}"),
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&complete));
        assert_eq!(result.is_ok(), accepted, "{flag}: {:?}", result.err());
    }
}

#[test]
fn omitted_constructor_runtime_field_establishes_its_zero_value() {
    let source = r#"
        data Tree { case Empty; case Node(child: Tree, flag: bool); }
        machine restricted(left: Tree, right: Tree) -> Tree
        requires left == right;
        terminates;
        { left }
        machine caller()
        requires restricted(Tree::Node { child: Tree::Empty }, Tree::Node { child: Tree::Empty, flag: false })
            == restricted(Tree::Node { child: Tree::Empty }, Tree::Node { child: Tree::Empty, flag: false });
        {}
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .expect("the omitted runtime bool field denotes false");
}

#[test]
fn constructor_body_guarantees_compare_omitted_runtime_fields() {
    for (flag, accepted) in [("true", false), ("false", true)] {
        let source = format!(
            r#"
            data Tree {{ case Empty; case Node(child: Tree, flag: bool); }}
            machine construct() -> Tree
            ensures result == (Tree::Node {{ child: Tree::Empty, flag: {flag} }});
            {{ transition {{ _ -> Tree::Node {{ child: Tree::Empty }} }} }}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{flag}: {:?}", result.err());
    }
}

#[test]
fn constructor_zero_fields_compose_in_common_nested_and_integer_positions() {
    for (declarations, omitted, supplied, changed) in [
        (
            "data Tree { flag: bool; case Empty; case Node(child: Tree); }",
            "Tree::Empty {}",
            "Tree::Empty { flag: false }",
            "Tree::Empty { flag: true }",
        ),
        (
            "data Tree { flag: bool; case Empty; case Node(child: Tree); }",
            "Tree::Empty",
            "Tree::Empty { flag: false }",
            "Tree::Empty { flag: true }",
        ),
        (
            "data Bits { flag: bool; } data Tree { case Empty; case Node(child: Tree, bits: Bits); }",
            "Tree::Node { child: Tree::Empty }",
            "Tree::Node { child: Tree::Empty, bits: Bits { flag: false } }",
            "Tree::Node { child: Tree::Empty, bits: Bits { flag: true } }",
        ),
        (
            "data Tree { case Empty; case Node(child: Tree, count: u64); }",
            "Tree::Node { child: Tree::Empty }",
            "Tree::Node { child: Tree::Empty, count: 0x0 }",
            "Tree::Node { child: Tree::Empty, count: 1 }",
        ),
    ] {
        for (explicit, accepted) in [(supplied, true), (changed, false)] {
            for (left, right) in [(omitted, explicit), (explicit, omitted)] {
                let source = format!(
                    r#"
                    {declarations}
                    machine restricted(left: Tree, right: Tree) -> Tree
                    requires left == right;
                    terminates;
                    {{ left }}
                    machine caller()
                    requires restricted({left}, {right}) == restricted({left}, {right});
                    {{}}
                "#
                );
                let result = crate::lower_typed_trees(parse_typed_trees(&source));
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "{left} == {right}: {:?}",
                    result.err()
                );
            }
            let source = format!(
                r#"
                {declarations}
                machine construct() -> Tree
                ensures result == ({explicit});
                {{ transition {{ _ -> {omitted} }} }}
            "#
            );
            let result = crate::lower_typed_trees(parse_typed_trees(&source));
            assert_eq!(
                result.is_ok(),
                accepted,
                "body {omitted} == {explicit}: {:?}",
                result.err()
            );
        }
    }
}

#[test]
fn zero_value_case_observation_keeps_common_fields_distinct_from_the_tag() {
    for (observation, accepted) in [("in Flags::First", true), ("in Flags::Second", false)] {
        let source = format!(
            r#"
        data Flags {{ ready: bool; case First; case Second; }}
        machine zero_case()
        ensures zero_value<Flags>() {observation};
        {{}}
    "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(
            result.is_ok(),
            accepted,
            "{observation}: {:?}",
            result.err()
        );
    }
    for (ready, accepted) in [("false", true), ("true", false)] {
        let source = format!(
            r#"
            data Flags {{ ready: bool; case First; case Second; }}
            data Tree {{ case Empty; case Node(child: Tree, flags: Flags); }}
            machine zero_value_fields()
            ensures (Tree::Node {{ child: Tree::Empty, flags: zero_value<Flags>() }})
                == (Tree::Node {{ child: Tree::Empty, flags: Flags::First {{ ready: {ready} }} }});
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{ready}: {:?}", result.err());
    }
}

#[test]
fn membership_guarantees_do_not_fix_the_returned_common_fields() {
    for (other, accepted) in [("true", true), ("false", false)] {
        let source = format!(
            r#"
            data Tree {{ flag: bool; case Empty; case Node(child: Tree); }}
            machine make(flag: bool) -> Tree
            ensures result in Tree::Empty;
            {{ transition {{ _ -> Tree::Empty {{ flag: flag }} }} }}
            machine compare()
            ensures make(true) == make({other});
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{other}: {:?}", result.err());
    }
}

#[test]
fn nested_first_case_defaults_must_establish_their_predicate() {
    // Positive predicate evidence is not yet transported into implicit zero
    // construction. A valid but unestablished predicate remains unknown, not
    // an excuse to accept the false predicate beside it.
    for (condition, accepted) in [
        ("", true),
        (" where count > 0", false),
        (" where count >= 0", false),
    ] {
        let source = format!(
            r#"
            data Choice {{ case First(count: u64){condition}; case Later; }}
            data Tree {{ case Empty; case Node(child: Tree, choice: Choice); }}
            machine restricted(left: Tree, right: Tree) -> Tree
            requires left == right;
            terminates;
            {{ left }}
            machine caller()
            requires restricted(Tree::Node {{ child: Tree::Empty }}, Tree::Node {{ child: Tree::Empty }})
                == restricted(Tree::Node {{ child: Tree::Empty }}, Tree::Node {{ child: Tree::Empty }});
            {{}}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{condition}: {:?}", result.err());
    }
}

#[test]
fn case_membership_requires_does_not_establish_zero_common_fields() {
    let source = r#"
        data Tree { flag: bool; case Empty; case Node(child: Tree); }
        machine false_default(value: Tree)
        requires value in Tree::Empty;
        ensures value == Tree::Empty;
        {}
    "#;
    crate::lower_typed_trees(parse_typed_trees(source))
        .map(|_| ())
        .expect_err("a tag hypothesis says nothing about common fields");
}

#[test]
fn case_matching_preserves_common_field_values_without_zeroing_them() {
    for (common_fields, accepted) in [("", false), ("flag: value.flag", true)] {
        let node_fields = if common_fields.is_empty() {
            "child: child".to_owned()
        } else {
            format!("child: child, {common_fields}")
        };
        let source = format!(
            r#"
            data Tree {{ flag: bool; case Empty; case Node(child: Tree); }}
            machine reconstruct(value: Tree) -> Tree
            ensures result == value;
            {{
                transition value {{
                    Tree::Empty -> Tree::Empty {{ {common_fields} }}
                    Tree::Node {{ child }} -> Tree::Node {{ {node_fields} }}
                }}
            }}
        "#
        );
        let result = crate::lower_typed_trees(parse_typed_trees(&source));
        assert_eq!(
            result.is_ok(),
            accepted,
            "{common_fields}: {:?}",
            result.err()
        );
    }
}

#[test]
fn omitted_constructor_fields_do_not_invent_proof_inhabitants_or_gated_values() {
    for field in ["child: Tree", "flag [erased]: bool", "count: u64 [1..=10]"] {
        let source = format!(
            r#"
            data Tree {{ case Empty; case Node({field}); }}
            machine restricted(left: Tree, right: Tree) -> Tree
            requires left == right;
            terminates;
            {{ left }}
            machine caller()
            requires restricted(Tree::Node {{}}, Tree::Node {{}}) == restricted(Tree::Node {{}}, Tree::Node {{}});
            {{}}
        "#
        );
        crate::lower_typed_trees(parse_typed_trees(&source))
            .map(|_| ())
            .expect_err("omission cannot manufacture an established value or erased proof");
    }
}

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
