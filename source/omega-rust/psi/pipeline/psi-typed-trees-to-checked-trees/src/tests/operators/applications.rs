use super::*;

mod const_arguments;
mod property_bounds;

#[test]
fn exact_operator_application_does_not_bind_a_same_spelled_foreign_nominal() {
    let operator_symbol = SymbolHandle::from_arena_index(151);
    let binder_symbol = SymbolHandle::from_arena_index(152);
    let foreign_symbol = SymbolHandle::from_arena_index(153);
    let mut program = psi_typed_trees::TypedTrees::default();
    let foreign_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: foreign_symbol,
            name: Identifier::generated("Element"),
        });
    let mut operator = operator_with_spelling(operator_symbol, OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut operator,
        psi_typed_trees::data::TypeParameter {
            symbol: binder_symbol,
            name: Identifier::generated("Element"),
            kind: psi_typed_trees::data::TypeParameterKind::Type,
            bounds: psi_typed_trees::data::DataProperties::default(),
        },
    );
    for name in ["left", "right"] {
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(name),
                type_reference: foreign_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_operator(operator);

    assert!(
        psi_typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &program.operators()[0],
            &[Some(foreign_type), Some(foreign_type)],
        )
        .is_none()
    );
}

#[test]
fn exact_operator_application_rejects_unresolved_nominal_argument() {
    let operator_symbol = SymbolHandle::from_arena_index(154);
    let binder_symbol = SymbolHandle::from_arena_index(155);
    let mut program = psi_typed_trees::TypedTrees::default();
    let binder_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: binder_symbol,
            name: Identifier::generated("Element"),
        });
    let unresolved_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Missing"),
        });
    let mut operator = operator_with_spelling(operator_symbol, OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut operator,
        psi_typed_trees::data::TypeParameter {
            symbol: binder_symbol,
            name: Identifier::generated("Element"),
            kind: psi_typed_trees::data::TypeParameterKind::Type,
            bounds: Default::default(),
        },
    );
    for name in ["left", "right"] {
        program.push_operator_parameter(
            &mut operator,
            StateParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(name),
                type_reference: binder_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );
    }
    program.push_operator(operator);

    assert!(
        psi_typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &program.operators()[0],
            &[Some(unresolved_type), Some(unresolved_type)],
        )
        .is_none()
    );
}

#[test]
fn exact_operator_application_rejects_unsupported_binder_categories() {
    let mut program = psi_typed_trees::TypedTrees::default();
    let unsupported = [
        psi_typed_trees::data::TypeParameterKind::Machine {
            contract: Default::default(),
        },
        psi_typed_trees::data::TypeParameterKind::Proposition {
            contract: Default::default(),
        },
    ];
    for (ordinal, kind) in unsupported.into_iter().enumerate() {
        let mut operator = operator_with_spelling(
            SymbolHandle::from_arena_index(160 + ordinal as u32),
            OperatorSpelling::Add,
        );
        program.push_operator_type_parameter(
            &mut operator,
            psi_typed_trees::data::TypeParameter {
                symbol: SymbolHandle::from_arena_index(170 + ordinal as u32),
                name: Identifier::generated("Unsupported"),
                kind,
                bounds: Default::default(),
            },
        );
        assert!(
            psi_typed_trees::operator::closed_operator_application_for_operands(
                &program,
                &operator,
                &[],
            )
            .is_none()
        );
    }

    let mut lifetime_operator =
        operator_with_spelling(SymbolHandle::from_arena_index(180), OperatorSpelling::Add);
    lifetime_operator
        .lifetime_parameters
        .push(Identifier::generated("'value"));
    assert!(
        psi_typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &lifetime_operator,
            &[],
        )
        .is_none()
    );
}

#[test]
fn checked_boundary_operator_uses_retain_empty_and_typed_applications() {
    let checked = checked_program_from_source(
        r#"
        boundary operator == Number::equal(left: i32, right: i32) -> bool;
        boundary operator != Generic::not_equal<Element>(left: Element, right: Element) -> bool;

        machine equal(left: i32, right: i32) -> bool { left == right }
        machine not_equal(left: i32, right: i32) -> bool { left != right }
        "#,
    );

    let applications = &checked.facts.operators.boundary_applications;
    assert_eq!(applications.len(), 2);
    let empty = applications
        .iter()
        .find(|application| application.arguments.is_empty())
        .expect("monomorphic boundary use has one empty application");
    assert!(empty.requirement_symbol.is_valid());

    let typed = applications
        .iter()
        .find(|application| application.arguments.len() == 1)
        .expect("generic boundary use has one typed application");
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            type_reference,
        },
    ] = typed.arguments.as_slice()
    else {
        panic!("one typed application argument")
    };
    assert_eq!(*binder_owner, typed.requirement_symbol);
    assert_eq!(*binder_ordinal, 0);
    assert!(binder_symbol.is_valid());
    assert!(type_reference.is_valid());
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn checked_boundary_type_application_retains_declaration_order() {
    let checked = checked_program_from_source(
        r#"
        data Pair<Left, Right> { left: Left; right: Right; }

        boundary operator == Pair::equal<Left, Right>(
            left: Pair<Left, Right>,
            right: Pair<Left, Right>
        ) -> bool;

        machine equal(
            left: Pair<i32, u64>,
            right: Pair<i32, u64>
        ) -> bool {
            left == right
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one closed boundary application")
    };
    let arguments = application
        .arguments
        .iter()
        .map(|argument| match argument {
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_ordinal,
                type_reference,
                ..
            } => (
                *binder_ordinal,
                checked.typed.primitive_type_reference(*type_reference),
            ),
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { .. } => {
                panic!("type-only application retained a const argument")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        arguments,
        vec![
            (0, Some(psi_typed_trees::types::PrimitiveType::I32)),
            (1, Some(psi_typed_trees::types::PrimitiveType::U64)),
        ]
    );
}

#[test]
fn checked_named_monomorphic_boundary_use_retains_empty_application() {
    let checked = checked_program_from_source(
        r#"
        data F32 {}

        boundary operator F32::is_finite(value: f32) -> bool;
        machine finite(value: f32) -> bool { F32::is_finite(value) }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one named boundary application")
    };
    assert!(application.arguments.is_empty());
    let psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
        expression,
        origin,
    } = application.site
    else {
        panic!("named value call must retain an expression site")
    };
    assert!(matches!(
        checked.typed.expression_table.expression(expression),
        ExpressionNode::Call(_)
    ));
    let named_uses = checked.facts.operators.named_uses().collect::<Vec<_>>();
    let [named_use] = named_uses.as_slice() else {
        panic!("one named boundary use")
    };
    assert_eq!(origin, named_use.origin);
    assert_eq!(
        application.requirement_symbol,
        named_use.selected_operator_symbol
    );
}

#[test]
fn checked_named_unit_statement_boundary_use_retains_type_application() {
    let checked = checked_program_from_source(
        r#"
        data Sink {}

        boundary operator Sink::discard<Value>(value: Value) -> ();
        machine discard_value(value: i32) { Sink::discard<i32>(value); }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one normalized statement boundary application")
    };
    assert!(matches!(
        application.site,
        psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { .. }
    ));
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            type_reference,
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one checked type binding")
    };
    assert_eq!(*binder_owner, application.requirement_symbol);
    assert_eq!(*binder_ordinal, 0);
    assert!(binder_symbol.is_valid());
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn checked_named_generic_boundary_use_replays_inferred_type_application() {
    let checked = checked_program_from_source(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare(left: i32, right: i32) -> bool { Math::same(left, right) }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one inferred named boundary application")
    };
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            binder_ordinal,
            type_reference,
            ..
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one inferred type argument")
    };
    assert_eq!(*binder_ordinal, 0);
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn checked_named_generic_boundary_use_closes_from_landed_literals() {
    let checked = checked_program_from_source(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare() -> bool { Math::same(1i32, 2i32) }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one landed-literal boundary application")
    };
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            type_reference, ..
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one exact landed-literal type argument")
    };
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn named_generic_boundary_rejects_conflicting_landed_literal_application() {
    let tokens = Lexer::new(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare() -> bool { Math::same<i32>(1i32, 2u64) }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics =
        lower_typed_trees(typed).expect_err("conflicting landed literal types must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "cannot validate explicit static arguments because its operand application remains open or unresolved",
        )
    }));
}

#[test]
fn checked_named_generic_boundary_use_replays_explicit_type_application() {
    let checked = checked_program_from_source(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare(left: i32, right: i32) -> bool { Math::same<i32>(left, right) }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one explicit named boundary application")
    };
    assert_eq!(application.arguments.len(), 1);
}

#[test]
fn checked_named_generic_boundary_use_replays_nested_type_application() {
    let checked = checked_program_from_source(
        r#"
        data Math {}
        data Wrapper<Element> { value: Element; }

        boundary operator Math::same<Value>(left: Value, right: Value) -> bool;
        machine compare(
            left: Wrapper<i32>,
            right: Wrapper<i32>
        ) -> bool {
            Math::same<Wrapper<i32>>(left, right)
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one nested named boundary application")
    };
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            type_reference, ..
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one nested type argument")
    };
    let TypeReferenceNode::Generic { arguments, .. } = checked
        .typed
        .type_reference_table
        .type_reference(*type_reference)
    else {
        panic!("nested application retains generic type structure")
    };
    let [element] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("wrapper has one type argument")
    };
    assert_eq!(
        checked.typed.primitive_type_reference(*element),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn named_generic_boundary_static_type_must_equal_operand_application() {
    let tokens = Lexer::new(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare(left: i32, right: i32) -> bool { Math::same<u64>(left, right) }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("mismatched static type must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal the type inferred from its operands")
    }));
}

#[test]
fn monomorphic_named_boundary_rejects_static_arguments() {
    let tokens = Lexer::new(
        r#"
        data Math {}

        boundary operator Math::same(left: i32, right: i32) -> bool;
        machine compare(left: i32, right: i32) -> bool { Math::same<i32>(left, right) }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("static argument must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("monomorphic named operator `Math::same` takes no static arguments")
    }));
}

#[test]
fn checked_named_open_generic_boundary_application_remains_absent() {
    let checked = checked_program_from_source(
        r#"
        data Math {}

        boundary operator Math::same<Value>(left: Value, right: Value) -> bool;
        machine compare<Element>(left: Element, right: Element) -> bool {
            Math::same(left, right)
        }
        "#,
    );

    assert_eq!(checked.facts.operators.named_uses().count(), 1);
    assert!(checked.facts.operators.boundary_applications.is_empty());
}

#[test]
fn checked_named_bounded_generic_boundary_application_retains_exact_type() {
    let checked = checked_program_from_source(
        r#"
        data Math {}

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: i32, right: i32) -> bool {
            Math::same(left, right)
        }
        "#,
    );

    assert_eq!(checked.facts.operators.named_uses().count(), 1);
    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one closed bounded boundary application")
    };
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            type_reference,
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one exact checked type application")
    };
    assert_eq!(*binder_owner, application.requirement_symbol);
    assert_eq!(*binder_ordinal, 0);
    assert!(binder_symbol.is_valid());
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn checked_boundary_type_application_ignores_binder_renames() {
    let compile = |binder: &str| {
        checked_program_from_source(&format!(
            r#"
            boundary operator != Generic::not_equal<{binder}>(
                left: {binder},
                right: {binder}
            ) -> bool;
            machine compare(left: i32, right: i32) -> bool {{ left != right }}
            "#,
        ))
    };
    let original = compile("Element");
    let renamed = compile("Value");
    let project = |checked: &psi_checked_trees::CheckedTrees| {
        let [application] = checked.facts.operators.boundary_applications.as_slice() else {
            panic!("one exact boundary application")
        };
        application
            .arguments
            .iter()
            .map(|argument| match argument {
                psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                    binder_ordinal,
                    type_reference,
                    ..
                } => (
                    *binder_ordinal,
                    checked
                        .typed
                        .package_qualified_type_identity(*type_reference)
                        .into_string(),
                ),
                psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { .. } => {
                    panic!("type-only application retained a const argument")
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(project(&original), project(&renamed));
}

#[test]
fn checked_boundary_first_cohort_rejects_open_type_applications() {
    let checked = checked_program_from_source(
        r#"
        data Wrapper<Element> { value: Element; }

        boundary operator != Generic::not_equal<Value>(left: Value, right: Value) -> bool;

        machine compare<Element>(left: Element, right: Element) -> bool {
            left != right
        }

        machine compare_wrapped<Element>(
            left: Wrapper<Element>,
            right: Wrapper<Element>
        ) -> bool {
            left != right
        }
        "#,
    );

    assert_eq!(checked.facts.operators.resolved_uses().count(), 2);
    assert!(checked.facts.operators.boundary_applications.is_empty());
}

#[test]
fn checked_boundary_applications_preserve_distinct_use_provenance() {
    let checked = checked_program_from_source(
        r#"
        boundary operator == Number::equal(left: i32, right: i32) -> bool;

        machine first(left: i32, right: i32) -> bool { left == right }
        machine second(left: i32, right: i32) -> bool { left == right }
        "#,
    );

    let [first, second] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("two exact boundary applications")
    };
    assert_eq!(first.requirement_symbol, second.requirement_symbol);
    assert_ne!(first.site, second.site);
}

#[test]
fn specialized_generic_operator_provider_retains_exact_closed_realization() {
    let checked = checked_program_from_source(
        r#"
        pub data GenericMath {}
        pub boundary operator GenericMath::identity<Element>(value: Element) -> Element;

        pub data GenericProvider {}
        pub machine GenericProvider::identity<Value>(value: Value) -> Value
        satisfies GenericMath::identity
        { value }

        machine exercise(value: i32) -> i32 {
            GenericProvider::identity(value)
        }
        "#,
    );

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.instance
                    && machine.name.as_str() == "GenericProvider::identity"
            })
        })
        .expect("generic checked operator provider has one concrete specialization");
    let [realization] = specialization.operator_realizations.as_slice() else {
        panic!("specialization retains one exact operator realization")
    };
    let [
        psi_typed_trees::operator::ClosedOperatorApplicationArgument::Type {
            binder_symbol,
            type_reference,
        },
    ] = realization.arguments.as_slice()
    else {
        panic!("specialized realization retains one exact type argument")
    };
    assert!(realization.requirement_symbol.is_valid());
    assert!(binder_symbol.is_valid());
    assert_eq!(
        checked.typed.primitive_type_reference(*type_reference),
        Some(psi_typed_trees::types::PrimitiveType::I32)
    );
    assert!(!specialization.commitment.is_zero());
    assert_eq!(
        psi_validation::recompute_checked_machine_specialization_commitment(
            &checked,
            specialization.instance,
        )
        .expect("retained closed realization should replay"),
        specialization.commitment.as_bytes(),
    );

    let mut tampered = checked.clone();
    let specialization_index = tampered
        .typed
        .machine_specializations
        .iter()
        .position(|candidate| candidate.instance == specialization.instance)
        .expect("same specialization survives cloning");
    let psi_typed_trees::operator::ClosedOperatorApplicationArgument::Type {
        binder_symbol, ..
    } = &mut tampered.typed.machine_specializations[specialization_index].operator_realizations[0]
        .arguments[0]
    else {
        panic!("fixture retained one type application")
    };
    *binder_symbol = SymbolHandle::invalid();
    assert!(
        psi_validation::recompute_checked_machine_specialization_commitment(
            &tampered,
            tampered.typed.machine_specializations[specialization_index].instance,
        )
        .is_err(),
        "specialization replay rejects a retained binder substitution"
    );
}
