use super::parse_syntax_trees;
use omega_source_files_to_tokens::Lexer;
use omega_syntax_trees::expression::ExpressionNode;
use omega_syntax_trees::statement::StatementNode;
use omega_syntax_trees::types::TypeReferenceNode;

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

#[test]
fn parses_compiler_intrinsic_external_binding_as_a_closed_binding_case() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via Binding::CompilerIntrinsic("Console::write_byte");
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("compiler intrinsic binding should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "console_write_byte" =>
            {
                Some(machine)
            }
            _ => None,
        })
        .expect("external leaf machine");
    let clause = parsed
        .items
        .satisfies_clauses(machine.satisfies)
        .first()
        .expect("satisfies clause");
    assert!(matches!(
        clause.via.as_ref(),
        Some(omega_syntax_trees::item::ExternalBinding::CompilerIntrinsic { name })
            if name == "Console::write_byte"
    ));
}

#[test]
fn rejects_stable_identities_above_u64_max() {
    let source = "data TooLarge { #18446744073709551616 value: u8; }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("identity above u64::MAX must fail");
    assert!(error.message.contains("nonnegative u64"));
}

#[test]
fn parses_independently_numbered_cases_and_structured_payloads() {
    let source = r#"
        data Lookup<T> {
            case #1 Found(#1 value: T, retired #2);
            case #2 Missing;
            retired #3;
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    parse_syntax_trees(&tokens).expect("numbered cases and payloads should parse");
}

#[test]
fn parses_stable_field_identities_on_generic_data() {
    let source = "data Envelope<T> { #1 value: T; retired #2; }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    parse_syntax_trees(&tokens).expect("stable identities are ordinary generic data metadata");
}

#[test]
fn parses_zero_value_of_nested_generic_type_without_spacing_closes() {
    let source = r#"
        data Optional<T> {
            case None;
            case Some(value: T);
        }

        machine zero_is_none<T>()
        ensures
            zero_value<Optional<T>>() == Optional::None
        {
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens)
        .expect("`>>` must close the nested generic type and intrinsic type argument");
    assert!(
        parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| { matches!(expression, ExpressionNode::ZeroValue(_)) })
    );
}

#[test]
fn numbered_mixed_data_is_independent_of_member_order() {
    for source in [
        "data Mixed { #1 common: u8; case #1 First; }",
        "data Mixed { case #1 First; #1 common: u8; }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect("field and case identities have independent scopes");
    }
}

#[test]
fn enforces_all_or_nothing_numbering_per_identity_scope() {
    for source in [
        "data Bad { case #1 First; case Second; }",
        "data Bad { case #1 First; case #2 Second; retired #2; }",
        "data Bad { case #1 First(#1 value: u8, other: u8); }",
        "data Bad { case First; retired #2; }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect_err("inconsistent stable identities must fail");
    }
}

#[test]
fn preserves_erased_lifetime_parameters_separately_from_runtime_generics() {
    let source = r#"
        data View<'buf, T> {
            body: &'buf T;
        }

        machine borrow<'call>(value: &'call i32) -> &'call i32 {
            value
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");

    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data declaration");
    assert_eq!(data.lifetime_parameters.len(), 1);
    assert_eq!(data.lifetime_parameters[0].as_str(), "buf");
    assert_eq!(data.type_parameters.count(), 1);

    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine declaration");
    assert_eq!(machine.lifetime_parameters.len(), 1);
    assert_eq!(machine.lifetime_parameters[0].as_str(), "call");
    assert!(machine.type_parameters.is_empty());
}

#[test]
fn preserves_erased_lifetime_arguments_separately_from_runtime_generic_arguments() {
    let source = r#"
        data View<'buf, T> {
            body: &'buf T;
        }

        machine borrow<'call>(value: &'call i32) -> View<'call, i32> {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine declaration");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let TypeReferenceNode::Generic {
        lifetime_arguments,
        arguments,
        ..
    } = parsed.type_references.type_reference(state.return_type)
    else {
        panic!("return type should retain the generic application");
    };
    assert_eq!(
        lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["call"]
    );
    assert_eq!(
        parsed
            .type_references
            .type_reference_handles(*arguments)
            .len(),
        1
    );
}

#[test]
fn rejects_duplicate_names_across_lifetime_and_runtime_generic_parameters() {
    let source = "data View<'value, value> { body: &'value i32; }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let diagnostic = parse_syntax_trees(&tokens).expect_err("duplicate generic must reject");
    assert!(
        diagnostic
            .message
            .contains("duplicate generic parameter `value`")
    );
}

#[test]
fn rejects_lifetime_parameters_and_arguments_after_runtime_generics() {
    for source in [
        "data Bad<T, 'buf> { body: &'buf T; }",
        "data View<'buf, T> { body: &'buf T; } machine bad<'call>(value: &'call i32) -> View<i32, 'call> {}",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let diagnostic =
            parse_syntax_trees(&tokens).expect_err("late lifetime generic must reject");
        assert!(
            diagnostic.message.contains("lifetime")
                && diagnostic
                    .message
                    .contains("precede type, const, and machine"),
            "unexpected diagnostic: {}",
            diagnostic.message
        );
    }
}

#[test]
fn parses_dungeon_machine_surface() {
    let source = r#"
        machine Game::new() -> Game {
            let game: Game;
            transition {
                _ -> game
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    assert_eq!(parsed.root_item_count(), 1);
}

#[test]
fn parses_consecutive_bodyless_boundary_machines() {
    let source = r#"
        boundary data Carrier;

        boundary machine add(a: Carrier, b: Carrier) -> Carrier
        ensures add(a, b) == add(b, a);

        boundary machine multiply(a: Carrier, b: Carrier) -> Carrier
        ensures multiply(a, b) == multiply(b, a);
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens)
        .expect("a boundary-prefixed item must terminate the preceding accepted declaration");
    assert_eq!(parsed.root_item_count(), 3);
}

#[test]
fn parses_generic_standalone_conformance_arguments() {
    let source = r#"
        trait Converter<Source, Target> {
        }

        data Adapter {
        }

        Adapter satisfies Converter<i32, bool>;
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("generic conformance should parse");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");
    let arguments = parsed
        .type_references
        .type_reference_handles(conformance.trait_arguments);
    assert_eq!(arguments.len(), 2);
    assert!(matches!(
        parsed.type_references.type_reference(arguments[0]),
        TypeReferenceNode::Named(name) if name.as_str() == "i32"
    ));
    assert!(matches!(
        parsed.type_references.type_reference(arguments[1]),
        TypeReferenceNode::Named(name) if name.as_str() == "bool"
    ));
}

#[test]
fn parses_dungeon_state_flow() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) -> i32 {
            transition {
                _ -> running()
            }

            state running(&mut self) {
                transition {
                    _ -> 0
                }
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    assert_eq!(parsed.root_item_count(), 2);
}

#[test]
fn parses_attached_main_state_name_as_main() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state_handle = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(state_handle);
    assert_eq!(state.name.as_str(), "main");
}

#[test]
fn retired_spawn_forms_name_the_task_runtime_migration() {
    for source in [
        "machine run() { spawn { Worker::run(); } }",
        "machine run() { let task: i32 = spawn { Worker::run() }; }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("spawn must be retired");
        let rendered = error.message;
        assert!(rendered.contains("spawn { ... }") && rendered.contains("Task<T>"));
    }
}

#[test]
fn retired_provides_declarations_name_the_external_leaf_migration() {
    for declaration in [
        "demo_target provides Flags { open_read -> Syscall(0) }",
        "host demo_target provides Flags { open_read -> Syscall(0) }",
    ] {
        let source =
            format!("boundary trait Flags {{ machine open_read() -> i32; }}\n{declaration}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("provides syntax must be retired");
        assert!(
            error
                .message
                .contains("`provides` declarations are retired")
                && error
                    .message
                    .contains("satisfies Trait::method via Binding::Case")
        );
    }
}

#[test]
fn erased_join_type_is_rejected_but_join_names_are_ordinary() {
    let retired = "machine run(task: Join<i32>) {}";
    let tokens = Lexer::new(retired)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("Join<T> must be retired");
    let rendered = error.message;
    assert!(rendered.contains("Join<T>") && rendered.contains("finish()"));

    let ordinary = r#"
        data Join { value: i32; }
        machine Join::join(&self) -> i32 {
            transition { _ -> self.value }
        }
    "#;
    let tokens = Lexer::new(ordinary)
        .tokenize()
        .expect("tokenize should succeed");
    parse_syntax_trees(&tokens).expect("Join/join are ordinary names after TR1");
}

#[test]
fn linear_property_is_first_class_on_data_and_type_parameters() {
    let source = r#"
        data Receipt [linear] {}
        data Envelope<T [linear]> [linear] { value: T; }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("linear properties should parse");
    let data: Vec<_> = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect();

    assert_eq!(data.len(), 2);
    assert_eq!(
        data[0].properties.multiplicity,
        omega_core::semantics::Multiplicity::Linear
    );
    assert_eq!(
        data[1].properties.multiplicity,
        omega_core::semantics::Multiplicity::Linear
    );
    let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
    assert_eq!(
        parameter.bounds.multiplicity,
        omega_core::semantics::Multiplicity::Linear
    );
}

#[test]
fn copy_and_linear_properties_are_mutually_exclusive() {
    for source in ["data Bad [copy, linear] {}", "data Bad [linear, copy] {}"] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens)
            .expect_err("copy and linear must not coexist on one declaration");
        assert!(error.message.contains("mutually exclusive"));
    }
}

#[test]
fn carry_property_parses_all_four_axes_on_data_and_bounds() {
    use omega_core::semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension,
    };
    let source = r#"
        data Lease [
            carry(suspension: forbidden, cpu: same, thread: any, address: stable,),
        ] {}
        data Envelope<T [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )]> { value: T; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("four-axis carry properties should parse");
    let data = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        data[0].properties.carry,
        Some(CarryPolicy {
            suspension: CarrySuspension::Forbidden,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        })
    );
    let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
    assert_eq!(parameter.bounds.carry, Some(CarryPolicy::PERMISSIVE));
}

#[test]
fn carry_property_requires_every_axis_and_retires_send() {
    let missing = "data Bad [carry(suspension: allowed, cpu: any, thread: any)] {}";
    let tokens = Lexer::new(missing).tokenize().expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("partial carry policy must reject");
    assert!(error.message.contains("missing address"));

    let retired = "data Bad [send] {}";
    let tokens = Lexer::new(retired).tokenize().expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("send must remain retired");
    assert!(error.message.contains("`[send]` is retired"));
}

#[test]
fn boundary_data_parses_as_an_opaque_carrier_without_a_shape() {
    let tokens = Lexer::new("boundary data ProviderToken [linear];")
        .tokenize()
        .expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("opaque boundary data should parse");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data item");

    assert_eq!(
        data.supply_mode,
        omega_core::semantics::DataSupplyMode::BoundaryOpaque
    );
    assert!(data.members.is_empty());
    assert_eq!(
        data.properties.multiplicity,
        omega_core::semantics::Multiplicity::Linear
    );
}

#[test]
fn parses_plain_and_boundary_traits() {
    let source = r#"
        trait Drawable {
            machine draw(&self, canvas: &mut Canvas);
        }

        boundary trait Console {
            machine write_line(text: String)
            reaches
                Console;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let traits: Vec<_> = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .collect();

    assert_eq!(traits.len(), 2);
    assert_eq!(traits[0].name.as_str(), "Drawable");
    assert!(!traits[0].is_boundary);
    assert_eq!(traits[0].machines.len(), 1);
    assert_eq!(traits[1].name.as_str(), "Console");
    assert!(traits[1].is_boundary);
    assert_eq!(traits[1].machines.len(), 1);
    let signature_handle = parsed.items.state_signatures(traits[1].machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Console");
}

#[test]
fn parses_independent_operational_clauses_on_machines_and_requirements() {
    let source = r#"
        machine run() reaches Console suspends; blocks; {
        }

        trait Worker {
            machine wait() reaches Clock suspends; blocks; ensures true;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("operational clauses should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine item");
    assert!(machine.suspends);
    assert!(machine.blocks);
    let service_reaches = parsed
        .items
        .identifier_path_members(machine.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Console");

    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    assert!(signature.suspends);
    assert!(signature.blocks);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Clock");
}

#[test]
fn rejects_duplicate_operational_clauses() {
    let source = "machine run() suspends; suspends; {}";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("duplicate clause must fail");
    assert!(
        error.message.contains("duplicate `suspends;`"),
        "got: {}",
        error.message
    );
}

#[test]
fn rejects_legacy_effects_reach_clause() {
    let source = "machine run() effects Console {}";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("legacy reach spelling must fail");
    assert!(
        error.message.contains("`effects` reach clause is retired"),
        "got: {}",
        error.message
    );
    assert!(error.message.contains("`reaches <Service> + ...`"));
}

#[test]
fn rejects_operational_members_in_service_reach_rows() {
    for (retired, replacement) in [
        ("Suspend", "suspends;"),
        ("Block", "blocks;"),
        ("thread_block", "blocks;"),
    ] {
        let source = format!("machine run() reaches Console, {retired} {{}}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("retired effect member must fail");
        assert!(error.message.contains("boundary-service identities only"));
        assert!(
            error.message.contains(replacement),
            "got: {}",
            error.message
        );
    }
}

#[test]
fn parses_machine_contract_clauses() {
    let source = r#"
        machine distinct_indices(i: usize, j: usize)
        requires
            i < j
        ensures
            i != j
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let contracts = parsed.items.capability_contracts(machine.contracts);

    assert_eq!(contracts.len(), 2);
    assert!(matches!(
        contracts[0].kind,
        omega_syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert!(matches!(
        contracts[1].kind,
        omega_syntax_trees::item::CapabilityContractKind::Ensures
    ));
    assert!(contracts[0].token_count > 0);
    assert!(contracts[1].token_count > 0);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
}

#[test]
fn parses_explicit_state_arrival_requires() {
    let source = r#"
        machine walk(value: i32) {
            transition value > 0 {
                true -> positive(value)
                false -> done()
            }

            state positive(value: i32)
            requires
                value > 0
            {
            }

            state done() {
            }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("state arrival requires should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let positive = parsed
        .items
        .state_handles(machine.states)
        .iter()
        .map(|handle| parsed.items.state(*handle))
        .find(|state| state.name.as_str() == "positive")
        .expect("positive state");
    let contracts = parsed.items.capability_contracts(positive.contracts);
    assert_eq!(contracts.len(), 1);
    assert!(matches!(
        contracts[0].kind,
        omega_syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
}

#[test]
fn rejects_exit_contract_clauses_on_explicit_states() {
    let source = r#"
        machine walk() {
            state done()
            ensures
                true
            {
            }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("states admit arrival requires rather than exit contracts");
    assert!(
        error
            .message
            .contains("state signatures admit only arrival `requires`")
    );
}

#[test]
fn parses_machine_termination_clauses() {
    let source = r#"
        machine walk(items: &[Item], remaining: usize)
        terminates by remaining -> Nat::Descending;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert!(machine.terminates);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.decreases)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.decrease_order)
            .len(),
        2
    );
}

#[test]
fn parses_machine_termination_tuple_subjects() {
    // The argumented ranking-view spelling: the arrow's left side is the
    // ranked-subject tuple, bound in order to the named view's parameters.
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by (index, limit) -> Nat::BoundedDistance;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert!(machine.terminates);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.decreases)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.decrease_order)
            .len(),
        2
    );
}

#[test]
fn parses_machine_termination_argumented_view() {
    // TPR3: an ARGUMENTED view names its bound as an argument
    // (`Nat::IncreasingTo(limit)`) -- the bound is part of the view; the
    // subject stays alone on the arrow's left.
    let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by index -> Nat::IncreasingTo(limit);
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert!(machine.terminates);
    // The by-form supplies only the witness -- never the public guarantee.
    assert!(!machine.terminates_guarantee);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.decreases)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.decrease_order)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.decrease_view_arguments)
            .len(),
        1
    );
}

#[test]
fn parses_trait_requirement_termination_guarantee() {
    // TPR4 (decision 23): a bodyless requirement authors the PUBLIC
    // guarantee with bare `terminates` -- previously the signature clause
    // parser's skip-any-token fallback ATE it silently.
    let source = r#"
        trait Worker {
            machine run(&mut self, n: u64) -> u64 terminates;
            machine peek(&self) -> u64;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait root item");

    let signatures: Vec<_> = parsed
        .items
        .state_signatures(trait_definition.machines)
        .iter()
        .map(|handle| parsed.items.state_signature(*handle))
        .collect();
    assert_eq!(signatures.len(), 2);
    assert!(
        signatures[0].terminates_guarantee,
        "run authored `terminates`"
    );
    assert!(!signatures[1].terminates_guarantee, "peek promised nothing");
}

#[test]
fn rejects_ranking_witness_on_trait_requirement() {
    // The witness belongs to implementations: a bodyless requirement has no
    // body to prove.
    let source = r#"
        trait Worker {
            machine run(&mut self, n: u64) -> u64 terminates by n;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("the witness must be rejected");
    assert!(
        error
            .message
            .contains("does not belong on a bodyless requirement"),
        "got: {}",
        error.message
    );
}

#[test]
fn parses_data_default_domain_where_clause() {
    // R2 rung 1 (ch12 "Dependent Data"): the where clause between the data
    // signature and the body -- bare field names, comma-separated facts,
    // trailing comma tolerated.
    let source = r#"
        data MemoryMap
        where
            count <= len,
            stride >= 40,
        {
            len: u32;
            stride: u32;
            count: u32;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");

    assert_eq!(parsed.items.proof_facts(data.where_facts).len(), 2);
    assert_eq!(parsed.items.data_members(data.members).len(), 3);
}

#[test]
fn parses_value_and_policy_domain_chain_as_one_constrained_type() {
    let source = r#"
        data Sample {
            value: f32 in Finite & Saturating;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let field = parsed
        .items
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            omega_syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("data field");
    let TypeReferenceNode::Constrained { constraints, .. } =
        parsed.type_references.type_reference(field.type_reference)
    else {
        panic!("domain chain should produce one constrained type");
    };
    let constraints = parsed.type_references.constraints(*constraints);
    assert!(matches!(
        constraints,
        [
            omega_syntax_trees::types::TypeConstraintNode::Domain(name),
            omega_syntax_trees::types::TypeConstraintNode::ArithmeticDomain(
                omega_core::arithmetic::ArithmeticDomain::Saturating
            )
        ] if name.as_str() == "Finite"
    ));
}

#[test]
fn parses_compiler_owned_carry_atoms_and_expands_portable() {
    let source = r#"
        data Sample {
            local: u64 in Carry::MovableAddress;
            portable: u64 in Carry::Portable;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let fields = parsed
        .items
        .data_members(data.members)
        .iter()
        .filter_map(|member| match member {
            omega_syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();

    let names = |field: &omega_syntax_trees::item::DataField| {
        let TypeReferenceNode::Constrained { constraints, .. } =
            parsed.type_references.type_reference(field.type_reference)
        else {
            panic!("carry permission should produce a constrained type");
        };
        parsed
            .type_references
            .constraints(*constraints)
            .iter()
            .map(|constraint| match constraint {
                omega_syntax_trees::types::TypeConstraintNode::Domain(name) => {
                    name.as_str().to_owned()
                }
                other => panic!("carry permission became {other:?}"),
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(names(fields[0]), ["Carry::MovableAddress"]);
    assert_eq!(
        names(fields[1]),
        omega_core::semantics::CarryPermission::ALL.map(|permission| permission.name().to_owned())
    );
}

#[test]
fn expands_carry_portable_contract_guarantee_to_four_atomic_facts() {
    let source = r#"
        machine grant(value: u64) -> u64
        ensures
            result in Carry::Portable;
        {
            transition { _ -> value }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let [contract] = parsed.items.capability_contracts(machine.contracts) else {
        panic!("one ensures contract");
    };
    let domains = parsed
        .items
        .proof_facts(contract.facts)
        .iter()
        .map(|fact| match fact {
            omega_syntax_trees::item::ProofFact::Membership(membership) => parsed
                .items
                .identifier_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
            other => panic!("portable atom became {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        domains,
        omega_core::semantics::CarryPermission::ALL.map(|permission| permission.name().to_owned())
    );
}

#[test]
fn rejects_bare_arrow_transition_in_explicit_state_body() {
    let source = r#"
        machine Main::main(&mut self) {
            transition { _ -> running() }

            state running(&mut self) {
                -> finished();
            }

            state finished(&mut self) {
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("parse should reject bare arrows in explicit state bodies");
    assert!(
        error
            .message
            .contains("explicit state bodies must use the `transition` keyword"),
        "unexpected parse error: {}",
        error.message
    );
}

#[test]
fn parses_slice_range_indexing_into_range_expression() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self) -> usize {
            let values: [usize; 4] = [1, 2, 3, 4];
            let view: &[usize] = values.as_slice();
            let tail: &[usize] = view[1..];
            tail.len
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should accept slice range surface");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state_handle = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(state_handle);
    let statement_handle = parsed
        .items
        .statements(state.statements)
        .get(2)
        .copied()
        .expect("tail local");
    let statement = parsed.statements.statement(statement_handle);
    let StatementNode::LocalData(local) = statement else {
        panic!("expected local data statement");
    };
    let ExpressionNode::Indexed(indexed) = parsed.expressions.expression(local.initial_value)
    else {
        panic!("expected indexed initializer");
    };
    let ExpressionNode::Range(range) = parsed.expressions.expression(indexed.index) else {
        panic!("expected range index expression");
    };
    assert_eq!(
        parsed.expressions.display_name(indexed.index),
        "1..",
        "unexpected range display"
    );
    assert!(range.start.is_valid(), "expected explicit range start");
    assert!(!range.end.is_valid(), "expected open-ended range");
}

#[test]
fn parses_structural_recast_targets_as_type_references() {
    let source = r#"
        machine inspect(bytes: [u8; 4]) {
            let fixed: &[u8; 4] = &bytes as &[u8; 4];
            let slice: &[u8] = &bytes as &[u8];
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("array and slice recast targets should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let locals = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();

    let ExpressionNode::Cast(fixed) = parsed.expressions.expression(locals[0].initial_value) else {
        panic!("fixed-array initializer should be a recast");
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length: omega_syntax_trees::types::FixedArrayLength::Literal(4),
    } = parsed.type_references.type_reference(fixed.target_type)
    else {
        panic!("fixed-array target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u8"
    ));

    let ExpressionNode::Cast(slice) = parsed.expressions.expression(locals[1].initial_value) else {
        panic!("slice initializer should be a recast");
    };
    let TypeReferenceNode::Slice { element_type } =
        parsed.type_references.type_reference(slice.target_type)
    else {
        panic!("slice target should retain its structural type");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*element_type),
        TypeReferenceNode::Named(name) if name.as_str() == "u8"
    ));
}

#[test]
fn parses_trait_machine_contract_clauses() {
    let source = r#"
        boundary trait Filesystem {
            machine open(path: String)
            requires
                path in String::NonEmpty
            ensures
                handle in FileHandle::Open
            reaches
                Filesystem;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
            _ => None,
        })
        .expect("trait root item");
    let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
    let signature = parsed.items.state_signature(signature_handle);
    let contracts = parsed.items.capability_contracts(signature.contracts);
    let service_reaches = parsed
        .items
        .identifier_path_members(signature.service_reaches);

    assert_eq!(contracts.len(), 2);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Filesystem");
}

#[test]
fn parses_executable_domain_membership_expression() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_data_destructure_transition_guard_as_subject_member_guard() {
    let source = r#"
        data Player {
            health: i32;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) {
            match self.player {
                Player::Alive -> done()
                Player { health, .. } if health > 5 -> done()
                _ -> done()
            }

            state done() {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    // The destructure arm ALSO mints an exhaustiveness-marker let
    // (`__arm_destructure#...`) ahead of the transition statements; index
    // among the TRANSITIONS only.
    let statement = parsed
        .items
        .statements(state.statements)
        .iter()
        .copied()
        .filter(|handle| {
            matches!(
                parsed.statements.statement(*handle),
                StatementNode::Transition(_)
            )
        })
        .nth(1)
        .expect("data-pattern transition");
    let StatementNode::Transition(transition) = parsed.statements.statement(statement) else {
        panic!("second arm should be a transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("data-pattern arm should lower to a guard expression");
    };
    let ExpressionNode::Binary(comparison) = parsed.expressions.expression(guard) else {
        panic!("data-pattern guard should be a comparison");
    };
    assert!(matches!(
        parsed.expressions.expression(comparison.left),
        ExpressionNode::Member(_)
    ));
}

#[test]
fn parses_record_field_value_pattern_as_subject_member_equality() {
    let source = r#"
        data Header [copy] { ok: i32; version: i32; }
        machine inspect(h: Header) -> i32 {
            transition h {
                Header { ok: 0, version } -> version
                _ -> 1
            }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("field-value pattern should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .map(|handle| parsed.items.state(handle))
        .expect("entry state");
    let transition = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            omega_syntax_trees::statement::StatementNode::Transition(transition) => {
                Some(transition)
            }
            _ => None,
        })
        .next()
        .expect("first transition arm");
    let omega_syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("field-value arm must have an equality guard");
    };
    let omega_syntax_trees::expression::ExpressionNode::Binary(equality) =
        parsed.expressions.expression(guard)
    else {
        panic!("field-value pattern should lower to binary equality");
    };
    assert_eq!(
        equality.operator,
        omega_syntax_trees::expression::BinaryOperator::Equal
    );
    assert!(matches!(
        parsed.expressions.expression(equality.left),
        omega_syntax_trees::expression::ExpressionNode::Member(_)
    ));
}

#[test]
fn parses_asm_jmp_block_as_transition_statement() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            asm {
                jmp other()
            }

            state other() {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("asm transition");
    assert!(matches!(
        parsed.statements.statement(statement),
        StatementNode::Transition(_)
    ));
}

/// The asm mnemonic desugar: each known-contract instruction lowers to a call
/// on its unnameable `asm#...` intrinsic (`in` through an assignment); unknown
/// mnemonics -- including `db` -- are rejected at parse time.
#[test]
fn parses_asm_mnemonics_as_intrinsic_calls() {
    let source = r#"
        data Main {
            port: u16;
        }

        machine Main::main(&mut self) {
            let mut status: u8 = 0;
            asm { hlt }
            asm { out self.port, status }
            asm { in status, self.port }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statements = parsed.items.statements(state.statements).to_vec();
    assert_eq!(statements.len(), 4, "let + three asm statements");

    let StatementNode::Call(hlt) = parsed.statements.statement(statements[1]).clone() else {
        panic!("asm {{ hlt }} should desugar to a call statement");
    };
    assert_eq!(hlt.target.as_str(), "asm#hlt");
    assert!(hlt.receiver.is_empty());
    assert_eq!(hlt.arguments.count(), 0);

    let StatementNode::Call(out) = parsed.statements.statement(statements[2]).clone() else {
        panic!("asm {{ out .. }} should desugar to a call statement");
    };
    assert_eq!(out.target.as_str(), "asm#port_out");
    assert_eq!(out.arguments.count(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[3]).clone() else {
        panic!("asm {{ in .. }} should desugar to an assignment");
    };
    let ExpressionNode::Call(port_in) = parsed.expressions.expression(read.value).clone() else {
        panic!("asm {{ in .. }} assignment value should be the intrinsic call");
    };
    assert_eq!(port_in.target.as_str(), "asm#port_in");
    assert_eq!(port_in.arguments.count(), 1);
    assert!(!port_in.receiver.is_valid());
}

#[test]
fn parses_multiple_known_asm_instructions_in_one_block() {
    let source = r#"
        data Main {
            port: u16;
        }

        machine Main::main(&mut self) {
            let mut status: u8 = 0;
            asm {
                out self.port, status;
                in status, self.port;
                hlt
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let statements = parsed
        .items
        .statements(parsed.items.state(entry).statements)
        .to_vec();

    assert_eq!(statements.len(), 4, "let + three asm instructions");
    assert!(matches!(
        parsed.statements.statement(statements[1]),
        StatementNode::Call(call) if call.target.as_str() == "asm#port_out"
    ));
    assert!(matches!(
        parsed.statements.statement(statements[2]),
        StatementNode::Assignment(_)
    ));
    assert!(matches!(
        parsed.statements.statement(statements[3]),
        StatementNode::Call(call) if call.target.as_str() == "asm#hlt"
    ));
}

#[test]
fn parses_x86_memory_fences_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self) {
            asm where clobbers none { lfence; sfence; mfence }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = parsed
        .items
        .statements(parsed.items.state(entry).statements);

    assert_eq!(statements.len(), 3);
    for (statement, target) in statements
        .iter()
        .zip(["asm#lfence", "asm#sfence", "asm#mfence"])
    {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("fence should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn parses_x86_interrupt_control_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self) reaches MachineControl {
            asm where clobbers none { cli; sti }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = parsed
        .items
        .statements(parsed.items.state(entry).statements);

    assert_eq!(statements.len(), 2);
    for (statement, target) in statements.iter().zip(["asm#cli", "asm#sti"]) {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("interrupt control should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn parses_x86_flags_as_explicit_value_operations() {
    let source = r#"
        data Main { saved: u64; }

        machine Main::main(&mut self) reaches MachineControl {
            asm where clobbers r10, r15 {
                pushfq self.saved;
                popfq self.saved
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = parsed.items.statements(state.statements);
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(snapshot) = parsed.statements.statement(statements[0]) else {
        panic!("pushfq should desugar to a destination assignment");
    };
    let ExpressionNode::Call(call) = parsed.expressions.expression(snapshot.value) else {
        panic!("pushfq assignment should contain the snapshot intrinsic");
    };
    assert_eq!(call.target.as_str(), "asm#pushfq");
    assert_eq!(call.arguments.count(), 0);

    let StatementNode::Call(restore) = parsed.statements.statement(statements[1]) else {
        panic!("popfq should desugar to a call statement");
    };
    assert_eq!(restore.target.as_str(), "asm#popfq");
    assert_eq!(restore.arguments.count(), 1);
}

#[test]
fn parses_x86_msr_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(&mut self) reaches MachineControl {
            asm where clobbers rax, rcx, rdx, r10, r11, r15 {
                rdmsr self.value, 3221225600;
                wrmsr 3221225600, self.value
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = parsed.items.statements(state.statements);
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[0]) else {
        panic!("rdmsr should desugar to a destination assignment");
    };
    let ExpressionNode::Call(read_call) = parsed.expressions.expression(read.value) else {
        panic!("rdmsr assignment should contain the read intrinsic");
    };
    assert_eq!(read_call.target.as_str(), "asm#rdmsr");
    assert_eq!(read_call.arguments.count(), 1);

    let StatementNode::Call(write) = parsed.statements.statement(statements[1]) else {
        panic!("wrmsr should desugar to a call statement");
    };
    assert_eq!(write.target.as_str(), "asm#wrmsr");
    assert_eq!(write.arguments.count(), 2);
}

#[test]
fn parses_x86_control_registers_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(&mut self) reaches MachineControl {
            asm where clobbers rax, r10, r11, r15 {
                read_cr0 self.value;
                write_cr0 self.value;
                read_cr2 self.value;
                read_cr3 self.value;
                write_cr3 self.value;
                read_cr4 self.value;
                write_cr4 self.value
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = parsed.items.statements(state.statements);
    assert_eq!(statements.len(), 7);

    for (statement_index, target) in [
        (0, "asm#read_cr0"),
        (2, "asm#read_cr2"),
        (3, "asm#read_cr3"),
        (5, "asm#read_cr4"),
    ] {
        let StatementNode::Assignment(read) =
            parsed.statements.statement(statements[statement_index])
        else {
            panic!("control-register read should desugar to assignment");
        };
        let ExpressionNode::Call(call) = parsed.expressions.expression(read.value) else {
            panic!("control-register assignment should contain read intrinsic");
        };
        assert_eq!(call.target.as_str(), target);
        assert_eq!(call.arguments.count(), 0);
    }

    for (statement_index, target) in [
        (1, "asm#write_cr0"),
        (4, "asm#write_cr3"),
        (6, "asm#write_cr4"),
    ] {
        let StatementNode::Call(write) = parsed.statements.statement(statements[statement_index])
        else {
            panic!("control-register write should desugar to call");
        };
        assert_eq!(write.target.as_str(), target);
        assert_eq!(write.arguments.count(), 1);
    }
}

#[test]
fn parses_multi_instruction_asm_in_states_and_trait_defaults() {
    let source = r#"
        trait Idle {
            machine idle(&mut self) {
                asm { hlt; hlt }
            }
        }

        data Main {}

        machine Main::main(&mut self) {
            transition { _ -> next() }

            state next() {
                asm { hlt; hlt }
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait root item");
    let default_signature = parsed
        .items
        .state_signatures(trait_definition.machines)
        .first()
        .map(|handle| parsed.items.state_signature(*handle))
        .expect("default trait signature");
    assert_eq!(
        parsed
            .items
            .statements(default_signature.default_body)
            .len(),
        2,
        "trait default should retain both asm instructions"
    );
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let explicit = parsed
        .items
        .state_handles(machine.states)
        .get(1)
        .copied()
        .expect("explicit state");
    assert_eq!(
        parsed
            .items
            .statements(parsed.items.state(explicit).statements)
            .len(),
        2,
        "explicit state should retain both asm instructions"
    );
}

#[test]
fn rejects_ambiguous_or_empty_multi_instruction_asm_blocks() {
    for (block, expected) in [
        (
            "asm { out self.port, self.value hlt }",
            "multiple asm instructions must be separated by `;`",
        ),
        (
            "asm { jmp done(); hlt }",
            "an asm control transfer must be the final instruction",
        ),
        (
            "asm {}",
            "an asm block must contain at least one known instruction",
        ),
        (
            "asm where requires true {}",
            "an asm block must contain at least one known instruction",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{
                port: u16;
                value: u8;
            }}

            machine Main::main(&mut self) {{
                {block}
                state done() {{}}
            }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("asm block should reject");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}`, got `{rendered}`"
        );
    }
}

#[test]
fn rejects_asm_availability_and_unmodeled_operation_classes() {
    for (instruction, expected) in [
        ("iretq", "deriver-only"),
        ("lidt self.value", "deriver-only"),
        ("ret", "creates a hidden control exit"),
        (
            "ldr x0, self.value",
            "no structured operand provenance/permission contract",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{ value: i32; }}
            machine Main::main(&mut self) {{ asm {{ {instruction} }} }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("asm instruction should reject");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}` for `{instruction}`, got `{rendered}`"
        );
    }
}

#[test]
fn parses_exact_asm_where_clobber_contracts() {
    for block in [
        "asm where clobbers none { hlt }",
        "asm where clobbers r11, rax, rdx, r10, r15 { out self.port, self.value }",
    ] {
        let source = format!(
            r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(&mut self) {{ {block} }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect("exact asm clobber contract should parse");
    }
}

#[test]
fn parses_asm_where_facts_at_entry_and_exit() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }
        machine Main::main(&mut self) {
            asm where
                requires self.ready
                clobbers rax, rdx, r10, r11, r15
                ensures self.ready
            { out self.port, self.value }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("asm facts should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = parsed.items.statements(state.statements);
    assert_eq!(statements.len(), 3);
    assert!(matches!(
        parsed.statements.statement(statements[0]),
        StatementNode::AssemblyFact(fact)
            if fact.kind == omega_syntax_trees::statement::AssemblyFactKind::Requires
    ));
    assert!(matches!(
        parsed.statements.statement(statements[1]),
        StatementNode::Call(call) if call.target.as_str() == "asm#port_out"
    ));
    assert!(matches!(
        parsed.statements.statement(statements[2]),
        StatementNode::AssemblyFact(fact)
            if fact.kind == omega_syntax_trees::statement::AssemblyFactKind::Ensures
    ));
}

#[test]
fn rejects_inexact_asm_where_clobber_contracts() {
    for (block, expected) in [
        (
            "asm where clobbers rax, rdx, r10 { out self.port, self.value }",
            "missing `r11`",
        ),
        ("asm where clobbers rax { hlt }", "not clobbered `rax`"),
        ("asm where clobbers { hlt }", "spell `clobbers none`"),
        (
            "asm where ensures true { hlt }",
            "requires a falling-through block",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(&mut self) {{ {block} }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("inexact asm contract should reject");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}` for `{block}`, got `{rendered}`"
        );
    }
}

/// Opaque asm forms have no attributable contract; only known-contract
/// instructions compile (privileged_effects_and_binary_trust, LOCKED point 2).
#[test]
fn rejects_unknown_asm_mnemonics() {
    for block in ["asm { db 0xF4 }", "asm { swapgs }"] {
        let source = format!(
            r#"
            data Main {{
                value: i32;
            }}

            machine Main::main(&mut self) {{
                {block}
            }}
            "#
        );

        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("unknown mnemonic must not parse");
        let message = format!("{error:?}");
        assert!(
            message.contains("only known-contract instructions compile"),
            "unexpected error for {block}: {message}"
        );
    }
}

#[test]
fn parses_executable_domain_membership_intersection_expression() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive & Player::Charged) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_executable_domain_membership_union_expression() {
    let source = r#"
        data Player {
            health: i32;
            mana: i32;
        }

        data Main {
            alive: Player;
        }

        machine Main::main(&mut self) {
            transition (self.alive in Player::Alive | Player::Charged) {
                (true) -> done()
                _ -> done()
            }

            state done(&mut self) {}
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let entry = parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state");
    let state = parsed.items.state(entry);
    let statement = parsed
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("entry transition");
    let omega_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let omega_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn parses_export_items_with_optional_alias() {
    let source = r#"
        export internal_regex::Match as Match;
        export Grep::search;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let exports = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Export(export) => Some(export),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(exports.len(), 2);
    let first_path = parsed.items.identifier_path_members(exports[0].path);
    assert_eq!(first_path.len(), 2);
    assert_eq!(first_path[0].as_str(), "internal_regex");
    assert_eq!(first_path[1].as_str(), "Match");
    assert_eq!(
        exports[0].alias.as_ref().map(|alias| alias.as_str()),
        Some("Match")
    );
    let second_path = parsed.items.identifier_path_members(exports[1].path);
    assert_eq!(second_path.len(), 2);
    assert_eq!(second_path[0].as_str(), "Grep");
    assert_eq!(second_path[1].as_str(), "search");
    assert!(exports[1].alias.is_none());
}

#[test]
fn parses_domain_definition_surface() {
    let source = r#"
        domain Player::Alive {
            self in Player::Valid;
            self.health > 0
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name.as_str(), "Player::Alive");
    assert!(domains[0].target_type.is_valid());
    assert_eq!(
        domains[0].predicate_body,
        omega_core::semantics::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[0].facts).len(), 2);
    assert!(domains[0].body_token_count > 3);

    let facts = parsed.items.proof_facts(domains[0].facts);
    assert!(matches!(
        facts[0],
        omega_syntax_trees::item::ProofFact::Membership(_)
    ));
    assert!(matches!(
        facts[1],
        omega_syntax_trees::item::ProofFact::Expression(_)
    ));
}

#[test]
fn parses_equivalent_bodyless_domain_spellings_distinct_from_true_predicate() {
    let source = r#"
        domain Reservation::Issued;
        domain Reservation::Recorded {}
        domain Reservation::Universal { true; }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            omega_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 3);
    for domain in &domains[..2] {
        assert_eq!(
            domain.predicate_body,
            omega_core::semantics::DomainPredicateBody::Bodyless
        );
        assert!(domain.facts.is_empty());
        assert_eq!(domain.body_token_count, 0);
    }
    assert_eq!(
        domains[2].predicate_body,
        omega_core::semantics::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[2].facts).len(), 1);
    assert!(domains[2].body_token_count > 0);
}

#[test]
fn parses_transparent_domain_alias_as_an_independent_record() {
    let source = r#"
        pub domain Socket::Usable =
            Socket::Connected & Socket::Authenticated;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domain = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("alias domain");

    let alias = domain.alias.as_ref().expect("transparent alias record");
    assert!(domain.is_public);
    let paths = alias
        .constituents
        .iter()
        .map(|path| {
            parsed
                .items
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths, ["Socket::Connected", "Socket::Authenticated"]);
    assert_eq!(
        domain.predicate_body,
        omega_core::semantics::DomainPredicateBody::Bodyless
    );
    assert!(domain.facts.is_empty());
    assert!(domain.operators.is_empty());
}

#[test]
fn parses_self_parameter_with_dedicated_self_type() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let parameter = parsed.items.state_parameter(
        parsed
            .items
            .state_parameters(state.parameters)
            .first()
            .copied()
            .expect("self parameter"),
    );

    assert!(parameter.is_self);
    let TypeReferenceNode::Reference {
        referee,
        is_mutable,
        ..
    } = parsed
        .type_references
        .type_reference(parameter.type_reference)
    else {
        panic!("&mut self should retain its reference ownership mode");
    };
    assert!(*is_mutable);
    assert!(matches!(
        parsed.type_references.type_reference(*referee),
        TypeReferenceNode::SelfType
    ));
}

#[test]
fn parses_self_expression_as_dedicated_node() {
    let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {
            self;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("expression statement"),
    );
    let StatementNode::Expression(expression) = statement else {
        panic!("expected expression statement");
    };

    assert!(matches!(
        parsed.expressions.expression(*expression),
        ExpressionNode::SelfValue
    ));
}

#[test]
fn parses_nested_call_arguments_as_contiguous_expression_spans() {
    let source = r#"
        data Player {
            xp: i32;
            level: i32;
        }

        data Main {
            xp_table: Player;
        }

        machine Main::main(&mut self, player: &mut Player) {
            player.xp = max(0, player.xp - self.xp_required(player.level));

            state xp_required(&mut self, level: i32) -> i32 {
                10
            }
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let statement = parsed.statements.statement(
        parsed
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("assignment statement"),
    );
    let StatementNode::Assignment(assignment) = statement else {
        panic!("expected assignment statement");
    };

    assert_eq!(
        parsed.expressions.display_name(assignment.value),
        "max(0, player.xp - self.xp_required(player.level))"
    );
}

#[test]
fn rejects_self_as_ordinary_declaration_name() {
    let source = r#"
        data self {}
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    assert!(parse_syntax_trees(&tokens).is_err());
}

#[test]
fn parses_machine_parameter_with_mandatory_contract() {
    let source = r#"
        data Card {}
        data Deck {}

        machine Deck::best<machine Key>(&self, card: &Card) -> u64
        where machine Key(value: &Card) -> u64
        reaches Console
        requires value in Card::Scorable
        {
            0
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("machine parameter should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name.as_str(), "Key");
    let omega_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(contract),
    } = &parameters[0].kind
    else {
        panic!("Key should carry its authored machine contract");
    };
    assert_eq!(contract.name.as_str(), "Key");
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
    assert!(contract.return_type.is_valid());
    assert_eq!(
        parsed
            .items
            .identifier_path_members(contract.service_reaches)
            .len(),
        1
    );
    assert_eq!(
        parsed.items.capability_contracts(contract.contracts).len(),
        1
    );
}

#[test]
fn parses_higher_order_machine_parameter_contract() {
    let source = r#"
        machine apply<machine Schema, machine Sample>(value: u64) -> u64
        where machine Schema<machine Inner>(value: u64) -> u64
        where machine Inner(value: u64) -> u64;
        where machine Sample(value: u64) -> u64;
        {
            Schema<Sample>(value)
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("higher-order contract should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 2);
    let omega_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(schema),
    } = &parameters[0].kind
    else {
        panic!("Schema should carry its authored machine contract");
    };
    let nested = parsed.items.type_parameters(schema.type_parameters);
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name.as_str(), "Inner");
    let omega_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(inner),
    } = &nested[0].kind
    else {
        panic!("Inner should carry its authored nested contract");
    };
    assert_eq!(parsed.items.state_parameters(inner.parameters).len(), 1);
}

#[test]
fn rejects_higher_order_parameter_without_nested_contract() {
    let source = r#"
        machine apply<machine Schema>()
        where machine Schema<machine Inner>()
        {
        }
        "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("missing nested contract must fail");
    assert!(
        error
            .message
            .contains("machine parameter `Inner` requires an authored declaration-site contract"),
        "got: {}",
        error.message
    );
}

#[test]
fn rejects_machine_parameter_without_authored_contract() {
    let source = r#"
        machine map<machine F>() -> u64 {
            0
        }
        "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("missing contract must fail");
    assert!(
        error
            .message
            .contains("requires an authored declaration-site contract"),
        "got: {}",
        error.message
    );
}

#[test]
fn parses_machine_parameter_on_proof_data_declaration() {
    let source = r#"
        data Stream<machine S>
        where machine S(index: u64) -> u64;
        {
            case More(tail: Stream<S>);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("proof-data machine parameter should parse");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("Stream declaration");
    let parameters = parsed.items.type_parameters(data.type_parameters);
    assert_eq!(parameters.len(), 1);
    let omega_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(contract),
    } = &parameters[0].kind
    else {
        panic!("S should retain its authored callable contract");
    };
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
    assert!(contract.return_type.is_valid());
}

#[test]
fn parses_proof_quotient_data_declaration() {
    let source = r#"
        data Carrier {
            case Value;
        }

        data Quotient = Carrier % Laws::equivalent;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("quotient declaration should parse");
    let quotient = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Quotient" => {
                data.quotient.as_ref()
            }
            _ => None,
        })
        .expect("Quotient metadata");
    assert!(matches!(
        parsed.type_references.type_reference(quotient.carrier),
        TypeReferenceNode::Named(name) if name.as_str() == "Carrier"
    ));
    assert_eq!(
        parsed
            .items
            .identifier_path_members(quotient.relation)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Laws", "equivalent"]
    );
}

#[test]
fn quotient_rejects_runtime_data_properties() {
    let source = "data Quotient [copy] = Carrier % equivalent;";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("quotient properties must reject");
    assert!(
        error
            .message
            .contains("quotient data declaration cannot declare runtime data properties"),
        "got: {}",
        error.message
    );
}

#[test]
fn parses_static_machine_symbol_call_argument() {
    let source = r#"
        data Card {}

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {
        }

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("static machine argument should parse");
    let (_, call) = parsed
        .expressions
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            omega_syntax_trees::expression::ExpressionNode::Call(call)
                if !call.machine_arguments.is_empty() =>
            {
                Some((handle, call))
            }
            _ => None,
        })
        .expect("generic call expression");
    assert_eq!(call.machine_arguments.len(), 1);
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Card", "power"]
    );
}

#[test]
fn destructure_marker_preserves_double_underscore_field_as_one_component() {
    let source = r#"
        data Pair { left__value: i32; right: i32; }
        machine inspect(pair: Pair) {
            let { left__value, right as _ } = pair;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("destructure should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
    let marker = parsed
        .items
        .statements(state.statements)
        .iter()
        .find_map(|handle| match parsed.statements.statement(*handle) {
            StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__destructure#") =>
            {
                Some(local)
            }
            _ => None,
        })
        .expect("destructure marker local");

    assert_eq!(
        marker.name.as_str(),
        "__destructure#left__value#right",
        "the internal delimiter must not split repeated underscores"
    );
}
