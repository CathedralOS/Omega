use crate::lowerer::lower_symbol_resolved_trees;

#[test]
fn retains_exact_nominal_machine_parameter_identity_in_typed_trees() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };

    let source = r#"
        boundary trait WindowProcedure {
            machine call(value: u32) -> u64;
        }

        machine register<machine Selected>()
        where machine Selected satisfies WindowProcedure::call;
        {}
    "#;
    let typed = crate::front_end::typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register machine");
    let parameter = typed
        .machine_type_parameters(machine)
        .first()
        .expect("Selected parameter");
    let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
        panic!("Selected should be a machine parameter")
    };
    let typed_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
    } = contract
    else {
        panic!("Selected should retain nominal identity")
    };
    let typed_trees::data::MachineParameterContractView::Nominal {
        trait_definition: definition,
        requirement: signature,
    } = typed
        .machine_parameter_contract_view(contract)
        .expect("valid exact requirement")
    else {
        panic!("nominal view")
    };

    assert_eq!(*trait_definition, definition.symbol);
    assert_eq!(*requirement, signature.symbol);
    assert_eq!(definition.name.as_str(), "WindowProcedure");
    assert_eq!(signature.name.as_str(), "call");
    assert_ne!(parameter.symbol, signature.symbol);
    assert_eq!(typed.state_signature_parameters(signature).len(), 1);
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::TypeReference
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == definition.symbol)
    }));
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind() == Kind::StaticPathSegment
            && matches!(selection.target(), Target::Resolved(target) if target.selected_symbol() == signature.symbol)
    }));
}

#[test]
fn retains_typed_name_owned_conformance_telescope() {
    let source = r#"
        trait Converter<'view, Source, Target> {}

        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<'scope, Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;
    let typed = crate::front_end::typed_program(source);
    let conformance = typed.conformances().first().expect("one conformance");

    assert_eq!(conformance.lifetime_parameters.len(), 1);
    assert_eq!(conformance.lifetime_parameters[0].as_str(), "scope");
    assert_eq!(conformance.trait_lifetime_arguments, vec![0]);
    assert_eq!(
        typed.snapshot().roots.conformances[0].trait_lifetime_arguments,
        vec![0]
    );
    let parameters = typed.conformance_type_parameters(conformance);
    assert_eq!(parameters.len(), 3);
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );
    assert_eq!(conformance.carrier_symbol, parameters[0].symbol);
    let converter = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Converter")
        .expect("Converter trait");
    assert_eq!(conformance.trait_symbol, converter.symbol);
    let arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    assert_eq!(arguments.len(), 2);
    assert!(matches!(
        typed.type_reference_table.type_reference(arguments[0]),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == parameters[0].symbol && name.as_str() == "Source"
    ));
}

#[test]
fn retains_typed_named_conformance_visibility_and_snapshot_identity() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub PublicCircle: Circle satisfies Shape;
        PrivateCircle: Circle satisfies Shape;
    "#;
    let typed = crate::front_end::typed_program(source);
    let conformances = typed.conformances();

    assert_eq!(conformances.len(), 2);
    assert!(conformances[0].is_public);
    assert!(!conformances[1].is_public);
    let snapshot = typed.snapshot_json().expect("typed conformance snapshot");
    assert!(snapshot.contains("\"name\":\"PublicCircle\""));
    assert!(snapshot.contains("\"is_public\":true"));
    assert!(snapshot.contains("\"trait_name\":\"Shape\""));
}

#[test]
fn retains_typed_explicit_conformance_binder_identity() {
    let source = r#"
        trait Ranked {}

        machine sort<Element, Order: Element satisfies Ranked>(
            values: &mut [Element]
        ) {}
    "#;
    let typed = crate::front_end::typed_program(source);
    let machine = typed.machines().first().expect("machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one explicit conformance binder");
    };

    assert_eq!(
        bound.binder_name.as_ref().map(|name| name.as_str()),
        Some("Order")
    );
    assert!(bound.binder.is_some_and(|symbol| symbol.is_valid()));
    assert_eq!(
        bound.subject,
        typed.machine_type_parameters(machine)[0].symbol
    );
    let snapshot = typed.snapshot();
    assert_eq!(
        snapshot.roots.machines[0].conformance_bounds[0]
            .binder
            .as_deref(),
        Some("Order")
    );
}

#[test]
fn retains_typed_selected_conformance_bound_application() {
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
    let typed = crate::front_end::typed_program(source);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("inspect machine");
    let [bound] = machine.conformance_bounds.as_slice() else {
        panic!("one selected conformance bound");
    };
    let selected = bound
        .selected_conformance
        .as_ref()
        .expect("selected conformance");
    assert_eq!(bound.selected_conformance_symbol(), Some(selected.symbol));
    assert_eq!(
        bound.selected_conformance_name().map(|name| name.as_str()),
        Some("FullEncoding")
    );
    let application = selected.application.as_ref().expect("complete application");
    assert_eq!(application.lifetime_arguments[0].as_str(), "view");
    assert_eq!(application.arguments.len(), 4);
    assert!(application.arguments[2].const_literal.is_some());
    assert!(
        application
            .arguments
            .iter()
            .all(|argument| { argument.const_literal.is_some() || argument.symbol.is_valid() })
    );

    let snapshot = typed.snapshot();
    assert!(snapshot.roots.machines.iter().any(|machine| {
        machine.name == "inspect"
            && machine.conformance_bounds[0].selected_conformance.is_some()
            && machine.conformance_bounds[0]
                .selected_conformance_symbol
                .is_some()
    }));
}

#[test]
fn retains_proof_static_evidence_projection_through_resolved_and_typed_trees() {
    let source = r#"
        trait Evidence {
            machine modulus() -> i32;
        }

        proposition holds() evidence Evidence;

        machine consume<machine Witness>()
        where machine Witness() -> i32;
        {}

        machine caller()
        requires proof: holds()
        {
            consume<proof.modulus>();
        }
    "#;
    let resolved = crate::front_end::resolved_program(source);
    let resolved_caller = resolved
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("resolved caller");
    let resolved_projection = resolved
        .machine_state_handles(resolved_caller.states)
        .iter()
        .flat_map(|state| resolved.state_statements(resolved.machine_state(*state).statements))
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::Statement::Call(call)
                if call.target.as_str() == "consume" =>
            {
                call.machine_arguments[0].evidence_projection.as_ref()
            }
            _ => None,
        })
        .expect("resolved projection");
    assert_eq!(resolved_projection.term.as_str(), "proof");
    assert_eq!(resolved_projection.member.as_str(), "modulus");

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let typed_caller = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .expect("typed caller");
    let typed_call = typed
        .machine_states(typed_caller)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "consume" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("typed call");
    let projection = typed_call.machine_arguments[0]
        .evidence_projection
        .as_ref()
        .expect("typed projection");
    assert_eq!(projection.term.as_str(), "proof");
    assert_eq!(projection.member.as_str(), "modulus");
    assert!(!typed_call.machine_arguments[0].symbol.is_valid());
    let snapshot = typed.snapshot_json().expect("typed snapshot");
    assert!(snapshot.contains("\"term\":\"proof\",\"member\":\"modulus\""));
}

#[test]
fn retains_typed_evidence_forwarding_owner_identity() {
    let source = r#"
        trait Evidence {}
        proposition carries(value: i32) evidence Evidence;
        machine forward(value: i32)
        requires input_proof: carries(value)
        ensures output_proof: carries(value)
        {
            output_proof = input_proof;
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let [forwarding] = typed.evidence_forwardings.as_slice() else {
        panic!("one typed evidence forwarding expected");
    };
    assert!(forwarding.machine_symbol.is_valid());
    assert!(forwarding.state_symbol.is_valid());
    assert_eq!(forwarding.target.as_str(), "output_proof");
    assert_eq!(forwarding.source.as_str(), "input_proof");
    assert_eq!(forwarding.source_conformance, None);
    assert_eq!(typed.snapshot().evidence_forwardings.len(), 1);
}

#[test]
fn copies_exact_literal_and_case_membership_symbols_into_typed_tables() {
    let source = r#"
        data Token {
            value: u32;
            case Issued(code: u32);
        }
        machine path() -> u32 { Token::Issued::code }
        machine record() -> Token { Token { value: 1 } }
        machine issue() -> Token { Token::Issued { code: 2 } }
        machine is_issued(token: Token) -> bool { token in Token::Issued }
    "#;
    let resolved = crate::front_end::resolved_program(source);
    let expected_type = resolved
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let expected_case = resolved
        .data_members(resolved.data_definitions[0].members)
        .iter()
        .find_map(|member| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
            _ => None,
        })
        .expect("Issued case");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type exact selections");

    let authored_path = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Name(path)
                if typed.expression_table.name_path_members(path.members).len() == 3 =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("three-segment typed path");
    let authored_path_symbols = typed
        .expression_table
        .name_path_member_symbols(authored_path.member_symbols);
    assert_eq!(authored_path_symbols.len(), 3);
    assert!(authored_path_symbols.iter().all(|symbol| symbol.is_valid()));

    let literals = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => Some(literal),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(literals.len(), 2);
    for literal in literals {
        assert_eq!(literal.type_symbol, expected_type);
        assert_eq!(literal.case_name.is_some(), literal.case_symbol.is_some());
        assert!(
            typed
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .all(|field| field.field_symbol.is_valid())
        );
    }

    let case_path = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Name(path)
                if typed
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(["Token", "Issued"]) =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("lowered exact case path");
    assert_eq!(
        typed
            .expression_table
            .name_path_member_symbols(case_path.member_symbols),
        [expected_type, expected_case]
    );
}

#[test]
fn typed_lowering_does_not_replace_the_authored_struct_selection_ledger() {
    let source = "data Item { value: u32; } machine make() -> Item { Item { value: 1 } }";
    let mut resolved = crate::front_end::resolved_program(source);
    let literal = resolved
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(
                expression,
                symbol_resolved_trees::expression::ExpressionNode::StructLiteral(_)
            )
            .then_some(handle)
        })
        .expect("struct literal");
    let symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) =
        resolved.tables.bodies.expressions.expression_mut(literal)
    else {
        unreachable!();
    };
    literal.type_symbol = symbols::SymbolHandle::invalid();

    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("typed lowering is not the authored package-admission gate");
    assert!(typed.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StructLiteralType
            && matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
}

#[test]
fn elaborates_omitted_erased_field_with_unique_nullary_constructor() {
    let source = r#"
        data Evidence {
            case Only;
            case WithPayload(value: i32);
        }
        data Certified {
            value: i32;
            proof [erased]: Evidence;
        }
        machine certify() -> Certified {
            Certified { value: 7 }
        }
    "#;
    let typed = crate::front_end::typed_program(source);

    let evidence = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Evidence")
        .expect("Evidence definition");
    let only = typed
        .data_members(evidence)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.name.as_str() == "Only" => {
                Some(variant)
            }
            _ => None,
        })
        .expect("Only variant");
    let literal = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str() == "Certified" =>
            {
                Some(literal)
            }
            _ => None,
        })
        .expect("Certified literal");
    let fields = typed.expression_table.struct_fields(literal.fields);
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["value", "proof"]
    );
    let proof = &fields[1];
    let typed_trees::expression::ExpressionNode::Name(path) =
        typed.expression_table.expression(proof.value)
    else {
        panic!("omitted proof should elaborate to a semantic name term");
    };
    assert_eq!(
        typed
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Evidence", "Only"]
    );
    assert_eq!(path.head_symbol, evidence.symbol);
    assert_eq!(path.symbol, only.symbol);
    assert_eq!(
        typed
            .expression_table
            .name_path_member_symbols(path.member_symbols),
        [evidence.symbol, only.symbol]
    );
}

#[test]
fn preserves_field_relevance_through_resolved_and_typed_trees() {
    let source = r#"
        data Certified {
            value: i32;
            proof [erased]: i32;
            case Wrapped(witness [erased]: i32);
        }
    "#;
    let resolved = crate::front_end::resolved_program(source);

    let resolved_data = resolved
        .data_definitions
        .iter()
        .next()
        .expect("resolved data");
    let resolved_members = resolved.data_members(resolved_data.members);
    let symbol_resolved_trees::data::DataMember::Field(resolved_value) = &resolved_members[0]
    else {
        panic!("resolved value field");
    };
    let symbol_resolved_trees::data::DataMember::Field(resolved_proof) = &resolved_members[1]
    else {
        panic!("resolved proof field");
    };
    let symbol_resolved_trees::data::DataMember::Variant(resolved_wrapped) = &resolved_members[2]
    else {
        panic!("resolved wrapped case");
    };
    let [resolved_witness] = resolved.data_payload_fields(resolved_wrapped.payload) else {
        panic!("one resolved payload field");
    };
    assert_eq!(
        resolved_value.relevance,
        language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        resolved_proof.relevance,
        language_core::BindingRelevance::Erased
    );
    assert_eq!(
        resolved_witness.relevance,
        language_core::BindingRelevance::Erased
    );
    let resolved_snapshot = resolved.snapshot_json().expect("resolved snapshot");
    assert!(resolved_snapshot.contains("\"relevance\":\"relevant\""));
    assert!(resolved_snapshot.contains("\"relevance\":\"erased\""));

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let typed_data = typed.data_definitions().first().expect("typed data");
    let typed_members = typed.data_members(typed_data);
    let typed_trees::data::DataMember::Field(typed_value) = &typed_members[0] else {
        panic!("typed value field");
    };
    let typed_trees::data::DataMember::Field(typed_proof) = &typed_members[1] else {
        panic!("typed proof field");
    };
    let typed_trees::data::DataMember::Variant(typed_wrapped) = &typed_members[2] else {
        panic!("typed wrapped case");
    };
    let [typed_witness] = typed.data_payload_fields(typed_wrapped) else {
        panic!("one typed payload field");
    };
    assert_eq!(
        typed_value.relevance,
        language_core::BindingRelevance::Relevant
    );
    assert_eq!(
        typed_proof.relevance,
        language_core::BindingRelevance::Erased
    );
    assert_eq!(
        typed_witness.relevance,
        language_core::BindingRelevance::Erased
    );
    let typed_snapshot = typed.snapshot_json().expect("typed snapshot");
    assert!(typed_snapshot.contains("\"relevance\":\"relevant\""));
    assert!(typed_snapshot.contains("\"relevance\":\"erased\""));
}

#[test]
fn retains_subjectless_conformance_and_exact_typed_rows() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let [conformance] = typed.conformances() else {
        panic!("one typed conformance");
    };
    assert!(matches!(
        conformance.subject,
        typed_trees::trait_definition::ConformanceSubject::Subjectless
    ));
    assert!(conformance.symbol.is_valid());
    assert_eq!(
        conformance.alias.as_ref().map(|name| name.as_str()),
        Some("ConcreteEvidence")
    );
    let Some(rows) = typed.closed_conformance_rows(conformance) else {
        panic!("closed rows retained");
    };
    let [row] = rows else {
        panic!("one exact typed row");
    };
    assert!(row.declaring_trait.is_valid());
    assert!(row.requirement.is_valid());
    assert!(row.realization_machine.is_valid());
    assert!(row.realization_state.is_valid());
}
