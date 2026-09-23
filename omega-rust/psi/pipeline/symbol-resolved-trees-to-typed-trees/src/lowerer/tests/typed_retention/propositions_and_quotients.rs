use crate::lowerer::lower_symbol_resolved_trees;

#[test]
fn types_nested_index_hoists_from_explicit_local_collections() {
    let source = r#"
        data Main {}
        machine Main::main(
            &mut self,
            i: u64 [0..=1],
            j: u64 [0..=2]
        ) {
            let g: [[i32 in Wrapping; 3]; 2] = [[1, 2, 3], [4, 5, 6]];
            g[i][j] = g[i][j] + 1;
        }
    "#;
    let typed = crate::front_end::typed_program(source);

    let machine = &typed.machines()[0];
    let state = &typed.machine_states(machine)[0];
    let hoisted = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__hoist_") =>
            {
                Some(local)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        hoisted.len(),
        1,
        "the indexed RHS should have one value hoist"
    );
    let typed_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = typed
        .type_reference_table
        .type_reference(hoisted[0].type_reference)
    else {
        panic!("the hoist must inherit the authored constrained element type");
    };
    assert!(matches!(
        typed.type_reference_table.type_reference(*base_type),
        typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "i32"
    ));
    assert!(matches!(
        typed.type_reference_table.constraints(*constraints),
        [typed_trees::types::TypeConstraintNode::ArithmeticDomain(
            numerics::arithmetic::ArithmeticDomain::Wrapping
        )]
    ));
}

#[test]
fn generic_proposition_applications_remain_proof_facts_when_typed() {
    let source = r#"
        trait Reflexive<C, proposition Relation>
        where proposition Relation(left: C, right: C);
        {
            machine prove(value: C) ensures Relation(value, value);
        }
    "#;
    let typed = crate::front_end::typed_program(source);

    let trait_definition = &typed.traits()[0];
    let [_, relation] = typed.trait_type_parameters(trait_definition) else {
        panic!("trait should retain a proposition parameter");
    };
    assert!(matches!(
        relation.kind,
        typed_trees::data::TypeParameterKind::Proposition { .. }
    ));
    let [signature] = typed.trait_machine_signatures(trait_definition) else {
        panic!("trait should retain one proof signature");
    };
    let [contract] = typed.state_signature_contracts(signature) else {
        panic!("proof signature should retain one ensures contract");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("Relation(value, value) should be a proposition proof fact");
    };
    assert_eq!(application.proposition, relation.symbol);
    assert!(
        typed
            .normalize_proposition_application(application, None)
            .is_some()
    );
}

#[test]
fn proposition_declarations_and_fact_applications_remain_distinct_when_typed() {
    let source = r#"
        pub proposition related(left: i32, right: i32);

        machine preserve(left: i32, right: i32)
        requires related(left, right)
        {
        }
    "#;
    let typed = crate::front_end::typed_program(source);

    assert_eq!(typed.propositions().len(), 1);
    assert!(typed.propositions()[0].is_public);
    assert!(
        typed
            .snapshot_json()
            .expect("typed proposition snapshot")
            .contains("\"is_public\":true")
    );
    assert!(matches!(
        typed.propositions()[0].body,
        typed_trees::proposition::PropositionBody::Primitive
    ));
    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its requires contract");
    };
    let [fact] = typed.proof_facts.span_or_empty(contract.facts) else {
        panic!("requires should retain one proposition fact");
    };
    let typed_trees::domain::ProofFact::Proposition(application) = fact else {
        panic!("proposition application must not become a Boolean expression");
    };
    assert_eq!(application.proposition, typed.propositions()[0].symbol);
    assert_eq!(
        typed
            .expression_table
            .expression_handles(application.arguments)
            .len(),
        2
    );
}

#[test]
fn const_declaration_visibility_survives_typed_lowering_and_snapshots() {
    let source = r#"
        pub const PUBLIC_LIMIT: u64 = 4;
        const Limits::PRIVATE_LIMIT: u64 = 2;
    "#;
    let typed = crate::front_end::typed_program(source);

    assert_eq!(typed.const_declarations().len(), 2);
    assert!(typed.const_declarations()[0].is_public);
    assert!(!typed.const_declarations()[1].is_public);
    let snapshot = typed.snapshot_json().expect("typed const snapshot");
    assert!(snapshot.contains("\"name\":\"PUBLIC_LIMIT\""));
    assert!(snapshot.contains("\"is_public\":true"));
}

#[test]
fn proposition_type_and_const_arguments_retain_categories_and_identity() {
    let source = r#"
        proposition indexed<T, const N: i32>();
        proposition forwarded<T, const N: i32>() = indexed<T, N>();

        machine use_selected()
        requires forwarded<i32, 7>()
        {
        }
    "#;
    let typed = crate::front_end::typed_program(source);

    let [contract] = typed.machine_contracts(&typed.machines()[0]) else {
        panic!("machine should retain its proposition requirement");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("requires should retain the proposition application");
    };
    assert!(matches!(
        application.binder_arguments[0].kind,
        typed_trees::proposition::PropositionBinderArgumentKind::Type
    ));
    assert!(matches!(
        application.binder_arguments[1].kind,
        typed_trees::proposition::PropositionBinderArgumentKind::Const
    ));
    assert_eq!(application.binder_arguments[0].display_name(), "i32");
    assert_eq!(application.binder_arguments[1].display_name(), "7");
    let normalized = typed
        .normalize_proposition_application(application, None)
        .expect("transparent application should normalize");
    assert_eq!(
        normalized.identity_label(),
        "proposition:fact:indexed<i32,7>()"
    );
}

#[test]
fn proposition_static_arguments_reject_wrong_binder_categories_and_const_types() {
    for (source, expected) in [
        (
            r#"
                proposition indexed<T, const N: i32>();
                machine wrong() requires indexed<7, i32>() {}
            "#,
            "type binder `T` received a const literal",
        ),
        (
            r#"
                proposition indexed<const N: bool>();
                machine wrong() requires indexed<1>() {}
            "#,
            "cannot receive integer literal `1` as `bool`",
        ),
    ] {
        let diagnostic = crate::front_end::typed_program_result(source)
            .expect_err("wrong proposition binder category must reject");
        assert!(
            diagnostic.message.contains(expected),
            "unexpected diagnostic: {}",
            diagnostic.message
        );
    }
}

#[test]
fn proposition_type_and_const_arguments_forward_through_machine_binders() {
    let source = r#"
        proposition indexed<T, const N: i32>();

        machine forward<T, const N: i32>()
        requires indexed<T, N>()
        {
        }
    "#;
    let resolved_program = crate::front_end::resolved_program(source);
    let [resolved_contract] =
        resolved_program.machine_contracts(&resolved_program.roots.machines[0])
    else {
        panic!("machine should retain one resolved contract");
    };
    let [symbol_resolved_trees::domain::ProofFact::Expression(resolved_application)] =
        resolved_program.proof_facts(resolved_contract.facts)
    else {
        panic!("requires should retain one resolved expression fact");
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(resolved_call) = resolved_program
        .tables
        .bodies
        .expressions
        .expression(*resolved_application)
    else {
        panic!("resolved fact should remain the indexed application");
    };
    assert!(
        resolved_call.machine_arguments[0].symbol.is_valid(),
        "forwarded type argument should resolve"
    );
    assert!(
        resolved_call.machine_arguments[1].symbol.is_valid(),
        "forwarded const argument should resolve"
    );
    let typed = lower_symbol_resolved_trees(&resolved_program).expect("typing should succeed");

    let machine = &typed.machines()[0];
    let parameters = typed.machine_type_parameters(machine);
    let [contract] = typed.machine_contracts(machine) else {
        panic!("generic machine should retain its proposition requirement");
    };
    let [typed_trees::domain::ProofFact::Proposition(application)] =
        typed.proof_facts.span_or_empty(contract.facts)
    else {
        panic!("requires should retain the proposition application");
    };
    assert_eq!(application.binder_arguments[0].symbol, parameters[0].symbol);
    assert_eq!(application.binder_arguments[1].symbol, parameters[1].symbol);
    assert_eq!(
        typed
            .normalize_proposition_application(application, None)
            .expect("generic application should normalize")
            .identity_label(),
        "proposition:fact:indexed<T,N>()"
    );
}

#[test]
fn retains_exact_sealed_quotient_operation_request_without_admitting_it() {
    let source = r#"
        data Representative { value: i32; }

        machine representative(value: Representative) -> Representative { value }
        machine representative_respects(left: Representative, right: Representative) {}
        machine wrapper(value: Representative) -> Representative {
            Quotient::lift<representative, representative_respects>(value)
        }
    "#;
    let typed = crate::front_end::typed_program(source);
    let request = typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => call.quotient_operation.as_ref(),
            _ => None,
        })
        .expect("sealed quotient request");

    assert_eq!(
        request.kind,
        typed_trees::expression::QuotientOperationKind::Lift
    );
    assert_eq!(
        typed.symbols.name(
            typed
                .symbols
                .get(request.representative_operation.symbol)
                .parent,
        ),
        "representative"
    );
    assert_eq!(
        typed
            .symbols
            .get(request.representative_operation.symbol)
            .kind,
        symbols::SymbolKind::State
    );
    assert_eq!(
        typed.symbols.name(
            typed
                .symbols
                .get(request.theorem_evidence[0].application.symbol)
                .parent,
        ),
        "representative_respects"
    );
    assert_eq!(
        typed
            .symbols
            .get(request.theorem_evidence[0].application.symbol)
            .kind,
        symbols::SymbolKind::State
    );
}
