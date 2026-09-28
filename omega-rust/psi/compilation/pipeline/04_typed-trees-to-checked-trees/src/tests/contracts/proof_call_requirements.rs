use super::parse_typed_trees;

const RESTRICTED: &str = r#"
    data Nat { case Zero; case Succ(previous: Nat); }
    machine restricted(value: Nat) -> Nat
        requires value == Nat::Zero;
        terminates;
    { value }
"#;

const CASE_RESTRICTED: &str = r#"
    data Tree { marked: bool; case Empty; case Node(child: Tree); }
    machine selected(left: Tree, right: Tree) -> Tree
    requires right in Tree::Empty;
    terminates;
    { left }
"#;

#[test]
fn case_citation_keeps_result_identity_through_named_states() {
    for (arguments, accepted) in [("other, known", true), ("known, other", false)] {
        let source = format!(
            "{CASE_RESTRICTED}\n\
             machine empty(marked: bool) -> Tree ensures result in Tree::Empty; terminates; {{\n\
                 transition {{ _ -> Tree::Empty {{ marked: marked }} }} }}\n\
             machine node(child: Tree) -> Tree ensures result in Tree::Node; terminates; {{\n\
                 transition {{ _ -> Tree::Node {{ child: child }} }} }}\n\
             machine caller() -> Tree terminates; {{\n\
                 let known: Tree = empty(true);\n\
                 let other: Tree = node(known);\n\
                 transition {{ _ -> next({arguments}) }}\n\
                 state next(left: Tree, right: Tree) -> Tree {{ transition {{ _ -> (selected(left, right)) }} }} }}"
        );
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{arguments}: {:?}", result.err());
    }
}

#[test]
fn returned_value_cannot_rebind_result_inside_compound_postconditions() {
    for fact in [
        "(result in Tree::Empty) && (true == true)",
        "result == Tree::Empty",
    ] {
        for body in [
            "Tree::Empty",
            "transition { _ -> Tree::Empty }",
            "transition choice { Tree::Empty -> Tree::Empty Tree::Node { child } -> Tree::Empty }",
        ] {
            let source = format!(
                "{CASE_RESTRICTED}\n\
                machine bad(result: Tree, choice: Tree) -> Tree ensures {fact}; terminates; {{ {body} }}"
            );
            let diagnostics = crate::lower_typed_trees(
                parse_typed_trees(&source),
                &crate::CheckingRequest::settled(),
            )
            .map(|_| ())
            .expect_err("compound postconditions retain the input result binder");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("ensures")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn result_parameter_keeps_its_own_hypothesis_when_returned_value_differs() {
    for body in [
        "Tree::Node { child: result }",
        "transition choice { Tree::Empty -> Tree::Node { child: result } Tree::Node { child } -> Tree::Node { child: child } }",
    ] {
        let source = format!(
            "{CASE_RESTRICTED}\n\
        machine independent(result: Tree, choice: Tree) -> Tree\n\
        requires result in Tree::Empty; ensures (result in Tree::Empty) && (true == true);\n\
        terminates; {{ {body} }}"
        );
        crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        )
        .expect("the guarantee constrains the parameter, not the returned Node");
    }
}

#[test]
fn case_citation_requires_preexisting_evidence() {
    for (premise, accepted) in [("requires value in Tree::Empty;", true), ("", false)] {
        let source = format!(
            "{CASE_RESTRICTED}\n\
             machine forward(value: Tree) -> Tree\n\
             requires value in Tree::Empty; ensures result in Tree::Empty; terminates; {{ value }}\n\
             machine caller(value: Tree) -> Tree {premise} terminates; {{\n\
                 let known: Tree = forward(value);\n\
                 transition {{ _ -> (selected(known, known)) }} }}"
        );
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn case_citation_does_not_imply_zero_common_fields() {
    let source = r#"
        data Tree { marked: bool; case Empty; case Node(child: Tree); }
        machine empty() -> Tree ensures result in Tree::Empty; terminates;
        { transition { _ -> Tree::Empty { marked: true } } }
        machine zero_only(value: Tree) -> Tree requires value == Tree::Empty; terminates; { value }
        machine caller() -> Tree terminates; {
            let known: Tree = empty();
            transition { _ -> (zero_only(known)) }
        }
    "#;
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
    .map(|_| ())
    .expect_err("a case guarantee supplies no common-field equation");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}

#[test]
fn case_citation_cannot_borrow_an_unrelated_same_spelled_field() {
    let source = r#"
        data Tree { marked: bool; case Empty; case Node(child: Tree); }
        machine empty() -> Tree ensures result in Tree::Empty; terminates;
        { transition { _ -> Tree::Empty { marked: false } } }
        machine gate(value: Tree) -> Tree
        requires (value in Tree::Empty) && value.marked == true;
        terminates; { value }
        machine caller(value: Tree) -> Tree
        requires value.marked == true;
        terminates; {
            let known: Tree = empty();
            transition { _ -> (gate(known)) }
        }
    "#;
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
    .map(|_| ())
    .expect_err("the caller's field premise is not a fact about the completed result");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}

#[test]
fn case_premise_keeps_exact_state_binders() {
    for (premise, accepted) in [("", false), ("requires value in Tree::Empty;", true)] {
        let argument = if accepted { "Tree::Empty" } else { "other" };
        let source = format!(
            "{CASE_RESTRICTED} machine caller(value: Tree, other: Tree)\n\
             requires value in Tree::Empty;\n\
             {{ transition {{ _ -> next({argument}) }}\n\
               state next(value: Tree) {premise}\n\
               requires selected(value, value) == selected(value, value); {{}} }}"
        );
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
}

#[test]
fn case_premise_classifies_payloads_without_equating_them() {
    for (argument, accepted) in [
        ("(Tree::Node { child: Tree::Empty, marked: true })", true),
        ("(Tree::Empty { marked: true })", false),
    ] {
        let source = format!(
            "data Tree {{ marked: bool; case Empty; case Node(child: Tree); }}\n\
             machine node(value: Tree) -> Tree requires value in Tree::Node; terminates; {{ value }}\n\
             machine caller() ensures node({argument}) == node({argument}); {{}}"
        );
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{argument}: {:?}", result.err());
    }
}

#[test]
fn case_premise_environment_preserves_local_projection_origin() {
    for (initializer, accepted) in [("Holder { tree: value }", true), ("make(value)", false)] {
        for forwarding in [false, true] {
            let body = if forwarding {
                "transition { _ -> next(candidate) } state next(known: Tree) -> Tree { transition { _ -> (selected(known, known)) } }"
            } else {
                "transition { _ -> (selected(candidate, candidate)) }"
            };
            let source = format!(
                "{CASE_RESTRICTED}\n\
                 data Holder {{ tree: Tree; }}\n\
                 machine make(value: Tree) -> Holder terminates; {{ transition {{ _ -> Holder {{ tree: value }} }} }}\n\
                 machine caller(value: Tree) -> Tree requires value in Tree::Empty; terminates; {{\n\
                     let holder: Holder = {initializer};\n\
                     let candidate: Tree = holder.tree;\n\
                     {body} }}"
            );
            let result = crate::lower_typed_trees(
                parse_typed_trees(&source),
                &crate::CheckingRequest::settled(),
            );
            assert_eq!(
                result.is_ok(),
                accepted,
                "{initializer}, forwarding={forwarding}: {:?}",
                result.as_ref().err()
            );
            if let Err(diagnostics) = result {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("requires")),
                    "{diagnostics:?}"
                );
            }
        }
    }
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
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
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
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
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
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{premise}: {:?}", result.err());
    }
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
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
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
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
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
                let result = crate::lower_typed_trees(
                    parse_typed_trees(&source),
                    &crate::CheckingRequest::settled(),
                );
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
            let result = crate::lower_typed_trees(
                parse_typed_trees(&source),
                &crate::CheckingRequest::settled(),
            );
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
        let result = crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        );
        assert_eq!(result.is_ok(), accepted, "{condition}: {:?}", result.err());
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
        crate::lower_typed_trees(
            parse_typed_trees(&source),
            &crate::CheckingRequest::settled(),
        )
        .map(|_| ())
        .expect_err("omission cannot manufacture an established value or erased proof");
    }
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
    let diagnostics = crate::lower_typed_trees(
        parse_typed_trees(source),
        &crate::CheckingRequest::settled(),
    )
    .map(|_| ())
    .expect_err("the predecessor may be Zero despite structural descent");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires"))
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
        if let Some(target) = crate::semantic::calls::transition_call_target(
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
    let diagnostics = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .map(|_| ())
        .expect_err("guard requires is unestablished");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:?}"
    );
}
