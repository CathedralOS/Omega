use super::parse_syntax_trees;
use psi_language_core::ReferenceAccess;
use psi_source_files_to_tokens::Lexer;
use psi_syntax_trees::expression::ExpressionNode;
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::TypeReferenceNode;

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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
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
        psi_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(psi_syntax_trees::item::MachineParameterContract::RequirementIdentity)
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("public machine");

    assert!(machine.is_public);
    assert!(!machine.boundary);
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
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
        psi_syntax_trees::item::Item::Trait(definition) if definition.is_public
    )));
    assert!(parsed.root_items().any(|item| matches!(
        item,
        psi_syntax_trees::item::Item::WireData(definition) if definition.is_public
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
fn parses_relevance_on_numbered_wire_fields() {
    let tokens = Lexer::new(
        r#"
        data Message {
            #0 value: u32;
            #1 proof [erased]: Evidence;
            version v0 {
                #7 historical_proof [erased]: Evidence;
            }
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("numbered relevance should parse");
    let schema = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::WireData(schema) => Some(schema),
            _ => None,
        })
        .expect("wire schema");
    let fields = parsed
        .items
        .wire_data_members(schema.members)
        .iter()
        .filter_map(|member| match member {
            psi_syntax_trees::item::WireDataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert!(!fields[0].relevance.is_erased());
    assert!(fields[1].relevance.is_erased());
    let version = parsed
        .items
        .wire_data_members(schema.members)
        .iter()
        .find_map(|member| match member {
            psi_syntax_trees::item::WireDataMember::Version(version) => Some(version),
            _ => None,
        })
        .expect("wire version");
    let [psi_syntax_trees::item::WireDataMember::Field(historical)] =
        parsed.items.wire_data_members(version.members)
    else {
        panic!("historical wire field");
    };
    assert!(historical.relevance.is_erased());
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
            psi_syntax_trees::item::Item::Proposition(proposition) => Some(proposition),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(propositions.len(), 3);
    assert!(propositions[0].is_public);
    assert!(!propositions[1].is_public);
    assert!(matches!(
        propositions[0].body,
        psi_syntax_trees::item::PropositionBody::Primitive
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
        psi_syntax_trees::item::PropositionBody::Witness { .. }
    ));
    assert!(matches!(
        propositions[2].body,
        psi_syntax_trees::item::PropositionBody::Transparent { proposition }
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
            psi_syntax_trees::item::Item::Const(declaration) => Some(declaration),
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
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
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [trait_definition] = trait_definitions.as_slice() else {
        panic!("one trait expected");
    };
    let parameters = parsed
        .items
        .type_parameters(trait_definition.type_parameters);
    let psi_syntax_trees::item::TypeParameterKind::Proposition {
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

#[test]
fn parses_compiler_intrinsic_external_binding_as_a_closed_binding_case() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
        }

        machine console_write_byte(byte: i32)
        satisfies Console::write_byte
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("compiler intrinsic binding should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine)
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
        Some(psi_syntax_trees::item::ExternalBinding::CompilerIntrinsic)
    ));
    let via_source_span = clause
        .via_keyword_source_span
        .expect("external binding retains exact `via` custody");
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
        via Binding::CompilerIntrinsic("Console::write_byte");
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data declaration");
    let members = parsed.items.data_members(data.members);
    let psi_syntax_trees::item::DataMember::Field(value) = &members[0] else {
        panic!("value field");
    };
    let psi_syntax_trees::item::DataMember::Field(proof) = &members[1] else {
        panic!("proof field");
    };
    let psi_syntax_trees::item::DataMember::Variant(wrapped) = &members[2] else {
        panic!("wrapped case");
    };
    let [witness] = parsed.items.data_payload_fields(wrapped.payload) else {
        panic!("one payload field");
    };

    assert_eq!(
        value.relevance,
        psi_language_core::BindingRelevance::Relevant
    );
    assert_eq!(proof.relevance, psi_language_core::BindingRelevance::Erased);
    assert_eq!(
        witness.relevance,
        psi_language_core::BindingRelevance::Erased
    );
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
            psi_syntax_trees::item::Item::Machine(machine) if machine.name == "zero_is_none" => {
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data declaration");
    assert_eq!(data.lifetime_parameters.len(), 1);
    assert_eq!(data.lifetime_parameters[0].as_str(), "buf");
    assert_eq!(data.type_parameters.count(), 1);

    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
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
        trait Converter<Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<Source, u64>
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    let parameters = parsed.items.type_parameters(conformance.type_parameters);
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].name.as_str(), "Source");
    assert!(matches!(
        parameters[0].kind,
        psi_syntax_trees::item::TypeParameterKind::Type
    ));
    assert_eq!(parameters[1].name.as_str(), "Width");
    assert!(matches!(
        parameters[1].kind,
        psi_syntax_trees::item::TypeParameterKind::Const { .. }
    ));
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(contract)),
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");
    assert!(matches!(
        conformance.subject,
        psi_syntax_trees::item::ConformanceSubject::Subjectless
    ));
    assert_eq!(
        conformance.alias.as_ref().map(|alias| alias.as_str()),
        Some("ConcreteEvidence")
    );
    assert!(matches!(
        conformance.body,
        psi_syntax_trees::item::ConformanceBody::Closed { .. }
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
            psi_syntax_trees::item::Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("conformance root item");
    let psi_syntax_trees::item::ConformanceBody::Closed { members } = conformance.body else {
        panic!("block must remain structurally distinct from bodyless attached-requirement lookup");
    };
    let [inline, reference] = parsed.items.conformance_members(members) else {
        panic!("two retained conformance members");
    };
    assert!(matches!(
        inline,
        psi_syntax_trees::item::ConformanceMember::Machine(machine)
            if machine.name.as_str() == "before"
    ));
    let psi_syntax_trees::item::ConformanceMember::Reference {
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
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "inspect" =>
            {
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
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "inspect" =>
            {
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Trait(trait_definition)
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
                .contains("satisfies ... via Binding::DllImport"),
        "got: {}",
        error.message
    );
}

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
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize retired capability entry");
    let error = parse_syntax_trees(&tokens).expect_err("capability entry must be retired");
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
    let Some(psi_syntax_trees::item::Item::Capability(capability)) = syntax.root_items().next()
    else {
        panic!("expected capability");
    };
    let [
        psi_syntax_trees::item::CapabilityMember::Field(field),
        psi_syntax_trees::item::CapabilityMember::State(_),
    ] = syntax.items.capability_members(capability.members)
    else {
        panic!("expected ordinary field followed by current state member");
    };
    assert_eq!(field.name.as_str(), "entry");
}

#[test]
fn retired_explicit_machine_entry_members_name_the_machine_body_migration() {
    for retired in [
        "machine run { entry() {} }",
        "machine run { entry begin() {} }",
        "machine run { pub entry() {} }",
        "machine run { pub entry begin() {} }",
    ] {
        let tokens = Lexer::new(retired)
            .tokenize()
            .expect("tokenize retired explicit machine entry");
        let error = parse_syntax_trees(&tokens)
            .expect_err("explicit machine entry members must be retired");
        assert!(
            error
                .message
                .contains("explicit nested `entry` / `pub entry` machine members are retired")
                && error.message.contains("`machine` head")
                && error.message.contains("directly in the machine body")
                && error.message.contains("`pub machine`"),
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
        let tokens = Lexer::new(retired)
            .tokenize()
            .expect("tokenize retired trailing boundary contract");
        let error = parse_syntax_trees(&tokens)
            .expect_err("trailing boundary contract clauses must be retired");
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .collect();

    assert_eq!(data.len(), 2);
    assert_eq!(
        data[0].properties.multiplicity,
        psi_language_core::Multiplicity::Linear
    );
    assert_eq!(
        data[1].properties.multiplicity,
        psi_language_core::Multiplicity::Linear
    );
    let parameter = &parsed.items.type_parameters(data[1].type_parameters)[0];
    assert_eq!(
        parameter.bounds.multiplicity,
        psi_language_core::Multiplicity::Linear
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
    use psi_language_core::{
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data item");

    assert_eq!(
        data.supply_mode,
        psi_language_core::DataSupplyMode::BoundaryOpaque
    );
    assert!(data.members.is_empty());
    assert_eq!(
        data.properties.multiplicity,
        psi_language_core::Multiplicity::Linear
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
            psi_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
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
fn parses_trait_owned_fixed_operator_requirements() {
    let source = r#"
        trait Ranked<T> {
            operator < compare(left: T, right: T) -> bool;
            operator equivalent(left: T, right: T) -> bool;
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("trait operators should parse");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("Ranked trait");
    let requirements = parsed.items.state_signatures(trait_definition.machines);

    assert_eq!(requirements.len(), 2);
    assert_eq!(
        parsed.items.state_signature(requirements[0]).spelling,
        Some(psi_language_core::OperatorSpelling::Less)
    );
    assert_eq!(parsed.items.state_signature(requirements[1]).spelling, None);
}

#[test]
fn parses_independent_operational_clauses_on_machines_and_requirements() {
    let source = r#"
        machine run() reaches Console suspends; blocks; {
        }

        trait Worker {
            machine wait() reaches Clock suspends; blocks; ensures true;
        }

        machine schedule<machine Callback>()
        where machine Callback() suspends; blocks;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("operational clauses should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine item");
    assert!(machine.suspends);
    assert!(machine.blocks);
    assert_eq!(machine.suspends_keyword_source_spans.len(), 1);
    assert_eq!(machine.blocks_keyword_source_spans.len(), 1);
    let service_reaches = parsed
        .items
        .identifier_path_members(machine.service_reaches);
    assert_eq!(service_reaches.len(), 1);
    assert_eq!(service_reaches[0].as_str(), "Console");

    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait item");
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
    assert_eq!(service_reaches[0].as_str(), "Clock");

    let structural_machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "schedule" =>
            {
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
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(contract)),
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("crash buckets should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine item");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    let psi_syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contracts[0].kind
    else {
        panic!("first contract should be a crash bucket");
    };
    assert_eq!(*cause, psi_syntax_trees::item::CrashCause::Trap);
    assert_eq!(parsed.items.proof_facts(contracts[0].facts).len(), 2);

    let psi_syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contracts[1].kind
    else {
        panic!("second contract should be a crash bucket");
    };
    assert_eq!(*cause, psi_syntax_trees::item::CrashCause::Abort);
    assert!(parsed.items.proof_facts(contracts[1].facts).is_empty());

    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait item");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
    let [contract] = parsed.items.capability_contracts(signature.contracts) else {
        panic!("requirement should carry one crash bucket");
    };
    let psi_syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contract.kind else {
        panic!("requirement contract should be a crash bucket");
    };
    assert_eq!(*cause, psi_syntax_trees::item::CrashCause::Abort);
    assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
}

#[test]
fn parses_guarded_crash_bucket_on_operator_contract() {
    let source = r#"
        operator / divide(numerator: i32, denominator: i32) -> i32
        crashes Trap
            denominator == 0;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("operator crash bucket should parse");
    let operator = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Operator(operator) => Some(operator),
            _ => None,
        })
        .expect("operator item");
    let [contract] = parsed.items.capability_contracts(operator.contracts) else {
        panic!("operator should carry one crash bucket");
    };
    let psi_syntax_trees::item::CapabilityContractKind::Crashes { cause } = &contract.kind else {
        panic!("operator contract should be a crash bucket");
    };
    assert_eq!(*cause, psi_syntax_trees::item::CrashCause::Trap);
    assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    let keyword = contract
        .keyword_source_span
        .expect("operator crash keyword span");
    assert_eq!(&source[keyword.span.start..keyword.span.end], "crashes");
}

#[test]
fn parses_fixed_operator_tokens_in_declaration_heads() {
    let source = r#"
        pub operator + add(left: i32, right: i32) -> i32;
        boundary operator [] Slice::index(items: &[u8], index: u64) -> u8;
        boundary operator [..] Slice::range(items: &[u8], start: u64, end: u64) -> &[u8];
        operator named(left: i32, right: i32) -> i32;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("fixed-token heads should parse");
    let operators = parsed
        .root_items()
        .filter_map(|item| match item {
            psi_syntax_trees::item::Item::Operator(operator) => Some(operator),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(operators.len(), 4);
    assert!(operators[0].is_public);
    assert!(operators[1..].iter().all(|operator| !operator.is_public));
    assert_eq!(
        operators[0].spelling,
        Some(psi_language_core::OperatorSpelling::Add)
    );
    assert_eq!(
        operators[1].spelling,
        Some(psi_language_core::OperatorSpelling::Index)
    );
    assert_eq!(
        operators[2].spelling,
        Some(psi_language_core::OperatorSpelling::Range)
    );
    assert_eq!(operators[3].spelling, None);
}

#[test]
fn rejects_retired_operator_spelling_clause() {
    let source = "operator add(left: i32, right: i32) -> i32 spelling +;";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("retired clause must reject");

    assert!(
        error.message.contains("expected `;`"),
        "got: {}",
        error.message
    );
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
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("retired invariant must reject");
        assert!(error.message.contains(expected), "got: {}", error.message);
    }
}

#[test]
fn rejects_retired_provider_item_and_operator_clause() {
    let tokens = Lexer::new("provider omega::host::WriteBytes : HostAbiCall;")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("provider items must reject");
    assert!(
        error.message.contains("expected one of"),
        "got: {}",
        error.message
    );

    let tokens = Lexer::new(
        "boundary operator [] Slice::index(items: &[u8], index: u64) -> u8 provider Slice;",
    )
    .tokenize()
    .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("operator provider clauses must reject");
    assert!(
        error.message.contains("expected `;`"),
        "got: {}",
        error.message
    );
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
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("unknown crash cause must fail");
    assert!(
        error
            .message
            .contains("unknown crash cause `Panic`; expected `Trap` or `Abort`"),
        "got: {}",
        error.message
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
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("explicit crash should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let [statement] = parsed.items.statements(state.statements) else {
        panic!("crash-only body should contain one statement");
    };
    let StatementNode::Transition(transition) = parsed.statements.statement(*statement) else {
        panic!("crash should lower to an explicit transition exit");
    };
    assert_eq!(
        transition.exit,
        psi_syntax_trees::statement::TransitionExit::Crash(
            psi_syntax_trees::item::CrashCause::Abort
        )
    );

    let tokens = Lexer::new("machine fail() { trap; }")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("retired trap statement must fail");
    assert!(error.message.contains("write `crash Trap;`"));
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
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("boundary trait");
    let signature = parsed
        .items
        .state_signature(parsed.items.state_signatures(trait_definition.machines)[0]);
    assert!(signature.service_reach_is_installation_bound);
    assert_eq!(signature.service_reaches.len(), 2);
}

#[test]
fn parses_installation_bound_reach_on_top_level_boundary_requirement() {
    let source = r#"
        boundary machine InterruptAcknowledgement::complete(acknowledgement: u64)
        reaches <= MachineControl + PortIo;
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("top-level boundary requirement should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("boundary machine");
    assert!(machine.boundary);
    assert!(machine.bodyless);
    assert!(machine.service_reach_is_installation_bound);
    assert_eq!(machine.service_reaches.len(), 2);
}

#[test]
fn rejects_top_level_installation_bound_reach_outside_fresh_boundary_requirement() {
    for source in [
        "machine complete() reaches <= MachineControl {}",
        "boundary machine complete() reaches <= MachineControl {}",
        "x86_64 machine complete() reaches <= MachineControl {}",
        "boundary machine complete() satisfies Completion::complete reaches <= MachineControl;",
        "Named: Subject satisfies Trait { machine complete() reaches <= MachineControl {} }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens)
            .expect_err("installation-bound reach must stay on a fresh boundary requirement");
        assert!(
            error.message.contains("`reaches <= Bound`"),
            "got: {}",
            error.message
        );
    }
}

#[test]
fn rejects_installation_bound_reach_outside_bodyless_boundary_requirement() {
    for source in [
        "trait Completion { machine complete() reaches <= MachineControl; }",
        "boundary trait Completion { machine complete() reaches <= MachineControl {} }",
        "machine run<machine Op>() where machine Op() reaches <= MachineControl; {}",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens)
            .expect_err("installation-bound reach must stay on a bodyless boundary requirement");
        assert!(
            error.message.contains("`reaches <= Bound`"),
            "got: {}",
            error.message
        );
    }
}

#[test]
fn rejects_empty_or_mixed_installation_bound_reach_rows() {
    for source in [
        "boundary trait Completion { machine complete() reaches <=; }",
        "boundary trait Completion { machine complete() reaches MachineControl reaches <= PortIo; }",
        "boundary trait Completion { machine complete() reaches <= MachineControl reaches PortIo; }",
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens)
            .expect_err("empty or mixed installation-bound reach must reject");
        assert!(
            error.message.contains("installation-bound reach"),
            "got: {}",
            error.message
        );
    }
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");
    let contracts = parsed.items.capability_contracts(machine.contracts);

    assert_eq!(contracts.len(), 2);
    assert!(matches!(
        contracts[0].kind,
        psi_syntax_trees::item::CapabilityContractKind::Requires
    ));
    assert!(matches!(
        contracts[1].kind,
        psi_syntax_trees::item::CapabilityContractKind::Ensures
    ));
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("named contracts parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    assert_eq!(
        contracts[0].binding.as_ref().map(|name| name.as_str()),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1].binding.as_ref().map(|name| name.as_str()),
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("outcome-specific ensures parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let contracts = parsed.items.capability_contracts(machine.contracts);
    assert_eq!(contracts.len(), 2);
    for contract in contracts {
        let psi_syntax_trees::item::CapabilityContractKind::EnsuresForResultCase { result_case } =
            &contract.kind
        else {
            panic!("guarded guarantee row")
        };
        let names = parsed.items.identifier_path_members(*result_case);
        assert_eq!(names[0].as_str(), "Outcome");
        assert_eq!(names[1].as_str(), "Success");
        assert_eq!(parsed.items.proof_facts(contract.facts).len(), 1);
    }
    assert!(contracts[0].binding.is_none());
    assert_eq!(
        contracts[1].binding.as_ref().map(|name| name.as_str()),
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
            "machine choose() -> Outcome ensures selected: true; ensures Outcome::Success -> { selected: true; } {}",
            "duplicate machine-wide public ensures selector",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize invalid group");
        let error = parse_syntax_trees(&tokens).expect_err("invalid group must reject");
        assert!(
            error.message.contains(expected),
            "expected {expected:?}, got: {}",
            error.message
        );
    }
}

#[test]
fn rejects_named_contract_with_multiple_propositions() {
    let source = r#"
        proposition carries(value: i32);
        machine invalid(value: i32)
        requires proof: carries(value); carries(value)
        {
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("one evidence term cannot bind two facts");
    assert!(
        error.message.contains("exactly one proposition"),
        "{}",
        error.message
    );
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("named signature contracts parse");
    let worker = parsed
        .root_items()
        .filter_map(|item| match item {
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .find(|definition| definition.name.as_str() == "Worker")
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
            .map(|binding| binding.as_str()),
        Some("input_proof")
    );
    assert_eq!(
        contracts[1]
            .binding
            .as_ref()
            .map(|binding| binding.as_str()),
        Some("output_proof")
    );
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
        psi_syntax_trees::item::CapabilityContractKind::Requires
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item");

    // The by-form supplies only the witness -- never the public guarantee.
    assert!(!machine.terminates_guarantee);
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_subjects)
            .len(),
        1
    );
    assert_eq!(
        parsed
            .items
            .identifier_path_members(machine.ranking_view)
            .len(),
        2
    );
    assert_eq!(
        parsed
            .expressions
            .expression_handles(machine.ranking_view_arguments)
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
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let field = parsed
        .items
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            psi_syntax_trees::item::DataMember::Field(field) => Some(field),
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
            psi_syntax_trees::types::TypeConstraintNode::Domain(name),
            psi_syntax_trees::types::TypeConstraintNode::ArithmeticDomain(
                psi_numerics::arithmetic::ArithmeticDomain::Saturating
            )
        ] if name.name.as_str() == "Finite"
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("open index expression should parse");
    let rate = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Rate" => Some(data),
            _ => None,
        })
        .expect("Rate data");
    let field = parsed
        .items
        .data_members(rate.members)
        .iter()
        .find_map(|member| match member {
            psi_syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .expect("Rate::value field");
    let TypeReferenceNode::Constrained { constraints, .. } =
        parsed.type_references.type_reference(field.type_reference)
    else {
        panic!("quantity should be a constrained carrier");
    };
    let [psi_syntax_trees::types::TypeConstraintNode::Domain(domain)] =
        parsed.type_references.constraints(*constraints)
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
    assert_eq!(
        binary.operator,
        psi_syntax_trees::expression::BinaryOperator::Divide
    );
    assert_eq!(parsed.expressions.display_name(binary.left), "A");
    assert_eq!(parsed.expressions.display_name(binary.right), "B");
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("data root item");
    let fields = parsed
        .items
        .data_members(data.members)
        .iter()
        .filter_map(|member| match member {
            psi_syntax_trees::item::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();

    let names = |field: &psi_syntax_trees::item::DataField| {
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
                psi_syntax_trees::types::TypeConstraintNode::Domain(name) => {
                    name.name.as_str().to_owned()
                }
                other => panic!("carry permission became {other:?}"),
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(names(fields[0]), ["Carry::MovableAddress"]);
    assert_eq!(
        names(fields[1]),
        psi_language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::ProofFact::Membership(membership) => parsed
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
        psi_language_core::CarryPermission::ALL.map(|permission| permission.name().to_owned())
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("array and slice recast targets should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
        length: psi_syntax_trees::types::FixedArrayLength::Literal(4),
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
            psi_syntax_trees::item::Item::Trait(trait_definition) => Some(trait_definition),
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
    for (contract, keyword) in contracts.iter().zip(["requires", "ensures"]) {
        let span = contract
            .keyword_source_span
            .expect("trait contract keyword span");
        assert_eq!(&source[span.span.start..span.span.end], keyword);
    }
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let psi_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let psi_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let psi_syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize outcome arm");
    let parsed = parse_syntax_trees(&tokens).expect("outcome selector arm should parse");
    let transition = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "inspect" =>
            {
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
    assert_eq!(selectors[0].output_field.as_str(), "selected");
    assert_eq!(selectors[0].binding.as_str(), "local");
    assert_eq!(selectors[1].output_field.as_str(), "shorthand");
    assert_eq!(selectors[1].binding.as_str(), "shorthand");
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
        let tokens = Lexer::new(source).tokenize().expect("tokenize outcome arm");
        let parsed = parse_syntax_trees(&tokens).expect("outcome arm should parse");
        let inspect = parsed
            .root_items()
            .find_map(|item| match item {
                psi_syntax_trees::item::Item::Machine(machine)
                    if machine.name.as_str() == "inspect" =>
                {
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
                    ExpressionNode::Call(call) if call.target.as_str() == "produce"
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
                    ExpressionNode::Call(call) if call.target.as_str() == "produce"
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
fn rejects_duplicate_and_noncase_outcome_proof_selectors() {
    for (source, expected) in [
        (
            "machine inspect(result: Outcome) { transition result { Outcome::Success { ; selected: local, selected: other } -> {} } }",
            "outcome proof selector `selected` is bound more than once",
        ),
        (
            "machine inspect(result: Record) { transition result { Record { ; selected: local } -> {} } }",
            "outcome proof selectors require an exact",
        ),
    ] {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize rejected selector arm");
        let error = parse_syntax_trees(&tokens).expect_err("invalid selector arm must reject");
        assert!(error.message.contains(expected), "got: {}", error.message);
    }
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::statement::StatementNode::Transition(transition) => Some(transition),
            _ => None,
        })
        .next()
        .expect("first transition arm");
    let psi_syntax_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        panic!("field-value arm must have an equality guard");
    };
    let psi_syntax_trees::expression::ExpressionNode::Binary(equality) =
        parsed.expressions.expression(guard)
    else {
        panic!("field-value pattern should lower to binary equality");
    };
    assert_eq!(
        equality.operator,
        psi_syntax_trees::expression::BinaryOperator::Equal
    );
    assert!(matches!(
        parsed.expressions.expression(equality.left),
        psi_syntax_trees::expression::ExpressionNode::Member(_)
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Trait(definition) => Some(definition),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            if fact.kind == psi_syntax_trees::statement::AssemblyFactKind::Requires
    ));
    assert!(matches!(
        parsed.statements.statement(statements[1]),
        StatementNode::Call(call) if call.target.as_str() == "asm#port_out"
    ));
    assert!(matches!(
        parsed.statements.statement(statements[2]),
        StatementNode::AssemblyFact(fact)
            if fact.kind == psi_syntax_trees::statement::AssemblyFactKind::Ensures
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let psi_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let psi_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let psi_syntax_trees::statement::StatementNode::Transition(transition) =
        parsed.statements.statement(statement)
    else {
        panic!("entry should start with transition")
    };
    let psi_syntax_trees::statement::TransitionGuardNode::When(subject) = transition.guard else {
        panic!("transition should lower as a guarded expression");
    };
    assert!(matches!(
        parsed.expressions.expression(subject),
        ExpressionNode::Binary(_)
    ));
}

#[test]
fn rejects_retired_export_items_with_direction() {
    let tokens = Lexer::new("export internal_regex::Match as Match;")
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("retired export item must reject");

    assert!(
        error.message.contains("the `export` item is retired"),
        "got: {}",
        error.message
    );

    let tokens = Lexer::new("machine export() { }")
        .tokenize()
        .expect("tokenize ordinary identifier");
    parse_syntax_trees(&tokens).expect("export remains available as an ordinary identifier");
}

#[test]
fn parses_domain_definition_surface() {
    let source = r#"
        domain Player::Alive
        requires
            self in Player::Valid;
            self.health > 0
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            psi_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].name.as_str(), "Player::Alive");
    assert!(domains[0].target_type.is_valid());
    assert_eq!(
        domains[0].predicate_body,
        psi_language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[0].facts).len(), 2);
    assert!(domains[0].semantic_clause_token_count > 3);

    let facts = parsed.items.proof_facts(domains[0].facts);
    assert!(matches!(
        facts[0],
        psi_syntax_trees::item::ProofFact::Membership(_)
    ));
    assert!(matches!(
        facts[1],
        psi_syntax_trees::item::ProofFact::Expression(_)
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domain = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("domain");

    assert_eq!(
        domain.predicate_body,
        psi_language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domain.facts).len(), 1);
    assert_eq!(
        domain
            .authored_routes
            .iter()
            .map(|route| {
                route
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::")
            })
            .collect::<Vec<_>>(),
        ["Reservations::confirm", "Imported::Reservations::restore"]
    );
    assert!(domain.operators.is_empty());
}

#[test]
fn parses_explicit_progress_profile_classification() {
    let source = r#"
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant_weak_fair;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domain = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("domain");

    assert_eq!(
        domain.classification,
        Some(psi_language_core::DomainClassification::ProgressProfile)
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
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("classification must reject");
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
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("classification order must reject");
    assert!(
        error
            .message
            .contains("classification must appear immediately after the domain head"),
        "got: {}",
        error.message
    );
}

#[test]
fn rejects_legacy_domain_route_bodies_with_migration_guidance() {
    let source = r#"
        domain Reservation::Issued {
            Reservations::issue;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("route bodies must be retired");
    assert!(
        error.message.contains("domain route bodies are retired"),
        "got: {}",
        error.message
    );
    assert!(error.message.contains("`established by Trait::requirement"));
}

#[test]
fn rejects_legacy_domain_body_predicates_with_migration_guidance() {
    let source = r#"
        domain Player::Alive {
            self.health > 0;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("body predicates must be retired");
    assert!(
        error
            .message
            .contains("domain predicates must be written in `requires`"),
        "got: {}",
        error.message
    );
    assert!(error.message.contains("`established by Trait::requirement"));
}

#[test]
fn rejects_nested_domain_operators_with_top_level_home_guidance() {
    let source = r#"
        domain i32::Degrees {
            operator + add(left: i32, right: i32) -> i32;
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens).expect_err("nested operators must be retired");
    assert!(
        error
            .message
            .contains("domain operators must be ordinary top-level declarations"),
        "got: {}",
        error.message
    );
    assert!(
        error
            .message
            .contains("`operator Type::Domain::operation ...`")
    );
}

#[test]
fn parses_equivalent_bodyless_domain_spellings_distinct_from_true_predicate() {
    let source = r#"
        domain Reservation::Issued;
        domain Reservation::Recorded;
        domain Reservation::Universal
        requires
            true;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let domains = parsed
        .root_items()
        .filter_map(|item| match item {
            psi_syntax_trees::item::Item::Domain(domain) => Some(domain),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(domains.len(), 3);
    for domain in &domains[..2] {
        assert_eq!(
            domain.predicate_body,
            psi_language_core::DomainPredicateBody::Bodyless
        );
        assert!(domain.facts.is_empty());
        assert_eq!(domain.semantic_clause_token_count, 0);
    }
    assert_eq!(
        domains[2].predicate_body,
        psi_language_core::DomainPredicateBody::Present
    );
    assert_eq!(parsed.items.proof_facts(domains[2].facts).len(), 1);
    assert!(domains[2].semantic_clause_token_count > 0);
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
            psi_syntax_trees::item::Item::Domain(domain) => Some(domain),
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
        psi_language_core::DomainPredicateBody::Bodyless
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
        referee, access, ..
    } = parsed
        .type_references
        .type_reference(parameter.type_reference)
    else {
        panic!("&mut self should retain its reference ownership mode");
    };
    assert_eq!(*access, ReferenceAccess::Mutable);
    assert!(matches!(
        parsed.type_references.type_reference(*referee),
        TypeReferenceNode::SelfType
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
    let caller = parsed
        .root_items()
        .filter_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .find(|machine| machine.name.as_str() == "caller")
        .expect("caller machine");
    let state = parsed.items.state(
        *parsed
            .items
            .state_handles(caller.states)
            .first()
            .expect("caller entry state"),
    );
    let StatementNode::Call(call) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("caller statement should be a call");
    };
    let argument = parsed.statements.expression_handles(call.arguments)[0];
    let ExpressionNode::Borrow(borrow) = parsed.expressions.expression(argument) else {
        panic!("argument should retain one closed borrow-expression node");
    };
    assert_eq!(borrow.access, ReferenceAccess::WriteOnly);
    assert_eq!(
        parsed.expressions.display_name(argument),
        "&write destination"
    );
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
fn parses_positional_erased_evidence_call_lane_after_semicolon() {
    let source = r#"
        machine Main::main(value: i32, first_proof: Evidence, second_proof: Evidence) {
            consume(value; first_proof, second_proof);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("evidence lane should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
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
            .map(|name| name.as_str())
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("evidence-only lane should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
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
    assert_eq!(call.evidence_arguments[0].as_str(), "proof");
}

#[test]
fn parses_evidence_lane_on_named_transition_without_dropping_it() {
    let source = r#"
        machine Main::main(proof: Evidence) {
            transition { _ -> next(; proof) }
            state next() {}
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("named transition evidence lane should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::Transition(transition) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("expected transition statement");
    };
    let psi_syntax_trees::statement::TransitionTargetNode::Named {
        evidence_arguments, ..
    } = parsed.statements.transition_target(transition.target)
    else {
        panic!("expected named transition target");
    };
    assert_eq!(
        evidence_arguments
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["proof"]
    );
}

#[test]
fn tail_self_call_rewrite_retains_the_authored_target_span() {
    let source = r#"
        machine repeat(remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            repeat(remaining - 1)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("tail self call should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let StatementNode::Transition(transition) = parsed
        .statements
        .statement(parsed.items.statements(state.statements)[0])
    else {
        panic!("tail self call should rewrite to a transition");
    };
    let psi_syntax_trees::statement::TransitionTargetNode::Named { source_span, .. } =
        parsed.statements.transition_target(transition.target)
    else {
        panic!("tail self call should retain a named target");
    };
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name.as_str(), "Key");
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(contract)),
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
fn parses_nominal_machine_parameter_requirement() {
    let source = r#"
        machine register<machine Selected>()
        where machine Selected satisfies platform::WindowProcedure::call;
        {
            Selected()
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("nominal machine contract should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let [parameter] = parsed.items.type_parameters(machine.type_parameters) else {
        panic!("expected one machine parameter");
    };
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Nominal { requirement }),
    } = &parameter.kind
    else {
        panic!("Selected should retain its nominal requirement path");
    };
    assert_eq!(
        parsed
            .items
            .identifier_path_members(*requirement)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["platform", "WindowProcedure", "call"]
    );
    assert!(!machine.bodyless);
}

#[test]
fn parses_bodyless_nominal_machine_parameter_with_one_semicolon() {
    let source = r#"
        boundary machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("bodyless nominal binder should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("bodyless generic machine");
    assert!(machine.bodyless);
    assert!(matches!(
        parsed.items.type_parameters(machine.type_parameters)[0].kind,
        psi_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(psi_syntax_trees::item::MachineParameterContract::Nominal { .. })
        }
    ));
}

#[test]
fn parses_structural_and_nominal_machine_parameter_contracts_together() {
    let source = r#"
        machine apply<machine Schema, machine Selected>()
        where machine Schema(value: u64) -> u64;
        where machine Selected satisfies WindowProcedure::call;
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("mixed machine contracts should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert!(matches!(
        parameters[0].kind,
        psi_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(_))
        }
    ));
    assert!(matches!(
        parameters[1].kind,
        psi_syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(psi_syntax_trees::item::MachineParameterContract::Nominal { .. })
        }
    ));
}

#[test]
fn rejects_malformed_nominal_machine_parameter_requirements() {
    let cases = [
        (
            "where machine Selected satisfies WindowProcedure;",
            "must name an exact `Trait::requirement`",
        ),
        (
            "where machine Selected satisfies WindowProcedure::call as Choice;",
            "do not accept `as Name`",
        ),
        (
            "where machine Selected satisfies WindowProcedure::call via Native;",
            "cannot use `via`",
        ),
        (
            "where machine Selected satisfies WindowProcedure<u64>::call;",
            "generic trait arguments are not supported",
        ),
    ];

    for (contract, expected) in cases {
        let source = format!("machine register<machine Selected>() {contract} {{ Selected() }}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let error = parse_syntax_trees(&tokens).expect_err("malformed nominal binder must reject");
        assert!(
            error.message.contains(expected),
            "expected {expected:?}, got: {}",
            error.message
        );
    }
}

#[test]
fn rejects_one_off_machine_member_requirements_instead_of_discarding_them() {
    let source = r#"
        data Device {}

        machine poll_once<T>(device: &mut T)
        where machine T::poll(&mut self)
        {
        }
        "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let error = parse_syntax_trees(&tokens)
        .expect_err("a one-off member requirement has no semantic carrier and must reject");
    assert!(
        error
            .message
            .contains("one-off `where machine T::member(...)` requirements are unsupported"),
        "unexpected diagnostic: {}",
        error.message
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("generic machine");
    let parameters = parsed.items.type_parameters(machine.type_parameters);
    assert_eq!(parameters.len(), 2);
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(schema)),
    } = &parameters[0].kind
    else {
        panic!("Schema should carry its authored machine contract");
    };
    let nested = parsed.items.type_parameters(schema.type_parameters);
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name.as_str(), "Inner");
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(inner)),
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
            psi_syntax_trees::item::Item::Data(data) => Some(data),
            _ => None,
        })
        .expect("Stream declaration");
    let parameters = parsed.items.type_parameters(data.type_parameters);
    assert_eq!(parameters.len(), 1);
    let psi_syntax_trees::item::TypeParameterKind::Machine {
        contract: Some(psi_syntax_trees::item::MachineParameterContract::Structural(contract)),
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
            psi_syntax_trees::item::Item::Data(data) if data.name.as_str() == "Quotient" => {
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
            psi_syntax_trees::expression::ExpressionNode::Call(call)
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
fn parses_nested_static_conformance_application() {
    let source = r#"
        trait Encodes<T> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>: Bytes satisfies Encodes<Output> {}

        machine send<T, Encoding: Bytes satisfies Encodes<T>>() {}
        machine caller() {
            send<Message, SequenceEncoding<'scope, Bytes, Message>>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("nested static application should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_syntax_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "send" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("generic call expression");
    let application = call.machine_arguments[1]
        .application
        .as_ref()
        .expect("nested conformance application");
    assert_eq!(
        call.machine_arguments[1].path[0].as_str(),
        "SequenceEncoding"
    );
    assert_eq!(application.lifetime_arguments[0].as_str(), "scope");
    assert_eq!(application.arguments.len(), 2);
    assert_eq!(application.arguments[0].path[0].as_str(), "Bytes");
    assert_eq!(application.arguments[1].path[0].as_str(), "Message");
    assert_eq!(
        call.display_name(&parsed.expressions),
        "send<Message, SequenceEncoding<'scope, Bytes, Message>>()"
    );
}

#[test]
fn parses_evidence_term_member_as_a_distinct_proof_static_argument() {
    let source = r#"
        machine consume<machine Witness>()
        where machine Witness();
        {}

        machine caller() {
            consume<proof.modulus>();
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let parsed = parse_syntax_trees(&tokens).expect("evidence projection should parse");
    let call = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_syntax_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "consume" =>
            {
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
    assert_eq!(projection.term.as_str(), "proof");
    assert_eq!(projection.member.as_str(), "modulus");
    assert_eq!(
        call.display_name(&parsed.expressions),
        "consume<proof.modulus>()"
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
            psi_syntax_trees::item::Item::Machine(machine) => Some(machine),
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
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let parsed = parse_syntax_trees(&tokens).expect("proof-output lane should parse");
    let machine = parsed
        .root_items()
        .find_map(|item| match item {
            psi_syntax_trees::item::Item::Machine(machine)
                if machine.name.as_str() == "consume" =>
            {
                Some(machine)
            }
            _ => None,
        })
        .expect("consume machine");
    let state = parsed.items.state(
        parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state"),
    );
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
    assert_eq!(package.bindings[0].output_field.as_str(), "value");
    assert_eq!(package.bindings[0].binding.as_str(), "value");
    assert_eq!(package.bindings[1].output_field.as_str(), "proof");
    assert_eq!(package.bindings[1].binding.as_str(), "local_proof");
}

#[test]
fn retired_generated_proof_package_has_directed_migration() {
    let source = r#"
        machine produce() ensures proof: true { proof = true; }
        machine consume() { let { proof: local } = produce(); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("old package spelling must reject");
    assert!(error.message.contains("proof-output packages are retired"));
    assert!(
        error
            .message
            .contains("let (value; public_output: local_term)")
    );
}
