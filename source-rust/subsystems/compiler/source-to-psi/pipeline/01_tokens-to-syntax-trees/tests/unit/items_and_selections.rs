use crate::frontend_fixture::Lexer;
use crate::parse_syntax_trees;
use language_core::BindingRelevance;
use source_text::Span;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::PropositionBody;
use syntax_trees::statement::StatementHandle;
use syntax_trees::statement::StatementNode;
use syntax_trees::statement::TransitionGuardNode;

#[test]
fn name_expression_source_custody_retains_complete_authored_paths() {
    for expression in ["VALUE", "settings::VALUE", "settings :: nested :: VALUE"] {
        let source =
            format!("machine read() -> u64 {{ let selected: u64 = {expression}; selected }}");
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let source_id = source_text::SourceId(17);
        let parsed = crate::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let statements = first_machine_statements(&parsed);
        let StatementNode::LocalData(local) = parsed.statements.statement(statements[0]) else {
            panic!("local initializer")
        };
        assert!(matches!(
            parsed.expressions.expression(local.initial_value),
            ExpressionNode::Name { .. }
        ));
        let start = source.find(expression).expect("authored name expression");
        assert_eq!(
            parsed.expressions.source_span(local.initial_value),
            source_text::SourceSpan::new(source_id, Span::new(start, start + expression.len()))
        );
    }
}

#[test]
fn name_expression_source_custody_survives_nested_constant_leaves() {
    let source = "const SELECTED: Point = Point { value: VALUE, nested: Pair { value: settings::VALUE } }; const ALIAS: u64 = settings::VALUE;";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let source_id = source_text::SourceId(23);
    let parsed = crate::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let names = parsed
        .expressions
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Name { .. })
                .then_some(parsed.expressions.source_span(handle))
        })
        .collect::<Vec<_>>();
    let expected = [
        "value: VALUE",
        "value: settings::VALUE",
        "= settings::VALUE",
    ]
    .map(|occurrence| {
        let name = occurrence.split_once(' ').expect("name after prefix").1;
        let start = source.find(occurrence).expect("authored constant leaf") + occurrence.len()
            - name.len();
        source_text::SourceSpan::new(source_id, Span::new(start, start + name.len()))
    });
    assert_eq!(names, expected);
}

#[test]
fn local_bindings_parse_their_type_and_initializer_once() {
    let tokens = Lexer::new("machine sample() { let value: Buffer<count(4)> = make(9); }")
        .tokenize()
        .unwrap();
    let parsed = parse_syntax_trees(&tokens).unwrap();
    assert_eq!(parsed.type_references.const_expression_nodes().len(), 1);
    for name in ["count", "make"] {
        assert_eq!(
            parsed
                .expressions
                .iter_expressions()
                .filter(|(_, expression)| {
                    matches!(expression, ExpressionNode::Call(call) if call.target.as_str(&parsed) == name)
                })
                .count(),
            1,
            "one authored call to {name} must produce one expression",
        );
    }
}

#[test]
fn member_calls_preserve_their_written_initializer_and_binding_properties() {
    for operation in [
        "fetch_add(operand(), NoOrdering)",
        "fetch_sub(operand(), NoOrdering)",
        "fetch_xor(operand(), NoOrdering)",
        "fetch_or(operand(), NoOrdering)",
        "fetch_and(operand(), NoOrdering)",
        "swap(operand(), NoOrdering)",
        "compare_exchange(0, operand(), NoOrdering, NoOrdering)",
    ] {
        for (binding, mutable, relevance) in [
            ("prior", false, BindingRelevance::Relevant),
            ("mut prior", true, BindingRelevance::Relevant),
            ("prior [erased]", false, BindingRelevance::Erased),
        ] {
            let source =
                format!("machine update(cell: u64) {{ let {binding}: u64 = cell.{operation}; }}");
            let parsed = crate::tests::parse_source_unwrap(&source);
            let statements = first_machine_statements(&parsed);
            let [local] = statements else {
                panic!("one authored binding: {source}")
            };
            let StatementNode::LocalData(local) = parsed.statements.statement(*local) else {
                panic!("result binding")
            };
            assert_eq!(local.is_mutable, mutable);
            assert_eq!(local.relevance, relevance);
            assert!(matches!(
                parsed.expressions.expression(local.initial_value),
                ExpressionNode::Call(_)
            ));
            assert_eq!(parsed.expressions.iter_expressions().filter(|(_, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target.as_str(&parsed) == "operand")
            }).count(), 1, "{source}");
        }
    }
}

#[test]
fn unmatched_atomic_shapes_publish_only_the_ordinary_binding() {
    for expression in [
        "make().fetch_add(1, NoOrdering)",
        "make().swap(1, NoOrdering)",
        "make().compare_exchange(0, 1, NoOrdering, NoOrdering)",
        "fetch_add(1, NoOrdering)",
        "cell.unrelated(1, NoOrdering)",
    ] {
        let source = format!("machine sample() {{ let value: u64 = {expression}; }}");
        let parsed = crate::tests::parse_source_unwrap(&source);
        assert_eq!(first_machine_statements(&parsed).len(), 1, "{source}");
        assert!(
            !parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| matches!(expression, ExpressionNode::Atomic(_))),
            "{source}"
        );
    }
}

#[test]
fn malformed_binding_reports_its_initializer_error() {
    let tokens = Lexer::new("machine sample() { let value: u64 = ; }")
        .tokenize()
        .unwrap();
    let error = parse_syntax_trees(&tokens).unwrap_err();
    assert!(error.message.contains("expression"), "{}", error.message);
}

#[test]
fn trait_defaults_keep_the_authored_member_call() {
    let tokens = Lexer::new(
        "trait Updates { machine update(cell: u64) { let prior: u64 = cell.swap(1, NoOrdering); } }",
    ).tokenize().unwrap();
    let parsed = parse_syntax_trees(&tokens).unwrap();
    let definition = crate::tests::root_trait(&parsed, "trait");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(definition.machines)[0]);
    let statements = parsed.items.statements(signature.default_body);
    assert_eq!(statements.len(), 1);
    assert!(matches!(
        parsed.statements.statement(statements[0]),
        StatementNode::LocalData(_)
    ));
}

fn first_machine_statements(parsed: &syntax_trees::SyntaxTrees) -> &[StatementHandle] {
    let machine = crate::tests::root_machine(parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    parsed.items.statements(state.statements)
}

#[test]
fn terminal_acknowledgements_preserve_named_and_parenthesized_call_forms() {
    for (target, named, suspend_marker, block_marker) in [
        ("block next(input; proof)", true, false, true),
        ("suspend block next(input; proof)", true, true, true),
        ("(block next(input; proof))", false, false, true),
        // `block(...)` is an ordinary call of the contextual name.
        ("block (next)(input; proof)", false, false, false),
        ("block pick().next(input; proof)", false, false, true),
    ] {
        let source = format!("machine run(input: u32) -> u32 {{ transition {{ _ -> {target} }} }}");
        let parsed = crate::tests::parse_source_unwrap(&source);
        let machine = crate::tests::root_machine_unwrap(&parsed);
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let transition =
            &crate::tests::dispatch(&parsed, parsed.items.statements(state.statements)[0]).arms[0];
        match parsed.statements.transition_target(transition.target) {
            syntax_trees::statement::TransitionTargetNode::Named {
                operational_acknowledgement,
                evidence_arguments,
                ..
            } if named => {
                assert_eq!(
                    operational_acknowledgement.acknowledges_block, block_marker,
                    "{target}"
                );
                assert_eq!(
                    operational_acknowledgement.acknowledges_suspend, suspend_marker,
                    "{target}"
                );
                assert_eq!(evidence_arguments[0].as_str(&parsed), "proof");
            }
            syntax_trees::statement::TransitionTargetNode::Value(expression) if !named => {
                let ExpressionNode::Call(call) = parsed.expressions.expression(*expression) else {
                    panic!("parenthesized call: {target}")
                };
                assert_eq!(
                    call.operational_acknowledgement.acknowledges_block, block_marker,
                    "{target}"
                );
                assert_eq!(
                    call.operational_acknowledgement.acknowledges_suspend, suspend_marker,
                    "{target}",
                );
                if !block_marker {
                    let ExpressionNode::Call(callee) = parsed.expressions.expression(call.callee)
                    else {
                        panic!("computed result of the ordinary block call: {target}")
                    };
                    assert_eq!(callee.target.as_str(&parsed), "block");
                    assert!(!callee.operational_acknowledgement.acknowledges_block);
                    assert!(!callee.operational_acknowledgement.acknowledges_suspend);
                }
                assert_eq!(call.evidence_arguments[0].as_str(&parsed), "proof");
            }
            _ => panic!("acknowledgement changed target form: {target}"),
        }
    }
}

#[test]
fn tail_targets_preserve_other_receiver_calls_without_reclassifying_state_coordinates() {
    for (target, named) in [
        ("next", true),
        ("Main::next", true),
        ("other.next", false),
        ("main.child.next", false),
        ("other.child.next", false),
    ] {
        let source = format!(
            "machine Main::next(main: @Main, input: u64) -> u64 {{ transition {{ _ -> {target}(input; proof) }} }}"
        );
        let parsed = crate::tests::parse_source_unwrap(&source);
        let machine = crate::tests::root_machine_unwrap(&parsed);
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let transition =
            &crate::tests::dispatch(&parsed, parsed.items.statements(state.statements)[0]).arms[0];
        let target_node = parsed.statements.transition_target(transition.target);
        if named {
            let syntax_trees::statement::TransitionTargetNode::Named {
                evidence_arguments, ..
            } = target_node
            else {
                panic!("state/namespace target changed: {target}");
            };
            assert_eq!(evidence_arguments[0].as_str(&parsed), "proof");
        } else {
            let syntax_trees::statement::TransitionTargetNode::Value(expression) = target_node
            else {
                panic!("receiver call became a state transfer: {target}");
            };
            let ExpressionNode::Call(call) = parsed.expressions.expression(*expression) else {
                panic!("receiver call lost: {target}");
            };
            assert!(call.receiver.is_valid());
            assert_eq!(call.target.as_str(&parsed), "next");
            assert_eq!(call.arguments.count(), 1);
            assert_eq!(call.evidence_arguments[0].as_str(&parsed), "proof");
            if target == "other.next" {
                let ExpressionNode::Name {
                    members: receiver,
                    application: None,
                } = parsed.expressions.expression(call.receiver)
                else {
                    panic!("direct receiver identity lost");
                };
                assert_eq!(
                    parsed
                        .expressions
                        .identifier_path_members(receiver.to_owned())[0]
                        .as_str(&parsed),
                    "other"
                );
            }
        }
    }
}

#[test]
fn opposite_boolean_subject_arms_retain_their_authored_patterns_and_shared_subject() {
    for first in [true, false] {
        let source = format!(
            "machine value() -> u8 {{ transition probe() {{ {first} -> 1u8, {} -> 2u8 }} }}",
            !first
        );
        let parsed = crate::tests::parse_source_unwrap(&source);
        let mut copied = syntax_trees::SyntaxTrees::new_with_generation(parsed.source_id, 90);
        copied.extend_from(&parsed);
        assert_eq!(copied.snapshot(), parsed.snapshot());
        let statements = first_machine_statements(&parsed);
        assert_eq!(statements.len(), 1);
        let dispatch = crate::tests::dispatch(&parsed, statements[0]);
        let &[subject] = parsed.statements.expression_handles(dispatch.subjects) else {
            panic!("one authored subject")
        };
        assert!(matches!(
            parsed.expressions.expression(subject),
            ExpressionNode::Call(_)
        ));
        assert_eq!(dispatch.arms.len(), 2);
        for (arm, expected) in dispatch.arms.iter().zip([first, !first]) {
            let [pattern] = arm.patterns.as_ref() else {
                panic!("each arm retains its Boolean pattern")
            };
            assert!(!pattern.capture.is_valid());
            let syntax_trees::expression::MatchPattern::Value(value) = pattern.pattern else {
                panic!("authored value pattern")
            };
            assert!(
                matches!(parsed.expressions.expression(value),ExpressionNode::Boolean(value) if *value == expected)
            );
        }
    }
}

#[test]
fn copying_a_dispatch_preserves_wildcard_positions_and_structural_bindings() {
    let source = "data Pair { v: u32; } machine inspect(first: Pair, second: bool) -> u32 {
        transition (first, second) { (Pair { v }, _) if v > 0u32 -> {}, (_, false) -> 7u32 }
    }";
    let parsed = crate::tests::parse_source_unwrap(source);
    let mut copied = syntax_trees::SyntaxTrees::new_with_generation(parsed.source_id, 90);
    copied.extend_from(&parsed);
    let find = |item: &syntax_trees::item::Item| item.as_machine().cloned();
    let machine = copied.root_items().find_map(find).unwrap();
    let state = copied
        .items
        .state(copied.items.state_handles(machine.states)[0]);
    let dispatch = crate::tests::dispatch(&copied, copied.items.statements(state.statements)[0]);
    let subjects = copied.statements.expression_handles(dispatch.subjects);
    let first = &dispatch.arms[0].patterns;
    let last = &dispatch.arms[1].patterns;
    use syntax_trees::expression::MatchPattern;
    assert!(matches!(first[0].pattern, MatchPattern::Destructure(_)));
    assert_eq!(
        copied.expressions.match_subject(first[0].capture).subject,
        subjects[0]
    );
    let syntax_trees::statement::TransitionGuardNode::When(guard) = dispatch.arms[0].guard else {
        panic!("tuple guard")
    };
    let ExpressionNode::Binary(comparison) = copied.expressions.expression(guard) else {
        panic!("comparison")
    };
    let ExpressionNode::Member(field) = copied.expressions.expression(comparison.left) else {
        panic!("bound field")
    };
    let ExpressionNode::SavedSubject(saved) = copied.expressions.expression(field.receiver) else {
        panic!("captured subject")
    };
    assert_eq!(saved.capture, first[0].capture);
    assert!(matches!(first[1].pattern, MatchPattern::Wildcard));
    assert!(matches!(last[0].pattern, MatchPattern::Wildcard));
    let MatchPattern::Value(value) = last[1].pattern else {
        panic!("second column tests false")
    };
    assert!(matches!(
        copied.expressions.expression(value),
        ExpressionNode::Boolean(false)
    ));
}

#[test]
fn unrelated_or_nonpair_boolean_guards_keep_their_tests() {
    for body in [
        "transition { probe() -> 1u8, !probe() -> 2u8 }",
        "transition probe() { true -> 1u8, true -> 2u8 }",
        "transition probe() { true -> 1u8, true -> 2u8, false -> 3u8 }",
        "transition probe() { true -> 1u8 } transition probe() { false -> 2u8 }",
    ] {
        let source = format!("machine value() -> u8 {{ {body} }}");
        let parsed = crate::tests::parse_source_unwrap(&source);
        let machine = crate::tests::root_machine_unwrap(&parsed);
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statements = parsed.items.statements(state.statements);
        assert_eq!(
            statements.len(),
            body.matches("transition").count(),
            "each authored block has its own boundary"
        );
        let mut arms = 0;
        for statement in statements {
            for arm in &crate::tests::dispatch(&parsed, *statement).arms {
                arms += 1;
                assert!(
                    matches!(arm.guard, TransitionGuardNode::When(_))
                        || arm.patterns.iter().any(|slot| !matches!(
                            slot.pattern,
                            syntax_trees::expression::MatchPattern::Wildcard
                        )),
                    "{body}"
                );
            }
        }
        assert!(arms >= 2, "{body}");
    }
}

#[test]
fn boolean_transition_targets_are_literals_without_parentheses() {
    for (spelling, expected) in [
        ("false", false),
        ("true", true),
        ("(false)", false),
        ("(true)", true),
    ] {
        let source = format!("machine value() -> bool {{ transition {{ _ -> {spelling} }} }}");
        let parsed = crate::tests::parse_source_unwrap(&source);
        let machine = crate::tests::root_machine_unwrap(&parsed);
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statement = parsed.items.statements(state.statements)[0];
        let transition = &crate::tests::dispatch(&parsed, statement).arms[0];
        let syntax_trees::statement::TransitionTargetNode::Value(expression) =
            parsed.statements.transition_target(transition.target)
        else {
            panic!("literal value target");
        };
        assert!(
            matches!(parsed.expressions.expression(*expression), ExpressionNode::Boolean(value) if *value == expected)
        );
    }
}

#[test]
fn source_target_declarations_are_retired() {
    let tokens = Lexer::new("target linux_x86_64 { }")
        .tokenize()
        .expect("tokenize retired target declaration");
    let error = parse_syntax_trees(&tokens).expect_err("target declarations must not parse");
    crate::tests::expect_error(&error, "`target` declarations are retired");
}

#[test]
fn atomic_spellings_are_ordinary_member_calls_through_parsing() {
    for member in [
        "load",
        "store",
        "fetch_add",
        "swap",
        "compare_exchange",
        "compare_exchange_once",
    ] {
        let input = format!(
            "data Registry {{ {member}: machine(key: u64, value: u64) -> u64 reaches none crashes none terminates; }} machine call(registry: Registry) -> u64 {{ (registry.{member})(1u64, 42u64) }}"
        );
        let trees = crate::tests::parse_source_unwrap(&input);
        assert!(trees.expressions.iter_expressions().any(|(_, node)| matches!(node, ExpressionNode::Call(call) if call.target.as_str(&trees) == member && call.arguments.count() == 2)));
    }
}

#[test]
fn old_remains_an_ordinary_parameter_and_local_identifier() {
    let tokens =
        Lexer::new("machine migrate(old: u64) -> u64 { let old_copy: u64 = old; old_copy }")
            .tokenize()
            .expect("tokenize ordinary old identifiers");
    let parsed = parse_syntax_trees(&tokens).expect("old must not be a globally reserved word");
    let machine = crate::tests::root_machine(&parsed, "migration machine");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("generated entry state"),
    );
    let [parameter] = parsed.items.state_parameters(state.parameters) else {
        panic!("one ordinary parameter")
    };
    assert_eq!(
        parsed
            .items
            .state_parameter(*parameter)
            .name
            .as_str(&parsed),
        "old"
    );
    assert!(
        parsed
            .items
            .statements(state.statements)
            .iter()
            .any(|statement| matches!(
                parsed.statements.statement(*statement),
                StatementNode::LocalData(local) if local.name.as_str(&parsed) == "old_copy"
            ))
    );
}

#[test]
fn entry_remains_an_ordinary_machine_name_with_a_generated_internal_entry() {
    let tokens = Lexer::new("machine entry() {}")
        .tokenize()
        .expect("tokenize ordinary entry declaration");
    let parsed = parse_syntax_trees(&tokens).expect("entry must remain a declaration name");
    let machine = crate::tests::root_machine(&parsed, "ordinary entry machine");
    assert_eq!(machine.name.as_str(&parsed), "entry");
    let [state_handle] = parsed.items.state_handles(machine.states) else {
        panic!("ordinary machine must retain one generated internal entry");
    };
    let state = parsed.items.state(*state_handle);
    assert_eq!(state.name.as_str(&parsed), "entry");
}

#[test]
fn trait_machine_parameter_is_requirement_identity() {
    let tokens = Lexer::new("trait PrivateCallbackSlot<machine Requirement> {}")
        .tokenize()
        .expect("tokenize trait machine requirement parameter");
    let parsed = parse_syntax_trees(&tokens).expect("parse trait machine requirement parameter");
    let trait_definition = crate::tests::root_trait(&parsed, "PrivateCallbackSlot trait");
    let [parameter] = parsed
        .items
        .type_parameters(trait_definition.type_parameters)
    else {
        panic!("one trait machine requirement parameter")
    };
    assert_eq!(parameter.name.as_str(&parsed), "Requirement");
    assert!(matches!(
        parameter.kind,
        syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::RequirementIdentity)
        }
    ));
}

#[test]
fn provider_selection_retains_two_structural_type_paths() {
    let source = r#"
        machine build(builder: @mut Build) {
            Build::select_provider::<host::Console, application::ConsoleProvider>(builder);
        }
    "#;
    let parsed = crate::tests::parse_source_verbose(
        source,
        "tokenize provider selection",
        "parse provider selection",
    );
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str(&parsed) == "select_provider" => {
                Some(call)
            }
            _ => None,
        })
        .expect("provider-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(
        crate::tests::argument_path(&parsed, &call.machine_arguments[0].path),
        ["host", "Console"]
    );
    assert_eq!(
        crate::tests::argument_path(&parsed, &call.machine_arguments[1].path),
        ["application", "ConsoleProvider"]
    );
    assert!(!call.target.as_str(&parsed).contains('#'));
}

#[test]
fn service_exclusion_retains_namespace_type_path_and_explicit_build_argument() {
    let source = r#"
        machine build(builder: @mut Build) {
            Build::exclude_service::<host::Console>(builder);
        }
    "#;
    let parsed = crate::tests::parse_source_verbose(
        source,
        "tokenize service exclusion",
        "parse service exclusion",
    );
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str(&parsed) == "exclude_service" => {
                Some(call)
            }
            _ => None,
        })
        .expect("service-exclusion call");

    assert_eq!(call.machine_arguments.len(), 1);
    assert_eq!(
        crate::tests::argument_path(&parsed, &call.machine_arguments[0].path),
        ["host", "Console"]
    );
    assert!(call.machine_arguments[0].application.is_none());
    assert!(call.target_is_static);
    assert_eq!(
        parsed.expressions.expression_handles(call.arguments).len(),
        1
    );
}

#[test]
fn service_exclusion_rejects_retired_receiver_calls() {
    for (source, expected) in [
        (
            "machine build(builder: @mut Build) { builder.exclude_service::<host::Console, other::Log>(); }",
            "receiver-style service exclusion is retired",
        ),
        (
            "machine build(builder: @mut Build) { builder.exclude_service::<host::Console>(1); }",
            "receiver-style service exclusion is retired",
        ),
        (
            "machine build(builder: @mut Build) { builder.exclude_service::<>(); }",
            "exclude_service",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize rejected service exclusion");
        let error = parse_syntax_trees(&tokens)
            .err()
            .unwrap_or_else(|| panic!("a malformed service exclusion must not parse: {source}"));
        let message = format!("{error:?}");
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn provider_selection_retains_one_explicit_composition_mode_argument() {
    let source = r#"
        machine build(builder: @mut Build) {
            Build::select_provider::<host::Console, application::ConsoleProvider>(
                builder, CompositionMode::Independent
            );
        }
    "#;
    let parsed = crate::tests::parse_source_verbose(
        source,
        "tokenize independent provider selection",
        "parse independent provider selection",
    );
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str(&parsed) == "select_provider" => {
                Some(call)
            }
            _ => None,
        })
        .expect("provider-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    let [builder, mode] = parsed.expressions.expression_handles(call.arguments) else {
        panic!("explicit Build and retained composition-mode arguments")
    };
    assert!(call.target_is_static);
    assert!(matches!(
        parsed.expressions.expression(*builder),
        ExpressionNode::Name { .. }
    ));
    let ExpressionNode::Name {
        members: mode,
        application: None,
    } = parsed.expressions.expression(*mode)
    else {
        panic!("composition mode remains one ordinary name expression")
    };
    assert_eq!(
        parsed
            .expressions
            .identifier_path_members(mode.to_owned())
            .iter()
            .map(|member| member.as_str(&parsed))
            .collect::<Vec<_>>(),
        ["CompositionMode", "Independent"]
    );
}

#[test]
fn root_binding_statement_retains_operand_paths_and_the_authored_bind_span() {
    let source = r#"
        machine build(builder: @mut Build) {
            Build::bind(builder, linux_x86_64::ProgramEntry, Main::main);
        }
    "#;
    let parsed =
        crate::tests::parse_source_verbose(source, "tokenize root binding", "parse root binding");
    let machine = crate::tests::root_machine(&parsed, "build machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::RootBinding(binding) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("expected a dedicated root-binding statement");
    };
    assert_eq!(
        binding
            .slot
            .iter()
            .map(|member| member.as_str(&parsed))
            .collect::<Vec<_>>(),
        ["linux_x86_64", "ProgramEntry"]
    );
    assert_eq!(
        binding
            .implementation
            .iter()
            .map(|member| member.as_str(&parsed))
            .collect::<Vec<_>>(),
        ["Main", "main"]
    );
    let span = binding.source_span.span;
    assert!(span.start < span.end);
    assert_eq!(&source[span.start..span.end], "bind");
}

#[test]
fn root_binding_statement_rejects_malformed_operands() {
    for operands in [
        "",
        "Slot",
        "Slot,",
        "Slot, Entry, Extra",
        "12, Entry",
        "Slot::, Entry",
    ] {
        let source =
            format!("machine build(builder: @mut Build) {{ Build::bind(builder, {operands}); }}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize malformed root binding");
        assert!(
            parse_syntax_trees(&tokens).is_err(),
            "accepted operands: {operands}"
        );
    }
}

#[test]
fn root_binding_statement_retains_computed_operands_without_a_declaration_path() {
    for operand in ["setup::retain(entry)", "12"] {
        let source = format!(
            "machine build(builder: @mut Build) {{ Build::bind(builder, Slot, {operand}); }}"
        );
        let parsed = crate::tests::parse_source_verbose(
            &source,
            "root binding tokens",
            "description expression syntax",
        );
        let machine = crate::tests::root_machine(&parsed, "build machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let StatementNode::RootBinding(binding) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("expected root binding");
        };
        assert!(binding.implementation.is_empty());
        assert!(binding.implementation_operand.is_valid());
    }
}

#[test]
fn opaque_representation_selection_retains_data_and_conformance_paths() {
    let source = r#"
        machine build(builder: @mut Build) {
            Build::select_representation::<interrupt::Ack, platform::AckRepresentation>(builder);
        }
    "#;
    let parsed = crate::tests::parse_source_verbose(
        source,
        "tokenize opaque representation selection",
        "parse opaque representation selection",
    );
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call)
                if call.target.as_str(&parsed) == "select_representation" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("opaque representation-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(
        crate::tests::argument_path(&parsed, &call.machine_arguments[0].path),
        ["interrupt", "Ack"]
    );
    assert_eq!(
        crate::tests::argument_path(&parsed, &call.machine_arguments[1].path),
        ["platform", "AckRepresentation"]
    );
}

#[test]
fn rejects_pub_when_the_declaration_cannot_retain_visibility() {
    let tokens = Lexer::new("public measure Counter::Zero(value: i32) -> i32 { 0 }")
        .tokenize()
        .expect("tokenize declaration");
    let error = parse_syntax_trees(&tokens).expect_err("visibility loss must reject");
    crate::tests::expect_error(&error, "silently private API");
}

#[test]
fn owns_non_utf8_string_literal_bytes_in_syntax_tree() {
    let tokens = Lexer::new(
        r#"
        machine emit() {
            Console::write_line("\x80A");
        }
        "#,
    )
    .tokenize()
    .expect("tokenize raw-byte escape");
    let parsed = parse_syntax_trees(&tokens).expect("raw bytes are syntax payload");
    let snapshot = parsed.snapshot_json().expect("snapshot");
    assert!(snapshot.contains("\"bytes\":[128,65]"), "{snapshot}");
}

#[test]
fn parses_relevance_on_ordinary_numbered_fields() {
    let tokens = Lexer::new(
        r#"
        data Message {
            #0 value: u32;
            #1 proof [erased]: Evidence;
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("numbered relevance should parse");
    let schema = crate::tests::root_data(&parsed, "ordinary numbered data");
    let fields = parsed
        .items
        .data_members(schema.members)
        .iter()
        .filter_map(|member| match member {
            syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert!(!fields[0].relevance.is_erased());
    assert!(fields[1].relevance.is_erased());
}

#[test]
fn parses_primitive_witness_and_transparent_proposition_declarations() {
    let source = r#"
        public proposition related(left: i32, right: i32);

        proposition converges_together<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        ) evidence ConvergenceEvidence<Left, Right>;

        proposition reflexive(value: i32) = related(value, value);
    "#;
    let parsed = crate::tests::parse_source(source, "all settled proposition forms should parse");
    let propositions = crate::tests::root_propositions(&parsed);

    assert_eq!(propositions.len(), 3);
    assert!(propositions[0].is_public);
    assert!(!propositions[1].is_public);
    assert!(matches!(propositions[0].body, PropositionBody::Primitive));
    assert_eq!(
        parsed
            .items
            .state_parameters(propositions[0].parameters)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .items
            .type_parameters(propositions[1].type_parameters)
            .len(),
        2
    );
    assert!(matches!(
        propositions[1].body,
        PropositionBody::Witness { .. }
    ));
    assert!(matches!(
        propositions[2].body,
        PropositionBody::Transparent { proposition }
            if matches!(parsed.expressions.expression(proposition), ExpressionNode::Call(_))
    ));
    assert!(propositions[0].transparent_formula_source_span.is_none());
    assert!(propositions[1].transparent_formula_source_span.is_none());
    let formula_span = propositions[2]
        .transparent_formula_source_span
        .expect("transparent proposition formula span");
    assert_eq!(
        &source[formula_span.span.start..formula_span.span.end],
        "related(value, value)"
    );

    let snapshot = parsed
        .snapshot_json()
        .expect("proposition syntax should snapshot");
    assert!(snapshot.contains("\"kind\":\"proposition\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"kind\":\"witness\""));
    assert!(snapshot.contains("\"kind\":\"transparent\""));
}

#[test]
fn struct_literal_retains_its_complete_authored_source_span() {
    let source = "const ORIGIN: Point = Point { x: 1, y: 2 };";
    let parsed = crate::tests::parse_source_verbose(
        source,
        "tokenize structured const declaration",
        "parse structured const declaration",
    );
    let declaration = crate::tests::root_const(&parsed, "structured const declaration");
    let span = parsed.expressions.source_span(declaration.initializer).span;
    assert_eq!(&source[span.start..span.end], "Point { x: 1, y: 2 }");
}

#[test]
fn array_literal_retains_its_complete_authored_source_span() {
    for initializer in ["[]", "[1, 2]", "[[1], [2]]", "utf16\"hello\""] {
        let source = format!("const VALUES: [u16; 2] = {initializer};");
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let source_id = source_text::SourceId(23);
        let parsed = crate::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let declaration = crate::tests::root_const(&parsed, "array const declaration");
        assert!(matches!(
            parsed.expressions.expression(declaration.initializer),
            ExpressionNode::ArrayLiteral(_)
        ));
        let start = source.find(initializer).unwrap();
        assert_eq!(
            parsed.expressions.source_span(declaration.initializer),
            source_text::SourceSpan::new(source_id, Span::new(start, start + initializer.len())),
            "{initializer}"
        );
    }
}

#[test]
fn parses_type_and_integer_const_proposition_arguments() {
    let source = r#"
        proposition indexed<T, const N: i32>();
        proposition selected() = indexed::<i32, 7>();
    "#;
    let parsed = crate::tests::parse_source(source, "static proposition arguments should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str(&parsed) == "indexed" => Some(call),
            _ => None,
        })
        .expect("transparent proposition should contain the indexed application");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(call.machine_arguments[0].path[0].as_str(&parsed), "i32");
    assert_eq!(
        call.machine_arguments[1]
            .const_literal
            .as_ref()
            .map(|literal| literal.to_string()),
        Some("7".to_owned())
    );
    assert_eq!(call.display_name(&parsed), "indexed::<i32, 7>()");
    let snapshot = parsed.snapshot_json().expect("syntax should snapshot");
    assert!(snapshot.contains("\"machine_arguments\":[[{\"text\":\"i32\""));
    assert!(snapshot.contains(",\"7\"]"));
}

#[test]
fn proposition_declarations_reject_runtime_or_ambiguous_body_shapes() {
    for source in [
        "proposition bad(value: i32) -> bool;",
        "proposition bad(value: i32) { Evidence; OtherEvidence; }",
        "proposition bad(value: i32) { value }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect_err("invalid proposition body must reject");
    }
}

#[test]
fn proposition_declarations_reject_retired_brace_evidence_with_migration_guidance() {
    let tokens = Lexer::new("proposition old(value: i32) { Evidence; }")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("retired proposition evidence must reject");

    assert!(
        error
            .message
            .contains("`{ Evidence; }` proposition evidence is retired")
    );
    assert!(error.message.contains("`evidence Evidence;`"));
}

#[test]
fn parses_trait_proposition_parameter_with_authored_signature() {
    let source = r#"
        trait Reflexive<C, proposition Relation>
        requires proposition Relation(left: C, right: C);
        {
        }
    "#;
    let parsed = crate::tests::parse_source(source, "proposition parameter should parse");
    let trait_definitions = crate::tests::root_traits(&parsed);
    let [trait_definition] = trait_definitions.as_slice() else {
        panic!("one trait expected");
    };
    let parameters = parsed
        .items
        .type_parameters(trait_definition.type_parameters);
    let syntax_trees::item::TypeParameterKind::Proposition {
        contract: Some(contract),
    } = &parameters[1].kind
    else {
        panic!("Relation should retain a proposition signature");
    };
    assert_eq!(contract.name.as_str(&parsed), "Relation");
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 2);
    let snapshot = parsed.snapshot_json().expect("snapshot should succeed");
    assert!(snapshot.contains("\"kind\":\"proposition\""));
    assert!(snapshot.contains("\"proposition_contract\""));

    let retired = crate::tests::parse_tokenized_error(
        "trait Reflexive<C, proposition Relation>\nwhere proposition Relation(left: C, right: C);\n{}",
        "retired `where proposition` must reject",
    );
    crate::tests::expect_error(&retired, "write `requires proposition Relation(...);`");
}

#[test]
fn trait_proposition_parameter_requires_authored_signature() {
    let tokens = Lexer::new("trait Reflexive<C, proposition Relation> {}")
        .tokenize()
        .expect("tokenize should succeed");
    let diagnostic = parse_syntax_trees(&tokens)
        .expect_err("a proposition parameter without its signature must reject");
    assert!(
        diagnostic
            .message
            .contains("requires an authored declaration-site signature")
    );
}

#[test]
fn parses_stable_identities_through_the_full_u64_range() {
    let source = r#"
        data MaximumIdentity {
            #18446744073709551615 value: u8;
            retired #18446744073709551614;
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    parse_syntax_trees(&tokens).expect("u64 stable identities should parse");
}
