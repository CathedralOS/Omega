use crate::parser::parse_syntax_trees;
use language_core::ReferenceAccess;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::types::TypeReferenceNode;

#[test]
fn parses_compiler_intrinsic_external_binding_as_a_closed_binding_case() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via ForeignBinding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("compiler intrinsic binding should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine)
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
        Some(syntax_trees::item::ExternalBinding::CompilerIntrinsic)
    ));
    let via_source_span = clause
        .via_keyword_source_span
        .expect("external binding retains exact `via` custody");
    let via_start = source.find("via").expect("authored `via`");
    assert_eq!(via_source_span.span.start, via_start);
    assert_eq!(via_source_span.span.end, via_start + "via".len());
}

#[test]
fn exact_requirement_application_parses_lifetimes_before_type_arguments() {
    let source = r#"
        trait Reads<'scope, Item> {
            machine read(value: &'scope Item) -> &'scope Item;
        }

        machine read<'view, Item>(value: &'view Item) -> &'view Item
            satisfies Reads<'view, Item>::read
        {
            value
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("parse exact requirement application");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "read" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("realizing machine");
    let [clause] = parsed.items.satisfies_clauses(machine.satisfies) else {
        panic!("one exact requirement application")
    };
    assert_eq!(
        clause
            .lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["view"],
    );
    assert_eq!(
        parsed
            .type_references
            .type_reference_handles(clause.arguments)
            .len(),
        1
    );

    let misplaced = Lexer::new(
        "machine read<'view, Item>(value: &'view Item) satisfies Reads<Item, 'view>::read {}",
    )
    .tokenize()
    .expect("tokenize misplaced lifetime");
    let error = parse_syntax_trees(&misplaced)
        .expect_err("a target-trait lifetime after a type argument must reject");
    assert!(
        error
            .message
            .contains("lifetime arguments precede type, const, and machine arguments")
    );
}

#[test]
fn parses_ordinary_via_machine_call_without_bootstrap_binding_reconstruction() {
    let source = r#"
        boundary trait Kernel32Requirements {
            machine write_file();
        }

        machine write_file_binding() -> i32 {
            0
        }

        machine kernel32_write_file()
        satisfies Kernel32Requirements::write_file
        via write_file_binding();
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("ordinary via call should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "kernel32_write_file" =>
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
    assert!(
        clause.via.is_none(),
        "ordinary calls are not bootstrap bindings"
    );
    assert!(matches!(
        parsed.expressions.expression(clause.via_expression),
        syntax_trees::expression::ExpressionNode::Call(_)
    ));
    let via_source_span = clause
        .via_keyword_source_span
        .expect("ordinary via expression retains exact keyword custody");
    let via_start = source.find("via").expect("authored `via`");
    assert_eq!(via_source_span.span.start, via_start);
    assert_eq!(via_source_span.span.end, via_start + "via".len());
}

#[test]
fn rejects_legacy_named_compiler_intrinsic_payload() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via ForeignBinding::CompilerIntrinsic("Console::write_byte");
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("compiler intrinsic identity is derived, never authored");
    assert!(
        error.message.contains("found punctuation `(`"),
        "{}",
        error.message
    );
}

#[test]
fn retired_vtable_slot_rejects_before_consuming_its_payload() {
    let source = r#"
        boundary trait Firmware {
            machine invoke(this: addr);
        }

        machine legacy(this: addr)
        satisfies Firmware::invoke
        via ForeignBinding::VtableSlot(not_an_integer_payload);
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize retired vtable slot");
    let error = parse_syntax_trees(&tokens).expect_err("authored numeric vtable slots are retired");
    assert_eq!(
        error.message,
        "`ForeignBinding::VtableSlot` is retired; declare the foreign table layout and use \
         `ForeignBinding::VtableField(field)`"
    );
}

#[test]
fn retired_string_backed_dllimport_names_the_evaluated_producer_migration() {
    let source = r#"
        trait Contract {
            machine call(value: i64) -> i64;
        }

        machine call_external(value: i64) -> i64
        satisfies Contract::call
        via ForeignBinding::DllImport("legacy.dll", "call");
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize retired string-backed import");
    let error = parse_syntax_trees(&tokens)
        .expect_err("authored string-backed import bootstrap is retired");
    assert_eq!(
        error.message,
        "`ForeignBinding::DllImport(\"module\", \"symbol\")` is retired; return the \
         compiler-owned `ForeignBinding::DllImport { import: DllImport::Case { .. } }` \
         value from one `via` producer machine instead"
    );
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
fn parses_field_relevance_on_record_and_case_payload_bindings() {
    let source = r#"
        data Certified {
            value: i32;
            proof [erased]: i32;
            case Wrapped(witness [erased]: i32);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("field relevance should parse");
    let data = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data declaration");
    let members = parsed.items.data_members(data.members);
    let syntax_trees::item::DataMember::Field(value) = &members[0] else {
        panic!("value field");
    };
    let syntax_trees::item::DataMember::Field(proof) = &members[1] else {
        panic!("proof field");
    };
    let syntax_trees::item::DataMember::Variant(wrapped) = &members[2] else {
        panic!("wrapped case");
    };
    let [witness] = parsed.items.data_payload_fields(wrapped.payload) else {
        panic!("one payload field");
    };

    assert_eq!(value.relevance, language_core::BindingRelevance::Relevant);
    assert_eq!(proof.relevance, language_core::BindingRelevance::Erased);
    assert_eq!(witness.relevance, language_core::BindingRelevance::Erased);
    let snapshot = parsed.snapshot_json().expect("syntax should snapshot");
    assert!(snapshot.contains("\"relevance\":\"relevant\""));
    assert!(snapshot.contains("\"relevance\":\"erased\""));
}

#[test]
fn rejects_unknown_or_duplicate_field_relevance_properties() {
    for (source, expected) in [
        (
            "data Bad { proof [copy]: i32; }",
            "unknown data-field binding property `copy`",
        ),
        (
            "data Bad { proof [erased, erased]: i32; }",
            "duplicate binding property `erased`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("invalid field property must reject");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn parses_binding_relevance_on_signature_parameters_and_locals() {
    // `[erased]` marks any authored binding occurrence, not only data
    // members: signature parameters and `let` locals retain the same
    // relevance marker (PROOF-RELEVANCE-MIGRATION).
    let source = r#"
        data Main { v: i32; }
        machine Main::main(&mut self, n: i32, bound [erased]: i32) {
            let x [erased]: i32 = 0;
            let mut y: i32 = 1;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("binding relevance should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine declaration");
    let [state] = parsed.items.state_handles(machine.states) else {
        panic!("one state");
    };
    let state = parsed.items.state(*state);
    let parameters = parsed
        .items
        .state_parameters(state.parameters)
        .iter()
        .map(|handle| parsed.items.state_parameter(*handle))
        .collect::<Vec<_>>();
    let [receiver, n, bound] = parameters.as_slice() else {
        panic!("three parameters");
    };
    assert!(receiver.is_self);
    assert_eq!(
        receiver.relevance,
        language_core::BindingRelevance::Relevant
    );
    assert_eq!(n.relevance, language_core::BindingRelevance::Relevant);
    assert_eq!(bound.relevance, language_core::BindingRelevance::Erased);

    let locals = parsed
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|handle| match parsed.statements.statement(*handle) {
            syntax_trees::statement::StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [x, y] = locals.as_slice() else {
        panic!("two locals");
    };
    assert_eq!(x.relevance, language_core::BindingRelevance::Erased);
    assert!(!x.is_mutable);
    assert_eq!(y.relevance, language_core::BindingRelevance::Relevant);
    assert!(y.is_mutable);

    for (source, expected) in [
        (
            "machine m(x [copy]: i32) {}",
            "unknown parameter binding property `copy`",
        ),
        (
            "machine m(x [erased, erased]: i32) {}",
            "duplicate binding property `erased`",
        ),
        (
            "data Main { v: i32; } machine Main::main(&mut self) { let x [bogus]: i32 = 0; }",
            "unknown local binding property `bogus`",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("invalid binding property must reject");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn empty_binding_brackets_stay_relevant_on_signature_parameters_and_locals() {
    // Consistent with data fields, `[]` is a legal no-op binding bracket.
    for source in [
        "machine m(x []: i32) {}",
        "data Main { v: i32; } machine Main::main(&mut self) { let x []: i32 = 0; }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect("empty binding brackets parse as relevant");
    }
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
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name == "zero_is_none" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("zero_is_none machine");
    assert_eq!(
        parsed.items.state_handles(machine.states).len(),
        1,
        "an empty checked body still has a zero-argument Unit entry"
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
            syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data declaration");
    assert_eq!(data.lifetime_parameters.len(), 1);
    assert_eq!(data.lifetime_parameters[0].as_str(), "buf");
    assert_eq!(data.type_parameters.count(), 1);

    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            syntax_trees::item::Item::Machine(machine) => Some(machine),
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

        ScalarConversion: Adapter satisfies Converter<i32, bool> {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("generic conformance should parse");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
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
    assert_eq!(
        conformance.alias.as_ref().map(|alias| alias.as_str()),
        Some("ScalarConversion")
    );
}

#[test]
fn parses_name_owned_generic_conformance_telescope() {
    let source = r#"
        trait Converter<'view, Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<'scope, Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("generic conformance should parse");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    assert_eq!(conformance.trait_lifetime_arguments.len(), 1);
    assert_eq!(conformance.trait_lifetime_arguments[0].as_str(), "scope");
    let parameters = parsed.items.type_parameters(conformance.type_parameters);
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].name.as_str(), "Source");
    assert!(matches!(
        parameters[0].kind,
        syntax_trees::item::TypeParameterKind::Type
    ));
    assert_eq!(parameters[1].name.as_str(), "Width");
    assert!(matches!(
        parameters[1].kind,
        syntax_trees::item::TypeParameterKind::Const { .. }
    ));
    let syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
    } = &parameters[2].kind
    else {
        panic!("Convert should retain its authored machine contract");
    };
    assert_eq!(contract.name.as_str(), "Convert");
    assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
}

#[test]
fn parses_named_concrete_subjectless_conformance_block() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("subjectless conformance should parse");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");
    assert!(matches!(
        conformance.subject,
        syntax_trees::item::ConformanceSubject::Subjectless
    ));
    assert_eq!(
        conformance.alias.as_ref().map(|alias| alias.as_str()),
        Some("ConcreteEvidence")
    );
    assert!(matches!(
        conformance.body,
        syntax_trees::item::ConformanceBody::Closed { .. }
    ));
}

#[test]
fn name_first_subjectless_conformance_requires_a_closed_body() {
    for source in [
        "satisfies Evidence { }",
        "ConcreteEvidence: satisfies Evidence;",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        assert!(
            parse_syntax_trees(&tokens).is_err(),
            "subjectless shorthand must not parse: {source}"
        );
    }
}

#[test]
fn retired_named_conformance_headers_direct_the_name_first_migration() {
    for (source, replacement) in [
        (
            "Item satisfies Shape as Primary {}",
            "Name: Subject satisfies Trait",
        ),
        (
            "satisfies Evidence as ConcreteEvidence {}",
            "`Name: satisfies Trait { ... }`",
        ),
        ("Item satisfies Shape {}", "Name: Subject satisfies Trait"),
        ("Item satisfies Shape;", "Name: Subject satisfies Trait"),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("retired header must reject");
        assert!(error.message.contains("is retired"));
        assert!(error.message.contains(replacement));
    }
}

#[test]
fn parses_closed_conformance_block_members() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
            machine Self::rank_value(&self) -> u32;
        }

        data Card { power: u32; }

        machine Card::stable_rank_value(&self) -> u32 { }

        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool { }

            Ranked::rank_value = Card::stable_rank_value;
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("closed conformance block should parse");
    let conformance = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");
    let syntax_trees::item::ConformanceBody::Closed { members } = conformance.body else {
        panic!("block must remain structurally distinct from bodyless attached-requirement lookup");
    };
    let [inline, reference] = parsed.items.conformance_members(members) else {
        panic!("two retained conformance members");
    };
    assert!(matches!(
        inline,
        syntax_trees::item::ConformanceMember::Machine(machine)
            if machine.name.as_str() == "before"
    ));
    let syntax_trees::item::ConformanceMember::Reference {
        declaring_trait,
        requirement,
        target,
    } = reference
    else {
        panic!("second row is an explicit machine reference");
    };
    assert_eq!(declaring_trait.as_str(), "Ranked");
    assert_eq!(requirement.as_str(), "rank_value");
    assert_eq!(
        parsed
            .items
            .identifier_path_members(*target)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Card", "stable_rank_value"]
    );
}

#[test]
fn closed_conformance_member_cannot_repeat_satisfaction() {
    let source = r#"
        trait Ranked { machine Self::before(&self, other: &Self) -> bool; }
        data Card { }
        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool
                satisfies Ranked::before
            { }
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let diagnostic = parse_syntax_trees(&tokens).expect_err("nested satisfaction must reject");
    assert!(
        diagnostic
            .message
            .contains("already belongs to its enclosing conformance"),
        "unexpected diagnostic: {}",
        diagnostic.message
    );
}

#[test]
fn retains_generic_and_named_conformance_bounds() {
    let source = r#"
        trait Converter<Message> { }
        data Card { }
        PowerOrder: Card satisfies Converter<i32>;

        machine inspect<T, Message>(value: &T)
        where
            T satisfies Converter<Message>,
            Message satisfies Card::PowerOrder
        { }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("conformance bounds should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "inspect" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("generic machine");

    let [ordinary, named] = machine.conformance_bounds.as_slice() else {
        panic!("two retained conformance bounds");
    };
    assert_eq!(ordinary.subject.as_str(), "T");
    assert_eq!(ordinary.carrier.as_str(), "Converter");
    assert!(ordinary.selected_conformance.is_none());
    assert_eq!(ordinary.arguments.len(), 1);
    assert_eq!(named.subject.as_str(), "Message");
    assert_eq!(named.carrier.as_str(), "Card");
    assert_eq!(
        named
            .selected_conformance
            .as_ref()
            .and_then(|selected| selected.path.last())
            .map(|name| name.as_str()),
        Some("PowerOrder")
    );
}

#[test]
fn retains_complete_selected_conformance_application_in_bound() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}

        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<'scope, Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine inspect<'view, Element>(value: &'view Element)
        where Element satisfies Card::FullEncoding<'view, Card, Message, 7, rank>
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("selected application should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) if machine.name.as_str() == "inspect" => {
                Some(machine)
            }
            _ => None,
        })
        .expect("inspect machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one selected conformance bound");
    };
    let selected = bound
        .selected_conformance
        .as_ref()
        .expect("selected conformance");
    assert_eq!(selected.path[0].as_str(), "FullEncoding");
    let application = selected.application.as_ref().expect("complete application");
    assert_eq!(application.lifetime_arguments[0].as_str(), "view");
    assert_eq!(application.arguments.len(), 4);
    assert_eq!(application.arguments[0].path[0].as_str(), "Card");
    assert_eq!(application.arguments[1].path[0].as_str(), "Message");
    assert_eq!(
        application.arguments[2]
            .const_literal
            .as_ref()
            .map(|literal| literal.text()),
        Some("7")
    );
    assert_eq!(application.arguments[3].path[0].as_str(), "rank");
}

#[test]
fn parses_explicit_conformance_binder_in_machine_telescope() {
    let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("explicit conformance binder should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");

    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 1, "Order is not a runtime type parameter");
    assert_eq!(parameters[0].name.as_str(), "Element");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };
    assert_eq!(
        bound.binder.as_ref().map(|name| name.as_str()),
        Some("Order")
    );
    assert_eq!(bound.subject.as_str(), "Element");
    assert_eq!(bound.carrier.as_str(), "Ranked");
    assert!(bound.selected_conformance.is_none());
}

#[test]
fn retains_named_dynamic_conformance_path() {
    let source = r#"
        machine inspect(value: &dyn Card::PowerOrder) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("named dynamic conformance should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            .expect("value parameter"),
    );
    let TypeReferenceNode::Reference { referee, .. } = parsed
        .type_references
        .type_reference(parameter.type_reference)
    else {
        panic!("parameter should be borrowed");
    };
    assert!(matches!(
        parsed.type_references.type_reference(*referee),
        TypeReferenceNode::DynamicTrait { name, conformance }
            if name.as_str() == "Card"
                && conformance.as_ref().is_some_and(|name| name.as_str() == "PowerOrder")
    ));
}

#[test]
fn distinguishes_shared_mutable_and_write_only_reference_access() {
    let source = r#"
        machine borrow(shared: &u8, mutable: &mut u8, output: &write u8) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("reference access modes should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let accesses = parsed
        .items
        .state_parameters(state.parameters)
        .iter()
        .map(|parameter| {
            let parameter = parsed.items.state_parameter(*parameter);
            let TypeReferenceNode::Reference { access, .. } = parsed
                .type_references
                .type_reference(parameter.type_reference)
            else {
                panic!("parameter should be borrowed");
            };
            *access
        })
        .collect::<Vec<_>>();

    assert_eq!(
        accesses,
        vec![
            ReferenceAccess::Shared,
            ReferenceAccess::Mutable,
            ReferenceAccess::WriteOnly,
        ]
    );
}

#[test]
fn retains_generic_trait_header_conformance_bounds() {
    let source = r#"
        trait CallingPolicy { }
        trait Calling<C>
        where C satisfies CallingPolicy
        { }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("trait header bound should parse");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            syntax_trees::item::Item::Trait(trait_definition)
                if trait_definition.name.as_str() == "Calling" =>
            {
                Some(trait_definition)
            }
            _ => None,
        })
        .expect("generic trait");
    let [bound] = trait_definition.conformance_bounds.as_slice() else {
        panic!("one retained trait bound");
    };
    assert_eq!(bound.subject.as_str(), "C");
    assert_eq!(bound.carrier.as_str(), "CallingPolicy");
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
            syntax_trees::item::Item::Machine(machine) => Some(machine),
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
fn retired_library_block_names_the_boundary_provider_migration() {
    let source = r#"
        library TestHost = "TestHost.dylib" calling_convention c {
            entry host_write(fd: i32, count: u64) -> i32
                symbol "_host_write"
                boundary host
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize retired library block");
    let error = parse_syntax_trees(&tokens).expect_err("library blocks must be retired");
    assert!(
        error.message.contains("legacy `library")
            && error.message.contains("is retired")
            && error
                .message
                .contains("producer machine returning `ForeignBinding::DllImport"),
        "got: {}",
        error.message
    );
}

#[test]
fn retired_abi_declaration_names_requirement_owned_calling_policy() {
    let tokens = Lexer::new(r#"abi "C""#)
        .tokenize()
        .expect("tokenize retired ABI declaration");
    let error = parse_syntax_trees(&tokens).expect_err("ABI declaration must reject");
    assert!(error.message.contains("`boundary machine ...`"));
    assert!(error.message.contains("satisfied requirement"));
    assert!(error.message.contains("wiki/spec/build/calling_plans.md"));
    assert!(!error.message.contains("inferred from the image"));
    assert!(!error.message.contains("boundary(<Plan>)"));
}
