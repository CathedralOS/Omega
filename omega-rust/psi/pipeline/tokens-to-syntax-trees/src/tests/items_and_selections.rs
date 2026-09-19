use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementNode;

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
                    matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == name)
                })
                .count(),
            1,
            "one authored call to {name} must produce one expression",
        );
    }
}

#[test]
fn atomic_bindings_classify_one_initializer_and_preserve_binding_properties() {
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
            ("prior", false, language_core::BindingRelevance::Relevant),
            ("mut prior", true, language_core::BindingRelevance::Relevant),
            (
                "prior [erased]",
                false,
                language_core::BindingRelevance::Erased,
            ),
        ] {
            let source =
                format!("machine update(cell: u64) {{ let {binding}: u64 = cell.{operation}; }}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let parsed = parse_syntax_trees(&tokens).unwrap();
            let statements = first_machine_statements(&parsed);
            let [local, write] = statements else {
                panic!("binding and atomic write: {source}")
            };
            let StatementNode::LocalData(local) = parsed.statements.statement(*local) else {
                panic!("result binding")
            };
            assert_eq!(local.is_mutable, mutable);
            assert_eq!(local.relevance, relevance);
            assert!(
                matches!(parsed.expressions.expression(local.initial_value), ExpressionNode::Integer(value) if *value == numerics::literals::IntegerLiteral::zero())
            );
            let StatementNode::Assignment(write) = parsed.statements.statement(*write) else {
                panic!("atomic write")
            };
            assert!(matches!(
                parsed.expressions.expression(write.value),
                ExpressionNode::Atomic(_)
            ));
            assert_eq!(parsed.expressions.iter_expressions().filter(|(_, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "operand")
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
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
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
fn trait_defaults_share_atomic_binding_dispatch() {
    let tokens = Lexer::new(
        "trait Updates { machine update(cell: u64) { let prior: u64 = cell.swap(1, NoOrdering); } }",
    ).tokenize().unwrap();
    let parsed = parse_syntax_trees(&tokens).unwrap();
    let definition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(definition.machines)[0]);
    let statements = parsed.items.statements(signature.default_body);
    assert_eq!(statements.len(), 2);
    assert!(matches!(
        parsed.statements.statement(statements[0]),
        StatementNode::LocalData(_)
    ));
    assert!(matches!(
        parsed.statements.statement(statements[1]),
        StatementNode::Assignment(_)
    ));
}

fn first_machine_statements(
    parsed: &syntax_trees::SyntaxTrees,
) -> &[syntax_trees::statement::StatementHandle] {
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    parsed.items.statements(state.statements)
}

#[test]
fn tail_targets_preserve_other_receiver_calls_without_reclassifying_state_coordinates() {
    for (target, named) in [
        ("next", true),
        ("self.next", true),
        ("Main::next", true),
        ("other.next", false),
        ("self.child.next", false),
        ("other.child.next", false),
    ] {
        let source = format!(
            "machine Main::next(&self, input: u64) -> u64 {{ transition {{ _ -> {target}(input; proof) }} }}"
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .unwrap();
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let StatementNode::Transition(transition) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("expected transition: {target}");
        };
        let target_node = parsed.statements.transition_target(transition.target);
        if named {
            let syntax_trees::statement::TransitionTargetNode::Named {
                evidence_arguments, ..
            } = target_node
            else {
                panic!("state/namespace target changed: {target}");
            };
            assert_eq!(evidence_arguments[0].as_str(), "proof");
        } else {
            let syntax_trees::statement::TransitionTargetNode::Value(expression) = target_node
            else {
                panic!("receiver call became a state transfer: {target}");
            };
            let ExpressionNode::Call(call) = parsed.expressions.expression(*expression) else {
                panic!("receiver call lost: {target}");
            };
            assert!(call.receiver.is_valid());
            assert_eq!(call.target.as_str(), "next");
            assert_eq!(call.arguments.count(), 1);
            assert_eq!(call.evidence_arguments[0].as_str(), "proof");
            if target == "other.next" {
                let ExpressionNode::Name(receiver) = parsed.expressions.expression(call.receiver)
                else {
                    panic!("direct receiver identity lost");
                };
                assert_eq!(
                    parsed.expressions.identifier_path_members(*receiver)[0].as_str(),
                    "other"
                );
            }
        }
    }
}

#[test]
fn opposite_boolean_subject_arms_test_the_subject_once() {
    for first in [true, false] {
        let source = format!(
            "machine value() -> u8 {{
                 transition probe() {{ {first} -> 1u8 {} -> 2u8 }}
             }}",
            !first
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .unwrap();
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statements = parsed.items.statements(state.statements);
        assert_eq!(statements.len(), 2);
        let StatementNode::Transition(first_arm) = parsed.statements.statement(statements[0])
        else {
            panic!("first authored arm");
        };
        let syntax_trees::statement::TransitionGuardNode::When(expression) = first_arm.guard else {
            panic!("first arm tests the subject");
        };
        let ExpressionNode::Binary(comparison) = parsed.expressions.expression(expression) else {
            panic!("authored Boolean pattern comparison");
        };
        assert!(matches!(
            parsed.expressions.expression(comparison.left),
            ExpressionNode::Call(_)
        ));
        assert!(matches!(parsed.expressions.expression(comparison.right),
            ExpressionNode::Boolean(value) if *value == first));
        let StatementNode::Transition(last_arm) = parsed.statements.statement(statements[1]) else {
            panic!("second authored arm");
        };
        assert!(matches!(
            last_arm.guard,
            syntax_trees::statement::TransitionGuardNode::Always
        ));
    }
}

#[test]
fn unrelated_or_nonpair_boolean_guards_keep_their_tests() {
    for body in [
        "transition { probe() -> 1u8 !probe() -> 2u8 }",
        "transition probe() { true -> 1u8 true -> 2u8 }",
        "transition probe() { true -> 1u8 true -> 2u8 false -> 3u8 }",
        "transition probe() { true -> 1u8 } transition probe() { false -> 2u8 }",
    ] {
        let source = format!("machine value() -> u8 {{ {body} }}");
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .unwrap();
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statements = parsed.items.statements(state.statements);
        for statement in statements {
            let StatementNode::Transition(transition) = parsed.statements.statement(*statement)
            else {
                panic!("authored transition");
            };
            assert!(
                matches!(
                    transition.guard,
                    syntax_trees::statement::TransitionGuardNode::When(_)
                ),
                "{body}"
            );
        }
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
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .unwrap();
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statement = parsed.items.statements(state.statements)[0];
        let StatementNode::Transition(transition) = parsed.statements.statement(statement) else {
            panic!("transition");
        };
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
    assert!(
        error.message.contains("`target` declarations are retired"),
        "unexpected diagnostic: {}",
        error.message,
    );
}

#[test]
fn decisive_compare_exchange_desugar_derives_scalar_result_custody() {
    let tokens = Lexer::new(
        "machine update(cell: u64) { let prior: u64 = cell.compare_exchange(0, 1, NoOrdering, NoOrdering); }",
    )
    .tokenize()
    .expect("tokenize decisive compare-exchange");
    let parsed = parse_syntax_trees(&tokens).expect("parse decisive compare-exchange");
    let atomic = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Atomic(atomic) => Some(atomic),
            _ => None,
        })
        .expect("decisive compare-exchange desugars to an atomic expression");

    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::CompareExchange { .. }
    ));
    assert_eq!(
        atomic.result_custody,
        language_core::atomic::AtomicExpressionResultCustody::Scalar
    );
}

#[test]
fn single_attempt_compare_exchange_is_not_admitted_as_public_atomic_source() {
    let tokens = Lexer::new(
        "machine update(cell: u64) { let outcome: u64 = cell.compare_exchange_once(0, 1, NoOrdering, NoOrdering); }",
    )
    .tokenize()
    .expect("tokenize fenced single-attempt spelling");
    let parsed = parse_syntax_trees(&tokens).expect("parse fenced spelling as an ordinary call");

    assert!(
        parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(
                    expression,
                    ExpressionNode::Call(call) if call.target.as_str() == "compare_exchange_once"
                )
            })
    );
    assert!(
        !parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| {
                matches!(
                    expression,
                    ExpressionNode::Atomic(atomic)
                        if matches!(
                            atomic.ordering,
                            language_core::atomic::AtomicOrderingPlan::CompareExchangeOnce { .. }
                        )
                )
            })
    );
}

#[test]
fn old_remains_an_ordinary_parameter_and_local_identifier() {
    let tokens =
        Lexer::new("machine migrate(old: u64) -> u64 { let old_copy: u64 = old; old_copy }")
            .tokenize()
            .expect("tokenize ordinary old identifiers");
    let parsed = parse_syntax_trees(&tokens).expect("old must not be a globally reserved word");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("migration machine");
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
        parsed.items.state_parameter(*parameter).name.as_str(),
        "old"
    );
    assert!(
        parsed
            .items
            .statements(state.statements)
            .iter()
            .any(|statement| matches!(
                parsed.statements.statement(*statement),
                StatementNode::LocalData(local) if local.name.as_str() == "old_copy"
            ))
    );
}

#[test]
fn entry_remains_an_ordinary_machine_name_with_a_generated_internal_entry() {
    let tokens = Lexer::new("machine entry() {}")
        .tokenize()
        .expect("tokenize ordinary entry declaration");
    let parsed = parse_syntax_trees(&tokens).expect("entry must remain a declaration name");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("ordinary entry machine");
    assert_eq!(machine.name.as_str(), "entry");
    let [state_handle] = parsed.items.state_handles(machine.states) else {
        panic!("ordinary machine must retain one generated internal entry");
    };
    let state = parsed.items.state(*state_handle);
    assert_eq!(state.name.as_str(), "entry");
}

#[test]
fn trait_machine_parameter_is_requirement_identity() {
    let tokens = Lexer::new("trait PrivateCallbackSlot<machine Requirement> {}")
        .tokenize()
        .expect("tokenize trait machine requirement parameter");
    let parsed = parse_syntax_trees(&tokens).expect("parse trait machine requirement parameter");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("PrivateCallbackSlot trait");
    let [parameter] = parsed
        .items
        .type_parameters(trait_definition.type_parameters)
    else {
        panic!("one trait machine requirement parameter")
    };
    assert_eq!(parameter.name.as_str(), "Requirement");
    assert!(matches!(
        parameter.kind,
        syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::RequirementIdentity)
        }
    ));
}

#[test]
fn retains_public_data_visibility_in_syntax() {
    let tokens = Lexer::new("pub data PublicRecord { value: u32; } data PrivateRecord {}")
        .tokenize()
        .expect("tokenize data visibility");
    let parsed = parse_syntax_trees(&tokens).expect("parse data visibility");
    let definitions = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(definitions.len(), 2);
    assert!(definitions[0].is_public);
    assert!(!definitions[1].is_public);
    assert!(
        parsed
            .snapshot_json()
            .expect("snapshot")
            .contains("\"is_public\":true")
    );
}

#[test]
fn retains_public_machine_visibility_in_syntax() {
    let tokens = Lexer::new("pub machine Package::entry() { }")
        .tokenize()
        .expect("tokenize public machine");
    let parsed = parse_syntax_trees(&tokens).expect("parse public machine");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("public machine");

    assert!(machine.is_public);
    assert!(!machine.boundary);
    assert!(!machine.is_top_level_boundary_requirement);
    assert!(
        parsed
            .snapshot_json()
            .expect("snapshot")
            .contains("\"is_public\":true")
    );
}

#[test]
fn provider_selection_retains_two_structural_type_paths() {
    let source = r#"
        machine build(builder: &mut Build) {
            builder.select_provider<host::Console, application::ConsoleProvider>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize provider selection");
    let parsed = parse_syntax_trees(&tokens).expect("parse provider selection");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "select_provider" => Some(call),
            _ => None,
        })
        .expect("provider-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["host", "Console"]
    );
    assert_eq!(
        call.machine_arguments[1]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["application", "ConsoleProvider"]
    );
    assert!(!call.target.as_str().contains('#'));
}

#[test]
fn service_exclusion_retains_one_structural_type_path_and_no_value_arguments() {
    let source = r#"
        machine build(builder: &mut Build) {
            builder.exclude_service<host::Console>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize service exclusion");
    let parsed = parse_syntax_trees(&tokens).expect("parse service exclusion");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "exclude_service" => Some(call),
            _ => None,
        })
        .expect("service-exclusion call");

    assert_eq!(call.machine_arguments.len(), 1);
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["host", "Console"]
    );
    assert!(call.machine_arguments[0].application.is_none());
    assert!(
        parsed
            .expressions
            .expression_handles(call.arguments)
            .is_empty(),
        "the marker carries no value argument"
    );
}

#[test]
fn service_exclusion_rejects_extra_paths_and_value_arguments() {
    for (source, expected) in [
        (
            "machine build(builder: &mut Build) { builder.exclude_service<host::Console, other::Log>(); }",
            "exactly one plain type path",
        ),
        (
            "machine build(builder: &mut Build) { builder.exclude_service<host::Console>(1); }",
            "takes no value arguments",
        ),
        (
            "machine build(builder: &mut Build) { builder.exclude_service<>(); }",
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
        machine build(builder: &mut Build) {
            builder.select_provider<host::Console, application::ConsoleProvider>(
                CompositionMode::Independent
            );
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize independent provider selection");
    let parsed = parse_syntax_trees(&tokens).expect("parse independent provider selection");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "select_provider" => Some(call),
            _ => None,
        })
        .expect("provider-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    let [mode] = parsed.expressions.expression_handles(call.arguments) else {
        panic!("one retained composition-mode argument")
    };
    let ExpressionNode::Name(mode) = parsed.expressions.expression(*mode) else {
        panic!("composition mode remains one ordinary name expression")
    };
    assert_eq!(
        parsed
            .expressions
            .identifier_path_members(*mode)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["CompositionMode", "Independent"]
    );
}

#[test]
fn root_binding_statement_retains_operand_paths_and_the_authored_bind_span() {
    let source = r#"
        machine build(builder: &mut Build) {
            builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize root binding");
    let parsed = parse_syntax_trees(&tokens).expect("parse root binding");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("build machine");
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
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["linux_x86_64", "ProgramEntry"]
    );
    assert_eq!(
        binding
            .implementation
            .iter()
            .map(|member| member.as_str())
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
        "Slot, 12",
        "Slot::, Entry",
    ] {
        let source =
            format!("machine build(builder: &mut Build) {{ builder.roots.bind({operands}); }}");
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
fn opaque_representation_selection_retains_data_and_conformance_paths() {
    let source = r#"
        machine build(builder: &mut Build) {
            builder.select_representation<interrupt::Ack, platform::AckRepresentation>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize opaque representation selection");
    let parsed = parse_syntax_trees(&tokens).expect("parse opaque representation selection");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "select_representation" => {
                Some(call)
            }
            _ => None,
        })
        .expect("opaque representation-selection call");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["interrupt", "Ack"]
    );
    assert_eq!(
        call.machine_arguments[1]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["platform", "AckRepresentation"]
    );
}

#[test]
fn rejects_pub_when_the_declaration_cannot_retain_visibility() {
    let tokens = Lexer::new("pub measure Counter::Zero(value: i32) -> i32 { 0 }")
        .tokenize()
        .expect("tokenize declaration");
    let error = parse_syntax_trees(&tokens).expect_err("visibility loss must reject");
    assert!(
        error.message.contains("silently private API"),
        "{}",
        error.message
    );
}

#[test]
fn retains_public_name_first_conformance_visibility() {
    let tokens =
        Lexer::new("pub trait Ranked {} pub data Card {} pub PowerOrder: Card satisfies Ranked {}")
            .tokenize()
            .expect("tokenize public conformance");
    let parsed = parse_syntax_trees(&tokens).expect("parse public conformance");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("public name-first conformance");

    assert!(conformance.is_public);
    assert_eq!(
        conformance.alias.as_ref().map(|name| name.as_str()),
        Some("PowerOrder")
    );
    let snapshot = parsed.snapshot_json().expect("snapshot public conformance");
    assert!(snapshot.contains("\"kind\":\"conformance\""));
    assert!(snapshot.contains("\"is_public\":true"));
}

#[test]
fn retains_public_trait_and_numbered_data_visibility() {
    let tokens = Lexer::new("pub trait Shape {} pub data Envelope { #1 value: u32; }")
        .tokenize()
        .expect("tokenize public declarations");
    let parsed = parse_syntax_trees(&tokens).expect("parse public declarations");

    assert!(parsed.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Trait(definition) if definition.is_public
    )));
    assert!(parsed.root_items().any(|item| matches!(
        item,
        syntax_trees::item::Item::Data(definition) if definition.is_public
    )));
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
    let schema = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(schema) => Some(schema),
            _ => None,
        })
        .expect("ordinary numbered data");
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
        pub proposition related(left: i32, right: i32);

        proposition converges_together<machine Left, machine Right>(
            left: Stream<Left>,
            right: Stream<Right>
        ) evidence ConvergenceEvidence<Left, Right>;

        proposition reflexive(value: i32) = related(value, value);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("all settled proposition forms should parse");
    let propositions = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Proposition(proposition) => Some(proposition),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(propositions.len(), 3);
    assert!(propositions[0].is_public);
    assert!(!propositions[1].is_public);
    assert!(matches!(
        propositions[0].body,
        syntax_trees::item::PropositionBody::Primitive
    ));
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
        syntax_trees::item::PropositionBody::Witness { .. }
    ));
    assert!(matches!(
        propositions[2].body,
        syntax_trees::item::PropositionBody::Transparent { proposition }
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
fn parses_public_and_private_const_declarations() {
    let source = r#"
        pub const PUBLIC_LIMIT: u64 = 4;
        const Limits::PRIVATE_LIMIT: u64 = 2;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize const declarations");
    let parsed = parse_syntax_trees(&tokens).expect("parse const declarations");
    let declarations = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Const(declaration) => Some(declaration),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(declarations.len(), 2);
    assert!(declarations[0].is_public);
    assert!(!declarations[1].is_public);
    let snapshot = parsed.snapshot_json().expect("const syntax snapshot");
    assert!(snapshot.contains("\"kind\":\"const\""));
    assert!(snapshot.contains("\"is_public\":true"));
}

#[test]
fn struct_literal_retains_its_complete_authored_source_span() {
    let source = "const ORIGIN: Point = Point { x: 1, y: 2 };";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize structured const declaration");
    let parsed = parse_syntax_trees(&tokens).expect("parse structured const declaration");
    let declaration = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Const(declaration) => Some(declaration),
            _ => None,
        })
        .expect("structured const declaration");
    let span = parsed.expressions.source_span(declaration.value).span;
    assert_eq!(&source[span.start..span.end], "Point { x: 1, y: 2 }");
}

#[test]
fn parses_public_and_private_named_conformances() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub PublicCircle: Circle satisfies Shape;
        PrivateCircle: Circle satisfies Shape;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize named conformances");
    let parsed = parse_syntax_trees(&tokens).expect("parse named conformances");
    let conformances = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(conformances.len(), 2);
    assert!(conformances[0].is_public);
    assert!(!conformances[1].is_public);
    let snapshot = parsed.snapshot_json().expect("conformance syntax snapshot");
    assert!(snapshot.contains("\"kind\":\"conformance\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"is_public\":false"));
}

#[test]
fn parses_type_and_integer_const_proposition_arguments() {
    let source = r#"
        proposition indexed<T, const N: i32>();
        proposition selected() = indexed<i32, 7>();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("static proposition arguments should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target.as_str() == "indexed" => Some(call),
            _ => None,
        })
        .expect("transparent proposition should contain the indexed application");

    assert_eq!(call.machine_arguments.len(), 2);
    assert_eq!(call.machine_arguments[0].path[0].as_str(), "i32");
    assert_eq!(
        call.machine_arguments[1]
            .const_literal
            .as_ref()
            .map(|literal| literal.text()),
        Some("7")
    );
    assert_eq!(call.display_name(&parsed.expressions), "indexed<i32, 7>()");
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
        where proposition Relation(left: C, right: C);
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("proposition parameter should parse");
    let trait_definitions = parsed
        .root_items()
        .filter_map(|item| match item {
            syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .collect::<Vec<_>>();
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
    assert_eq!(contract.name.as_str(), "Relation");
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 2);
    let snapshot = parsed.snapshot_json().expect("snapshot should succeed");
    assert!(snapshot.contains("\"kind\":\"proposition\""));
    assert!(snapshot.contains("\"proposition_contract\""));
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
