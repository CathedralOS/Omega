/*
Unit tests for the Omega syntax parser across declarations, contracts,
expressions, inline assembly, patterns, and type constraints.
*/

use crate::diagnostics::parse_error::ParseError;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::State;
use syntax_trees::item::StateHandle;

pub(crate) mod bindings_and_conformances {
    use crate::parser::parse_syntax_trees;
    use language_core::BindingRelevance;
    use language_core::ReferenceAccess;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::Item;
    use syntax_trees::item::TypeParameterKind;
    use syntax_trees::types::TypeReferenceNode;

    #[test]
    fn parses_compiler_intrinsic_external_binding_as_a_closed_binding_case() {
        let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32) reaches;
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via ForeignBinding::CompilerIntrinsic;
    "#;
        let parsed = super::parse_source(source, "compiler intrinsic binding should parse");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "console_write_byte" => {
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
        let parsed =
            super::parse_source_verbose(source, "tokenize", "parse exact requirement application");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "read" => Some(machine),
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
                .map(|argument| argument.as_str(&parsed))
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
            machine write_file() reaches;
        }

        machine write_file_binding() -> i32 {
            0
        }

        machine kernel32_write_file()
        satisfies Kernel32Requirements::write_file
        via write_file_binding();
    "#;
        let parsed = super::parse_source(source, "ordinary via call should parse");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "kernel32_write_file" => {
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
            machine write_byte(byte: i32) reaches;
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via ForeignBinding::CompilerIntrinsic("Console::write_byte");
    "#;
        let error = super::parse_tokenized_error(
            source,
            "compiler intrinsic identity is derived, never authored",
        );
        super::expect_error(&error, "found punctuation `(`");
    }

    #[test]
    fn retired_vtable_slot_rejects_before_consuming_its_payload() {
        let source = r#"
        boundary trait Firmware {
            machine invoke(this: addr) reaches;
        }

        machine legacy(this: addr)
        satisfies Firmware::invoke
        via ForeignBinding::VtableSlot(not_an_integer_payload);
    "#;
        let error = super::parse_source_error(
            source,
            "tokenize retired vtable slot",
            "authored numeric vtable slots are retired",
        );
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
        let error = super::parse_source_error(
            source,
            "tokenize retired string-backed import",
            "authored string-backed import bootstrap is retired",
        );
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
        let error = super::parse_tokenized_error(source, "identity above u64::MAX must fail");
        assert!(error.message.contains("nonnegative u64"));
    }

    #[test]
    fn parses_independently_numbered_cases_and_structured_payloads() {
        let source = r#"
        data Lookup<T> {
            #1 case Found(#1 value: T, retired #2);
            #2 case Missing;
            retired #3;
        }
    "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect("numbered cases and payloads should parse");
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
        let parsed = super::parse_source(source, "field relevance should parse");
        let data = super::root_data(&parsed, "data declaration");
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

        assert_eq!(value.relevance, BindingRelevance::Relevant);
        assert_eq!(proof.relevance, BindingRelevance::Erased);
        assert_eq!(witness.relevance, BindingRelevance::Erased);
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
            let error = super::parse_tokenized_error(source, "invalid field property must reject");
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    #[test]
    fn parses_binding_relevance_on_signature_parameters_and_locals() {
        let source = r#"
        data Main { v: i32; }
        machine Main::main(main: &mut Main, n: i32, bound [erased]: i32) {
            let x [erased]: i32 = 0;
            let mut y: i32 = 1;
        }
    "#;
        let parsed = super::parse_source(source, "binding relevance should parse");
        let machine = super::root_machine(&parsed, "machine declaration");
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
        assert_eq!(receiver.relevance, BindingRelevance::Relevant);
        assert_eq!(n.relevance, BindingRelevance::Relevant);
        assert_eq!(bound.relevance, BindingRelevance::Erased);

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
        assert_eq!(x.relevance, BindingRelevance::Erased);
        assert!(!x.is_mutable);
        assert_eq!(y.relevance, BindingRelevance::Relevant);
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
                "data Main { v: i32; } machine Main::main(main: &mut Main) { let x [bogus]: i32 = 0; }",
                "unknown local binding property `bogus`",
            ),
        ] {
            let error =
                super::parse_tokenized_error(source, "invalid binding property must reject");
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    #[test]
    fn empty_binding_brackets_stay_relevant_on_signature_parameters_and_locals() {
        for source in [
            "machine m(x []: i32) {}",
            "data Main { v: i32; } machine Main::main(main: &mut Main) { let x []: i32 = 0; }",
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
                Item::Machine(machine) if machine.name.as_str(&parsed) == "zero_is_none" => {
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
            "data Mixed { #1 common: u8; #1 case First; }",
            "data Mixed { #1 case First; #1 common: u8; }",
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
            "data Bad { #1 case First; case Second; }",
            "data Bad { #1 case First; #2 case Second; retired #2; }",
            "data Bad { #1 case First(#1 value: u8, other: u8); }",
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

        let parsed = super::parse_source(source, "parse should succeed");

        let data = super::root_data(&parsed, "data declaration");
        assert_eq!(data.lifetime_parameters.len(), 1);
        assert_eq!(data.lifetime_parameters[0].as_str(&parsed), "buf");
        assert_eq!(data.type_parameters.count(), 1);

        let machine = super::root_machine(&parsed, "machine declaration");
        assert_eq!(machine.lifetime_parameters.len(), 1);
        assert_eq!(machine.lifetime_parameters[0].as_str(&parsed), "call");
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

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine declaration");
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
                .map(|argument| argument.as_str(&parsed))
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

        ScalarConversion satisfies Converter<i32, bool> {}
    "#;

        let parsed = super::parse_source(source, "generic conformance should parse");
        let conformance = super::root_conformance(&parsed, "conformance root item");
        let arguments = parsed
            .type_references
            .type_reference_handles(conformance.trait_arguments);
        assert_eq!(arguments.len(), 2);
        assert!(matches!(
            parsed.type_references.type_reference(arguments[0]),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "i32"
        ));
        assert!(matches!(
            parsed.type_references.type_reference(arguments[1]),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "bool"
        ));
        assert_eq!(
            conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str(&parsed)),
            Some("ScalarConversion")
        );
    }

    #[test]
    fn parses_name_owned_generic_conformance_telescope() {
        let source = r#"
        trait Converter<'view, Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>
            satisfies Converter<'scope, Source, u64>
        requires machine Convert(value: Source) -> u64;
        {}
    "#;

        let parsed = super::parse_source(source, "generic conformance should parse");
        let conformance = super::root_conformance(&parsed, "conformance root item");

        assert_eq!(conformance.lifetime_parameters.len(), 1);
        assert_eq!(conformance.lifetime_parameters[0].as_str(&parsed), "scope");
        assert_eq!(conformance.trait_lifetime_arguments.len(), 1);
        assert_eq!(
            conformance.trait_lifetime_arguments[0].as_str(&parsed),
            "scope"
        );
        let parameters = parsed.items.type_parameters(conformance.type_parameters);
        assert_eq!(parameters.len(), 3);
        assert_eq!(parameters[0].name.as_str(&parsed), "Source");
        assert!(matches!(parameters[0].kind, TypeParameterKind::Type));
        assert_eq!(parameters[1].name.as_str(&parsed), "Width");
        assert!(matches!(
            parameters[1].kind,
            TypeParameterKind::Const { .. }
        ));
        let TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
        } = &parameters[2].kind
        else {
            panic!("Convert should retain its authored machine contract");
        };
        assert_eq!(contract.name.as_str(&parsed), "Convert");
        assert_eq!(parsed.items.state_parameters(contract.parameters).len(), 1);
    }

    #[test]
    fn parses_named_implementation_block() {
        let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;

        let parsed = super::parse_source(source, "named implementation should parse");
        let conformance = super::root_conformance(&parsed, "conformance root item");
        assert_eq!(
            conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str(&parsed)),
            Some("ConcreteEvidence")
        );
        assert_eq!(
            parsed.items.conformance_members(conformance.members).len(),
            1
        );
    }

    #[test]
    fn named_implementation_requires_a_closed_body() {
        for source in [
            "satisfies Evidence { }",
            "ConcreteEvidence satisfies Evidence;",
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize should succeed");
            assert!(
                parse_syntax_trees(&tokens).is_err(),
                "bodyless implementation must not parse: {source}"
            );
        }
    }

    #[test]
    fn data_subject_satisfaction_is_rejected() {
        for source in [
            "trait Ranked<Item> { } data Card { } PowerOrder: Card satisfies Ranked<Card> { }",
            "trait Ranked<Item> { } data Card { } PowerOrder: Card satisfies Ranked<Card>;",
            "trait Ranked<Item> { } PowerOrder<Item>: Item satisfies Ranked<Item> { }",
        ] {
            let error = super::parse_tokenized_error(source, "data cannot satisfy a trait");
            super::expect_error(&error, "data does not satisfy traits");
        }
    }

    #[test]
    fn qualified_trait_requirement_names_are_rejected() {
        let error = super::parse_source_error(
            "trait Ranked<Item> { machine Self::before(value: &Item, other: &Item) -> bool; }",
            "tokenize should succeed",
            "a requirement has no subject qualifier",
        );
        super::expect_error(&error, "declares its bare name");
    }

    #[test]
    fn retired_conformance_alias_order_is_rejected() {
        for source in [
            "Item satisfies Shape as Primary {}",
            "satisfies Evidence as ConcreteEvidence {}",
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize should succeed");
            parse_syntax_trees(&tokens)
                .expect_err("the implementation name must precede satisfies");
        }
    }

    #[test]
    fn parses_closed_conformance_block_members() {
        let source = r#"
        trait Ranked<Item> {
            machine before(value: &Item, other: &Item) -> bool;
            machine rank_value(value: &Item) -> u32;
        }

        data Card { power: u32; }

        machine Card::stable_rank_value(value: &Card) -> u32 { }

        PowerOrder satisfies Ranked<Card> {
            machine before(value: &Card, other: &Card) -> bool { }

            machine rank_value(value: &Card) -> u32 { Card::stable_rank_value(value) }
        }
    "#;

        let parsed = super::parse_source(source, "closed conformance block should parse");
        let conformance = super::root_conformance(&parsed, "conformance root item");
        let [before, rank_value] = parsed.items.conformance_members(conformance.members) else {
            panic!("two retained conformance members");
        };
        for (member, expected) in [(before, "before"), (rank_value, "rank_value")] {
            assert!(
                matches!(
                    member,
                    syntax_trees::item::ConformanceMember::Machine(machine)
                        if machine.name.as_str(&parsed) == expected
                ),
                "member `{expected}` is an inline machine"
            );
        }
    }

    #[test]
    fn reference_rows_are_retired() {
        let source = r#"
        trait Ranked<Item> { machine rank_value(value: &Item) -> u32; }
        data Card { power: u32; }
        machine Card::stable_rank_value(value: &Card) -> u32 { }
        PowerOrder satisfies Ranked<Card> {
            Ranked::rank_value = Card::stable_rank_value;
        }
    "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let diagnostic = parse_syntax_trees(&tokens).expect_err("a reference row must reject");
        assert!(
            diagnostic
                .message
                .contains("reference row (`Trait::member = Existing::machine;`) is retired"),
            "unexpected diagnostic: {}",
            diagnostic.message
        );
    }

    #[test]
    fn closed_conformance_member_carries_one_exact_edge_for_its_own_requirement() {
        let source = r#"
        trait Ranked<Item> { machine before(value: &Item, other: &Item) -> bool; }
        data Card { }
        PowerOrder satisfies Ranked<Card> {
            machine before(value: &Card, other: &Card) -> bool
                satisfies Ranked<Card>::before
            { }
        }
    "#;

        let parsed = super::parse_source(source, "an exact edge on a member should parse");
        let conformance = super::root_conformance(&parsed, "conformance root item");
        let [member] = parsed.items.conformance_members(conformance.members) else {
            panic!("one retained conformance member");
        };
        let syntax_trees::item::ConformanceMember::Machine(machine) = member else {
            panic!("the member is an inline machine");
        };
        let [edge] = parsed.items.satisfies_clauses(machine.satisfies) else {
            panic!("the member retains its one exact edge");
        };
        assert_eq!(edge.trait_name.as_str(&parsed), "Ranked");
    }

    #[test]
    fn rejects_retired_machine_where_clause() {
        let source = r#"
        trait Converter<Message> { }

        machine inspect<T, Message>(value: &T)
        where
            T satisfies Converter<Message>
        { }
    "#;

        let error = super::parse_tokenized_error(source, "a machine `where` clause is retired");
        super::expect_error(&error, "found identifier `where`");
    }

    #[test]
    fn parses_evidence_binder_in_machine_telescope() {
        let source = r#"
        trait Ranked<Element> {}

        machine sort<Element, Order satisfies Ranked<Element>>(
            values: &mut [Element]
        ) {}
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize", "an evidence binder should parse");
        let machine = super::root_machine(&parsed, "machine");

        let parameters = parsed.items.type_parameters(machine.type_parameters);
        assert_eq!(parameters.len(), 1, "Order is not a runtime type parameter");
        assert_eq!(parameters[0].name.as_str(&parsed), "Element");
        let [bound] = machine.conformance_bounds.as_slice() else {
            panic!("one evidence binder");
        };
        assert_eq!(bound.binder.as_str(&parsed), "Order");
        assert_eq!(bound.trait_name.as_str(&parsed), "Ranked");
        assert_eq!(bound.arguments.len(), 1);
    }

    #[test]
    fn rejects_subject_spelled_evidence_binder() {
        let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
        let error =
            super::parse_source_error(source, "tokenize", "a conformance has no data subject");
        super::expect_error(&error, "a conformance has no data subject");
    }

    #[test]
    fn parses_dynamic_selector_trait_application() {
        let source = r#"
        machine inspect(order: dyn Ranked<Card>) {}
    "#;

        let parsed = super::parse_source(source, "dynamic selector should parse");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        let parameter = parsed.items.state_parameter(
            parsed
                .items
                .state_parameters(state.parameters)
                .first()
                .copied()
                .expect("value parameter"),
        );
        let TypeReferenceNode::DynamicTrait {
            trait_name,
            trait_arguments,
        } = parsed
            .type_references
            .type_reference(parameter.type_reference)
        else {
            panic!("parameter should be a dynamic selector");
        };
        assert_eq!(trait_name.as_str(&parsed), "Ranked");
        let arguments = parsed
            .type_references
            .type_reference_handles(*trait_arguments);
        assert_eq!(arguments.len(), 1);
        assert!(matches!(
            parsed.type_references.type_reference(arguments[0]),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "Card"
        ));
    }

    #[test]
    fn rejects_data_carrier_dynamic_selector() {
        let source = r#"
        machine inspect(value: &dyn Card::PowerOrder) {}
    "#;
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("a data-carrier selector is retired");
        super::expect_error(&error, "`dyn Data::Implementation` is retired");
    }

    #[test]
    fn distinguishes_shared_mutable_and_write_only_reference_access() {
        let source = r#"
        machine borrow(shared: &u8, mutable: &mut u8, output: &write u8) {}
    "#;

        let parsed = super::parse_source(source, "reference access modes should parse");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
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
    fn parses_trait_header_evidence_binder() {
        let source = r#"
        trait CallingPolicy { }
        trait Calling<Policy satisfies CallingPolicy>
        { }
    "#;

        let parsed = super::parse_source(source, "trait header binder should parse");
        let trait_definition = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Trait(trait_definition)
                    if trait_definition.name.as_str(&parsed) == "Calling" =>
                {
                    Some(trait_definition)
                }
                _ => None,
            })
            .expect("generic trait");
        let [bound] = trait_definition.conformance_bounds.as_slice() else {
            panic!("one evidence binder");
        };
        assert_eq!(bound.binder.as_str(&parsed), "Policy");
        assert_eq!(bound.trait_name.as_str(&parsed), "CallingPolicy");
    }

    #[test]
    fn parses_attached_main_state_name_as_main() {
        let source = r#"
        data Main {
        }

        machine Main::main(main: &mut Main) {}
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        assert_eq!(state.name.as_str(&parsed), "main");
    }

    #[test]
    fn retired_spawn_forms_name_the_task_runtime_migration() {
        for source in [
            "machine run() { spawn { Worker::run(); } }",
            "machine run() { let task: i32 = spawn { Worker::run() }; }",
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize should succeed",
                "spawn must be retired",
            );
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
            let source = format!(
                "boundary trait Flags {{ machine open_read() -> i32 reaches; }}\n{declaration}"
            );
            let error = super::parse_tokenized_error(&source, "provides syntax must be retired");
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
        let error = super::parse_source_error(
            source,
            "tokenize retired library block",
            "library blocks must be retired",
        );
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
}
pub(crate) mod body_sequences {
    use crate::parse_syntax_trees;
    use source_files_to_tokens::Lexer;
    use syntax_trees::item::Item;
    use syntax_trees::statement::StatementNode;
    use syntax_trees::statement::TransitionGuardNode;

    #[test]
    fn final_values_are_completion_edges_but_semicolon_expressions_are_not() {
        for body in [
            "callee()",
            "(callee())",
            "callee() + 1",
            "42 machine helper() {}",
        ] {
            let source = format!("machine run() -> u64 {{ {body} }}");
            let trees = super::parse_source(&source, "final value");
            let machine = super::root_machine(&trees, "machine");
            let state = super::entry_state(&trees, machine);
            let statements = trees.items.statements(state.statements);
            let StatementNode::Transition(completion) = trees.statements.statement(statements[0])
            else {
                panic!("final value must carry its completion edge");
            };
            assert!(matches!(completion.guard, TransitionGuardNode::Always));
            assert!(!completion.continuation.is_valid());
            assert!(completion.ends_dispatch);
            assert!(matches!(
                trees.statements.transition_target(completion.target),
                syntax_trees::statement::TransitionTargetNode::Value(_)
            ));
        }
        let trees = super::parse_source("machine run() { callee(); 42; }", "ordinary statements");
        let machine = super::root_machine(&trees, "machine");
        let state = super::entry_state(&trees, machine);
        let statements = trees.items.statements(state.statements);
        assert!(matches!(
            trees.statements.statement(statements[0]),
            StatementNode::Call(_)
        ));
        assert!(matches!(
            trees.statements.statement(statements[1]),
            StatementNode::Expression(_)
        ));
    }

    #[test]
    fn nested_machine_declarations_and_copies_retain_direct_child_cohorts() {
        let source = "machine outer() -> u64 { 42 machine first() { machine inner() {} } machine second() {} }";
        let trees = super::parse_source(source, "nested declarations after a value completion");
        let outer = super::root_machine(&trees, "outer machine");
        assert_eq!(trees.root_items().count(), 1);
        assert_eq!(outer.nested_machines.len(), 2);
        let children = outer
            .nested_machines
            .handles()
            .map(|handle| {
                let Item::Machine(child) = trees.items.item(handle) else {
                    panic!("machine child");
                };
                (child.name.as_str(&trees), child.nested_machines.len())
            })
            .collect::<Vec<_>>();
        assert_eq!(children, [("first", 1), ("second", 0)]);

        let mut copied = syntax_trees::SyntaxTrees::default();
        let Item::Machine(outer_copy) =
            copied.copy_item_from(&trees, &Item::Machine(outer.clone()))
        else {
            panic!("machine copy");
        };
        let children = outer_copy
            .nested_machines
            .handles()
            .map(|handle| {
                let Item::Machine(child) = copied.items.item(handle) else {
                    panic!("copied machine child");
                };
                (child.name.as_str(&copied), child.nested_machines.len())
            })
            .collect::<Vec<_>>();
        assert_eq!(children, [("first", 1), ("second", 0)]);
    }

    #[test]
    fn nested_machine_layout_rejects_resumed_statements_and_package_exports() {
        for (source, expected) in [
            (
                "machine outer() { machine helper() {} helper(); }",
                "executable statements cannot follow",
            ),
            (
                "machine outer() { public machine helper() {} }",
                "cannot be package-exported",
            ),
        ] {
            let tokens = Lexer::new(source).tokenize().unwrap();
            let error = parse_syntax_trees(&tokens).unwrap_err();
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    #[test]
    fn entry_and_nested_bodies_preserve_expanded_statement_order_and_boundaries() {
        let body =
            "let first: u64 = 1; asm clobbers none { lfence; sfence } transition { _ -> done() }";
        for source in [
            format!("machine process() {{ {body} machine done() {{}} }}"),
            format!("machine process() {{ machine begin() {{ {body} }} machine done() {{}} }}"),
        ] {
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let trees = parse_syntax_trees(&tokens).unwrap();
            let Item::Machine(machine) = trees.root_items().next().unwrap() else {
                panic!("machine root")
            };
            let machine = if trees
                .items
                .state(trees.items.state_handles(machine.states)[0])
                .statements
                .is_empty()
            {
                machine
                    .nested_machines
                    .handles()
                    .find_map(|handle| match trees.items.item(handle) {
                        Item::Machine(child) if child.name.as_str(&trees) == "begin" => Some(child),
                        _ => None,
                    })
                    .expect("nested begin")
            } else {
                machine
            };
            let states = trees.items.state_handles(machine.states);
            assert_eq!(states.len(), 1);
            let statements = super::inline_assembly::instruction_statements(
                &trees,
                trees
                    .items
                    .statements(trees.items.state(states[0]).statements),
            );
            assert_eq!(statements.len(), 4);
            assert!(matches!(
                trees.statements.statement(statements[0]),
                StatementNode::LocalData(_)
            ));
            for (handle, expected) in [(statements[1], "asm#lfence"), (statements[2], "asm#sfence")]
            {
                assert!(
                    matches!(trees.statements.statement(handle), StatementNode::Call(call) if call.target.as_str(&trees) == expected)
                );
            }
            assert!(matches!(
                trees.statements.statement(statements[3]),
                StatementNode::Transition(_)
            ));
        }
    }

    #[test]
    fn every_machine_body_rejects_bare_arrows() {
        for (source, expected) in [
            ("machine process() { -> done() }", "machine entry bodies"),
            (
                "machine process() { machine begin() { -> done() } }",
                "machine entry bodies",
            ),
        ] {
            let tokens = Lexer::new(source).tokenize().unwrap();
            let error = parse_syntax_trees(&tokens).unwrap_err();
            assert!(error.message.starts_with(expected), "{}", error.message);
            assert_eq!(error.source_span.span.start, source.find("->").unwrap());
        }
    }
}
pub(crate) mod constructors {
    use crate::tokens::token_cursor::Input;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::ExpressionNode;

    #[test]
    fn constructor_paths_keep_authored_field_evaluation_order_and_full_span() {
        for name in [
            "Value",
            "settings :: Value",
            "dep :: settings :: Value :: Some",
        ] {
            let source = format!("{name} {{ second: observe_second(), first: observe_first() }}");
            let source_id = source::SourceId::default();
            let tokens = Lexer::new(&source).tokenize().expect("tokens");
            let mut trees = syntax_trees::SyntaxTrees::new(source_id);
            let retained_source = Input::retain_source(&tokens);
            let input = Input::new(source_id, &tokens, &retained_source, &mut trees);
            let (expression, rest) =
                crate::grammar::expressions::parse_expression::parse_expression_handle(
                    &mut trees, input,
                )
                .expect("qualified constructor");
            assert!(rest.tokens.is_empty());
            let ExpressionNode::StructLiteral(literal) = trees.expressions.expression(expression)
            else {
                panic!("constructor must remain an ordinary structural literal");
            };
            assert_eq!(
                literal.constructor_name.as_str(&trees),
                name.replace(' ', "")
            );
            let span = literal.constructor_name.source_span().span;
            assert_eq!(&source[span.start..span.end], name);
            let fields = trees.expressions.struct_fields(literal.fields);
            assert_eq!(fields.len(), 2);
            for (field, expected) in fields.iter().zip(["second", "first"]) {
                assert_eq!(field.name.as_str(&trees), expected);
                let ExpressionNode::Call(call) = trees.expressions.expression(field.value) else {
                    panic!("authored field call must be retained");
                };
                assert_eq!(call.target.as_str(&trees), format!("observe_{expected}"));
            }
        }
    }
}
pub(crate) mod definitions_and_contracts {
    use crate::parser::parse_syntax_trees;
    use language_core::DomainPredicatePresence;
    use language_core::ReferenceAccess;
    use source_files_to_tokens::Lexer;
    use std::sync::Arc;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::Item;
    use syntax_trees::item::ProofFact;
    use syntax_trees::statement::StatementNode;
    use syntax_trees::types::TypeReferenceNode;

    fn machine_first_call_argument(
        parsed: &syntax_trees::SyntaxTrees,
        machine_name: &str,
    ) -> syntax_trees::expression::ExpressionHandle {
        let machine = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .find(|machine| machine.name.as_str(parsed) == machine_name)
            .expect("caller machine");
        let state = parsed.items.state(
            *parsed
                .items
                .state_handles(machine.states)
                .first()
                .expect("caller entry state"),
        );
        let StatementNode::Call(call) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("caller statement should be a call");
        };
        parsed.statements.expression_handles(call.arguments)[0]
    }

    #[test]
    fn parses_domain_definition_surface() {
        let source = r#"
        domain Player::Alive
        requires
            self in Player::Valid;
            self.health > 0
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let domains = super::root_domains(&parsed);

        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].name.as_str(&parsed), "Player::Alive");
        assert!(domains[0].target_type.is_valid());
        assert_eq!(
            domains[0].predicate_presence,
            DomainPredicatePresence::Present
        );
        assert_eq!(parsed.items.proof_facts(domains[0].facts).len(), 2);
        assert!(domains[0].semantic_clause_token_count > 3);

        let facts = parsed.items.proof_facts(domains[0].facts);
        assert!(matches!(facts[0], ProofFact::Membership(_)));
        assert!(matches!(facts[1], ProofFact::Expression(_)));
        let source_slices = domains[0]
            .facts
            .handles()
            .map(|handle| {
                let span = parsed
                    .items
                    .proof_fact_source_span(handle)
                    .expect("authored proof fact source span");
                &source[span.span.start..span.span.end]
            })
            .collect::<Vec<_>>();
        assert_eq!(source_slices, ["self in Player::Valid", "self.health > 0"]);
    }

    #[test]
    fn parses_domain_on_applied_carrier_without_mixing_domain_arguments() {
        let source = "domain<T, const N: u64> Buffer<Pair<T, u32>, N>::Live<T>;";
        let parsed = super::parse_source(source, "applied carrier domain should parse");
        let domain = super::root_domain(&parsed, "domain");
        assert_eq!(domain.name.as_str(&parsed), "Live");
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = parsed.type_references.type_reference(domain.target_type)
        else {
            panic!("domain carrier must retain its type application");
        };
        assert_eq!(base_name.as_str(&parsed), "Buffer");
        let carrier_arguments = parsed.type_references.type_reference_handles(*arguments);
        assert_eq!(carrier_arguments.len(), 2);
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = parsed.type_references.type_reference(carrier_arguments[0])
        else {
            panic!("nested carrier argument must retain its application");
        };
        assert_eq!(base_name.as_str(&parsed), "Pair");
        assert_eq!(
            parsed
                .type_references
                .type_reference_handles(*arguments)
                .len(),
            2
        );
        assert!(matches!(
            parsed.type_references.type_reference(carrier_arguments[1]),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "N"
        ));
        let domain_arguments = parsed
            .type_references
            .type_reference_handles(domain.index_arguments);
        assert_eq!(domain_arguments.len(), 1);
        assert!(matches!(
            parsed.type_references.type_reference(domain_arguments[0]),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "T"
        ));
    }

    #[test]
    fn parses_domain_requires_and_requirement_routes_independently() {
        let source = r#"
        domain Reservation::Confirmed
        requires
            self.seats > 0
        established by
            Reservations::confirm,
            Imported::Reservations::restore;
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let domain = super::root_domain(&parsed, "domain");

        assert_eq!(domain.predicate_presence, DomainPredicatePresence::Present);
        assert_eq!(parsed.items.proof_facts(domain.facts).len(), 1);
        assert_eq!(
            domain
                .authored_routes
                .iter()
                .map(|route| {
                    route
                        .iter()
                        .map(|member| member.as_str(&parsed))
                        .collect::<Vec<_>>()
                        .join("::")
                })
                .collect::<Vec<_>>(),
            ["Reservations::confirm", "Imported::Reservations::restore"]
        );
        assert!(domain.operators.is_empty());
    }

    #[test]
    fn parses_unqualified_machine_establishment_routes() {
        let source = "domain u64::Issued established by issue, Factory::issue;";
        let parsed =
            super::parse_source_verbose(source, "tokens", "free and attached issuer paths");
        let domain = super::root_domain(&parsed, "domain");
        let routes = domain
            .authored_routes
            .iter()
            .map(|route| {
                route
                    .iter()
                    .map(|member| member.as_str(&parsed))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(routes, [vec!["issue"], vec!["Factory", "issue"]]);

        for malformed in ["", "issue::", "issue,", "issue()", "issue<u64>"] {
            let source = format!("domain u64::Issued established by {malformed};");
            let tokens = Lexer::new(&source).tokenize().expect("tokens");
            assert!(parse_syntax_trees(&tokens).is_err(), "{source}");
        }
    }

    #[test]
    fn parses_explicit_progress_profile_classification() {
        let source = r#"
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant_weak_fair;
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let domain = super::root_domain(&parsed, "domain");

        assert_eq!(
            domain.classification,
            Some(language_core::DomainClassification::ProgressProfile)
        );
        assert_eq!(domain.authored_routes.len(), 1);
        assert!(domain.semantic_clause_token_count > 0);
    }

    #[test]
    fn rejects_unknown_or_duplicate_domain_classifications() {
        for (source, expected) in [
            (
                "domain Scheduler::Fair satisfies UserProfile;",
                "unknown compiler-owned domain classification `UserProfile`",
            ),
            (
                "domain Scheduler::Fair satisfies ProgressProfile satisfies ProgressProfile;",
                "at most one compiler-owned classification",
            ),
        ] {
            let error = super::parse_tokenized_error(source, "classification must reject");
            assert!(error.message.contains(expected), "got: {}", error.message);
        }
    }

    #[test]
    fn rejects_domain_classification_after_requires() {
        let source = r#"
        domain Scheduler::Fair
        requires true
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        "#;
        let error = super::parse_tokenized_error(source, "classification order must reject");
        super::expect_error(
            &error,
            "classification must appear immediately after the domain head",
        );
    }

    #[test]
    fn rejects_legacy_domain_route_bodies_with_migration_guidance() {
        let source = r#"
        domain Reservation::Issued {
            Reservations::issue;
        }
        "#;

        let error = super::parse_tokenized_error(source, "route bodies must be retired");
        super::expect_error(&error, "domain route bodies are retired");
        assert!(error.message.contains("`established by Trait::requirement"));
    }

    #[test]
    fn rejects_legacy_domain_body_predicates_with_migration_guidance() {
        let source = r#"
        domain Player::Alive {
            self.health > 0;
        }
        "#;

        let error = super::parse_tokenized_error(source, "body predicates must be retired");
        super::expect_error(&error, "domain predicates must be written in `requires`");
        assert!(error.message.contains("`established by Trait::requirement"));
    }

    #[test]
    fn rejects_empty_domain_bodies_with_migration_guidance() {
        for source in [
            "domain u64::Tagged {}",
            "domain u64::Positive requires self > 0 { }",
            "domain<T> T::Tagged {}",
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize should succeed",
                "empty domain bodies must be retired",
            );
            super::expect_error(&error, "empty domain bodies are retired");
            assert!(error.message.contains("end the declaration with `;`"));
        }
    }

    #[test]
    fn domain_semicolon_preserves_following_mathematical_declarations() {
        for following in [
            "proposition same(value: u64) = value == value;",
            "let identity(value: u64): u64 = value;",
        ] {
            let source = format!("domain u64::Positive requires self > 0;\n{following}");
            let parsed = super::parse_source(&source, "each declaration owns its semicolon");
            let domain = parsed
                .root_items()
                .find_map(|item| match item {
                    Item::Domain(domain) => Some(domain),
                    _ => None,
                })
                .expect("domain declaration remains present");
            assert_eq!(parsed.items.proof_facts(domain.facts).len(), 1);
        }
    }

    #[test]
    fn rejects_nested_domain_operators_with_top_level_home_guidance() {
        let source = r#"
        domain i32::Degrees {
            operator + add(left: i32, right: i32) -> i32;
        }
        "#;

        let error = super::parse_tokenized_error(source, "nested operators must be retired");
        super::expect_error(
            &error,
            "domain operators must be ordinary top-level declarations",
        );
        assert!(
            error
                .message
                .contains("`operator Type::Domain::operation ...`")
        );
    }

    #[test]
    fn parses_domain_predicate_absence_distinct_from_true_predicate() {
        let source = r#"
        domain Reservation::Issued;
        domain Reservation::Recorded;
        domain Reservation::Universal
        requires
            true;
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let domains = super::root_domains(&parsed);

        assert_eq!(domains.len(), 3);
        for domain in &domains[..2] {
            assert_eq!(domain.predicate_presence, DomainPredicatePresence::Absent);
            assert!(domain.facts.is_empty());
            assert_eq!(domain.semantic_clause_token_count, 0);
        }
        assert_eq!(
            domains[2].predicate_presence,
            DomainPredicatePresence::Present
        );
        assert_eq!(parsed.items.proof_facts(domains[2].facts).len(), 1);
        assert!(domains[2].semantic_clause_token_count > 0);
    }

    #[test]
    fn parses_transparent_domain_alias_as_an_independent_record() {
        let source = r#"
        public domain Socket::Usable =
            Socket::Connected & Socket::Authenticated;
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let domain = super::root_domain(&parsed, "alias domain");

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
                    .map(|member| member.as_str(&parsed))
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .collect::<Vec<_>>();
        assert_eq!(paths, ["Socket::Connected", "Socket::Authenticated"]);
        assert_eq!(domain.predicate_presence, DomainPredicatePresence::Absent);
        assert!(domain.facts.is_empty());
        assert!(domain.operators.is_empty());
    }

    #[test]
    fn parses_explicit_parameter_with_named_carrier_type() {
        let source = r#"
        data Main {
        }

        machine Main::main(main: &mut Main) {}
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        let parameter = parsed.items.state_parameter(
            parsed
                .items
                .state_parameters(state.parameters)
                .first()
                .copied()
                .expect("explicit parameter"),
        );

        let TypeReferenceNode::Reference {
            referee, access, ..
        } = parsed
            .type_references
            .type_reference(parameter.type_reference)
        else {
            panic!("explicit mutable borrow should retain its reference ownership mode");
        };
        assert_eq!(*access, ReferenceAccess::Mutable);
        assert!(matches!(
            parsed.type_references.type_reference(*referee),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "Main"
        ));
    }

    #[test]
    fn parses_explicit_write_only_borrow_with_exact_access_mode() {
        let source = r#"
        machine fill(destination: &write bool, value: bool) {
            destination = value;
        }

        machine caller(destination: &mut bool) {
            fill(&write destination, true);
        }
    "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let argument = machine_first_call_argument(&parsed, "caller");
        let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(argument) else {
            panic!("argument should retain one closed borrow-expression node");
        };
        assert_eq!(borrow.access, ReferenceAccess::WriteOnly);
        assert_eq!(
            parsed.display_expression_name(argument),
            "&write destination"
        );
    }

    #[test]
    fn parses_explicit_shared_borrow_with_exact_access_mode() {
        let source = r#"
        machine observe(value: &bool) {}

        machine caller(source: &bool) {
            observe(&source);
        }
    "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let argument = machine_first_call_argument(&parsed, "caller");
        let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(argument) else {
            panic!("argument should retain one closed shared-borrow expression node");
        };
        assert_eq!(borrow.access, ReferenceAccess::Shared);
        assert_eq!(parsed.display_expression_name(argument), "&source");
    }

    #[test]
    fn parses_explicit_parameter_use_as_ordinary_name() {
        let source = r#"
        data Main {
        }

        machine Main::main(main: &mut Main) {
            main;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
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

        let ExpressionNode::Name(path) = parsed.expressions.expression(*expression) else {
            panic!("explicit parameter must remain an ordinary name");
        };
        assert_eq!(
            parsed.expressions.identifier_path_members(*path)[0].as_str(&parsed),
            "main"
        );
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

        machine Main::main(main: &mut Main, player: &mut Player) {
            player.xp = max(0, player.xp - xp_required(player.level));

            machine xp_required(level: i32) -> i32 {
                10
            }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
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
            parsed.display_expression_name(assignment.value),
            "max(0, player.xp - xp_required(player.level))"
        );
    }

    #[test]
    fn parses_positional_erased_evidence_call_lane_after_semicolon() {
        let source = r#"
        machine Main::main(value: i32, first_proof: Evidence, second_proof: Evidence) {
            consume(value; first_proof, second_proof);
        }
    "#;
        let parsed = super::parse_source_verbose(source, "tokenize", "evidence lane should parse");
        let machine = super::root_machine(&parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statement = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0]);
        let StatementNode::Call(call) = statement else {
            panic!("expected call statement");
        };

        assert_eq!(
            parsed.statements.expression_handles(call.arguments).len(),
            1
        );
        assert_eq!(
            call.evidence_arguments
                .iter()
                .map(|name| name.as_str(&parsed))
                .collect::<Vec<_>>(),
            ["first_proof", "second_proof"]
        );
    }

    #[test]
    fn parses_evidence_only_call_lane_with_leading_semicolon() {
        let source = r#"
        machine Main::main(proof: Evidence) {
            consume(; proof);
        }
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize", "evidence-only lane should parse");
        let machine = super::root_machine(&parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let StatementNode::Call(call) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("expected call statement");
        };
        assert!(
            parsed
                .statements
                .expression_handles(call.arguments)
                .is_empty()
        );
        assert_eq!(call.evidence_arguments[0].as_str(&parsed), "proof");
    }

    #[test]
    fn parses_evidence_lane_on_named_transition_without_dropping_it() {
        let source = r#"
        machine Main::main(proof: Evidence) {
            transition { _ -> next(; proof) }
            machine next() {}
        }
    "#;
        let parsed = super::parse_source_verbose(
            source,
            "tokenize",
            "named transition evidence lane should parse",
        );
        let machine = super::root_machine(&parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let StatementNode::Transition(transition) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("expected transition statement");
        };
        let syntax_trees::statement::TransitionTargetNode::Named {
            evidence_arguments, ..
        } = parsed.statements.transition_target(transition.target)
        else {
            panic!("expected named transition target");
        };
        assert_eq!(
            evidence_arguments
                .iter()
                .map(|name| name.as_str(&parsed))
                .collect::<Vec<_>>(),
            ["proof"]
        );
    }

    #[test]
    fn tail_self_call_retains_authored_call_until_resolution() {
        let source = r#"
        machine repeat(remaining: u64) -> u64
        terminates by remaining -> Nat::Descending
        {
            repeat(remaining - 1)
        }
    "#;
        let parsed = super::parse_source_verbose(source, "tokenize", "tail self call should parse");
        let machine = super::root_machine(&parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let StatementNode::Transition(transition) = parsed
            .statements
            .statement(parsed.items.statements(state.statements)[0])
        else {
            panic!("the final value completes the invocation");
        };
        assert!(transition.ends_dispatch);
        assert!(matches!(
            transition.guard,
            syntax_trees::statement::TransitionGuardNode::Always
        ));
        let syntax_trees::statement::TransitionTargetNode::Value(expression) =
            parsed.statements.transition_target(transition.target)
        else {
            panic!("parsing retains the authored value; resolution owns tail normalization");
        };
        let ExpressionNode::Call(call) = parsed.expressions.expression(*expression) else {
            panic!("the terminal expression retains its exact authored call");
        };
        let source_span = call.target.source_span();
        assert_eq!(
            &source[source_span.span.start..source_span.span.end],
            "repeat"
        );
        assert_eq!(source_span.span.start, source.rfind("repeat").unwrap());
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

        machine Deck::best<machine Key>(deck: &Deck, card: &Card) -> u64
        requires machine Key(value: &Card) -> u64
        reaches Console
        requires value in Card::Scorable;
        {
            0
        }
        "#;

        let parsed = super::parse_source(source, "machine parameter should parse");
        let machine = super::root_machine(&parsed, "generic machine");
        let parameters = parsed.items.type_parameters(machine.type_parameters);
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name.as_str(&parsed), "Key");
        let syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
        } = &parameters[0].kind
        else {
            panic!("Key should carry its authored machine contract");
        };
        assert_eq!(contract.name.as_str(&parsed), "Key");
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
    fn parses_nominal_machine_parameter_requirement() {
        let source = r#"
        machine register<machine Selected>()
        requires machine Selected satisfies platform::WindowProcedure::call;
        {
            Selected()
        }
        "#;

        let parsed = super::parse_source(source, "nominal machine contract should parse");
        let machine = super::root_machine(&parsed, "generic machine");
        let [parameter] = parsed.items.type_parameters(machine.type_parameters) else {
            panic!("expected one machine parameter");
        };
        let syntax_trees::item::TypeParameterKind::Machine {
            contract:
                Some(syntax_trees::item::MachineParameterContract::Nominal { requirement, .. }),
        } = &parameter.kind
        else {
            panic!("Selected should retain its nominal requirement path");
        };
        assert_eq!(
            parsed
                .items
                .identifier_path_members(*requirement)
                .iter()
                .map(|member| member.as_str(&parsed))
                .collect::<Vec<_>>(),
            ["platform", "WindowProcedure", "call"]
        );
        assert!(!machine.bodyless);
    }

    #[test]
    fn parses_bodyless_nominal_parameter_with_one_terminator_per_declaration() {
        let source = r#"
        boundary machine register<machine Selected>()
        requires machine Selected satisfies WindowProcedure::call;;
        "#;

        let parsed = super::parse_source(source, "bodyless nominal binder should parse");
        let machine = super::root_machine(&parsed, "bodyless generic machine");
        assert!(machine.bodyless);
        assert!(matches!(
            parsed.items.type_parameters(machine.type_parameters)[0].kind,
            syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(syntax_trees::item::MachineParameterContract::Nominal { .. })
            }
        ));
    }

    #[test]
    fn parses_target_scoped_bodyless_boundary_machine() {
        let source = r#"
        boundary trait Console { machine exit_process(code: i32) reaches; }
        data ConsoleNativeProvider {}
        linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(code: i32)
            satisfies Console::exit_process;
        "#;

        let parsed = super::parse_source_verbose(
            source,
            "tokenize target-scoped catalog leaf",
            "parse target-scoped catalog leaf",
        );
        let machine = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .find(|machine| machine.name.as_str(&parsed) == "ConsoleNativeProvider::exit_process")
            .expect("target-scoped catalog leaf");
        assert!(machine.boundary);
        assert!(machine.bodyless);
        assert_eq!(
            machine.target.as_ref().map(|target| target.as_str(&parsed)),
            Some("linux_x86_64"),
        );
        let [conformance] = parsed.items.satisfies_clauses(machine.satisfies) else {
            panic!("one exact satisfies edge")
        };
        assert!(conformance.via.is_none());
        assert!(!conformance.via_expression.is_valid());
    }

    #[test]
    fn parses_structural_and_nominal_machine_parameter_contracts_together() {
        let source = r#"
        machine apply<machine Schema, machine Selected>()
        requires machine Schema(value: u64) -> u64;
        requires machine Selected satisfies WindowProcedure::call;
        {
        }
        "#;

        let parsed = super::parse_source(source, "mixed machine contracts should parse");
        let machine = super::root_machine(&parsed, "generic machine");
        let parameters = parsed.items.type_parameters(machine.type_parameters);
        assert!(matches!(
            parameters[0].kind,
            syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(syntax_trees::item::MachineParameterContract::Structural(_))
            }
        ));
        assert!(matches!(
            parameters[1].kind,
            syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(syntax_trees::item::MachineParameterContract::Nominal { .. })
            }
        ));
    }

    #[test]
    fn retains_nominal_machine_parameter_trait_application() {
        let source = r#"
        trait Visitor<Context> {
            machine visit(context: &mut Context);
        }
        machine walk<Context, machine Step>(context: &mut Context)
        requires machine Step satisfies Visitor<Context>::visit;
        {
            Step(context);
        }
    "#;
        let parsed = super::parse_source(source, "an applied nominal binder should parse");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "walk" => Some(machine),
                _ => None,
            })
            .expect("generic machine");
        let step = parsed
            .items
            .type_parameters(machine.type_parameters)
            .iter()
            .find(|parameter| parameter.name.as_str(&parsed) == "Step")
            .expect("machine parameter");
        let syntax_trees::item::TypeParameterKind::Machine {
            contract:
                Some(syntax_trees::item::MachineParameterContract::Nominal {
                    requirement,
                    trait_arguments,
                }),
        } = &step.kind
        else {
            panic!("Step should retain its nominal requirement");
        };
        assert_eq!(
            parsed
                .items
                .identifier_path_members(*requirement)
                .iter()
                .map(|member| member.as_str(&parsed))
                .collect::<Vec<_>>(),
            vec!["Visitor", "visit"]
        );
        assert_eq!(trait_arguments.len(), 1);
    }

    #[test]
    fn rejects_malformed_nominal_machine_parameter_requirements() {
        let cases = [
            (
                "requires machine Selected satisfies WindowProcedure;",
                "must name an exact `Trait::requirement`",
            ),
            (
                "requires machine Selected satisfies WindowProcedure::call as Choice;",
                "do not accept `as Name`",
            ),
            (
                "requires machine Selected satisfies WindowProcedure::call via Native;",
                "cannot use `via`",
            ),
            (
                "requires machine Selected satisfies WindowProcedure<u64>;",
                "must be followed by `::requirement`",
            ),
        ];

        for (contract, expected) in cases {
            let source =
                format!("machine register<machine Selected>() {contract} {{ Selected() }}");
            let error =
                super::parse_tokenized_error(&source, "malformed nominal binder must reject");
            super::expect_error(&error, expected);
        }
    }

    #[test]
    fn rejects_one_off_machine_member_requirements_instead_of_discarding_them() {
        let source = r#"
        data Device {}

        machine poll_once<T>(device: &mut T)
        requires machine T::poll(value: &mut T)
        {
        }
        "#;

        let error = super::parse_source_error(
            source,
            "tokenize should succeed",
            "a one-off member requirement has no semantic carrier and must reject",
        );
        assert!(
            error
                .message
                .contains("one-off `requires machine T::member(...)` requirements are unsupported"),
            "unexpected diagnostic: {}",
            error.message
        );
    }

    #[test]
    fn parses_higher_order_machine_parameter_contract() {
        let source = r#"
        machine apply<machine Schema, machine Sample>(value: u64) -> u64
        requires machine Schema<machine Inner>(value: u64) -> u64
        requires machine Inner(value: u64) -> u64;
        ;
        requires machine Sample(value: u64) -> u64;
        {
            Schema<Sample>(value)
        }
        "#;

        let parsed = super::parse_source(source, "higher-order contract should parse");
        let machine = super::root_machine(&parsed, "generic machine");
        let parameters = parsed.items.type_parameters(machine.type_parameters);
        assert_eq!(parameters.len(), 2);
        let syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(schema)),
        } = &parameters[0].kind
        else {
            panic!("Schema should carry its authored machine contract");
        };
        let nested = parsed.items.type_parameters(schema.type_parameters);
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].name.as_str(&parsed), "Inner");
        let syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(inner)),
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
        requires machine Schema<machine Inner>()
        {
        }
        "#;
        let error = super::parse_tokenized_error(source, "missing nested contract must fail");
        super::expect_error(
            &error,
            "machine parameter `Inner` requires an authored declaration-site contract",
        );
    }

    #[test]
    fn rejects_machine_parameter_without_authored_contract() {
        let source = r#"
        machine map<machine F>() -> u64 {
            0
        }
        "#;
        let error = super::parse_tokenized_error(source, "missing contract must fail");
        super::expect_error(&error, "requires an authored declaration-site contract");
    }

    #[test]
    fn parses_machine_parameter_on_proof_data_declaration() {
        let source = r#"
        data Stream<machine S>
        requires machine S(index: u64) -> u64;
        {
            case More(tail: Stream<S>);
        }
    "#;
        let parsed = super::parse_source(source, "proof-data machine parameter should parse");
        let data = super::root_data(&parsed, "Stream declaration");
        let parameters = parsed.items.type_parameters(data.type_parameters);
        assert_eq!(parameters.len(), 1);
        let syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
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
        let parsed = super::parse_source(source, "quotient declaration should parse");
        let quotient = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) if data.name.as_str(&parsed) == "Quotient" => {
                    data.quotient.as_ref()
                }
                _ => None,
            })
            .expect("Quotient metadata");
        assert!(matches!(
            parsed.type_references.type_reference(quotient.carrier),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "Carrier"
        ));
        assert_eq!(
            parsed
                .items
                .identifier_path_members(quotient.relation)
                .iter()
                .map(|member| member.as_str(&parsed))
                .collect::<Vec<_>>(),
            vec!["Laws", "equivalent"]
        );
    }

    #[test]
    fn parses_quotient_equivalence_selection_requirement() {
        let source = r#"
        data Carrier { case Value; }
        data Quotient = Carrier % same
        requires same satisfies Equivalence<Carrier, same> as Laws;
    "#;
        let parsed = super::parse_source(source, "quotient selection should parse");
        let selection = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) if data.name.as_str(&parsed) == "Quotient" => {
                    data.quotient.as_ref()?.equivalence.as_ref()
                }
                _ => None,
            })
            .expect("quotient equivalence selection");
        assert_eq!(selection.trait_name.as_str(&parsed), "Equivalence");
        assert_eq!(selection.conformance_name.as_str(&parsed), "Laws");

        let retired = super::parse_tokenized_error(
            "data Carrier { case Value; }\ndata Quotient = Carrier % same\nwhere same satisfies Equivalence<Carrier, same> as Laws;",
            "retired quotient `where` must reject",
        );
        super::expect_error(&retired, "found identifier `where`");
    }

    #[test]
    fn parses_static_machine_symbol_call_argument() {
        let source = r#"
        data Card {}

        machine map<T, machine F>(value: &T)
        requires machine F(value: &T);
        {
        }

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
    "#;
        let parsed = super::parse_source(source, "static machine argument should parse");
        let (_, call) = parsed
            .expressions
            .iter_expressions()
            .find_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call) if !call.machine_arguments.is_empty() => {
                    Some((handle, call))
                }
                _ => None,
            })
            .expect("generic call expression");
        assert_eq!(call.machine_arguments.len(), 1);
        assert_eq!(
            super::argument_path(&parsed, &call.machine_arguments[0].path),
            vec!["Card", "power"]
        );
    }

    #[test]
    fn static_call_lookahead_keeps_delimiters_inside_argument_groups() {
        let parsed = super::parse_source(
            r#"machine caller() {
            select<match value { 0 -> 1, _ -> 2 }>();
            select<(Record { value: 3 }).value + 1>();
            select<[u8; 3]>();
        }"#,
            "grouped match arms, record fields and array extents stay inside static arguments",
        );
        let calls = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str(&parsed) == "select" => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        assert!(calls.iter().all(|call| call.machine_arguments.len() == 1));
    }

    #[test]
    fn parses_nested_static_conformance_application() {
        let source = r#"
        trait Encodes<T> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output> satisfies Encodes<Output> {}

        machine send<T, Encoding satisfies Encodes<T>>() {}
        machine caller() {
            send<Message, SequenceEncoding<'scope, Bytes, Message>>();
        }
    "#;
        let parsed = super::parse_source(source, "nested static application should parse");
        let call = parsed
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str(&parsed) == "send" => Some(call),
                _ => None,
            })
            .expect("generic call expression");
        let application = call.machine_arguments[1]
            .application
            .as_ref()
            .expect("nested conformance application");
        assert_eq!(
            call.machine_arguments[1].path[0].as_str(&parsed),
            "SequenceEncoding"
        );
        assert_eq!(application.lifetime_arguments[0].as_str(&parsed), "scope");
        assert_eq!(application.arguments.len(), 2);
        assert_eq!(application.arguments[0].path[0].as_str(&parsed), "Bytes");
        assert_eq!(application.arguments[1].path[0].as_str(&parsed), "Message");
        assert_eq!(
            call.display_name(&parsed),
            "send<Message, SequenceEncoding<'scope, Bytes, Message>>()"
        );
    }

    #[test]
    fn parses_evidence_term_member_as_a_distinct_proof_static_argument() {
        let source = r#"
        machine consume<machine Witness>()
        requires machine Witness();
        {}

        machine caller() {
            consume<proof.modulus>();
        }
    "#;
        let parsed = super::parse_source(source, "evidence projection should parse");
        let call = parsed
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str(&parsed) == "consume" => {
                    Some(call)
                }
                _ => None,
            })
            .expect("generic call expression");
        let [argument] = call.machine_arguments.as_ref() else {
            panic!("one proof-static argument")
        };
        assert!(argument.path.is_empty());
        assert!(argument.const_literal.is_none());
        let projection = argument
            .evidence_projection
            .as_ref()
            .expect("term.member projection");
        assert_eq!(projection.term.as_str(&parsed), "proof");
        assert_eq!(projection.member.as_str(&parsed), "modulus");
        assert_eq!(call.display_name(&parsed), "consume<proof.modulus>()");
    }

    #[test]
    fn destructure_marker_preserves_double_underscore_field_as_one_component() {
        let source = r#"
        data Pair { left__value: i32; right: i32; }
        machine inspect(pair: Pair) {
            let { left__value, right as _ } = pair;
        }
    "#;
        let parsed = super::parse_source(source, "destructure should parse");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        let marker = parsed
            .items
            .statements(state.statements)
            .iter()
            .find_map(|handle| match parsed.statements.statement(*handle) {
                StatementNode::LocalData(local)
                    if local.name.as_str(&parsed).starts_with("__destructure#") =>
                {
                    Some(local)
                }
                _ => None,
            })
            .expect("destructure marker local");

        assert_eq!(
            marker.name.as_str(&parsed),
            "__destructure#left__value#right",
            "the internal delimiter must not split repeated underscores"
        );
    }

    #[test]
    fn proof_output_binding_separates_type_and_prop_lanes() {
        let source = r#"
        machine produce() -> i32
        ensures proof: true
        { proof = true; 7 }

        machine consume() -> i32 {
            let (value; proof: local_proof) = produce();
            value
        }
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize", "proof-output lane should parse");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "consume" => {
                    Some(machine)
                }
                _ => None,
            })
            .expect("consume machine");
        let state = super::entry_state(&parsed, machine);
        let package = parsed
            .items
            .statements(state.statements)
            .iter()
            .find_map(|handle| match parsed.statements.statement(*handle) {
                StatementNode::ProofOutputBindingStatement(package) => Some(package),
                _ => None,
            })
            .expect("one internal proof-output binding carrier");
        assert_eq!(package.bindings.len(), 2);
        assert_eq!(package.bindings[0].output_field.as_str(&parsed), "value");
        assert_eq!(package.bindings[0].binding.as_str(&parsed), "value");
        assert_eq!(package.bindings[1].output_field.as_str(&parsed), "proof");
        assert_eq!(package.bindings[1].binding.as_str(&parsed), "local_proof");
    }

    #[test]
    fn retired_generated_proof_package_has_directed_migration() {
        let source = r#"
        machine produce() ensures proof: true { proof = true; }
        machine consume() { let { proof: local } = produce(); }
    "#;
        let error =
            super::parse_source_error(source, "tokenize", "old package spelling must reject");
        assert!(error.message.contains("proof-output packages are retired"));
        assert!(
            error
                .message
                .contains("let (value; public_output: local_term)")
        );
    }

    fn parse_domain_facts(source: &str) -> (syntax_trees::SyntaxTrees, Vec<ProofFact>) {
        let parsed = super::parse_source(source, "parse should succeed");
        let facts = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Domain(domain) => Some(domain.facts),
                _ => None,
            })
            .expect("one domain");
        let facts = parsed.items.proof_facts(facts).to_vec();
        (parsed, facts)
    }

    #[test]
    fn compound_proof_membership_keeps_ordinary_boolean_precedence() {
        use syntax_trees::expression::BinaryOperator;
        for (source, membership_on_left) in [
            (
                "domain Value::D requires false || self in Value::Case",
                false,
            ),
            (
                "domain Value::D requires self in Value::Case || false",
                true,
            ),
        ] {
            let (parsed, facts) = parse_domain_facts(source);
            let [ProofFact::Expression(root)] = facts.as_slice() else {
                panic!("compound fact must retain its Boolean operation: {facts:?}");
            };
            let ExpressionNode::Binary(binary) = parsed.expressions.expression(*root) else {
                panic!("expected Boolean operation");
            };
            assert_eq!(binary.operator, BinaryOperator::Or);
            let (membership, boolean) = if membership_on_left {
                (binary.left, binary.right)
            } else {
                (binary.right, binary.left)
            };
            assert!(matches!(
                parsed.expressions.expression(boolean),
                ExpressionNode::Boolean(false)
            ));
            let ExpressionNode::Membership(membership) = parsed.expressions.expression(membership)
            else {
                panic!("membership must apply to its operand, not the Boolean operation");
            };
            assert!(matches!(
                parsed.expressions.expression(membership.value),
                ExpressionNode::Name(_)
            ));
            let path = parsed
                .expressions
                .identifier_path_members(membership.domain)
                .iter()
                .map(|part| part.as_str(&parsed))
                .collect::<Vec<_>>();
            assert_eq!(path, ["Value", "Case"]);
        }
    }

    #[test]
    fn proof_fact_membership_carries_indexed_domain_application_by_argument() {
        let (parsed, facts) = parse_domain_facts(
            r#"
        domain Extent::Access
        requires
            self in Granted & Resident<SlotPlacement, Slot>
        "#,
        );
        let [
            ProofFact::Membership(granted),
            ProofFact::Membership(resident),
        ] = facts.as_slice()
        else {
            panic!("an `&` chain lowers to one membership fact per domain, got {facts:?}");
        };
        assert_eq!(
            parsed.items.identifier_path_members(granted.domain)[0].as_str(&parsed),
            "Granted"
        );
        assert!(granted.domain_arguments.is_empty());
        assert_eq!(
            parsed.items.identifier_path_members(resident.domain)[0].as_str(&parsed),
            "Resident"
        );
        let arguments = parsed
            .type_references
            .type_reference_handles(resident.domain_arguments)
            .iter()
            .map(
                |argument| match parsed.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(name) => name.as_str(&parsed).to_owned(),
                    other => panic!("index argument should be a named leaf, got {other:?}"),
                },
            )
            .collect::<Vec<_>>();
        assert_eq!(arguments, ["SlotPlacement", "Slot"]);
    }

    #[test]
    fn proof_fact_pipe_alternative_rejects_indexed_domain_application() {
        let tokens = Lexer::new(
            r#"
        domain Extent::Access
        requires
            self in Granted | Resident<SlotPlacement, Slot>
        "#,
        )
        .tokenize()
        .expect("tokenize should succeed");
        let error =
            parse_syntax_trees(&tokens).expect_err("a `|` alternative has no argument slot");
        super::expect_error(
            &error,
            "indexed domain applications are not carried by `|` proof-fact alternatives",
        );
    }

    #[test]
    fn proof_fact_carry_permission_rejects_index_arguments() {
        let tokens = Lexer::new(
            r#"
        domain Extent::Access
        requires
            self in Carry::AnyCpu<SlotPlacement>
        "#,
        )
        .tokenize()
        .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("carry permissions have no index");
        super::expect_error(
            &error,
            "compiler carry permissions do not take index arguments",
        );
    }

    #[test]
    fn declaration_lookahead_rewinds_private_rows_and_preserves_source_custody() {
        use crate::grammar::declarations::parse_declaration::starts_following_item;
        use crate::tokens::token_cursor::Input;
        use syntax_trees::SyntaxTrees;

        for (source, expected) in [
            ("operator < limit;", false),
            ("domain[0] > 0;", false),
            ("operator + invoke<Outer<Inner<i32>>>(limit) > 0;", false),
            ("operator + add(left: i32, right: i32) -> i32;", true),
            ("public domain [u8; (1 + 2)]::Bytes requires true;", true),
            ("operator + add(left: ) -> i32;", false),
            ("domain [u8; ]::Bytes;", false),
        ] {
            let tokens = Lexer::new(source).tokenize().expect("lookahead tokens");
            let retained_source = Input::retain_source(&tokens);
            let source_id = source::SourceId::default();
            let mut trees = SyntaxTrees::new_with_generation(source_id, 7);
            let input = Input::new(source_id, &tokens, &retained_source, &mut trees);
            let established = trees.expressions.insert(ExpressionNode::Boolean(true));
            trees.roots.constant_extent_probes.push((established, true));
            let roots = trees.roots.clone();
            let source_owners = Arc::strong_count(&retained_source);
            let mut untouched = trees.clone();
            let expected_next = untouched.expressions.insert(ExpressionNode::Boolean(false));
            drop(untouched);

            let outer = trees.append_checkpoint();
            for _ in 0..2 {
                assert_eq!(
                    starts_following_item(&mut trees, input),
                    expected,
                    "{source}"
                );
                assert_eq!(trees.roots, roots);
                assert_eq!(trees.expressions.iter_expressions().count(), 1);
                assert!(matches!(
                    trees.expressions.expression(established),
                    ExpressionNode::Boolean(true)
                ));
                assert_eq!(Arc::strong_count(&retained_source), source_owners);
            }
            let next = trees.expressions.insert(ExpressionNode::Boolean(false));
            assert_eq!(next, expected_next, "private probe handles must not escape");
            trees.rewind_appended(outer);
            assert_eq!(trees.roots, roots);
            assert_eq!(trees.expressions.iter_expressions().count(), 1);
        }
    }

    #[test]
    fn boundary_requirement_publishes_explicit_empty_reach_without_repeating_it_on_realizers() {
        let source = r#"
        public boundary trait Mark { machine read() -> u64 reaches; }
        MarkerMark satisfies Mark { machine read() -> u64 { 7 } }
        machine main() { let value: u64 = Mark::read(); }
    "#;
        let parsed = super::parse_source(
            source,
            "an empty boundary ceiling and inferred realizer parse",
        );
        let requirement = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Trait(definition) => Some(definition),
                _ => None,
            })
            .expect("boundary trait");
        let signature = parsed.items.state_signatures(requirement.machines)[0];
        let signature = parsed.items.state_signature(signature);
        assert!(signature.service_reaches.is_empty());
        assert_eq!(signature.service_reach_keyword_source_spans.len(), 1);

        let missing = source.replace("read() -> u64 reaches;", "read() -> u64;");
        let tokens = Lexer::new(&missing).tokenize().expect("valid tokens");
        let error = parse_syntax_trees(&tokens)
            .expect_err("a boundary requirement cannot infer its ceiling");
        super::expect_error(
            &error,
            "Mark::read` must declare an explicit `reaches` ceiling",
        );

        let nested = r#"
        boundary trait Mark {
            machine relay<machine Step>() -> u64
            requires machine Step() -> u64 reaches Mark;
            ensures true;
        }
    "#;
        let tokens = Lexer::new(nested)
            .tokenize()
            .expect("valid nested contract tokens");
        let error = parse_syntax_trees(&tokens)
            .expect_err("a nested binder ceiling does not annotate the requirement");
        super::expect_error(
            &error,
            "Mark::relay` must declare an explicit `reaches` ceiling",
        );
    }

    #[test]
    fn explicit_empty_rows_spell_none() {
        // `reaches none` is the empty service row with its keyword span
        // retained; `crashes none` publishes no crash contract. Both read
        // without the rule.
        let source = r#"
            public boundary trait Mark { machine read() -> u64 reaches none; }
            public machine quiet() -> u64 reaches none crashes none { 7 }
        "#;
        let parsed = super::parse_source(source, "explicit empty rows parse");
        let requirement = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Trait(definition) => Some(definition),
                _ => None,
            })
            .expect("boundary trait");
        let signature = parsed.items.state_signatures(requirement.machines)[0];
        let signature = parsed.items.state_signature(signature);
        assert!(signature.service_reaches.is_empty());
        assert_eq!(signature.service_reach_keyword_source_spans.len(), 1);
        let quiet = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "quiet" => Some(machine),
                _ => None,
            })
            .expect("quiet machine");
        assert!(
            parsed
                .items
                .identifier_path_members(quiet.service_reaches)
                .is_empty()
        );
        assert_eq!(quiet.service_reach_keyword_source_spans.len(), 1);
        assert!(
            parsed
                .items
                .capability_contracts(quiet.contracts)
                .is_empty()
        );

        for (rejected, message) in [
            (
                "reaches none Mark;",
                "`reaches none` admits no service members",
            ),
            (
                "reaches none crashes none crashes Trap;",
                "cannot follow `crashes none`",
            ),
            (
                "reaches none crashes Trap crashes none;",
                "cannot share a header with a `crashes Cause` route",
            ),
            ("reaches none crashes none true;", "admits no route guards"),
        ] {
            let source =
                format!("public boundary trait Mark {{ machine read() -> u64 {rejected} }}");
            let tokens = Lexer::new(&source).tokenize().expect("valid tokens");
            let error = parse_syntax_trees(&tokens).expect_err(rejected);
            super::expect_error(&error, message);
        }
    }
}
pub(crate) mod domains_and_transitions {
    use super::inline_assembly::instruction_statements;
    use crate::parser::parse_syntax_trees;
    use language_core::ReferenceAccess;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::BinaryOperator;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::Item;
    use syntax_trees::statement::StatementNode;
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceNode;

    fn assert_expression_count(
        trees: &syntax_trees::SyntaxTrees,
        handles: arena::HandleSpan<syntax_trees::expression::ExpressionHandle>,
        expected: usize,
    ) {
        assert_eq!(
            trees.expressions.expression_handles(handles).len(),
            expected
        );
    }

    fn assert_path_member_count(
        trees: &syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
        expected: usize,
    ) {
        assert_eq!(trees.items.identifier_path_members(path).len(), expected);
    }

    #[test]
    fn logical_domain_subject_preserves_ordinary_machine_operands() {
        let source = r#"
        data Player { health: u64; }
        domain Player::Alive requires self.health > 0;
        machine health(player: Player) -> u64 { player.health }
    "#;
        let parsed = super::parse_source_verbose(
            source,
            "tokenize logical subject",
            "parse logical subject",
        );
        let mut copied = syntax_trees::SyntaxTrees::default();
        copied.extend_from(&parsed);
        for trees in [&parsed, &copied] {
            super::root_domain(trees, "domain");
            for name in ["self", "player"] {
                assert_eq!(trees.expressions.iter_expressions().filter(|(_, node)| {
                let ExpressionNode::Name(path) = node else { return false; };
                matches!(trees.expressions.identifier_path_members(*path), [member] if member.as_str(trees) == name)
            }).count(), 1, "logical and ordinary subjects remain separate through copying");
            }
        }
    }

    #[test]
    fn logical_domain_subject_preserves_generic_references() {
        let tokens =
            Lexer::new("domain<const Limit: u64> u64::AtMost<Limit> requires self <= Limit;")
                .tokenize()
                .expect("tokenize indexed domain");
        let parsed = parse_syntax_trees(&tokens).expect("parse indexed logical subject");
        for name in ["self", "Limit"] {
            assert!(parsed.expressions.iter_expressions().any(|(_, node)| {
            let ExpressionNode::Name(path) = node else { return false; };
            matches!(parsed.expressions.identifier_path_members(*path), [member] if member.as_str(&parsed) == name)
        }));
        }
    }

    #[test]
    fn rejects_every_authored_domain_subject_shape() {
        for source in [
            "domain u64::Positive(value) requires value > 0;",
            "domain u64::Positive(self) requires self > 0;",
            "domain u64::Positive(value: u64) requires value > 0;",
            "domain u64::Positive(left, right) requires left > right;",
            "domain<const Limit: u64> u64::AtMost<Limit>(Limit) requires Limit > 0;",
            "domain u64::Tag(unused);",
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize retired subject");
            assert!(parse_syntax_trees(&tokens).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn accepts_exit_contract_clauses_on_nested_machines() {
        let source = r#"
        machine walk() {
            machine done()
            ensures
                true
            {
            }
        }
    "#;

        super::parse_source(source, "nested machines have ordinary exit contracts");
    }

    #[test]
    fn parses_machine_termination_clauses() {
        let source = r#"
        machine walk(items: &[Item], remaining: usize)
        terminates by remaining -> Nat::Descending
        {
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");

        assert_expression_count(&parsed, machine.ranking_subjects, 1);
        assert_path_member_count(&parsed, machine.ranking_view, 2);
    }

    #[test]
    fn parses_machine_termination_tuple_subjects() {
        let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by (index, limit) -> Nat::BoundedDistance
        {
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");

        assert_expression_count(&parsed, machine.ranking_subjects, 2);
        assert_path_member_count(&parsed, machine.ranking_view, 2);
    }

    #[test]
    fn parses_machine_termination_argumented_view() {
        let source = r#"
        machine walk(limit: usize, index: usize)
        terminates by index -> Nat::IncreasingTo(limit)
        {
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");

        assert!(!machine.terminates_guarantee);
        assert_expression_count(&parsed, machine.ranking_subjects, 1);
        assert_path_member_count(&parsed, machine.ranking_view, 2);
        assert_expression_count(&parsed, machine.ranking_view_arguments, 1);
    }

    #[test]
    fn parses_trait_requirement_termination_guarantee() {
        let source = r#"
        trait Worker<Work> {
            machine run(value: &mut Work, n: u64) -> u64 terminates;
            machine peek(value: &Work) -> u64;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let trait_definition = super::root_trait(&parsed, "trait root item");

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
        let source = r#"
        trait Worker<Work> {
            machine run(value: &mut Work, n: u64) -> u64 terminates by n;
        }
        "#;

        let error = super::parse_tokenized_error(source, "the witness must be rejected");
        super::expect_error(&error, "does not belong on a bodyless requirement");
    }

    #[test]
    fn parses_data_default_domain_requires_clause() {
        let retired = super::parse_tokenized_error(
            "data MemoryMap where count <= len { len: u32; count: u32; }",
            "retired data `where` must reject",
        );
        super::expect_error(&retired, "expected `{`, found identifier `where`");
        let source = r#"
        data MemoryMap
        requires
            count <= len,
            stride >= 40,
        {
            len: u32;
            stride: u32;
            count: u32;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let data = super::root_data(&parsed, "data root item");

        assert_eq!(parsed.items.proof_facts(data.where_facts).len(), 2);
        assert_eq!(parsed.items.data_members(data.members).len(), 3);
    }

    #[test]
    fn retains_complete_default_domain_predicate() {
        let source = r#"
        data Interval
            requires (self in Interval::Empty)
               || ((self in Interval::Range) && self.lo <= self.hi)
        {
            case Empty;
            case Range(lo: u64, hi: u64);
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let data = super::root_data(&parsed, "data root item");

        let mut fact_counts = parsed
            .items
            .data_members(data.members)
            .iter()
            .map(|member| match member {
                DataMember::Variant(variant) => parsed.items.proof_facts(variant.where_facts).len(),
                _ => panic!("expected only variant members"),
            });
        assert_eq!(
            fact_counts.next(),
            Some(0),
            "payload-less case without `where`"
        );
        assert_eq!(fact_counts.next(), Some(0), "no case-local predicate copy");
        assert!(fact_counts.next().is_none());
    }

    #[test]
    fn rejects_case_local_requires_clause() {
        let error = super::parse_source_error(
            "data Interval { case Range(lo: u64, hi: u64) requires lo <= hi; }",
            "tokenize retired case predicate",
            "case-local requires must reject",
        );
        assert!(
            error
                .message
                .contains("case-local predicate clauses are retired"),
            "got: {}",
            error.message
        );
    }

    #[test]
    fn parses_value_and_policy_domain_chain_as_one_constrained_type() {
        let source = r#"
        data Sample {
            value: f32 in Finite & Saturating;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let data = super::root_data(&parsed, "data root item");
        let field = parsed
            .items
            .data_members(data.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) => Some(field),
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
                TypeConstraintNode::Domain(name),
                TypeConstraintNode::ArithmeticDomain(
                    numerics::arithmetic::ArithmeticDomain::Saturating
                )
            ] if name.name.as_str(&parsed) == "Finite"
        ));
    }

    #[test]
    fn preserves_open_index_operator_expression_in_domain_argument() {
        let source = r#"
        data Unit {}
        domain<T, const U: Unit> T::Quantity<U>;
        data Rate<const A: Unit, const B: Unit> {
            value: f64 in Quantity<A / B>;
        }
        "#;

        let parsed = super::parse_source(source, "open index expression should parse");
        let rate = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) if data.name.as_str(&parsed) == "Rate" => Some(data),
                _ => None,
            })
            .expect("Rate data");
        let field = parsed
            .items
            .data_members(rate.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .expect("Rate::value field");
        let TypeReferenceNode::Constrained { constraints, .. } =
            parsed.type_references.type_reference(field.type_reference)
        else {
            panic!("quantity should be a constrained carrier");
        };
        let [TypeConstraintNode::Domain(domain)] = parsed.type_references.constraints(*constraints)
        else {
            panic!("quantity domain constraint");
        };
        let [argument] = parsed
            .type_references
            .type_reference_handles(domain.arguments)
        else {
            panic!("one quantity index");
        };
        let TypeReferenceNode::ConstExpression(expression) =
            parsed.type_references.type_reference(*argument)
        else {
            panic!("A / B should remain an open const expression");
        };
        let ExpressionNode::Binary(binary) = parsed.expressions.expression(*expression) else {
            panic!("open index should retain the divide expression");
        };
        assert_eq!(binary.operator, BinaryOperator::Divide);
        assert_eq!(parsed.display_expression_name(binary.left), "A");
        assert_eq!(parsed.display_expression_name(binary.right), "B");
    }

    #[test]
    fn parses_compiler_owned_carry_atoms_and_expands_portable() {
        let source = r#"
        data Sample {
            local: u64 in Carry::MovableAddress;
            portable: u64 in Carry::Portable;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let data = super::root_data(&parsed, "data root item");
        let fields = parsed
            .items
            .data_members(data.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();

        let names = |field: &syntax_trees::item::DataField| {
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
                    TypeConstraintNode::Domain(name) => name.name.as_str(&parsed).to_owned(),
                    other => panic!("carry permission became {other:?}"),
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(names(fields[0]), ["Carry::MovableAddress"]);
        assert_eq!(
            names(fields[1]),
            language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
        );
    }

    #[test]
    fn expands_carry_portable_contract_guarantee_to_four_atomic_facts() {
        let source = r#"
        machine grant(value: u64) -> u64
        ensures
            result in Carry::Portable
        {
            transition { _ -> value }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let [contract] = parsed.items.capability_contracts(machine.contracts) else {
            panic!("one ensures contract");
        };
        let domains = parsed
            .items
            .proof_facts(contract.facts)
            .iter()
            .map(|fact| match fact {
                syntax_trees::item::ProofFact::Membership(membership) => parsed
                    .items
                    .identifier_path_members(membership.domain)
                    .iter()
                    .map(|member| member.as_str(&parsed))
                    .collect::<Vec<_>>()
                    .join("::"),
                other => panic!("portable atom became {other:?}"),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            domains,
            language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
        );
    }

    #[test]
    fn parses_slice_range_indexing_into_range_expression() {
        let source = r#"
        data Main {}

        machine Main::main(main: &mut Main) -> usize {
            let values: [usize; 4] = [1, 2, 3, 4];
            let view: &[usize] = values.as_slice();
            let tail: &[usize] = view[1..];
            tail.len
        }
        "#;

        let parsed = super::parse_source(source, "parse should accept slice range surface");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
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
            parsed.display_expression_name(indexed.index),
            "1..",
            "unexpected range display"
        );
        assert!(range.start.is_valid(), "expected explicit range start");
        assert!(!range.end.is_valid(), "expected open-ended range");
        let operator_span = parsed.expressions.source_span(local.initial_value).span;
        assert_eq!(
            &source[operator_span.start..operator_span.end],
            "[",
            "indexed syntax must retain its authored operator token"
        );
    }

    #[test]
    fn parses_structural_recast_targets_as_type_references() {
        let source = r#"
        machine inspect(bytes: [u8; 4]) {
            let fixed: &[u8; 4] = &bytes as &[u8; 4];
            let slice: &[u8] = &bytes as &[u8];
        }
        "#;

        let parsed = super::parse_source(source, "array and slice recast targets should parse");
        let machine = super::root_machine(&parsed, "machine root item");
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

        let ExpressionNode::Borrow(fixed_borrow) =
            parsed.expressions.expression(locals[0].initial_value)
        else {
            panic!("fixed-array initializer should retain its shared borrow");
        };
        assert_eq!(fixed_borrow.access, ReferenceAccess::Shared);
        let ExpressionNode::Cast(fixed) = parsed.expressions.expression(fixed_borrow.target) else {
            panic!("fixed-array borrow target should be a recast");
        };
        let TypeReferenceNode::FixedArray {
            element_type,
            length: syntax_trees::types::FixedArrayLength::Literal(4),
        } = parsed.type_references.type_reference(fixed.target_type)
        else {
            panic!("fixed-array target should retain its structural type");
        };
        assert!(matches!(
            parsed.type_references.type_reference(*element_type),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "u8"
        ));

        let ExpressionNode::Borrow(slice_borrow) =
            parsed.expressions.expression(locals[1].initial_value)
        else {
            panic!("slice initializer should retain its shared borrow");
        };
        assert_eq!(slice_borrow.access, ReferenceAccess::Shared);
        let ExpressionNode::Cast(slice) = parsed.expressions.expression(slice_borrow.target) else {
            panic!("slice borrow target should be a recast");
        };
        let TypeReferenceNode::Slice { element_type } =
            parsed.type_references.type_reference(slice.target_type)
        else {
            panic!("slice target should retain its structural type");
        };
        assert!(matches!(
            parsed.type_references.type_reference(*element_type),
            TypeReferenceNode::Named(name) if name.as_str(&parsed) == "u8"
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

        let parsed = super::parse_source(source, "parse should succeed");
        let trait_definition = super::root_trait(&parsed, "trait root item");
        let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
        let signature = parsed.items.state_signature(signature_handle);
        let contracts = parsed.items.capability_contracts(signature.contracts);
        let service_reaches = parsed
            .items
            .identifier_path_members(signature.service_reaches);

        assert_eq!(contracts.len(), 2);
        for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
            let span = contract
                .keyword_source_span
                .expect("trait contract keyword span");
            assert_eq!(&source[span.span.start..span.span.end], keyword);
        }
        assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
        assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
        assert_eq!(service_reaches.len(), 1);
        assert_eq!(service_reaches[0].as_str(&parsed), "Filesystem");
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

        machine Main::main(main: &mut Main) {
            transition main.player {
                Player::Alive -> done()
                Player { health, .. } if health > 5 -> done()
                _ -> done()
            }

            machine done() {}
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let entry = super::entry_state_handle(&parsed, machine);
        let state = parsed.items.state(entry);
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
        assert!(matches!(
            transition.guard,
            syntax_trees::statement::TransitionGuardNode::Always
        ));
        let [pattern] = parsed.statements.transition_patterns(transition.patterns) else {
            panic!("data-pattern arm retains one subject pattern");
        };
        assert!(pattern.is_structural);
        let ExpressionNode::Binary(comparison) =
            parsed.expressions.expression(pattern.pattern.guard)
        else {
            panic!("data-pattern guard should be a comparison");
        };
        assert_eq!(comparison.operator, BinaryOperator::Greater);
        assert!(matches!(
            parsed.expressions.expression(comparison.left),
            ExpressionNode::Member(_)
        ));
    }

    #[test]
    fn parses_outcome_arm_payload_and_erased_proof_selectors() {
        let source = r#"
        data Outcome { case Success(value: i32); case Failure; }
        machine inspect(result: Outcome) -> i32 {
            transition result {
                Outcome::Success { value; selected: local, shorthand } -> value
                Outcome::Failure { } -> 0
            }
        }
    "#;
        let parsed = super::parse_source_verbose(
            source,
            "tokenize outcome arm",
            "outcome selector arm should parse",
        );
        let transition = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "inspect" => {
                    Some(machine)
                }
                _ => None,
            })
            .into_iter()
            .flat_map(|machine| parsed.items.state_handles(machine.states))
            .flat_map(|state| {
                parsed
                    .items
                    .statements(parsed.items.state(*state).statements)
            })
            .find_map(|statement| match parsed.statements.statement(*statement) {
                StatementNode::Transition(transition) if !transition.proof_selectors.is_empty() => {
                    Some(transition)
                }
                _ => None,
            })
            .expect("one transition arm with erased selectors");
        let selectors = parsed
            .statements
            .outcome_proof_selectors(transition.proof_selectors);
        assert_eq!(selectors.len(), 2);
        assert_eq!(selectors[0].output_field.as_str(&parsed), "selected");
        assert_eq!(selectors[0].binding.as_str(&parsed), "local");
        assert_eq!(selectors[1].output_field.as_str(&parsed), "shorthand");
        assert_eq!(selectors[1].binding.as_str(&parsed), "shorthand");
    }

    #[test]
    fn outcome_proof_selectors_do_not_change_the_runtime_statement_plan() {
        let sources = [
            r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; selected: local } -> {}
                Outcome::Failure { } -> {}
            }
        }
        "#,
            r#"
        data Outcome { case Success; case Failure; }
        machine produce() -> Outcome { Outcome::Success }
        machine inspect() {
            transition produce() {
                Outcome::Success { ; } -> {}
                Outcome::Failure { } -> {}
            }
        }
        "#,
        ];
        let mut plans = Vec::new();
        for source in sources {
            let parsed = super::parse_source_verbose(
                source,
                "tokenize outcome arm",
                "outcome arm should parse",
            );
            let inspect = parsed
                .root_items()
                .find_map(|item| match item {
                    Item::Machine(machine) if machine.name.as_str(&parsed) == "inspect" => {
                        Some(machine)
                    }
                    _ => None,
                })
                .expect("inspect machine");
            let entry = parsed
                .items
                .state_handles(inspect.states)
                .first()
                .copied()
                .expect("inspect entry state");
            let statements = parsed
                .items
                .statements(parsed.items.state(entry).statements);
            let producer_locals = statements
                .iter()
                .filter(|statement| {
                    let StatementNode::LocalData(local) = parsed.statements.statement(**statement)
                    else {
                        return false;
                    };
                    matches!(
                        parsed.expressions.expression(local.initial_value),
                        ExpressionNode::Call(call) if call.target.as_str(&parsed) == "produce"
                    )
                })
                .count();
            let transitions = statements
                .iter()
                .filter(|statement| {
                    matches!(
                        parsed.statements.statement(**statement),
                        StatementNode::Transition(_)
                    )
                })
                .count();
            let producer_calls = parsed
                .expressions
                .iter_expressions()
                .filter(|(_, expression)| {
                    matches!(
                        expression,
                        ExpressionNode::Call(call) if call.target.as_str(&parsed) == "produce"
                    )
                })
                .count();
            assert_eq!(producer_locals, 1, "the captured producer call is retained");
            assert_eq!(
                producer_calls, 1,
                "the producer call is evaluated exactly once"
            );
            plans.push((statements.len(), producer_locals, transitions));
        }
        assert_eq!(
            plans[0], plans[1],
            "erased proof selectors must not add or remove runtime statements"
        );
    }

    #[test]
    fn rejects_duplicate_and_defers_noncase_outcome_proof_selectors() {
        let error = super::parse_source_error(
            "machine inspect(result: Outcome) { transition result { Outcome::Success { ; selected: local, selected: other } -> {} } }",
            "tokenize rejected selector arm",
            "duplicate selector arm must reject",
        );
        assert!(
            error
                .message
                .contains("outcome proof selector `selected` is bound more than once"),
            "got: {}",
            error.message
        );

        let parsed = super::parse_source_verbose(
            "machine inspect(result: Record) { transition result { Record { ; selected: local } -> {} } }",
            "tokenize non-case selector arm",
            "head classification belongs to declaration selection",
        );
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        let transition = parsed
            .items
            .statements(state.statements)
            .iter()
            .find_map(|handle| match parsed.statements.statement(*handle) {
                StatementNode::Transition(transition) => Some(transition),
                _ => None,
            })
            .expect("selector arm");
        let selectors = parsed
            .statements
            .outcome_proof_selectors(transition.proof_selectors);
        assert_eq!(selectors.len(), 1);
        assert_eq!(selectors[0].output_field.as_str(&parsed), "selected");
        assert_eq!(selectors[0].binding.as_str(&parsed), "local");
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

        let parsed = super::parse_source(source, "field-value pattern should parse");
        let machine = super::root_machine(&parsed, "machine root item");
        let state = super::entry_state(&parsed, machine);
        let transition = parsed
            .items
            .statements(state.statements)
            .iter()
            .filter_map(|handle| match parsed.statements.statement(*handle) {
                syntax_trees::statement::StatementNode::Transition(transition) => Some(transition),
                _ => None,
            })
            .next()
            .expect("first transition arm");
        assert!(matches!(
            transition.guard,
            syntax_trees::statement::TransitionGuardNode::Always
        ));
        let [pattern] = parsed.statements.transition_patterns(transition.patterns) else {
            panic!("field-value arm retains one subject pattern");
        };
        assert!(pattern.is_structural);
        let [ok, version] = parsed.expressions.match_fields(pattern.pattern.fields) else {
            panic!("both authored fields are retained");
        };
        assert_eq!(ok.member.as_str(&parsed), "ok");
        let syntax_trees::expression::MatchFieldAction::Equal(value) = &ok.action else {
            panic!("field-value pattern should compare the subject member");
        };
        assert!(matches!(
            parsed.expressions.expression(*value),
            ExpressionNode::Integer(_)
        ));
        assert_eq!(version.member.as_str(&parsed), "version");
        assert!(matches!(
            version.action,
            syntax_trees::expression::MatchFieldAction::Bind(_)
        ));
        let syntax_trees::statement::TransitionTargetNode::Value(target) =
            parsed.statements.transition_target(transition.target)
        else {
            panic!("value arm");
        };
        assert!(matches!(
            parsed.expressions.expression(*target),
            ExpressionNode::Member(_)
        ));
    }

    #[test]
    fn parses_asm_jmp_block_as_transition_statement() {
        let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(main: &mut Main) {
            asm {
                jmp other()
            }

            machine other() {}
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let entry = super::entry_state_handle(&parsed, machine);
        let state = parsed.items.state(entry);
        let instructions =
            instruction_statements(&parsed, parsed.items.statements(state.statements));
        let statement = *instructions.first().expect("asm transition");
        assert!(matches!(
            parsed.statements.statement(statement),
            StatementNode::Transition(_)
        ));
    }

    #[test]
    fn parses_asm_mnemonics_as_intrinsic_calls() {
        let source = r#"
        data Main {
            port: u16;
        }

        machine Main::main(main: &mut Main) {
            let mut status: u8 = 0;
            asm { hlt }
            asm { out main.port, status }
            asm { in status, main.port }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let entry = super::entry_state_handle(&parsed, machine);
        let state = parsed.items.state(entry);
        let statements = instruction_statements(&parsed, parsed.items.statements(state.statements));
        assert_eq!(statements.len(), 4, "let + three asm statements");

        let StatementNode::Call(hlt) = parsed.statements.statement(statements[1]).clone() else {
            panic!("asm {{ hlt }} should desugar to a call statement");
        };
        assert_eq!(hlt.target.as_str(&parsed), "asm#hlt");
        assert!(hlt.receiver.is_empty());
        assert_eq!(hlt.arguments.count(), 0);

        let StatementNode::Call(out) = parsed.statements.statement(statements[2]).clone() else {
            panic!("asm {{ out .. }} should desugar to a call statement");
        };
        assert_eq!(out.target.as_str(&parsed), "asm#port_out");
        assert_eq!(out.arguments.count(), 2);

        let StatementNode::Assignment(read) = parsed.statements.statement(statements[3]).clone()
        else {
            panic!("asm {{ in .. }} should desugar to an assignment");
        };
        let ExpressionNode::Call(port_in) = parsed.expressions.expression(read.value).clone()
        else {
            panic!("asm {{ in .. }} assignment value should be the intrinsic call");
        };
        assert_eq!(port_in.target.as_str(&parsed), "asm#port_in");
        assert_eq!(port_in.arguments.count(), 1);
        assert!(!port_in.receiver.is_valid());
    }
}
pub(crate) mod expression_stack {
    use crate::grammar::expressions::parse_expression::parse_expression_handle;
    use source_files_to_tokens::Lexer;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };

    #[test]
    fn nested_aggregates_and_grouped_unary_prefixes_use_the_default_stack() {
        let value = (0..64).fold("compute(source)".to_owned(), |value, _| {
            format!("~({value})")
        });
        let mut value = format!("Layer0 {{ value: {value}, audit: compute(other) }}");
        for layer in 1..=8 {
            value = format!("Layer{layer} {{ children: [{value}], audit: compute(other) }}");
        }
        let tokens = Lexer::new(&value)
            .tokenize()
            .expect("tokenize nested values");
        let mut parsed = syntax_trees::SyntaxTrees::new(source::SourceId::default());
        let retained_source = crate::tokens::token_cursor::Input::retain_source(&tokens);
        let input = crate::tokens::token_cursor::Input::new(
            source::SourceId::default(),
            &tokens,
            &retained_source,
            &mut parsed,
        );
        let (_, rest) = parse_expression_handle(&mut parsed, input)
            .expect("parse nested values on the default stack");
        assert!(rest.tokens.is_empty());
        let count = |predicate: fn(&ExpressionNode) -> bool| {
            parsed
                .expressions
                .iter_expressions()
                .filter(|(_, expression)| predicate(expression))
                .count()
        };
        assert_eq!(
            count(|expression| matches!(expression, ExpressionNode::Unary(_))),
            64
        );
        assert_eq!(
            count(|expression| matches!(expression, ExpressionNode::StructLiteral(_))),
            9
        );
        assert_eq!(
            count(|expression| matches!(expression, ExpressionNode::ArrayLiteral(_))),
            8
        );
        assert_eq!(
            count(|expression| matches!(expression, ExpressionNode::Call(_))),
            10
        );
    }

    fn parse_expression(source: &str) -> (SyntaxTrees, ExpressionHandle) {
        let source_id = source::SourceId::default();
        let tokens = Lexer::new(source).tokenize().expect("tokenize expression");
        let mut parsed = SyntaxTrees::new(source_id);
        let retained_source = crate::tokens::token_cursor::Input::retain_source(&tokens);
        let input = crate::tokens::token_cursor::Input::new(
            source_id,
            &tokens,
            &retained_source,
            &mut parsed,
        );
        let (expression, rest) =
            parse_expression_handle(&mut parsed, input).expect("parse expression");
        assert!(
            rest.tokens.is_empty(),
            "expression must consume all input: {source}"
        );
        (parsed, expression)
    }

    #[test]
    fn operator_stack_retains_precedence_associativity_and_operator_spans() {
        let source = "a || b && c == d < e | f ^ g & h << i + j * k - l / m % n >> o";
        let (parsed, _) = parse_expression(source);
        let actual: Vec<_> = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Binary(binary) = expression else {
                    return None;
                };
                let span = parsed.expressions.source_span(handle).span;
                Some((binary.operator, &source[span.start..span.end]))
            })
            .collect();
        assert_eq!(
            actual,
            [
                (BinaryOperator::Multiply, "*"),
                (BinaryOperator::Add, "+"),
                (BinaryOperator::Divide, "/"),
                (BinaryOperator::Modulo, "%"),
                (BinaryOperator::Subtract, "-"),
                (BinaryOperator::ShiftLeft, "<<"),
                (BinaryOperator::ShiftRight, ">>"),
                (BinaryOperator::BitwiseAnd, "&"),
                (BinaryOperator::BitwiseXor, "^"),
                (BinaryOperator::BitwiseOr, "|"),
                (BinaryOperator::Less, "<"),
                (BinaryOperator::Equal, "=="),
                (BinaryOperator::And, "&&"),
                (BinaryOperator::Or, "||"),
            ]
        );
        let (parsed, root) = parse_expression("a - b - c");
        let ExpressionNode::Binary(root) = parsed.expressions.expression(root) else {
            panic!("binary root");
        };
        assert!(matches!(
            parsed.expressions.expression(root.left),
            ExpressionNode::Binary(_)
        ));
        assert_eq!(parsed.display_expression_name(root.right), "c");
    }

    /// A suffix chooses the policy of one operation and nothing else: every
    /// suffixed form parses as its base operator with that operator's
    /// precedence, and the operator span covers the suffix.
    #[test]
    fn policy_suffixes_keep_the_base_operator_and_record_its_policy() {
        use numerics::arithmetic::ArithmeticDomain::{Exact, Saturating, Trapping, Wrapping};
        let source = "a +% b *| c -! d /% e %| f <<! g >>% h >>| i >>! j >> k + l";
        let (parsed, _) = parse_expression(source);
        let actual: Vec<_> = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Binary(binary) = expression else {
                    return None;
                };
                let span = parsed.expressions.source_span(handle).span;
                Some((
                    binary.operator,
                    binary.policy,
                    &source[span.start..span.end],
                ))
            })
            .collect();
        assert_eq!(
            actual,
            [
                (BinaryOperator::Multiply, Saturating, "*|"),
                (BinaryOperator::Add, Wrapping, "+%"),
                (BinaryOperator::Divide, Wrapping, "/%"),
                (BinaryOperator::Modulo, Saturating, "%|"),
                (BinaryOperator::Subtract, Trapping, "-!"),
                (BinaryOperator::ShiftLeft, Trapping, "<<!"),
                (BinaryOperator::ShiftRight, Wrapping, ">>"),
                (BinaryOperator::ShiftRight, Saturating, ">>"),
                (BinaryOperator::ShiftRight, Trapping, ">>"),
                (BinaryOperator::Add, Exact, "+"),
                (BinaryOperator::ShiftRight, Exact, ">>"),
            ]
        );
    }

    /// Lexing takes the longest operator, so only a space keeps a logical
    /// negation apart from a trapping suffix; `>>` reads its suffix from the
    /// adjacent token for the same reason.
    #[test]
    fn a_space_separates_a_negated_operand_from_a_trapping_suffix() {
        use numerics::arithmetic::ArithmeticDomain::{Exact, Trapping};
        for (source, policy, negated) in [
            ("a +!b", Trapping, false),
            ("a + !b", Exact, true),
            ("a >>!b", Trapping, false),
            ("a >> !b", Exact, true),
        ] {
            let (parsed, root) = parse_expression(source);
            let ExpressionNode::Binary(binary) = parsed.expressions.expression(root) else {
                panic!("binary root in `{source}`");
            };
            assert_eq!(binary.policy, policy, "{source}");
            assert_eq!(
                matches!(
                    parsed.expressions.expression(binary.right),
                    ExpressionNode::Unary(_)
                ),
                negated,
                "{source}"
            );
        }
    }

    #[test]
    fn membership_remains_a_separate_operand_grammar() {
        let (parsed, _) = parse_expression("a + b in First & Second | Third == c || d");
        let subjects: Vec<_> = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Membership(membership) = expression else {
                    return None;
                };
                Some(membership.value)
            })
            .collect();
        assert_eq!(subjects.len(), 3);
        assert!(subjects.iter().all(|subject| *subject == subjects[0]));
        let operators: Vec<_> = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Binary(binary) = expression else {
                    return None;
                };
                Some(binary.operator)
            })
            .collect();
        assert_eq!(
            operators,
            [
                BinaryOperator::Add,
                BinaryOperator::And,
                BinaryOperator::Or,
                BinaryOperator::Equal,
                BinaryOperator::Or,
            ]
        );
        parse_expression("(a in First) + 1");
        let tokens = Lexer::new("machine test() { let value: u64 = a in First + 1; }")
            .tokenize()
            .expect("tokenize ungrouped membership");
        assert!(crate::parser::parse_syntax_trees(&tokens).is_err());
    }

    #[test]
    fn iterative_prefixes_preserve_borrows_literal_folding_and_call_acknowledgements() {
        let (parsed, root) = parse_expression("&write -7");
        let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(root) else {
            panic!("borrow");
        };
        assert_eq!(borrow.access, language_core::ReferenceAccess::WriteOnly);
        let ExpressionNode::Integer(literal) = parsed.expressions.expression(borrow.target) else {
            panic!("negated integer literal");
        };
        assert_eq!(*literal, numerics::literals::IntegerLiteral::from_value(-7));
        let (parsed, root) = parse_expression("&write move value");
        let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(root) else {
            panic!("borrow");
        };
        assert_eq!(borrow.access, language_core::ReferenceAccess::WriteOnly);
        assert_eq!(parsed.display_expression_name(borrow.target), "value");
        let (parsed, root) = parse_expression("suspend block perform()");
        let ExpressionNode::Call(call) = parsed.expressions.expression(root) else {
            panic!("call");
        };
        assert!(call.operational_acknowledgement.acknowledges_suspend);
        assert!(call.operational_acknowledgement.acknowledges_block);
        for (source, expected) in [
            ("suspend suspend perform()", "duplicate `suspend`"),
            ("block block perform()", "duplicate `block`"),
            ("block suspend perform()", "canonical order"),
            ("suspend ~perform()", "immediately before a call"),
            ("block 7", "immediately before a call"),
        ] {
            let source = format!("machine test() {{ let value: u64 = {source}; }}");
            let tokens = Lexer::new(&source)
                .tokenize()
                .expect("tokenize invalid prefix");
            let error =
                crate::parser::parse_syntax_trees(&tokens).expect_err("reject invalid prefix");
            assert!(error.message.contains(expected), "{}", error.message);
        }
        parse_expression("suspend()");
        parse_expression("block()");
        parse_expression(&format!("{}value", "~".repeat(4096)));
    }

    #[test]
    fn long_operator_chains_keep_bounded_parser_stack_and_exact_prefix_order() {
        let source = format!("{}value", "~".repeat(4096));
        let (parsed, mut expression) = parse_expression(&source);
        for offset in 0..4096 {
            let span = parsed.expressions.source_span(expression).span;
            assert_eq!((span.start, span.end), (offset, offset + 1));
            let ExpressionNode::Unary(unary) = parsed.expressions.expression(expression) else {
                panic!("each prefix retains its operand and source position");
            };
            assert_eq!(unary.operator, UnaryOperator::BitwiseNot);
            expression = unary.operand;
        }
        assert_eq!(parsed.display_expression_name(expression), "value");

        let source = std::iter::repeat_n("a | b ^ c & d << e + f * g", 1024)
            .collect::<Vec<_>>()
            .join(" | ");
        let (parsed, _) = parse_expression(&source);
        assert_eq!(
            parsed
                .expressions
                .iter_expressions()
                .filter(|(_, expression)| { matches!(expression, ExpressionNode::Binary(_)) })
                .count(),
            7 * 1024 - 1,
        );
    }
}
pub(crate) mod inline_assembly {
    use crate::parser::parse_syntax_trees;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::statement::StatementHandle;
    use syntax_trees::statement::StatementNode;

    pub(crate) fn instruction_statements(
        parsed: &syntax_trees::SyntaxTrees,
        statements: &[StatementHandle],
    ) -> Vec<StatementHandle> {
        let mut instructions = Vec::new();
        for handle in statements {
            match parsed.statements.statement(*handle) {
                StatementNode::Assembly(graph) => {
                    let blocks = parsed
                        .statements
                        .assembly_blocks(parsed.statements.assembly(*graph).blocks);
                    assert_eq!(blocks.len(), 1, "one implicit catalog block");
                    assert!(blocks[0].requires.is_empty() && blocks[0].ensures.is_empty());
                    instructions.extend_from_slice(
                        parsed
                            .statements
                            .assembly_instructions(blocks[0].instructions),
                    );
                }
                _ => instructions.push(*handle),
            }
        }
        instructions
    }

    fn main_statements(parsed: &syntax_trees::SyntaxTrees) -> Vec<StatementHandle> {
        let machine = super::root_machine(parsed, "machine root item");
        let entry = super::entry_state_handle(parsed, machine);
        let statements = parsed
            .items
            .statements(parsed.items.state(entry).statements);
        instruction_statements(parsed, statements)
    }

    #[test]
    fn parses_x86_memory_fences_as_zero_operand_intrinsics() {
        let source = r#"
        data Main {}

        machine Main::main(main: &mut Main) {
            asm clobbers none { lfence; sfence; mfence }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let statements = main_statements(&parsed);

        assert_eq!(statements.len(), 3);
        for (statement, target) in statements
            .iter()
            .zip(["asm#lfence", "asm#sfence", "asm#mfence"])
        {
            let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
                panic!("fence should desugar to a call statement");
            };
            assert_eq!(call.target.as_str(&parsed), target);
            assert!(call.receiver.is_empty());
            assert_eq!(call.arguments.count(), 0);
        }
    }

    #[test]
    fn parses_x86_interrupt_control_as_zero_operand_intrinsics() {
        let source = r#"
        data Main {}

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm clobbers none { cli; sti }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let statements = main_statements(&parsed);

        assert_eq!(statements.len(), 2);
        for (statement, target) in statements.iter().zip(["asm#cli", "asm#sti"]) {
            let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
                panic!("interrupt control should desugar to a call statement");
            };
            assert_eq!(call.target.as_str(&parsed), target);
            assert!(call.receiver.is_empty());
            assert_eq!(call.arguments.count(), 0);
        }
    }

    #[test]
    fn parses_x86_flags_as_explicit_value_operations() {
        let source = r#"
        data Main { saved: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm clobbers r10, r15 {
                pushfq main.saved;
                popfq main.saved
            }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let statements = main_statements(&parsed);
        assert_eq!(statements.len(), 2);

        let StatementNode::Assignment(snapshot) = parsed.statements.statement(statements[0]) else {
            panic!("pushfq should desugar to a destination assignment");
        };
        let ExpressionNode::Call(call) = parsed.expressions.expression(snapshot.value) else {
            panic!("pushfq assignment should contain the snapshot intrinsic");
        };
        assert_eq!(call.target.as_str(&parsed), "asm#pushfq");
        assert_eq!(call.arguments.count(), 0);

        let StatementNode::Call(restore) = parsed.statements.statement(statements[1]) else {
            panic!("popfq should desugar to a call statement");
        };
        assert_eq!(restore.target.as_str(&parsed), "asm#popfq");
        assert_eq!(restore.arguments.count(), 1);
    }

    #[test]
    fn parses_x86_msr_as_structured_value_operations() {
        let source = r#"
        data Main { value: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm clobbers rax, rcx, rdx, r10, r11, r15 {
                rdmsr main.value, 3221225600;
                wrmsr 3221225600, main.value
            }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let statements = main_statements(&parsed);
        assert_eq!(statements.len(), 2);

        let StatementNode::Assignment(read) = parsed.statements.statement(statements[0]) else {
            panic!("rdmsr should desugar to a destination assignment");
        };
        let ExpressionNode::Call(read_call) = parsed.expressions.expression(read.value) else {
            panic!("rdmsr assignment should contain the read intrinsic");
        };
        assert_eq!(read_call.target.as_str(&parsed), "asm#rdmsr");
        assert_eq!(read_call.arguments.count(), 1);

        let StatementNode::Call(write) = parsed.statements.statement(statements[1]) else {
            panic!("wrmsr should desugar to a call statement");
        };
        assert_eq!(write.target.as_str(&parsed), "asm#wrmsr");
        assert_eq!(write.arguments.count(), 2);
    }

    #[test]
    fn parses_x86_control_registers_as_structured_value_operations() {
        let source = r#"
        data Main { value: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm clobbers rax, r10, r11, r15 {
                read_cr0 main.value;
                write_cr0 main.value;
                read_cr2 main.value;
                read_cr3 main.value;
                write_cr3 main.value;
                read_cr4 main.value;
                write_cr4 main.value
            }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let statements = main_statements(&parsed);
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
            assert_eq!(call.target.as_str(&parsed), target);
            assert_eq!(call.arguments.count(), 0);
        }

        for (statement_index, target) in [
            (1, "asm#write_cr0"),
            (4, "asm#write_cr3"),
            (6, "asm#write_cr4"),
        ] {
            let StatementNode::Call(write) =
                parsed.statements.statement(statements[statement_index])
            else {
                panic!("control-register write should desugar to call");
            };
            assert_eq!(write.target.as_str(&parsed), target);
            assert_eq!(write.arguments.count(), 1);
        }
    }

    #[test]
    fn parses_multi_instruction_asm_in_nested_machines_and_trait_defaults() {
        let source = r#"
        trait Idle<Processor> {
            machine idle(value: &mut Processor) {
                asm { hlt; hlt }
            }
        }

        data Main {}

        machine Main::main(main: &mut Main) {
            transition { _ -> next() }

            machine next() {
                asm { hlt; hlt }
            }
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let trait_definition = super::root_trait(&parsed, "trait root item");
        let default_signature = parsed
            .items
            .state_signatures(trait_definition.machines)
            .first()
            .map(|handle| parsed.items.state_signature(*handle))
            .expect("default trait signature");
        assert_eq!(
            instruction_statements(
                &parsed,
                parsed.items.statements(default_signature.default_body)
            )
            .len(),
            2,
            "trait default should retain both asm instructions"
        );
        let machine = super::root_machine(&parsed, "machine root item");
        let child = machine
            .nested_machines
            .handles()
            .find_map(|handle| match parsed.items.item(handle) {
                syntax_trees::item::Item::Machine(child) => Some(child),
                _ => None,
            })
            .expect("nested machine");
        let explicit = parsed.items.state_handles(child.states)[0];
        assert_eq!(
            instruction_statements(
                &parsed,
                parsed
                    .items
                    .statements(parsed.items.state(explicit).statements)
            )
            .len(),
            2,
            "nested machine should retain both asm instructions"
        );
    }

    #[test]
    fn rejects_ambiguous_or_empty_multi_instruction_asm_blocks() {
        for (block, expected) in [
            (
                "asm { out main.port, main.value hlt }",
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
                "asm requires true {}",
                "an asm block must contain at least one known instruction",
            ),
        ] {
            let source = format!(
                r#"
            data Main {{
                port: u16;
                value: u8;
            }}

            machine Main::main(main: &mut Main) {{
                {block}
                machine done() {{}}
            }}
            "#
            );
            let error = super::parse_tokenized_error(&source, "asm block should reject");
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
            ("lidt main.value", "deriver-only"),
            ("ret", "creates a hidden control exit"),
            (
                "ldrb x0, main.value",
                "no structured operand provenance/permission contract",
            ),
            (
                "ldur x0, main.value",
                "no structured operand provenance/permission contract",
            ),
        ] {
            let source = format!(
                r#"
            data Main {{ value: i32; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
            );
            let error = super::parse_tokenized_error(&source, "asm instruction should reject");
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains(expected),
                "expected `{expected}` for `{instruction}`, got `{rendered}`"
            );
        }
    }

    #[test]
    fn parses_register_move_as_an_ordinary_checked_assignment() {
        let source = r#"
        data Main {
            value: i32;
            buffer: [u64; 4];
        }

        machine Main::main(main: &mut Main) {
            let mut scratch: u64 = 0;
            asm {
                mov main.value, 3;
                movq scratch, main.buffer[1]
            }
        }
        "#;

        let parsed = super::parse_source(source, "register moves should parse");
        let statements = main_statements(&parsed);

        assert_eq!(statements.len(), 3, "let + two desugared moves");
        let StatementNode::Assignment(first) = parsed.statements.statement(statements[1]) else {
            panic!("mov should desugar to an assignment");
        };
        assert!(matches!(
            parsed.expressions.expression(first.value),
            ExpressionNode::Integer(_)
        ));
        let StatementNode::Assignment(second) = parsed.statements.statement(statements[2]) else {
            panic!("movq should desugar to an assignment");
        };
        assert!(matches!(
            parsed.expressions.expression(second.value),
            ExpressionNode::Indexed(_)
        ));
    }

    #[test]
    fn parses_memory_transfers_as_place_assignments() {
        let source = r#"
        data Main {
            value: u64;
            buffer: [u64; 4];
        }

        machine Main::main(main: &mut Main) {
            asm {
                str main.value, main.buffer[0];
                ldr main.value, main.buffer[0]
            }
        }
        "#;

        let parsed = super::parse_source(source, "memory transfers should parse");
        let statements = main_statements(&parsed);

        assert_eq!(statements.len(), 2, "two desugared transfers");
        let StatementNode::Assignment(store) = parsed.statements.statement(statements[0]) else {
            panic!("str should desugar to an assignment");
        };
        assert!(matches!(
            parsed.expressions.expression(store.target),
            ExpressionNode::Indexed(_)
        ));
        let StatementNode::Assignment(load) = parsed.statements.statement(statements[1]) else {
            panic!("ldr should desugar to an assignment");
        };
        assert!(matches!(
            parsed.expressions.expression(load.value),
            ExpressionNode::Indexed(_)
        ));

        for instruction in [
            "ldr main.value, [main.buffer]",
            "str [main.buffer], main.value",
        ] {
            let source = format!(
                r#"
            data Main {{ value: u64; buffer: [u64; 4]; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
            );
            let error = super::parse_source_error(
                &source,
                "tokenize should succeed",
                "a bracketed transfer operand keeps the bracket refusal",
            );
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains("bracketed `[...]` memory addressing"),
                "expected bracketed-operand refusal for `{instruction}`, got `{rendered}`"
            );
        }
    }

    #[test]
    fn rejects_bracketed_memory_operands_on_register_moves() {
        for instruction in [
            "mov main.value, [main.other]",
            "mov [main.value], main.other",
            "movq main.value, [main.buffer]",
        ] {
            let source = format!(
                r#"
            data Main {{ value: i32; other: i32; buffer: [u64; 4]; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
            );
            let error = super::parse_tokenized_error(&source, "bracketed operand should reject");
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains("bracketed `[...]` memory addressing"),
                "expected bracketed-memory diagnostic for `{instruction}`, got `{rendered}`"
            );
        }
    }

    #[test]
    fn parses_exact_asm_where_clobber_contracts() {
        for block in [
            "asm clobbers none { hlt }",
            "asm clobbers r11, rax, rdx, r10, r15 { out main.port, main.value }",
        ] {
            let source = format!(
                r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(main: &mut Main) {{ {block} }}
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
        machine Main::main(main: &mut Main) {
            asm
                requires main.ready
                clobbers rax, rdx, r10, r11, r15
                ensures main.ready
            { out main.port, main.value }
        }
    "#;
        let parsed = super::parse_source_verbose(source, "tokenize", "asm facts should parse");
        let machine = super::root_machine(&parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let statements = parsed.items.statements(state.statements);
        assert_eq!(statements.len(), 1);
        let StatementNode::Assembly(graph) = parsed.statements.statement(statements[0]) else {
            panic!("one structured assembly scope");
        };
        let blocks = parsed
            .statements
            .assembly_blocks(parsed.statements.assembly(*graph).blocks);
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.requires.count(), 1);
        assert_eq!(block.ensures.count(), 1);
        for expression in parsed
            .statements
            .assembly_expressions(block.requires)
            .iter()
            .chain(parsed.statements.assembly_expressions(block.ensures))
        {
            assert!(matches!(
                parsed.expressions.expression(*expression),
                ExpressionNode::Member(_)
            ));
        }
        let instructions = parsed.statements.assembly_instructions(block.instructions);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(parsed.statements.statement(instructions[0]),
        StatementNode::Call(call) if call.target.as_str(&parsed) == "asm#port_out"));
    }

    #[test]
    fn rejects_inexact_asm_clobber_contracts() {
        for (block, expected) in [
            (
                "asm clobbers rax, rdx, r10 { out main.port, main.value }",
                "missing `r11`",
            ),
            ("asm clobbers rax { hlt }", "not clobbered `rax`"),
            ("asm clobbers { hlt }", "spell `clobbers none`"),
            ("asm where clobbers none { hlt }", "`clobbers <registers>`"),
            (
                "asm ensures true { hlt }",
                "requires a falling-through block",
            ),
        ] {
            let source = format!(
                r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(main: &mut Main) {{ {block} }}
            "#
            );
            let error = super::parse_tokenized_error(&source, "inexact asm contract should reject");
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains(expected),
                "expected `{expected}` for `{block}`, got `{rendered}`"
            );
        }
    }

    #[test]
    fn rejects_unknown_asm_mnemonics() {
        for block in ["asm { db 0xF4 }", "asm { swapgs }"] {
            let source = format!(
                r#"
            data Main {{
                value: i32;
            }}

            machine Main::main(main: &mut Main) {{
                {block}
            }}
            "#
            );

            let error = super::parse_tokenized_error(&source, "unknown mnemonic must not parse");
            let message = format!("{error:?}");
            assert!(
                message.contains("only known-contract instructions compile"),
                "unexpected error for {block}: {message}"
            );
        }
    }
}
pub(crate) mod items_and_selections {
    use crate::parser::parse_syntax_trees;
    use language_core::BindingRelevance;
    use language_core::atomic::AtomicExpressionResultCustody;
    use source::Span;
    use source_files_to_tokens::Lexer;
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
            let source_id = source::SourceId(17);
            let parsed = crate::parser::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            let statements = first_machine_statements(&parsed);
            let StatementNode::LocalData(local) = parsed.statements.statement(statements[0]) else {
                panic!("local initializer")
            };
            assert!(matches!(
                parsed.expressions.expression(local.initial_value),
                ExpressionNode::Name(_)
            ));
            let start = source.find(expression).expect("authored name expression");
            assert_eq!(
                parsed.expressions.source_span(local.initial_value),
                source::SourceSpan::new(source_id, Span::new(start, start + expression.len()))
            );
        }
    }

    #[test]
    fn name_expression_source_custody_survives_nested_constant_leaves() {
        let source = "const SELECTED: Point = Point { value: VALUE, nested: Pair { value: settings::VALUE } }; const ALIAS: u64 = settings::VALUE;";
        let tokens = Lexer::new(source).tokenize().unwrap();
        let source_id = source::SourceId(23);
        let parsed = crate::parser::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let names = parsed
            .expressions
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Name(_))
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
            source::SourceSpan::new(source_id, Span::new(start, start + name.len()))
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
                ("prior", false, BindingRelevance::Relevant),
                ("mut prior", true, BindingRelevance::Relevant),
                ("prior [erased]", false, BindingRelevance::Erased),
            ] {
                let source = format!(
                    "machine update(cell: u64) {{ let {binding}: u64 = cell.{operation}; }}"
                );
                let parsed = super::parse_source_unwrap(&source);
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
            let parsed = super::parse_source_unwrap(&source);
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
        let definition = super::root_trait(&parsed, "trait");
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

    fn first_machine_statements(parsed: &syntax_trees::SyntaxTrees) -> &[StatementHandle] {
        let machine = super::root_machine(parsed, "machine");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        parsed.items.statements(state.statements)
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
                "machine Main::next(main: &Main, input: u64) -> u64 {{ transition {{ _ -> {target}(input; proof) }} }}"
            );
            let parsed = super::parse_source_unwrap(&source);
            let machine = super::root_machine_unwrap(&parsed);
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
                    let ExpressionNode::Name(receiver) =
                        parsed.expressions.expression(call.receiver)
                    else {
                        panic!("direct receiver identity lost");
                    };
                    assert_eq!(
                        parsed.expressions.identifier_path_members(*receiver)[0].as_str(&parsed),
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
            let parsed = super::parse_source_unwrap(&source);
            let machine = super::root_machine_unwrap(&parsed);
            let state = parsed
                .items
                .state(parsed.items.state_handles(machine.states)[0]);
            let statements = parsed.items.statements(state.statements);
            assert_eq!(statements.len(), 3);
            let StatementNode::LocalData(capture) = parsed.statements.statement(statements[0])
            else {
                panic!("dispatch subject capture");
            };
            assert_eq!(
                capture.role,
                language_core::LocalBindingRole::DispatchSubject
            );
            assert!(matches!(
                parsed.expressions.expression(capture.initial_value),
                ExpressionNode::Call(_)
            ));
            let StatementNode::Transition(first_arm) = parsed.statements.statement(statements[1])
            else {
                panic!("first authored arm");
            };
            assert!(matches!(first_arm.guard, TransitionGuardNode::Always));
            let [pattern] = parsed.statements.transition_patterns(first_arm.patterns) else {
                panic!("first arm tests the subject");
            };
            let ExpressionNode::Binary(comparison) =
                parsed.expressions.expression(pattern.pattern.guard)
            else {
                panic!("authored Boolean pattern comparison");
            };
            assert_eq!(
                parsed.display_expression_name(comparison.left),
                capture.name.as_str(&parsed)
            );
            assert!(matches!(parsed.expressions.expression(comparison.right),
            ExpressionNode::Boolean(value) if *value == first));
            let StatementNode::Transition(last_arm) = parsed.statements.statement(statements[2])
            else {
                panic!("second authored arm");
            };
            assert!(matches!(last_arm.guard, TransitionGuardNode::Always));
            assert!(
                parsed
                    .statements
                    .transition_patterns(last_arm.patterns)
                    .is_empty(),
                "the opposite arm is the fall-through and tests nothing"
            );
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
            let parsed = super::parse_source_unwrap(&source);
            let machine = super::root_machine_unwrap(&parsed);
            let state = parsed
                .items
                .state(parsed.items.state_handles(machine.states)[0]);
            let statements = parsed.items.statements(state.statements);
            let mut arms = 0;
            for statement in statements {
                let transition = match parsed.statements.statement(*statement) {
                    StatementNode::Transition(transition) => transition,
                    StatementNode::LocalData(local)
                        if local.role == language_core::LocalBindingRole::DispatchSubject =>
                    {
                        continue;
                    }
                    _ => panic!("authored transition"),
                };
                arms += 1;
                let tested = matches!(transition.guard, TransitionGuardNode::When(_))
                    || parsed
                        .statements
                        .transition_patterns(transition.patterns)
                        .iter()
                        .any(|pattern| pattern.pattern.guard.is_valid());
                assert!(tested, "{body}");
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
            let parsed = super::parse_source_unwrap(&source);
            let machine = super::root_machine_unwrap(&parsed);
            let state = parsed
                .items
                .state(parsed.items.state_handles(machine.states)[0]);
            let statement = parsed.items.statements(state.statements)[0];
            let StatementNode::Transition(transition) = parsed.statements.statement(statement)
            else {
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
        super::expect_error(&error, "`target` declarations are retired");
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
        assert_eq!(atomic.result_custody, AtomicExpressionResultCustody::Scalar);
    }

    #[test]
    fn single_attempt_compare_exchange_keeps_its_outcome_instead_of_a_scalar_prior() {
        let tokens = Lexer::new(
        "machine update(cell: u64) { let outcome: u64 = cell.compare_exchange_once(0, 1, NoOrdering, NoOrdering); }",
    )
    .tokenize()
    .expect("tokenize single-attempt compare-exchange");
        let parsed = parse_syntax_trees(&tokens).expect("parse single-attempt compare-exchange");
        let statements = first_machine_statements(&parsed);

        let [statement] = statements else {
            panic!("one authored local, no result placeholder or assignment");
        };
        let StatementNode::LocalData(local) = parsed.statements.statement(*statement) else {
            panic!("the outcome binds the authored local");
        };
        assert_eq!(local.name.as_str(&parsed), "outcome");
        assert_eq!(local.role, language_core::LocalBindingRole::Value);
        let ExpressionNode::Atomic(atomic) = parsed.expressions.expression(local.initial_value)
        else {
            panic!("single-attempt compare-exchange is an atomic expression");
        };
        assert!(matches!(
            atomic.ordering,
            language_core::atomic::AtomicOrderingPlan::CompareExchangeOnce { .. }
        ));
        assert!(matches!(
            atomic.result_custody,
            AtomicExpressionResultCustody::CompareExchangeOnce(_)
        ));
        assert!(!atomic.result.is_valid());
        let ExpressionNode::Call(roster) = parsed.expressions.expression(atomic.value) else {
            panic!("retained operand roster");
        };
        assert_eq!(roster.target.as_str(&parsed), "compare_exchange_once");
        assert!(roster.receiver.is_valid());
        assert_eq!(roster.arguments.count(), 2);
    }

    #[test]
    fn old_remains_an_ordinary_parameter_and_local_identifier() {
        let tokens =
            Lexer::new("machine migrate(old: u64) -> u64 { let old_copy: u64 = old; old_copy }")
                .tokenize()
                .expect("tokenize ordinary old identifiers");
        let parsed = parse_syntax_trees(&tokens).expect("old must not be a globally reserved word");
        let machine = super::root_machine(&parsed, "migration machine");
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
        let machine = super::root_machine(&parsed, "ordinary entry machine");
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
        let parsed =
            parse_syntax_trees(&tokens).expect("parse trait machine requirement parameter");
        let trait_definition = super::root_trait(&parsed, "PrivateCallbackSlot trait");
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
        machine build(builder: &mut Build) {
            Build::select_provider<host::Console, application::ConsoleProvider>(builder);
        }
    "#;
        let parsed = super::parse_source_verbose(
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
            super::argument_path(&parsed, &call.machine_arguments[0].path),
            ["host", "Console"]
        );
        assert_eq!(
            super::argument_path(&parsed, &call.machine_arguments[1].path),
            ["application", "ConsoleProvider"]
        );
        assert!(!call.target.as_str(&parsed).contains('#'));
    }

    #[test]
    fn service_exclusion_retains_namespace_type_path_and_explicit_build_argument() {
        let source = r#"
        machine build(builder: &mut Build) {
            Build::exclude_service<host::Console>(builder);
        }
    "#;
        let parsed = super::parse_source_verbose(
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
            super::argument_path(&parsed, &call.machine_arguments[0].path),
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
                "machine build(builder: &mut Build) { builder.exclude_service<host::Console, other::Log>(); }",
                "receiver-style service exclusion is retired",
            ),
            (
                "machine build(builder: &mut Build) { builder.exclude_service<host::Console>(1); }",
                "receiver-style service exclusion is retired",
            ),
            (
                "machine build(builder: &mut Build) { builder.exclude_service<>(); }",
                "exclude_service",
            ),
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize rejected service exclusion");
            let error = parse_syntax_trees(&tokens).err().unwrap_or_else(|| {
                panic!("a malformed service exclusion must not parse: {source}")
            });
            let message = format!("{error:?}");
            assert!(message.contains(expected), "{source}: {message}");
        }
    }

    #[test]
    fn provider_selection_retains_one_explicit_composition_mode_argument() {
        let source = r#"
        machine build(builder: &mut Build) {
            Build::select_provider<host::Console, application::ConsoleProvider>(
                builder, CompositionMode::Independent
            );
        }
    "#;
        let parsed = super::parse_source_verbose(
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
            ExpressionNode::Name(_)
        ));
        let ExpressionNode::Name(mode) = parsed.expressions.expression(*mode) else {
            panic!("composition mode remains one ordinary name expression")
        };
        assert_eq!(
            parsed
                .expressions
                .identifier_path_members(*mode)
                .iter()
                .map(|member| member.as_str(&parsed))
                .collect::<Vec<_>>(),
            ["CompositionMode", "Independent"]
        );
    }

    #[test]
    fn root_binding_statement_retains_operand_paths_and_the_authored_bind_span() {
        let source = r#"
        machine build(builder: &mut Build) {
            Build::bind(builder, linux_x86_64::ProgramEntry, Main::main);
        }
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize root binding", "parse root binding");
        let machine = super::root_machine(&parsed, "build machine");
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
            let source = format!(
                "machine build(builder: &mut Build) {{ Build::bind(builder, {operands}); }}"
            );
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
                "machine build(builder: &mut Build) {{ Build::bind(builder, Slot, {operand}); }}"
            );
            let parsed = super::parse_source_verbose(
                &source,
                "root binding tokens",
                "description expression syntax",
            );
            let machine = super::root_machine(&parsed, "build machine");
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
        machine build(builder: &mut Build) {
            Build::select_representation<interrupt::Ack, platform::AckRepresentation>(builder);
        }
    "#;
        let parsed = super::parse_source_verbose(
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
            super::argument_path(&parsed, &call.machine_arguments[0].path),
            ["interrupt", "Ack"]
        );
        assert_eq!(
            super::argument_path(&parsed, &call.machine_arguments[1].path),
            ["platform", "AckRepresentation"]
        );
    }

    #[test]
    fn rejects_pub_when_the_declaration_cannot_retain_visibility() {
        let tokens = Lexer::new("public measure Counter::Zero(value: i32) -> i32 { 0 }")
            .tokenize()
            .expect("tokenize declaration");
        let error = parse_syntax_trees(&tokens).expect_err("visibility loss must reject");
        super::expect_error(&error, "silently private API");
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
        let schema = super::root_data(&parsed, "ordinary numbered data");
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
        let parsed = super::parse_source(source, "all settled proposition forms should parse");
        let propositions = super::root_propositions(&parsed);

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
        let parsed = super::parse_source_verbose(
            source,
            "tokenize structured const declaration",
            "parse structured const declaration",
        );
        let declaration = super::root_const(&parsed, "structured const declaration");
        let span = parsed.expressions.source_span(declaration.value).span;
        assert_eq!(&source[span.start..span.end], "Point { x: 1, y: 2 }");
    }

    #[test]
    fn array_literal_retains_its_complete_authored_source_span() {
        for initializer in ["[]", "[1, 2]", "[[1], [2]]", "utf16\"hello\""] {
            let source = format!("const VALUES: [u16; 2] = {initializer};");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let source_id = source::SourceId(23);
            let parsed = crate::parser::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            let declaration = super::root_const(&parsed, "array const declaration");
            assert!(matches!(
                parsed.expressions.expression(declaration.value),
                ExpressionNode::ArrayLiteral(_)
            ));
            let start = source.find(initializer).unwrap();
            assert_eq!(
                parsed.expressions.source_span(declaration.value),
                source::SourceSpan::new(source_id, Span::new(start, start + initializer.len())),
                "{initializer}"
            );
        }
    }

    #[test]
    fn parses_type_and_integer_const_proposition_arguments() {
        let source = r#"
        proposition indexed<T, const N: i32>();
        proposition selected() = indexed<i32, 7>();
    "#;
        let parsed = super::parse_source(source, "static proposition arguments should parse");
        let call = parsed
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if call.target.as_str(&parsed) == "indexed" => {
                    Some(call)
                }
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
        assert_eq!(call.display_name(&parsed), "indexed<i32, 7>()");
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
        let error =
            parse_syntax_trees(&tokens).expect_err("retired proposition evidence must reject");

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
        let parsed = super::parse_source(source, "proposition parameter should parse");
        let trait_definitions = super::root_traits(&parsed);
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

        let retired = super::parse_tokenized_error(
            "trait Reflexive<C, proposition Relation>\nwhere proposition Relation(left: C, right: C);\n{}",
            "retired `where proposition` must reject",
        );
        super::expect_error(&retired, "write `requires proposition Relation(...);`");
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
}
pub(crate) mod mathematical_definitions {
    use crate::parser::parse_syntax_trees;
    use source_files_to_tokens::Lexer;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::{
        MathematicalDefinitionBody, MathematicalTypeHandle, MathematicalTypeNode, TypeParameterKind,
    };
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;

    fn parse(source: &str) -> SyntaxTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("parse")
    }

    fn single_definition(parsed: &SyntaxTrees) -> &syntax_trees::item::MathematicalDefinition {
        let handles = parsed.root_mathematical_definition_handles();
        assert_eq!(handles.len(), 1, "exactly one mathematical definition");
        parsed.root_mathematical_definition(handles[0])
    }

    fn render_type(parsed: &SyntaxTrees, handle: MathematicalTypeHandle) -> String {
        match parsed.items.mathematical_type(handle) {
            MathematicalTypeNode::Ordinary(reference) => render_type_reference(parsed, *reference),
            MathematicalTypeNode::Arrow {
                binder,
                domain,
                codomain,
            } => {
                let domain = render_type(parsed, *domain);
                let codomain = render_type(parsed, *codomain);
                match binder {
                    Some(binder) => format!("({}: {domain}) -> {codomain}", binder.as_str(parsed)),
                    None => format!("{domain} -> {codomain}"),
                }
            }
            MathematicalTypeNode::Application { callee, arguments } => {
                let callee = render_type(parsed, *callee);
                let arguments = parsed
                    .expressions
                    .expression_handles(*arguments)
                    .iter()
                    .map(|argument| render_expression(parsed, *argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{callee}({arguments})")
            }
        }
    }

    fn render_expression(parsed: &SyntaxTrees, handle: ExpressionHandle) -> String {
        match parsed.expressions.expression(handle) {
            ExpressionNode::Name(path) => parsed
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str(parsed))
                .collect::<Vec<_>>()
                .join("::"),
            other => panic!("unexpected expression in test: {other:?}"),
        }
    }

    fn render_type_reference(parsed: &SyntaxTrees, handle: TypeReferenceHandle) -> String {
        match parsed.type_references.type_reference(handle) {
            TypeReferenceNode::Named(name) => name.as_str(parsed).to_owned(),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let arguments = parsed
                    .type_references
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| render_type_reference(parsed, *argument))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{arguments}>", base_name.as_str(parsed))
            }
            TypeReferenceNode::Unit => "()".to_owned(),
            TypeReferenceNode::ConstExpression(expression) => {
                match parsed.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => literal.to_string(),
                    other => panic!("unexpected static argument in test: {other:?}"),
                }
            }
            other => panic!("unexpected type reference in test: {other:?}"),
        }
    }

    #[test]
    fn parses_mathematical_let_with_relevant_and_erased_parameters() {
        let parsed = parse(
            "let greater_than(limit: i32, value [erased]: i32): core::Strict<0> =\n    value > limit;",
        );
        assert_eq!(parsed.root_item_handles().len(), 0);
        let definition = single_definition(&parsed);
        assert_eq!(definition.name.as_str(&parsed), "greater_than");
        assert!(!definition.is_public);
        assert!(definition.type_parameters.is_empty());

        let parameters = parsed.items.mathematical_parameters(definition.parameters);
        assert_eq!(parameters.len(), 2);
        let limit = parsed.items.mathematical_parameter(parameters[0]);
        assert_eq!(limit.name.as_str(&parsed), "limit");
        assert_eq!(limit.relevance, language_core::BindingRelevance::Relevant);
        assert_eq!(render_type(&parsed, limit.ty), "i32");
        let value = parsed.items.mathematical_parameter(parameters[1]);
        assert_eq!(value.name.as_str(&parsed), "value");
        assert_eq!(value.relevance, language_core::BindingRelevance::Erased);

        assert_eq!(render_type(&parsed, definition.result), "core::Strict<0>");
        assert!(matches!(
            definition.body,
            MathematicalDefinitionBody::Definition(_)
        ));
    }

    #[test]
    fn parses_universe_and_type_binder_telescope() {
        let parsed = parse(
            "let compose<u: core::Level, v: core::Level, w: core::Level,
             A: core::Type<u>, B: core::Type<v>, C: core::Type<w>>(
    f: B -> C, g: A -> B, value: A
): C = f(g(value));",
        );
        let definition = single_definition(&parsed);
        assert_eq!(definition.name.as_str(&parsed), "compose");

        let binders = parsed.items.type_parameters(definition.type_parameters);
        assert_eq!(binders.len(), 6);
        assert_eq!(binders[0].name.as_str(&parsed), "u");
        assert!(matches!(binders[0].kind, TypeParameterKind::Value { .. }));
        let TypeParameterKind::Value { type_reference } = &binders[3].kind else {
            panic!("`A: core::Type<u>` parses as a value binder");
        };
        assert_eq!(
            render_type_reference(&parsed, *type_reference),
            "core::Type<u>"
        );

        let parameters = parsed.items.mathematical_parameters(definition.parameters);
        assert_eq!(parameters.len(), 3);
        let f = parsed.items.mathematical_parameter(parameters[0]);
        assert_eq!(f.name.as_str(&parsed), "f");
        assert_eq!(render_type(&parsed, f.ty), "B -> C");
        let g = parsed.items.mathematical_parameter(parameters[1]);
        assert_eq!(render_type(&parsed, g.ty), "A -> B");
        assert_eq!(render_type(&parsed, definition.result), "C");
    }

    #[test]
    fn parses_boundary_let_as_bodyless_assumption() {
        let parsed = parse(
            "boundary let choose<u: core::Level, A: core::Type<u>>(
    inhabited: core::Squash<A>
): A;",
        );
        let definition = single_definition(&parsed);
        assert_eq!(definition.name.as_str(&parsed), "choose");
        assert_eq!(render_type(&parsed, definition.result), "A");
        assert_eq!(definition.body, MathematicalDefinitionBody::Assumption);

        let parameters = parsed.items.mathematical_parameters(definition.parameters);
        assert_eq!(parameters.len(), 1);
        let inhabited = parsed.items.mathematical_parameter(parameters[0]);
        assert_eq!(render_type(&parsed, inhabited.ty), "core::Squash<A>");
    }

    #[test]
    fn arrow_types_associate_right_and_bind_domains() {
        let parsed = parse("let fam(u: core::Level): A -> B -> C =\n    term;");
        let definition = single_definition(&parsed);
        assert_eq!(render_type(&parsed, definition.result), "A -> B -> C");

        let parsed = parse("let dependent(u: core::Level): (value: A) -> F(value) =\n    term;");
        let definition = single_definition(&parsed);
        assert_eq!(
            render_type(&parsed, definition.result),
            "(value: A) -> F(value)"
        );

        let parsed = parse("let higher(u: core::Level): (A -> B) -> C =\n    term;");
        let definition = single_definition(&parsed);
        assert_eq!(render_type(&parsed, definition.result), "A -> B -> C");
        assert!(matches!(
            parsed.items.mathematical_type(definition.result),
            MathematicalTypeNode::Arrow { domain, .. }
                if matches!(
                    parsed.items.mathematical_type(*domain),
                    MathematicalTypeNode::Arrow { .. }
                )
        ));
    }

    #[test]
    fn empty_parameter_list_names_a_nullary_definition() {
        let parsed = parse("let pi(): core::Real = term;");
        let definition = single_definition(&parsed);
        assert_eq!(definition.name.as_str(&parsed), "pi");
        assert!(definition.parameters.is_empty());
        assert_eq!(render_type(&parsed, definition.result), "core::Real");
    }

    #[test]
    fn top_level_let_requires_a_body_or_boundary() {
        let tokens = Lexer::new("let f(): A;").tokenize().expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
        super::expect_error(&error, "boundary let");
    }

    #[test]
    fn boundary_let_rejects_a_body() {
        let tokens = Lexer::new("boundary let f(): A = term;")
            .tokenize()
            .expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
        super::expect_error(&error, "`;`");
    }

    #[test]
    fn let_declaration_rejects_lifetime_and_machine_binders() {
        let tokens = Lexer::new("let f<'a>(): A = term;")
            .tokenize()
            .expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
        assert!(error.message.contains("lifetime"), "{}", error.message);

        let tokens = Lexer::new("let f<machine M>(): A = term;")
            .tokenize()
            .expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
        assert!(error.message.contains("machine"), "{}", error.message);
    }

    #[test]
    fn dependent_binder_requires_an_arrow() {
        let tokens = Lexer::new("let f(): (value: A) = term;")
            .tokenize()
            .expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
        assert!(error.message.contains("->"), "{}", error.message);
    }
}
pub(crate) mod numbered_data {
    use syntax_trees::item::{DataMember, Item};

    #[test]
    fn numbered_declarations_share_ordinary_data_and_source_identity() {
        for declaration in [
            "data Message { #7 value: u32; retired #8; }",
            "data Message<T> { #7 value: T; retired #8; }",
            "data Message [copy] { #7 value: u32; retired #8; }",
            "data Message requires value <= 10 { #7 value: u32; retired #8; }",
            "data Message<'scope> { #7 value: &'scope u32; retired #8; }",
            "data Message { #7 value: u32; #9 case Ready; }",
        ] {
            let parsed =
                super::parse_source_verbose(declaration, "tokens", "ordinary numbered declaration");
            let Some(Item::Data(definition)) = parsed.root_items().next() else {
                panic!("numbering must not select a different declaration representation");
            };
            let DataMember::Field(field) = &parsed.items.data_members(definition.members)[0] else {
                panic!("ordinary first field");
            };
            assert_eq!(field.identity, Some(7));
            assert_eq!(field.name.as_str(&parsed), "value");
            let span = field.name.source_span().span;
            assert_eq!(&declaration[span.start..span.end], "value");
        }
    }

    #[test]
    fn nested_version_blocks_reject_regardless_of_leading_member() {
        for members in [
            "version v1 { #1 value: u32; } #1 value: u32;",
            "#1 value: u32; version v1 { #1 value: u32; }",
            "retired #2; version v1 { #1 value: u32; } #1 value: u32;",
        ] {
            let declaration = format!("data Message {{ {members} }}");
            let error = super::parse_source_error(&declaration, "tokens", "retired version syntax");
            assert!(error.message.contains("data `version` blocks are retired"));
        }
    }

    #[test]
    fn numbered_data_rejects_duplicate_and_retired_identity_reuse() {
        for members in [
            "#7 left: u32; #7 right: u32;",
            "#7 value: u32; retired #7;",
            "#7 value: u32; retired #8; retired #8;",
        ] {
            let declaration = format!("data Message {{ {members} }}");
            let error = super::parse_source_error(&declaration, "tokens", "identity collision");
            assert!(error.message.contains("declared more than once"));
        }
    }
}
pub(crate) mod properties_and_requirements {
    use crate::parser::parse_syntax_trees;
    use language_core::DataSupplyMode;
    use language_core::Multiplicity;
    use language_core::OperatorSpelling;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::CapabilityContractKind;
    use syntax_trees::item::CapabilityMember;
    use syntax_trees::item::CrashCause;
    use syntax_trees::item::Item;
    use syntax_trees::item::Machine;
    use syntax_trees::statement::StatementNode;
    use syntax_trees::statement::TransitionExit;

    #[test]
    fn retired_capability_entry_names_the_boundary_provider_migration() {
        let source = r#"
        capability TestHost {
            entry host_write(fd: i32, count: u64) -> i32 {
                requires true;
                boundary host;
            }
        }
    "#;
        let error = super::parse_source_error(
            source,
            "tokenize retired capability entry",
            "capability entry must be retired",
        );
        assert!(
            error
                .message
                .contains("legacy `capability { entry ... }` host scaffold")
                && error.message.contains("is retired")
                && error.message.contains("`boundary trait`")
                && error
                    .message
                    .contains("satisfies Trait::requirement via Binding::..."),
            "got: {}",
            error.message
        );

        let current = r#"
        capability Current {
            entry: bool;
            state inspect() {
                requires true;
            }
        }
    "#;
        let tokens = Lexer::new(current)
            .tokenize()
            .expect("tokenize current capability surface");
        let syntax = parse_syntax_trees(&tokens)
            .expect("ordinary `entry` field and capability state must remain accepted");
        let Some(Item::Capability(capability)) = syntax.root_items().next() else {
            panic!("expected capability");
        };
        let [CapabilityMember::Field(field), CapabilityMember::State(_)] =
            syntax.items.capability_members(capability.members)
        else {
            panic!("expected ordinary field followed by current state member");
        };
        assert_eq!(field.name.as_str(&syntax), "entry");
    }

    #[test]
    fn ordinary_entry_named_calls_do_not_become_retired_members() {
        for body in [
            "entry();",
            "entry(value);",
            "entry()",
            "let answer: i32 = entry();",
        ] {
            let source = format!("machine entry() -> i32 {{ 7 }} machine run() {{ {body} }}");
            let tokens = Lexer::new(&source)
                .tokenize()
                .expect("tokenize ordinary entry call");
            parse_syntax_trees(&tokens).unwrap_or_else(|error| panic!("{body}: {}", error.message));
        }
    }

    #[test]
    fn retired_explicit_machine_entry_members_name_the_machine_body_migration() {
        for retired in [
            "machine run { entry() {} }",
            "machine run { entry begin() {} }",
            "machine run { public entry() {} }",
            "machine run { public entry begin() {} }",
        ] {
            let error = super::parse_source_error(
                retired,
                "tokenize retired explicit machine entry",
                "explicit machine entry members must be retired",
            );
            assert!(
                error.message.contains(
                    "explicit nested `entry` / `public entry` machine members are retired"
                ) && error.message.contains("`machine` head")
                    && error.message.contains("directly in the machine body")
                    && error.message.contains("`public machine`"),
                "got for {retired:?}: {}",
                error.message
            );
        }
    }

    #[test]
    fn retired_trailing_boundary_contracts_name_current_boundary_surfaces() {
        for retired in [
            "machine run() boundary host {}",
            "machine run() boundary LegacyHost {}",
            "capability Legacy { state run() { boundary host; } }",
            "capability Legacy { state run() { boundary LegacyHost; } }",
        ] {
            let error = super::parse_source_error(
                retired,
                "tokenize retired trailing boundary contract",
                "trailing boundary contract clauses must be retired",
            );
            assert!(
                error.message.contains(
                    "trailing `boundary host` and `boundary Name` contract clauses are retired"
                ) && error.message.contains("leading `boundary trait`")
                    && error.message.contains("`boundary machine`")
                    && error.message.contains("`boundary operator`")
                    && error
                        .message
                        .contains("`satisfies Trait::requirement via Binding::...`"),
                "got for {retired:?}: {}",
                error.message
            );
        }
    }

    #[test]
    fn erased_join_type_is_rejected_but_join_names_are_ordinary() {
        let retired = "machine run(task: Join<i32>) {}";
        let error = super::parse_tokenized_error(retired, "Join<T> must be retired");
        let rendered = error.message;
        assert!(rendered.contains("Join<T>") && rendered.contains("finish()"));

        let ordinary = r#"
        data Join { value: i32; }
        machine Join::join(join: &Join) -> i32 {
            transition { _ -> join.value }
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
        let parsed = super::parse_source(source, "linear properties should parse");
        let data: Vec<_> = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Data(data) => Some(data),
                _ => None,
            })
            .collect();

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].properties.multiplicity, Multiplicity::Linear);
        assert_eq!(data[1].properties.multiplicity, Multiplicity::Linear);
        let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
        assert_eq!(parameter.bounds.multiplicity, Multiplicity::Linear);
    }

    #[test]
    fn multiplicity_property_lists_accept_trailing_commas() {
        let source = "data Copy [copy,] {} data Linear [linear,] {}";
        let parsed = super::parse_source(source, "trailing property commas should parse");
        let multiplicities = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Data(data) => Some(data.properties.multiplicity),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            multiplicities,
            vec![Multiplicity::Unrestricted, Multiplicity::Linear,]
        );
    }

    #[test]
    fn copy_and_linear_properties_are_mutually_exclusive() {
        for source in ["data Bad [copy, linear] {}", "data Bad [linear, copy] {}"] {
            let error = super::parse_tokenized_error(
                source,
                "copy and linear must not coexist on one declaration",
            );
            assert!(error.message.contains("mutually exclusive"));
        }
    }

    #[test]
    fn carry_property_parses_all_four_axes_on_data_and_bounds() {
        use language_core::{
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
        let parsed = super::parse_source_verbose(
            source,
            "tokenize",
            "four-axis carry properties should parse",
        );
        let data = super::root_data_items(&parsed);

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
        let error =
            super::parse_source_error(missing, "tokenize", "partial carry policy must reject");
        assert!(error.message.contains("missing address"));

        let retired = "data Bad [send] {}";
        let error = super::parse_source_error(retired, "tokenize", "send must remain retired");
        assert!(error.message.contains("`[send]` is retired"));
    }

    #[test]
    fn boundary_data_parses_as_an_opaque_carrier_without_a_shape() {
        let tokens = Lexer::new("boundary data ProviderToken [linear];")
            .tokenize()
            .expect("tokenize");
        let parsed = parse_syntax_trees(&tokens).expect("opaque boundary data should parse");
        let data = super::root_data(&parsed, "data item");

        assert_eq!(data.supply_mode, DataSupplyMode::BoundaryOpaque);
        assert!(data.members.is_empty());
        assert_eq!(data.properties.multiplicity, Multiplicity::Linear);
    }

    #[test]
    fn parses_plain_and_boundary_traits() {
        let source = r#"
        trait Drawable<Shape> {
            machine draw(value: &Shape, canvas: &mut Canvas);
        }

        boundary trait Console {
            machine write_line(text: String)
            reaches
                Console;
        }
        "#;

        let parsed = super::parse_source(source, "parse should succeed");
        let traits: Vec<_> = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Trait(trait_definition) => Some(trait_definition),
                _ => None,
            })
            .collect();

        assert_eq!(traits.len(), 2);
        assert_eq!(traits[0].name.as_str(&parsed), "Drawable");
        assert!(!traits[0].is_boundary);
        assert_eq!(traits[0].machines.len(), 1);
        assert_eq!(traits[1].name.as_str(&parsed), "Console");
        assert!(traits[1].is_boundary);
        assert_eq!(traits[1].machines.len(), 1);
        let signature_handle = parsed.items.state_signatures(traits[1].machines)[0];
        let signature = parsed.items.state_signature(signature_handle);
        let service_reaches = parsed
            .items
            .identifier_path_members(signature.service_reaches);
        assert_eq!(service_reaches.len(), 1);
        assert_eq!(service_reaches[0].as_str(&parsed), "Console");
    }

    #[test]
    fn parses_trait_owned_fixed_operator_requirements() {
        let source = r#"
        trait Ranked<T> {
            machine < compare(left: T, right: T) -> bool;
            machine equivalent(left: T, right: T) -> bool;
        }
    "#;
        let parsed = super::parse_source(source, "trait operators should parse");
        let trait_definition = super::root_trait(&parsed, "Ranked trait");
        let requirements = parsed.items.state_signatures(trait_definition.machines);

        assert_eq!(requirements.len(), 2);
        assert_eq!(
            parsed.items.state_signature(requirements[0]).spelling,
            Some(OperatorSpelling::Less)
        );
        assert_eq!(parsed.items.state_signature(requirements[1]).spelling, None);
    }

    #[test]
    fn rejects_retired_operator_introducer_on_trait_requirements() {
        for source in [
            "trait Ranked<T> { operator < compare(left: T, right: T) -> bool; }",
            "trait Ranked<T> { operator equivalent(left: T, right: T) -> bool; }",
        ] {
            let error = super::parse_tokenized_error(source, "trait `operator` must reject");
            super::expect_error(&error, "the `operator` introducer is retired");
        }
    }

    #[test]
    fn parses_independent_operational_clauses_on_machines_and_requirements() {
        let source = r#"
        machine run() reaches Console suspends blocks {
        }

        trait Worker {
            machine wait() reaches Clock suspends blocks ensures true;
        }

        machine schedule<machine Callback>()
        requires machine Callback() suspends blocks;
        {
        }
        "#;

        let parsed = super::parse_source(source, "operational clauses should parse");
        let machine = super::root_machine(&parsed, "machine item");
        assert!(machine.suspends);
        assert!(machine.blocks);
        assert_eq!(machine.suspends_keyword_source_spans.len(), 1);
        assert_eq!(machine.blocks_keyword_source_spans.len(), 1);
        let service_reaches = parsed
            .items
            .identifier_path_members(machine.service_reaches);
        assert_eq!(service_reaches.len(), 1);
        assert_eq!(service_reaches[0].as_str(&parsed), "Console");

        let trait_definition = super::root_trait(&parsed, "trait item");
        let signature_handle = parsed.items.state_signatures(trait_definition.machines)[0];
        let signature = parsed.items.state_signature(signature_handle);
        assert!(signature.suspends);
        assert!(signature.blocks);
        assert_eq!(signature.suspends_keyword_source_spans.len(), 1);
        assert_eq!(signature.blocks_keyword_source_spans.len(), 1);
        let service_reaches = parsed
            .items
            .identifier_path_members(signature.service_reaches);
        assert_eq!(service_reaches.len(), 1);
        assert_eq!(service_reaches[0].as_str(&parsed), "Clock");

        let structural_machine = parsed
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&parsed) == "schedule" => {
                    Some(machine)
                }
                _ => None,
            })
            .expect("structural machine-parameter owner");
        let [parameter] = parsed
            .items
            .type_parameters(structural_machine.type_parameters)
        else {
            panic!("one structural machine parameter");
        };
        let syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(contract)),
        } = &parameter.kind
        else {
            panic!("Callback should retain its structural signature");
        };
        assert!(contract.suspends);
        assert!(contract.blocks);
        assert_eq!(contract.suspends_keyword_source_spans.len(), 1);
        assert_eq!(contract.blocks_keyword_source_spans.len(), 1);

        let suspends_starts = source
            .match_indices("suspends")
            .map(|(start, _)| start)
            .collect::<Vec<_>>();
        let blocks_starts = source
            .match_indices("blocks")
            .map(|(start, _)| start)
            .collect::<Vec<_>>();
        assert_eq!(suspends_starts.len(), 3);
        assert_eq!(blocks_starts.len(), 3);
        for (index, source_span) in [
            machine.suspends_keyword_source_spans[0],
            signature.suspends_keyword_source_spans[0],
            contract.suspends_keyword_source_spans[0],
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(source_span.span.start, suspends_starts[index]);
            assert_eq!(
                &source[source_span.span.start..source_span.span.end],
                "suspends"
            );
        }
        for (index, source_span) in [
            machine.blocks_keyword_source_spans[0],
            signature.blocks_keyword_source_spans[0],
            contract.blocks_keyword_source_spans[0],
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(source_span.span.start, blocks_starts[index]);
            assert_eq!(
                &source[source_span.span.start..source_span.span.end],
                "blocks"
            );
        }
    }

    #[test]
    fn parses_machine_requires_before_reaches_on_one_line() {
        let source = r#"
        data App { a: u32; b: u32; }

        machine App::main(app: &mut App) requires app.a <= app.b reaches Console {
        }
        "#;

        let parsed = super::parse_source(source, "requires before reaches should parse");
        let machine = super::root_machine(&parsed, "machine item");

        let service_reaches = parsed
            .items
            .identifier_path_members(machine.service_reaches);
        assert_eq!(service_reaches.len(), 1);
        assert_eq!(service_reaches[0].as_str(&parsed), "Console");

        let [contract] = parsed.items.capability_contracts(machine.contracts) else {
            panic!("machine should carry exactly one requires contract");
        };
        assert!(matches!(contract.kind, CapabilityContractKind::Requires));
        assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    }

    #[test]
    fn parses_contract_clause_orderings_and_predicate_boundaries() {
        let source = r#"
        data App { a: u32; b: u32; }

        machine App::reaches_then_requires(app: &mut App) reaches Console requires app.a <= app.b {
        }
        machine App::requires_semicolon_reaches(app: &mut App) requires app.a <= app.b reaches Console {
        }
        machine App::requires_newline_reaches(app: &mut App)
            requires app.a <= app.b
            reaches Console {
        }
        machine App::requires_only(app: &mut App) requires app.a <= app.b {
        }
        machine App::reaches_only(app: &mut App) reaches Console {
        }
        machine App::bare(app: &mut App) {
        }
        machine App::predicate_then_reaches(app: &mut App) ensures app.a settled reaches Console {
        }

        trait Visitor<Visited> {
            machine visit(value: &mut Visited) requires value.a <= value.b reaches Console;
        }
        "#;

        let parsed = super::parse_source(source, "clause orderings should parse");

        let machine = |name: &str| {
            parsed
                .root_items()
                .find_map(|item| match item {
                    Item::Machine(machine) if machine.name.as_str(&parsed) == name => Some(machine),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("machine {name}"))
        };
        let reach_count = |machine: &Machine| {
            parsed
                .items
                .identifier_path_members(machine.service_reaches)
                .len()
        };
        let contracts = |machine: &Machine| parsed.items.capability_contracts(machine.contracts);

        for (name, reaches, contracts_count) in [
            ("App::reaches_then_requires", 1usize, 1usize),
            ("App::requires_semicolon_reaches", 1, 1),
            ("App::requires_newline_reaches", 1, 1),
            ("App::requires_only", 0, 1),
            ("App::reaches_only", 1, 0),
            ("App::bare", 0, 0),
            ("App::predicate_then_reaches", 1, 1),
        ] {
            let machine = machine(name);
            assert_eq!(reach_count(machine), reaches, "{name} service reaches");
            assert_eq!(
                contracts(machine).len(),
                contracts_count,
                "{name} contracts"
            );
        }

        let predicate_machine = machine("App::predicate_then_reaches");
        let [contract] = contracts(predicate_machine) else {
            panic!("predicate machine should carry one ensures contract");
        };
        assert!(matches!(contract.kind, CapabilityContractKind::Ensures));
        let [fact] = parsed.items.proof_facts(contract.facts) else {
            panic!("ensures contract should carry one fact");
        };
        let syntax_trees::item::ProofFact::Expression(expression) = fact else {
            panic!("predicate fact should be an expression");
        };
        assert!(matches!(
            parsed.expressions.expression(*expression),
            ExpressionNode::Call(_)
        ));

        let trait_definition = super::root_trait(&parsed, "trait item");
        let signature = parsed
            .items
            .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
        assert_eq!(
            parsed
                .items
                .identifier_path_members(signature.service_reaches)
                .len(),
            1
        );
        let [signature_contract] = parsed.items.capability_contracts(signature.contracts) else {
            panic!("requirement should carry one requires contract");
        };
        assert!(matches!(
            signature_contract.kind,
            CapabilityContractKind::Requires
        ));
        assert_eq!(parsed.items.proof_facts(signature_contract.facts).len(), 1);
    }

    #[test]
    fn rejects_malformed_requires_fact_lists() {
        for source in [
            "machine f(a: u32, b: u32) requires a <= b bogus stray { }",
            "machine f() requires; { }",
            "machine f() requires",
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize should succeed");
            assert!(
                parse_syntax_trees(&tokens).is_err(),
                "malformed requires should reject: {source}"
            );
        }
    }

    #[test]
    fn parses_guarded_crash_buckets_on_machines_and_requirements() {
        let source = r#"
        machine divide(numerator: i32, denominator: i32)
        crashes Trap
            denominator == 0
            numerator == 0
        crashes Abort
        {
        }

        trait Fallible {
            machine run(flag: bool)
            crashes Abort
                flag;
        }
        "#;

        let parsed = super::parse_source(source, "crash buckets should parse");
        let machine = super::root_machine(&parsed, "machine item");
        let contracts = parsed.items.capability_contracts(machine.contracts);
        assert_eq!(contracts.len(), 2);
        let CapabilityContractKind::Crashes { cause } = &contracts[0].kind else {
            panic!("first contract should be a crash bucket");
        };
        assert_eq!(*cause, CrashCause::Trap);
        assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 2);

        let CapabilityContractKind::Crashes { cause } = &contracts[1].kind else {
            panic!("second contract should be a crash bucket");
        };
        assert_eq!(*cause, CrashCause::Abort);
        assert!(parsed.items.proof_facts(contracts[1].facts).is_empty());

        let trait_definition = super::root_trait(&parsed, "trait item");
        let signature = parsed
            .items
            .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
        let [contract] = parsed.items.capability_contracts(signature.contracts) else {
            panic!("requirement should carry one crash bucket");
        };
        let CapabilityContractKind::Crashes { cause } = &contract.kind else {
            panic!("requirement contract should be a crash bucket");
        };
        assert_eq!(*cause, CrashCause::Abort);
        assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    }

    #[test]
    fn parses_guarded_crash_bucket_on_operator_contract() {
        let source = r#"
        operator / divide(numerator: i32, denominator: i32) -> i32
        crashes Trap
            denominator == 0;
    "#;
        let parsed = super::parse_source(source, "operator crash bucket should parse");
        let operator = super::root_operator(&parsed, "operator item");
        let [contract] = parsed.items.capability_contracts(operator.contracts) else {
            panic!("operator should carry one crash bucket");
        };
        let CapabilityContractKind::Crashes { cause } = &contract.kind else {
            panic!("operator contract should be a crash bucket");
        };
        assert_eq!(*cause, CrashCause::Trap);
        assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
        let keyword = contract
            .keyword_source_span
            .expect("operator crash keyword span");
        assert_eq!(&source[keyword.span.start..keyword.span.end], "crashes");
    }

    #[test]
    fn parses_fixed_operator_tokens_in_declaration_heads() {
        let source = r#"
        public operator + add(left: i32, right: i32) -> i32;
        boundary operator [] Slice::index(items: &[u8], index: u64) -> u8;
        boundary operator [..] Slice::range(items: &[u8], start: u64, end: u64) -> &[u8];
        operator named(left: i32, right: i32) -> i32;
    "#;
        let parsed = super::parse_source(source, "fixed-token heads should parse");
        let operators = super::root_operators(&parsed);

        assert_eq!(operators.len(), 4);
        assert!(operators[0].is_public);
        assert!(operators[1..].iter().all(|operator| !operator.is_public));
        assert_eq!(operators[0].spelling, Some(OperatorSpelling::Add));
        assert_eq!(operators[1].spelling, Some(OperatorSpelling::Index));
        assert_eq!(operators[2].spelling, Some(OperatorSpelling::Range));
        assert_eq!(operators[3].spelling, None);
    }

    #[test]
    fn rejects_retired_operator_spelling_clause() {
        let source = "operator add(left: i32, right: i32) -> i32 spelling +;";
        let error = super::parse_tokenized_error(source, "retired clause must reject");

        super::expect_error(&error, "expected `;`");
    }

    #[test]
    fn parses_fixed_operator_tokens_on_machine_declarations() {
        let source = r#"
        public machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 {
            add_vectors(left, right)
        }
        machine [] Slice::index(items: Slice, index: u64) -> u8 {
            0u8
        }
        machine [..] Slice::range(items: Slice, start: u64, end: u64) -> Slice {
            items
        }
        boundary machine == Vec2::equal(left: Vec2, right: Vec2) -> bool;
        linux_x86_64 machine - subtract(left: u64, right: u64) -> u64 {
            left
        }
        machine ordinary(value: u64) -> u64 { value }
    "#;
        let parsed = super::parse_source(source, "machine token heads should parse");
        let machines = super::root_machines(&parsed);

        assert_eq!(machines.len(), 6);
        assert_eq!(machines[0].spelling, Some(OperatorSpelling::Add));
        assert!(machines[0].is_public);
        assert!(!machines[0].bodyless);
        assert_eq!(machines[0].name.as_str(&parsed), "Vec2::add");
        assert_eq!(machines[1].spelling, Some(OperatorSpelling::Index));
        assert_eq!(machines[2].spelling, Some(OperatorSpelling::Range));
        assert_eq!(machines[3].spelling, Some(OperatorSpelling::Equal));
        assert!(machines[3].boundary && machines[3].bodyless);
        assert_eq!(machines[4].spelling, Some(OperatorSpelling::Subtract));
        assert_eq!(
            machines[4]
                .target
                .as_ref()
                .map(|target| target.as_str(&parsed)),
            Some("linux_x86_64")
        );
        assert_eq!(machines[5].spelling, None);
    }

    #[test]
    fn rejects_malformed_operator_tokens_on_machine_declarations() {
        for (source, expected) in [
            (
                "machine && add(left: i32, right: i32) -> i32 { left }",
                "unknown operator spelling `&&`",
            ),
            (
                "machine () add(left: i32) -> i32 { left }",
                "unknown operator spelling `()`",
            ),
            (
                "machine += add(left: i32, right: i32) -> i32 { left }",
                "unknown operator spelling `+=`",
            ),
            (
                "machine ! add(left: i32) -> i32 { left }",
                "unknown operator spelling `!`",
            ),
            (
                "machine + + add(left: i32, right: i32) -> i32 { left }",
                "expected identifier",
            ),
            (
                "machine + add(left: i32, right: i32) -> i32 satisfies Math::add { left }",
                "cannot declare an operator token",
            ),
            (
                "machine + add(left: i32, right: i32) -> i32 satisfies Math::add via ForeignBinding::CompilerIntrinsic;",
                "cannot declare an operator token",
            ),
            (
                "Evidence satisfies Shape { machine + area() -> u64 { 1u64 } }",
                "a conformance member supplies an implementation",
            ),
        ] {
            let error = super::parse_tokenized_error(source, "malformed token head must reject");
            super::expect_error(&error, expected);
        }
    }

    #[test]
    fn rejects_all_retired_invariant_declaration_forms_with_direction() {
        for (source, expected) in [
            (
                "invariant Positive(value: i32) { value > 0 }",
                "the `invariant` declaration is retired",
            ),
            (
                "trait Counter { invariant self.value > 0; }",
                "the `invariant` clause is retired",
            ),
            (
                "machine Counter { invariant valid { } }",
                "the `invariant` machine member is retired",
            ),
        ] {
            let error = super::parse_tokenized_error(source, "retired invariant must reject");
            assert!(error.message.contains(expected), "got: {}", error.message);
        }
    }

    #[test]
    fn rejects_retired_provider_item_and_operator_clause() {
        let tokens = Lexer::new("provider omega::host::WriteBytes : HostAbiCall;")
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("provider items must reject");
        super::expect_error(&error, "expected one of");

        let tokens = Lexer::new(
            "boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 provider Slice;",
        )
        .tokenize()
        .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("operator provider clauses must reject");
        super::expect_error(&error, "expected `;`");
    }

    #[test]
    fn rejects_unknown_or_multiple_fixed_operator_tokens() {
        for source in [
            "operator && both(left: bool, right: bool) -> bool;",
            "operator + - ambiguous(left: i32, right: i32) -> i32;",
        ] {
            let tokens = Lexer::new(source)
                .tokenize()
                .expect("tokenize should succeed");
            assert!(
                parse_syntax_trees(&tokens).is_err(),
                "unknown or multiple fixed tokens must reject: {source}"
            );
        }
    }

    #[test]
    fn rejects_unknown_crash_causes() {
        let source = "machine fail() crashes Panic {}";
        let error = super::parse_tokenized_error(source, "unknown crash cause must fail");
        super::expect_error(
            &error,
            "unknown crash cause `Panic`; expected `Trap` or `Abort`",
        );
    }

    #[test]
    fn parses_explicit_crash_terminal_and_retires_trap_statement() {
        let source = r#"
        machine fail()
        crashes Abort
        {
            crash Abort;
        }
    "#;
        let parsed = super::parse_source(source, "explicit crash should parse");
        let machine = super::root_machine(&parsed, "machine item");
        let state = parsed
            .items
            .state(parsed.items.state_handles(machine.states)[0]);
        let [statement] = parsed.items.statements(state.statements) else {
            panic!("crash-only body should contain one statement");
        };
        let StatementNode::Transition(transition) = parsed.statements.statement(*statement) else {
            panic!("crash should lower to an explicit transition exit");
        };
        assert_eq!(transition.exit, TransitionExit::Crash(CrashCause::Abort));

        let tokens = Lexer::new("machine fail() { trap; }")
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("retired trap statement must fail");
        assert!(error.message.contains("write `crash Trap;`"));
    }

    #[test]
    fn rejects_duplicate_operational_clauses() {
        let source = "machine run() suspends suspends {}";
        let error = super::parse_tokenized_error(source, "duplicate clause must fail");
        super::expect_error(&error, "duplicate `suspends`");
    }

    #[test]
    fn rejects_legacy_effects_reach_clause() {
        let source = "machine run() effects Console {}";
        let error = super::parse_tokenized_error(source, "legacy reach spelling must fail");
        super::expect_error(&error, "`effects` reach clause is retired");
        assert!(error.message.contains("`reaches <Service> + ...`"));
    }

    #[test]
    fn parses_installation_bound_reach_on_bodyless_boundary_requirement() {
        let source = r#"
        boundary trait InterruptCompletion {
            machine complete(acknowledgement: u64)
            reaches <= MachineControl + PortIo;
        }
    "#;
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens)
            .expect("bounded installation reach should parse on a boundary requirement");
        let trait_definition = super::root_trait(&parsed, "boundary trait");
        let signature = parsed
            .items
            .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
        assert!(signature.service_reach_is_installation_bound);
        assert_eq!(signature.service_reaches.len(), 2);
    }

    #[test]
    fn rejects_retired_standalone_boundary_requirement() {
        for source in [
            "public boundary requirement InterruptAcknowledgement::complete<T>(acknowledgement: T) -> bool reaches <= MachineControl;",
            "boundary requirement Task::finish(task: u64);",
            "boundary requirement Package::operation() {}",
            "boundary requirement Package::operation() satisfies Contract::operation;",
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize retired boundary requirement",
                "a standalone boundary requirement must reject",
            );
            super::expect_error(&error, "are retired");
            super::expect_error(&error, "boundary trait");
        }
    }

    #[test]
    fn rejects_top_level_installation_bound_reach_outside_fresh_boundary_requirement() {
        for source in [
            "machine complete() reaches <= MachineControl {}",
            "boundary machine complete() reaches <= MachineControl {}",
            "x86_64 machine complete() reaches <= MachineControl {}",
            "boundary machine complete() satisfies Completion::complete reaches <= MachineControl;",
            "Named satisfies Trait { machine complete() reaches <= MachineControl {} }",
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize should succeed",
                "installation-bound reach must stay on a fresh boundary requirement",
            );
            super::expect_error(&error, "`reaches <= Bound`");
        }
    }

    #[test]
    fn rejects_installation_bound_reach_outside_bodyless_boundary_requirement() {
        for source in [
            "trait Completion { machine complete() reaches <= MachineControl; }",
            "boundary trait Completion { machine complete() reaches <= MachineControl {} }",
            "machine run<machine Op>() requires machine Op() reaches <= MachineControl; {}",
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize should succeed",
                "installation-bound reach must stay on a bodyless boundary requirement",
            );
            super::expect_error(&error, "`reaches <= Bound`");
        }
    }

    #[test]
    fn rejects_empty_or_mixed_installation_bound_reach_rows() {
        for source in [
            "boundary trait Completion { machine complete() reaches <=; }",
            "boundary trait Completion { machine complete() reaches MachineControl reaches <= PortIo; }",
            "boundary trait Completion { machine complete() reaches <= MachineControl reaches PortIo; }",
        ] {
            let error = super::parse_tokenized_error(
                source,
                "empty or mixed installation-bound reach must reject",
            );
            super::expect_error(&error, "installation-bound reach");
        }
    }

    #[test]
    fn rejects_operational_members_in_service_reach_rows() {
        for (retired, replacement) in [
            ("Suspend", "suspends"),
            ("Block", "blocks"),
            ("thread_block", "blocks"),
        ] {
            let source = format!("machine run() reaches Console, {retired} {{}}");
            let error = super::parse_tokenized_error(&source, "retired effect member must fail");
            assert!(error.message.contains("boundary-service identities only"));
            super::expect_error(&error, replacement);
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

        let parsed = super::parse_source(source, "parse should succeed");
        let machine = super::root_machine(&parsed, "machine root item");
        let contracts = parsed.items.capability_contracts(machine.contracts);

        assert_eq!(contracts.len(), 2);
        assert!(matches!(
            contracts[0].kind,
            CapabilityContractKind::Requires
        ));
        assert!(matches!(contracts[1].kind, CapabilityContractKind::Ensures));
        assert!(contracts[0].token_count > 0);
        assert!(contracts[1].token_count > 0);
        for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
            let span = contract
                .keyword_source_span
                .expect("authored contract keyword span");
            assert_eq!(&source[span.span.start..span.span.end], keyword);
        }
        assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
        assert_eq!(parsed.items.proof_facts(contracts[1].facts).len(), 1);
    }

    #[test]
    fn parses_named_machine_contract_evidence_bindings() {
        let source = r#"
        proposition carries(value: i32) evidence i32;
        machine forward(value: i32)
        requires input_proof: carries(value)
        ensures output_proof: carries(value)
        {
        }
    "#;
        let parsed = super::parse_source_verbose(source, "tokenize", "named contracts parse");
        let machine = super::root_machine(&parsed, "machine");
        let contracts = parsed.items.capability_contracts(machine.contracts);
        assert_eq!(contracts.len(), 2);
        assert_eq!(
            contracts[0]
                .binding
                .as_ref()
                .map(|name| name.as_str(&parsed)),
            Some("input_proof")
        );
        assert_eq!(
            contracts[1]
                .binding
                .as_ref()
                .map(|name| name.as_str(&parsed)),
            Some("output_proof")
        );
        assert!(
            contracts
                .iter()
                .all(|contract| parsed.items.proof_facts(contract.facts).len() == 1)
        );
    }

    #[test]
    fn parses_outcome_specific_ensures_as_rows_without_group_identity() {
        let source = r#"
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        ensures Outcome::Success -> {
            true;
            selected: true;
        }
        { Outcome::Success }
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize", "outcome-specific ensures parse");
        let machine = super::root_machine(&parsed, "machine");
        let contracts = parsed.items.capability_contracts(machine.contracts);
        assert_eq!(contracts.len(), 2);
        for contract in contracts {
            let CapabilityContractKind::EnsuresForResultCase { result_case } = &contract.kind
            else {
                panic!("guarded guarantee row")
            };
            let names = parsed.items.identifier_path_members(*result_case);
            assert_eq!(names[0].as_str(&parsed), "Outcome");
            assert_eq!(names[1].as_str(&parsed), "Success");
            assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
        }
        assert!(contracts[0].binding.is_none());
        assert_eq!(
            contracts[1]
                .binding
                .as_ref()
                .map(|name| name.as_str(&parsed)),
            Some("selected")
        );
    }

    #[test]
    fn rejects_ambiguous_and_duplicate_outcome_specific_ensures_surfaces() {
        for (source, expected) in [
            (
                "machine choose() ensures result == Outcome::Success -> { true; } -> Outcome {}",
                "rejects Boolean guards",
            ),
            (
                "machine choose() ensures Outcome::Success() -> { true; } -> Outcome {}",
                "rejects case-literal-shaped selectors",
            ),
            (
                "machine choose() -> Outcome ensures Outcome::Success -> { true; } ensures Outcome::Success -> { false; } {}",
                "duplicate outcome-specific ensures group",
            ),
            (
                "machine choose() -> Outcome ensures selected: true ensures Outcome::Success -> { selected: true; } {}",
                "duplicate machine-wide public ensures selector",
            ),
        ] {
            let error = super::parse_source_error(
                source,
                "tokenize invalid group",
                "invalid group must reject",
            );
            super::expect_error(&error, expected);
        }
    }

    #[test]
    fn rejects_named_contract_with_multiple_propositions() {
        let source = r#"
        proposition carries(value: i32);
        machine invalid(value: i32)
        requires proof: carries(value), carries(value)
        {
        }
    "#;
        let error = super::parse_source_error(
            source,
            "tokenize",
            "one evidence term cannot bind two facts",
        );
        super::expect_error(&error, "exactly one proposition");
    }

    #[test]
    fn parses_named_bodyless_signature_contract_bindings() {
        let source = r#"
        trait Evidence { machine witness(); }
        proposition ready() evidence Evidence;
        trait Worker {
            machine relay()
            requires input_proof: ready()
            ensures output_proof: ready();
        }
    "#;
        let parsed =
            super::parse_source_verbose(source, "tokenize", "named signature contracts parse");
        let worker = parsed
            .root_items()
            .filter_map(|item| match item {
                Item::Trait(definition) => Some(definition),
                _ => None,
            })
            .find(|definition| definition.name.as_str(&parsed) == "Worker")
            .expect("worker trait");
        let [requirement] = parsed.items.state_signatures(worker.machines) else {
            panic!("one worker requirement")
        };
        let contracts = parsed
            .items
            .capability_contracts(parsed.items.state_signature(*requirement).contracts);
        assert_eq!(contracts.len(), 2);
        assert_eq!(
            contracts[0]
                .binding
                .as_ref()
                .map(|binding| binding.as_str(&parsed)),
            Some("input_proof")
        );
        assert_eq!(
            contracts[1]
                .binding
                .as_ref()
                .map(|binding| binding.as_str(&parsed)),
            Some("output_proof")
        );
    }

    #[test]
    fn parses_nested_machine_requires() {
        let source = r#"
        machine walk(value: i32) {
            transition value > 0 {
                true -> positive(value)
                false -> done()
            }

            machine positive(value: i32)
            requires
                value > 0
            {
            }

            machine done() {
            }
        }
    "#;

        let parsed = super::parse_source(source, "nested machine requires should parse");
        let machine = super::root_machine(&parsed, "machine root item");
        let positive = machine
            .nested_machines
            .handles()
            .find_map(|handle| match parsed.items.item(handle) {
                Item::Machine(child) if child.name.as_str(&parsed) == "positive" => Some(child),
                _ => None,
            })
            .expect("positive machine");
        let contracts = parsed.items.capability_contracts(positive.contracts);
        assert_eq!(contracts.len(), 1);
        assert!(matches!(
            contracts[0].kind,
            CapabilityContractKind::Requires
        ));
        assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 1);
    }

    #[test]
    fn parses_unguarded_crash_bucket_on_a_bodyless_machine_head() {
        let source = r#"
        boundary machine == Float::equal(left: f32, right: f32) -> bool crashes Trap;

        machine compare(left: f32, right: f32) -> bool crashes Abort {
            left == right
        }
    "#;
        let parsed = super::parse_source(source, "bodyless crash bucket should parse");
        let machines = super::root_machines(&parsed);
        let [equal, compare] = machines.as_slice() else {
            panic!("two machine items");
        };
        assert!(equal.bodyless && equal.boundary);
        assert_eq!(
            equal.spelling,
            Some(syntax_trees::operator_spelling::OperatorSpelling::Equal)
        );
        let [contract] = parsed.items.capability_contracts(equal.contracts) else {
            panic!("the bodyless head carries one crash bucket");
        };
        let CapabilityContractKind::Crashes { cause } = &contract.kind else {
            panic!("crash bucket");
        };
        assert_eq!(*cause, CrashCause::Trap);
        assert!(parsed.items.proof_facts(contract.facts).is_empty());
        assert!(!compare.bodyless);
    }

    #[test]
    fn bare_bodyless_machine_signature_parses_and_bodyless_satisfies_without_via_rejects() {
        let tokens = Lexer::new("machine Float::meaning32(value: f32) -> FloatMeaning;")
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("bare bodyless signature should parse");
        let machine = super::root_machine(&parsed, "machine item");
        assert!(machine.bodyless && !machine.boundary && machine.spelling.is_none());
        assert_eq!(machine.name.as_str(&parsed), "Float::meaning32");

        let tokens = Lexer::new("machine realize(value: f32) -> f32 satisfies Math::identity;")
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("a bodyless realization needs `via`");
        super::expect_error(
            &error,
            "a machine without a body is the ACCEPTED boundary form",
        );
    }

    #[test]
    fn contextual_clause_names_retain_their_first_contract_proposition() {
        let parsed = super::parse_source_verbose(
            r#"machine check(blocks: i64, suspends: bool)
            requires blocks >= 0
            requires suspends
            blocks
            ensures blocks >= 0
            {}"#,
            "tokenize contextual contract subjects",
            "contextual subjects must not become empty contracts or effect clauses",
        );
        let machine = super::root_machine(&parsed, "machine");
        let contracts = parsed.items.capability_contracts(machine.contracts);
        assert_eq!(contracts.len(), 3);
        assert!(contracts.iter().all(|contract| contract.facts.count() == 1));
        assert!(machine.blocks);
    }
}
pub(crate) mod source_parsing {
    use crate::{parse, parse_source_units, parse_sources};
    use rayon::ThreadPoolBuilder;
    use source::{DependencyScope, LoadedSource, LoadedSources, SourceId, SourceOrigin};
    use source_files_to_tokens::{Lexer, lex_sources};
    use std::sync::Arc;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::Item;

    fn lexed_source(source_id: SourceId, path: &str, text: &str) -> tokens::LexedSources {
        let mut loaded = LoadedSources::default();
        loaded.batch = loaded.sources.insert_many([LoadedSource {
            source_id,
            path: path.into(),
            source: Arc::from(text),
            origin: Some(SourceOrigin::User),
            scope: DependencyScope::Build,
        }]);
        lex_sources(loaded).unwrap()
    }

    #[test]
    fn appending_sources_preserves_root_handles_and_source_identity() {
        let mut trees = SyntaxTrees::new(SourceId(11));
        let first_tokens = Lexer::new("data First {} /* trailing trivia */")
            .tokenize()
            .unwrap();
        let first = parse(&mut trees, SourceId(11), &first_tokens)
            .unwrap()
            .items;
        let original = trees.root_item(first[0]).clone();
        let lexed = lexed_source(SourceId(22), "second.omg", "data Second {} data Third {}");
        let backing = lexed.sources.span_or_empty(lexed.batch)[0].source.clone();
        let path_buffer = lexed.sources.span_or_empty(lexed.batch)[0]
            .path
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr();
        let parsed = parse_sources(lexed, &mut trees).unwrap();
        let sources = parsed.sources.span_or_empty(parsed.batch);
        assert_eq!(sources.len(), 1);
        let appended = &sources[0];
        assert_eq!(appended.source_id, SourceId(22));
        assert_eq!(appended.path, std::path::PathBuf::from("second.omg"));
        assert_eq!(appended.origin, Some(SourceOrigin::User));
        assert_eq!(appended.scope, DependencyScope::Build);
        assert!(Arc::ptr_eq(&appended.source, &backing));
        assert_eq!(
            appended.path.as_os_str().as_encoded_bytes().as_ptr(),
            path_buffer
        );
        let second = &appended.root_items;

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 2);
        assert_eq!(trees.root_item_handles(), [first[0], second[0], second[1]]);
        assert!(syntax_trees::SyntaxEquivalent::same_syntax(
            trees.root_item(first[0]),
            &original,
            &trees.text,
            &trees.text
        ));
        for (handle, expected_source) in [(first[0], SourceId(11)), (second[0], SourceId(22))] {
            let Item::Data(data) = trees.root_item(handle) else {
                panic!("data root")
            };
            assert_eq!(data.name.source_span().source_id, expected_source);
        }
        let trivia = Lexer::new(" // no declarations\n ").tokenize().unwrap();
        assert!(
            parse(&mut trees, SourceId(33), &trivia)
                .unwrap()
                .items
                .is_empty()
        );
        assert_eq!(trees.root_item_count(), 3);
        let empty = parse_sources(tokens::LexedSources::default(), &mut trees).unwrap();
        assert!(empty.sources.span_or_empty(empty.batch).is_empty());
        assert_eq!(trees.root_item_count(), 3);
    }

    #[test]
    fn parser_retains_one_worker_page_identity_through_nested_handles() {
        const PAGE_IDENTITY: u32 = 7;
        let source_id = SourceId(44);
        let tokens =
            Lexer::new("machine read(value: u64) -> u64 { let selected: u64 = value; selected }")
                .tokenize()
                .unwrap();
        let mut trees = SyntaxTrees::new_with_generation(source_id, PAGE_IDENTITY);
        let roots = parse(&mut trees, source_id, &tokens).unwrap().items;

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].generation(), PAGE_IDENTITY);
        assert!(
            trees
                .expressions
                .iter_expressions()
                .all(|(handle, _)| handle.generation() == PAGE_IDENTITY)
        );

        let Item::Machine(machine) = trees.root_item(roots[0]) else {
            panic!("machine root")
        };
        assert_eq!(machine.states.start().generation(), PAGE_IDENTITY);
        for state in trees.items.state_handles(machine.states) {
            assert_eq!(state.generation(), PAGE_IDENTITY);
            let state = trees.items.state(*state);
            assert_eq!(state.parameters.start().generation(), PAGE_IDENTITY);
            assert_eq!(state.statements.start().generation(), PAGE_IDENTITY);
            for parameter in trees.items.state_parameters(state.parameters) {
                assert_eq!(parameter.generation(), PAGE_IDENTITY);
                assert_eq!(
                    trees
                        .items
                        .state_parameter(*parameter)
                        .type_reference
                        .generation(),
                    PAGE_IDENTITY
                );
            }
            assert!(
                trees
                    .items
                    .statements(state.statements)
                    .iter()
                    .all(|statement| statement.generation() == PAGE_IDENTITY)
            );
        }
    }

    #[test]
    fn parallel_parser_pages_preserve_source_order_and_direct_handle_lookup() {
        let mut loaded = LoadedSources::default();
        loaded.batch = loaded.sources.insert_many((0..16).map(|position| {
            let padding = if position == 0 {
                "/* padding */".repeat(50_000)
            } else {
                String::new()
            };
            LoadedSource {
                source_id: SourceId(100 + position),
                path: format!("source-{position}.omg").into(),
                source: Arc::from(format!("{padding} data Item{position} {{}}")),
                origin: Some(SourceOrigin::User),
                scope: DependencyScope::Product,
            }
        }));
        let lexed = lex_sources(loaded).unwrap();
        let path_buffers = lexed
            .sources
            .span_or_empty(lexed.batch)
            .iter()
            .map(|source| source.path.as_os_str().as_encoded_bytes().as_ptr() as usize)
            .collect::<Vec<_>>();
        let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let parsed = pool.install(|| parse_source_units(lexed)).unwrap();

        assert_eq!(parsed.page_count(), 16);
        assert_eq!(parsed.sources().len(), 16);
        for (position, source) in parsed.sources().iter().enumerate() {
            assert_eq!(source.source().source_id, SourceId(100 + position));
            assert_eq!(source.source().root_items.len(), 1);
            assert_eq!(
                source.source().path.as_os_str().as_encoded_bytes().as_ptr() as usize,
                path_buffers[position]
            );
            let root = source.source().root_items[0];
            assert_eq!(root.generation(), source.syntax_handle().generation());
            let syntax = parsed.syntax_for(root).expect("root syntax page");
            let Item::Data(data) = syntax.root_item(root) else {
                panic!("data root")
            };
            assert_eq!(data.name.as_str(syntax), format!("Item{position}"));
        }
    }

    #[test]
    fn rejection_preserves_the_parsed_prefix_and_reports_the_current_source() {
        let mut trees = SyntaxTrees::new(SourceId(11));
        let tokens = Lexer::new("data Retained {} ;").tokenize().unwrap();
        let error = parse(&mut trees, SourceId(22), &tokens).unwrap_err();
        assert_eq!(error.source_span.source_id, SourceId(22));
        assert_eq!(error.source_span.span.start, 17);
        assert_eq!(trees.root_item_count(), 1);
        let Item::Data(data) = trees.root_items().next().unwrap() else {
            panic!("data root")
        };
        assert_eq!(data.name.as_str(&trees), "Retained");

        let errors = parse_sources(
            lexed_source(SourceId(33), "appended.omg", "data Another {}\n  ;"),
            &mut trees,
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.starts_with("appended.omg:2:3: "));
        assert_eq!(trees.root_item_count(), 2);
        let Item::Data(data) = trees.root_items().nth(1).unwrap() else {
            panic!("batch prefix must remain parsed");
        };
        assert_eq!(data.name.as_str(&trees), "Another");
        assert_eq!(data.name.source_span().source_id, SourceId(33));
    }

    #[test]
    fn import_bounds_preserve_interleaved_root_ordinals_on_both_parser_routes() {
        let cases = [
            ("", 0..0, vec![]),
            ("data Only {}", 0..0, vec![]),
            ("use alpha; data Tail {}", 0..1, vec![0]),
            ("data Head {} use alpha;", 1..2, vec![1]),
            (
                "data Head {} use alpha; let pi(): core::Real = term; data Middle {} use beta; data Tail {}",
                1..4,
                vec![1, 3],
            ),
        ];
        for (text, expected_range, expected_ordinals) in cases {
            let mut trees = SyntaxTrees::default();
            let parsed =
                parse_sources(lexed_source(SourceId(27), "imports.omg", text), &mut trees).unwrap();
            let source = &parsed.sources.span_or_empty(parsed.batch)[0];
            assert_eq!(source.import_root_range, expected_range);
            let actual = source.root_items[source.import_root_range.clone()]
                .iter()
                .enumerate()
                .filter_map(|(offset, root)| {
                    matches!(trees.root_item(*root), Item::Use(_))
                        .then_some(source.import_root_range.start + offset)
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected_ordinals);

            let pages =
                parse_source_units(lexed_source(SourceId(27), "imports.omg", text)).unwrap();
            let source = pages.sources()[0].source();
            assert_eq!(source.import_root_range, expected_range);
            let actual = source.root_items[source.import_root_range.clone()]
                .iter()
                .enumerate()
                .filter_map(|(offset, root)| {
                    matches!(
                        pages.syntax_for(*root).unwrap().root_item(*root),
                        Item::Use(_)
                    )
                    .then_some(source.import_root_range.start + offset)
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected_ordinals);
        }
    }

    #[test]
    fn syntax_names_remain_valid_after_all_token_and_source_rows_are_released() {
        let lexed = lexed_source(SourceId(55), "retained.omg", "data Retained {}");
        let mut syntax = SyntaxTrees::default();
        let parsed = parse_sources(lexed, &mut syntax).unwrap();
        let root = parsed.sources.span_or_empty(parsed.batch)[0].root_items[0];
        drop(parsed);
        let Item::Data(data) = syntax.root_item(root) else {
            panic!("data root")
        };
        assert_eq!(data.name.as_str(&syntax), "Retained");
        assert_eq!(data.name.source_span().source_id, SourceId(55));
        let retained_name = data.name.clone();
        let retained_text = syntax.text.clone();
        drop(syntax);
        assert_eq!(retained_name.as_str(&retained_text), "Retained");
    }

    #[test]
    fn float_spellings_share_forest_custody_and_survive_cross_forest_copy() {
        use syntax_trees::expression::ExpressionNode;
        let source: Arc<str> = Arc::from(
            "machine numbers() { let first: f32 = 1.25f32; let second: f64 = -2.5e-2f64; first }",
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .unwrap()
            .retain_source(source.clone());
        let syntax = crate::parse_syntax_trees(&tokens).unwrap();
        drop(tokens);
        assert_eq!(
            Arc::strong_count(&source),
            2,
            "one forest owner, no literal owners"
        );
        let original = syntax
            .expressions
            .iter_expressions()
            .filter_map(|(handle, node)| matches!(node, ExpressionNode::Float(_)).then_some(handle))
            .collect::<Vec<_>>();
        assert_eq!(
            original.len(),
            3,
            "positive, unsigned operand, folded negative"
        );
        let expected = original
            .iter()
            .map(|handle| {
                (
                    syntax.display_expression_name(*handle),
                    syntax.expressions.source_span(*handle),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            expected
                .iter()
                .any(|(spelling, _)| spelling == "-2.5e-2f64")
        );
        let mut copied = SyntaxTrees::default();
        let handles = original
            .iter()
            .map(|handle| copied.copy_expression_from(&syntax, *handle))
            .collect::<Vec<_>>();
        assert_eq!(
            Arc::strong_count(&source),
            3,
            "copies retain one page, not one owner per literal"
        );
        drop(syntax);
        assert_eq!(Arc::strong_count(&source), 2);
        drop(source);
        for (handle, (spelling, span)) in handles.into_iter().zip(expected) {
            assert_eq!(copied.display_expression_name(handle), spelling);
            assert_eq!(copied.expressions.source_span(handle), span);
        }
    }
}
pub(crate) mod static_parameters {
    use crate::parse_syntax_trees;
    use source_files_to_tokens::Lexer;

    #[test]
    fn retired_receiver_parameters_reject_in_every_signature_owner() {
        for parameter in ["self", "mut self", "&self", "&mut self", "&write self"] {
            for source in [
                format!("data Counter {{ value: u32; }} machine Counter::step({parameter}) {{}}"),
                format!("trait Operation {{ machine step({parameter}); }}"),
                format!("boundary trait Operation {{ machine step({parameter}) reaches; }}"),
                format!("machine run() {{ machine step({parameter}) {{}} }}"),
            ] {
                let tokens = Lexer::new(&source).tokenize().unwrap();
                let error = parse_syntax_trees(&tokens).expect_err("receiver must reject");
                super::expect_error(&error, "`self` receivers are retired");
            }
        }
    }

    #[test]
    fn implicit_self_expressions_and_transition_targets_reject() {
        for body in [
            "self",
            "self.value",
            "transition { _ -> self }",
            "transition { _ -> self() }",
        ] {
            let source = format!("machine run() {{ {body} }}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let error = parse_syntax_trees(&tokens).expect_err("implicit subject must reject");
            super::expect_error(&error, "implicit `self` is retired");
        }
    }

    #[test]
    fn explicit_owned_shared_mutable_and_write_only_parameters_parse() {
        let source = r#"
        data Counter { value: u32; }
        trait Readable<Readout> { machine read(counter: &Readout) -> u32; }
        machine consume(counter: Counter) {}
        machine read(counter: &Counter) -> u32 { counter.value }
        machine write(counter: &write Counter) { counter.value = 1; }
        machine Counter::clear(counter: &mut Counter) {
            transition { _ -> finish(counter) }
            machine finish(counter: &mut Counter) { counter.value = 0; }
        }
    "#;
        let tokens = Lexer::new(source).tokenize().unwrap();
        parse_syntax_trees(&tokens).expect("ordinary explicit parameters remain valid");
    }
}
pub(crate) mod type_constraints {
    use crate::parser::parse_syntax_trees;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::BinaryOperator;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::ProofFact;
    use syntax_trees::types::TypeReferenceNode;

    #[test]
    fn unary_const_arguments_retain_their_typed_evaluation_obligation() {
        use syntax_trees::item::{DataMember, Item};

        for argument in [
            "!false", "(!false)", "!!true", "~1u8", "(~1u8)", "!0", "~false",
        ] {
            let source = format!("data Main {{ value: Flag<{argument}>; }}");
            let parsed = super::parse_source_verbose(
                &source,
                "tokenize unary argument",
                "retain unary argument syntax",
            );
            let Item::Data(data) = parsed.root_items().next().unwrap() else {
                panic!("data declaration");
            };
            let [DataMember::Field(field)] = parsed.items.data_members(data.members) else {
                panic!("one field");
            };
            let TypeReferenceNode::Generic { arguments, .. } =
                parsed.type_references.type_reference(field.type_reference)
            else {
                panic!("generic field");
            };
            let [argument] = parsed.type_references.type_reference_handles(*arguments) else {
                panic!("one argument");
            };
            let TypeReferenceNode::ConstExpression(expression) =
                parsed.type_references.type_reference(*argument)
            else {
                panic!("retain expression until its carrier is known");
            };
            assert!(matches!(
                parsed.expressions.expression(*expression),
                ExpressionNode::Unary(_)
            ));
        }
    }

    #[test]
    fn fixed_array_equation_operands_retain_type_structure_through_grouping_and_copy() {
        for equation in [
            "Backing == [[Element; Count]; 2]",
            "(Backing == [[Element; Count]; 2])",
            "([[Element; Count]; 2]) == Backing",
        ] {
            let source = format!(
                "data Buffer<Backing, Element, const Count: u64> requires {equation} {{ storage: Backing; }}"
            );
            let parsed = super::parse_source_verbose(
                &source,
                "tokenize equation",
                "parse fixed-array type equation",
            );
            let item = parsed.root_items().next().expect("data item");
            let mut copied = syntax_trees::SyntaxTrees::default();
            let copied_item = copied.copy_item_from(&parsed, item);
            for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
                let syntax_trees::item::Item::Data(data) = item else {
                    panic!("expected data");
                };
                let [ProofFact::Expression(fact)] = trees.items.proof_facts(data.where_facts)
                else {
                    panic!("expected one equation");
                };
                let ExpressionNode::Binary(equation) = trees.expressions.expression(*fact) else {
                    panic!("expected equality");
                };
                assert_eq!(equation.operator, BinaryOperator::Equal);
                let operand = [equation.left, equation.right]
                    .into_iter()
                    .find_map(|operand| match trees.expressions.expression(operand) {
                        ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
                        _ => None,
                    })
                    .expect("one operand retains a type reference");
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: syntax_trees::types::FixedArrayLength::Literal(2),
                } = trees.type_references.type_reference(operand)
                else {
                    panic!("expected outer fixed array");
                };
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: syntax_trees::types::FixedArrayLength::ConstParameter(count),
                } = trees.type_references.type_reference(*element_type)
                else {
                    panic!("expected nested fixed array");
                };
                assert_eq!(count.as_str(trees), "Count");
                let TypeReferenceNode::Named(element) =
                    trees.type_references.type_reference(*element_type)
                else {
                    panic!("expected element binder");
                };
                assert_eq!(element.as_str(trees), "Element");
            }
        }
    }

    #[test]
    fn proof_fact_array_values_remain_value_expressions() {
        for equation in [
            "[1, 2] == [1, 2]",
            "([[1, 2], [3, 4]] == [[1, 2], [3, 4]])",
            "[match 1 { 1 -> 2, _ -> 3 }] == [2]",
            "&[1, 2] == &[1, 2]",
        ] {
            let source = format!("data Buffer requires {equation} {{ storage: u64; }}");
            let parsed = super::parse_source_verbose(
                &source,
                "tokenize array values",
                "parse array value equality",
            );
            assert!(
                parsed
                    .expressions
                    .iter_expressions()
                    .any(|(_, expression)| {
                        matches!(expression, ExpressionNode::ArrayLiteral(_))
                    })
            );
            assert!(
                !parsed
                    .expressions
                    .iter_expressions()
                    .any(|(_, expression)| {
                        matches!(expression, ExpressionNode::TypeExpression(_))
                    })
            );
        }
    }

    #[test]
    fn reference_operands_retain_access_and_element_type_role_through_copy() {
        for (access, expected) in [
            ("", language_core::ReferenceAccess::Shared),
            ("mut ", language_core::ReferenceAccess::Mutable),
            ("write ", language_core::ReferenceAccess::WriteOnly),
        ] {
            let source = format!(
                "data Buffer<Backing, Element, const Count: u64> requires Backing == (&{access}[&Element; Count]) {{ storage: Backing; }}"
            );
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let parsed = parse_syntax_trees(&tokens).expect("reference around a type-role operand");
            let item = parsed.root_items().next().unwrap();
            let mut copied = syntax_trees::SyntaxTrees::default();
            let copied_item = copied.copy_item_from(&parsed, item);
            for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
                let syntax_trees::item::Item::Data(data) = item else {
                    panic!("data");
                };
                let [ProofFact::Expression(fact)] = trees.items.proof_facts(data.where_facts)
                else {
                    panic!("equation");
                };
                let ExpressionNode::Binary(equation) = trees.expressions.expression(*fact) else {
                    panic!("equality");
                };
                let ExpressionNode::Borrow(borrow) = trees.expressions.expression(equation.right)
                else {
                    panic!("borrow syntax retains access until type-role resolution");
                };
                assert_eq!(borrow.access, expected);
                let ExpressionNode::TypeExpression(reference) =
                    trees.expressions.expression(borrow.target)
                else {
                    panic!("array operand retains its type role");
                };
                let TypeReferenceNode::FixedArray { element_type, .. } =
                    trees.type_references.type_reference(*reference)
                else {
                    panic!("array");
                };
                assert!(matches!(
                    trees.type_references.type_reference(*element_type),
                    TypeReferenceNode::Reference {
                        access: language_core::ReferenceAccess::Shared,
                        lifetime: None,
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn reference_value_operands_remain_borrows_and_recasts_reject_policy_suffixes() {
        let tokens = Lexer::new("data Buffer requires &left == &right { storage: u64; }")
            .tokenize()
            .unwrap();
        let parsed = parse_syntax_trees(&tokens).unwrap();
        assert!(
            !parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| matches!(expression, ExpressionNode::TypeExpression(_)))
        );
        for target in ["&u8", "& &u8", "&mut &write u8"] {
            let source =
                format!("machine view(value: &u64) -> &u8 {{ value as {target} in Wrapping }}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let error = parse_syntax_trees(&tokens).expect_err("recast policy must reject");
            assert!(
                error.message.contains("takes no arithmetic domain"),
                "{error:?}"
            );
            let without_policy = source.replace(" in Wrapping", "");
            let tokens = Lexer::new(&without_policy).tokenize().unwrap();
            parse_syntax_trees(&tokens).expect("the reference target itself is valid syntax");
        }
    }

    #[test]
    fn fixed_array_equation_elements_use_existing_generic_type_grammar() {
        let source = "data Buffer<Backing, Element, const Count: u64> requires Backing == [Pair<Element, Element>; Count] { storage: Backing; }";
        let parsed = super::parse_source_verbose(
            source,
            "tokenize generic element",
            "parse generic array element",
        );
        let array = parsed
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
                _ => None,
            })
            .expect("retained array type operand");
        let TypeReferenceNode::FixedArray { element_type, .. } =
            parsed.type_references.type_reference(array)
        else {
            panic!("expected fixed array");
        };
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = parsed.type_references.type_reference(*element_type)
        else {
            panic!("expected generic element");
        };
        assert_eq!(base_name.as_str(&parsed), "Pair");
        assert_eq!(
            parsed
                .type_references
                .type_reference_handles(*arguments)
                .len(),
            2
        );
    }

    #[test]
    fn fixed_array_equation_operands_use_the_type_grammars_rejections() {
        for operand in ["[Element;]", "[; 4]"] {
            let source = format!(
                "data Buffer<Backing, Element, const Count: u64> requires Backing == {operand} {{ storage: Backing; }}"
            );
            let tokens = Lexer::new(&source)
                .tokenize()
                .expect("tokenize malformed type");
            assert!(parse_syntax_trees(&tokens).is_err(), "{operand}");
        }
        for operand in ["[Element; -1]", "[Element; Count + 1]"] {
            let equation = format!(
                "data Buffer<Backing, Element, const Count: u64> requires Backing == {operand} {{ storage: Backing; }}"
            );
            let parsed = super::parse_source_verbose(
                &equation,
                "tokenize computed extent",
                "computed extents are retained for semantic evaluation",
            );
            let operand_type = parsed
                .expressions
                .iter_expressions()
                .find_map(|(_, expression)| match expression {
                    ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
                    _ => None,
                })
                .expect("the equation operand keeps its type role");
            let field = format!("data Buffer<Element, const Count: u64> {{ storage: {operand}; }}");
            let field_parsed = super::parse_source_verbose(
                &field,
                "tokenize computed extent",
                "computed extents are retained for semantic evaluation",
            );
            let syntax_trees::item::Item::Data(data) =
                field_parsed.root_items().next().expect("data item")
            else {
                panic!("data");
            };
            let [DataMember::Field(storage)] = field_parsed.items.data_members(data.members) else {
                panic!("one field");
            };
            for (trees, array) in [
                (&parsed, operand_type),
                (&field_parsed, storage.type_reference),
            ] {
                let TypeReferenceNode::FixedArray {
                    length: syntax_trees::types::FixedArrayLength::Expression(extent),
                    ..
                } = trees.type_references.type_reference(array)
                else {
                    panic!("computed extent retained as an expression: {operand}");
                };
                assert!(matches!(
                    trees.type_references.type_reference(*extent),
                    TypeReferenceNode::ConstExpression(_)
                ));
            }
        }
    }

    #[test]
    fn fixed_array_type_operand_is_not_runtime_value_syntax() {
        let parsed = super::parse_source(
            "machine main() -> u64 { [u8; 4] }",
            "type operands share the complete expression grammar",
        );
        assert!(
            parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| matches!(expression, ExpressionNode::TypeExpression(_)))
        );
        assert!(
            !parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| matches!(expression, ExpressionNode::ArrayLiteral(_)))
        );
    }

    #[test]
    fn comma_constraint_slots_inside_type_brackets_reject() {
        for source in [
            "data Main { value: &[u8, [non_empty]]; }",
            "data Main { value: [u8, x <= 10]; }",
            "data Main { value: [i32, 0..=10]; }",
            "machine m(x: [u8, nonzero]) { }",
        ] {
            let tokens = Lexer::new(source).tokenize().expect("tokenize");
            assert!(
                parse_syntax_trees(&tokens).is_err(),
                "inside-bracket constraint slot must reject: {source}"
            );
        }
    }
}
pub(crate) mod value_dispatch {
    use crate::grammar::expressions::parse_expression::parse_expression_handle;
    use crate::tokens::token_cursor::Input;
    use source::SourceId;
    use source_files_to_tokens::Lexer;
    use syntax_trees::expression::{ExpressionNode, MatchPattern};

    #[test]
    fn value_match_retains_subject_once_ordered_duplicate_arms_and_spans() {
        for separator in [" ", ", "] {
            let source = format!(
                "match read() {{ 0 -> first(){separator}0 -> second(){separator}_ -> last() }}"
            );
            let source_id = SourceId::default();
            let tokens = Lexer::new(&source).tokenize().expect("tokens");
            let mut trees = syntax_trees::SyntaxTrees::new(source_id);
            let retained_source = Input::retain_source(&tokens);
            let input = Input::new(source_id, &tokens, &retained_source, &mut trees);
            let (expression, rest) =
                parse_expression_handle(&mut trees, input).expect("ordered value match");
            assert!(rest.tokens.is_empty());
            let ExpressionNode::Match(dispatch) = trees.expressions.expression(expression) else {
                panic!("match must remain dispatch, not arithmetic");
            };
            assert!(matches!(
                trees.expressions.expression(dispatch.subject),
                ExpressionNode::Call(_)
            ));
            let arms = trees.expressions.match_arms(dispatch.arms);
            assert_eq!(arms.len(), 3);
            assert!(matches!(arms[0].pattern, MatchPattern::Value(_)));
            assert!(matches!(arms[1].pattern, MatchPattern::Value(_)));
            assert!(matches!(arms[2].pattern, MatchPattern::Wildcard));
            for (arm, target) in arms.iter().zip(["first", "second", "last"]) {
                let ExpressionNode::Call(call) = trees.expressions.expression(arm.value) else {
                    panic!("arm call");
                };
                assert_eq!(call.target.as_str(&trees), target);
                assert!(arm.source_span.span.end > arm.source_span.span.start);
            }
            assert_eq!(
                trees
                    .expressions
                    .iter_expressions()
                    .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
                    .count(),
                4
            );
            assert!(
                !trees
                    .expressions
                    .iter_expressions()
                    .any(|(_, node)| matches!(node, ExpressionNode::Binary(_)))
            );
        }
    }

    #[test]
    fn machine_tail_match_is_a_value_dispatch_not_source_transitions() {
        let tokens =
            Lexer::new("machine choose(value: i64) -> i64 { match value { 0 -> 7, _ -> value } }")
                .tokenize()
                .expect("tokens");
        let trees = crate::parser::parse_syntax_trees(&tokens).expect("tail value dispatch");
        assert_eq!(
            trees
                .expressions
                .iter_expressions()
                .filter(|(_, node)| matches!(node, ExpressionNode::Match(_)))
                .count(),
            1
        );
    }

    #[test]
    fn nested_initializer_match_retains_expression_local_dispatch() {
        let tokens = Lexer::new("machine choose(value: i64) -> i64 { let result: i64 = match value { 0 -> match value { _ -> 7 }, _ -> value }; result }")
        .tokenize().expect("tokens");
        let trees =
            crate::parser::parse_syntax_trees(&tokens).expect("nested initializer dispatch");
        let machine = trees
            .root_items()
            .find_map(|item| match item {
                syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine");
        let state = trees
            .items
            .state(trees.items.state_handles(machine.states)[0]);
        let local = trees
            .items
            .statements(state.statements)
            .iter()
            .find_map(|statement| match trees.statements.statement(*statement) {
                syntax_trees::statement::StatementNode::LocalData(local) => Some(local),
                _ => None,
            })
            .expect("initializer owner");
        let ExpressionNode::Match(outer) = trees.expressions.expression(local.initial_value) else {
            panic!("initializer retains outer dispatch");
        };
        let arms = trees.expressions.match_arms(outer.arms);
        assert_eq!(arms.len(), 2);
        let ExpressionNode::Match(inner) = trees.expressions.expression(arms[0].value) else {
            panic!("selected arm retains nested dispatch");
        };
        assert_eq!(trees.expressions.match_arms(inner.arms).len(), 1);
        assert!(matches!(
            trees.expressions.expression(arms[1].value),
            ExpressionNode::Name(_)
        ));
    }

    #[test]
    fn value_match_rejects_empty_dispatch_and_duplicate_bindings() {
        for source in [
            "match value {}",
            "match value { Record { first as same, second as same } -> same, _ -> 0 }",
        ] {
            let tokens = Lexer::new(source).tokenize().expect("tokens");
            let mut trees = syntax_trees::SyntaxTrees::new(SourceId::default());
            let retained_source = Input::retain_source(&tokens);
            let input = Input::new(SourceId::default(), &tokens, &retained_source, &mut trees);
            let error = parse_expression_handle(&mut trees, input)
                .err()
                .expect("empty dispatch or duplicate bindings reject");
            super::expect_error(
                &error,
                if source.contains("Record") {
                    "introduced more than once"
                } else {
                    "at least one arm"
                },
            );
        }
    }
}

fn parse_source(source: &str, parse_message: &str) -> SyntaxTrees {
    parse_source_verbose(source, "tokenize should succeed", parse_message)
}

fn parse_source_verbose(source: &str, tokenize_message: &str, parse_message: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect(tokenize_message);
    crate::parser::parse_syntax_trees(&tokens).expect(parse_message)
}

fn parse_source_unwrap(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    crate::parser::parse_syntax_trees(&tokens).unwrap()
}

fn argument_path<'p>(parsed: &'p SyntaxTrees, path: &'p [Identifier]) -> Vec<&'p str> {
    path.iter().map(|member| member.as_str(parsed)).collect()
}

macro_rules! root_item {
    ($name:ident -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name<'p>(
            parsed: &'p syntax_trees::SyntaxTrees,
            message: &str,
        ) -> &'p syntax_trees::item::$ty {
            parsed
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .expect(message)
        }
    };
    ($name:ident unwrap -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name(parsed: &syntax_trees::SyntaxTrees) -> &syntax_trees::item::$ty {
            parsed
                .root_items()
                .find_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .unwrap()
        }
    };
    ($name:ident all -> $variant:ident ($binding:ident): $ty:ident ;) => {
        fn $name(parsed: &syntax_trees::SyntaxTrees) -> Vec<&syntax_trees::item::$ty> {
            parsed
                .root_items()
                .filter_map(|item| match item {
                    syntax_trees::item::Item::$variant($binding) => Some($binding),
                    _ => None,
                })
                .collect()
        }
    };
}

root_item!(root_machine -> Machine(machine): Machine;);
root_item!(root_machine_unwrap unwrap -> Machine(machine): Machine;);
root_item!(root_trait -> Trait(definition): TraitDefinition;);
root_item!(root_data -> Data(definition): DataDefinition;);
root_item!(root_domain -> Domain(definition): DomainDefinition;);
root_item!(root_conformance -> Conformance(item): ConformanceItem;);
root_item!(root_const -> Const(definition): ConstDefinition;);
root_item!(root_operator -> Operator(definition): OperatorDefinition;);
root_item!(root_machines all -> Machine(machine): Machine;);
root_item!(root_domains all -> Domain(domain): DomainDefinition;);
root_item!(root_propositions all -> Proposition(proposition): PropositionDefinition;);
root_item!(root_traits all -> Trait(definition): TraitDefinition;);
root_item!(root_data_items all -> Data(definition): DataDefinition;);
root_item!(root_operators all -> Operator(operator): OperatorDefinition;);

fn entry_state_handle(parsed: &SyntaxTrees, machine: &syntax_trees::item::Machine) -> StateHandle {
    parsed
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state")
}

fn entry_state<'p>(parsed: &'p SyntaxTrees, machine: &syntax_trees::item::Machine) -> &'p State {
    parsed.items.state(entry_state_handle(parsed, machine))
}

fn parse_source_error(source: &str, tokenize_message: &str, error_message: &str) -> ParseError {
    let tokens = Lexer::new(source).tokenize().expect(tokenize_message);
    crate::parser::parse_syntax_trees(&tokens).expect_err(error_message)
}

fn parse_tokenized_error(source: &str, error_message: &str) -> ParseError {
    parse_source_error(source, "tokenize should succeed", error_message)
}

fn expect_error(error: &ParseError, needle: &str) {
    assert!(
        error.message.contains(needle),
        "unexpected diagnostic: {}",
        error.message
    );
}
