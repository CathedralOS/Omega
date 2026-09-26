use super::{Identifier, OperatorSpelling, StateParameter, SymbolHandle, TypeReferenceNode};
use crate::tests::front_end::{checked_program, checked_program_result, typed_program};
use crate::tests::operators::operator_with_spelling;

mod const_arguments;
mod property_bounds;

#[test]
fn exact_operator_application_rejects_unsupported_binder_categories() {
    let mut program = typed_trees::TypedTrees::default();
    let unsupported = [
        typed_trees::data::TypeParameterKind::Machine {
            contract: Default::default(),
        },
        typed_trees::data::TypeParameterKind::Proposition {
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
            typed_trees::data::TypeParameter {
                symbol: SymbolHandle::from_arena_index(170 + ordinal as u32),
                name: Identifier::generated("Unsupported"),
                kind,
                bounds: Default::default(),
            },
        );
        assert!(
            typed_trees::operator::closed_operator_application_for_operands(
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
        typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &lifetime_operator,
            &[],
        )
        .is_none()
    );
}

#[test]
fn checked_named_unit_statement_boundary_use_retains_type_application() {
    let checked = checked_program(
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
        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { .. }
    ));
    let [
        checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
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
        Some(typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn named_generic_boundary_rejects_conflicting_landed_literal_application() {
    let diagnostics = checked_program_result(
        r#"
        data Math {}

        boundary operator Math::same<Element>(left: Element, right: Element) -> bool;
        machine compare() -> bool { Math::same<i32>(1i32, 2u64) }
        "#,
    )
    .expect_err("conflicting landed literal types must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "cannot validate explicit static arguments because its operand application remains open or unresolved",
        )
    }));
}

#[test]
fn checked_named_generic_boundary_use_replays_nested_type_application() {
    let checked = checked_program(
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
    let [checked_trees::CheckedBoundaryOperatorApplicationArgument::Type { type_reference, .. }] =
        application.arguments.as_slice()
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
        Some(typed_trees::types::PrimitiveType::I32)
    );
}

#[test]
fn monomorphic_named_boundary_rejects_static_arguments() {
    let diagnostics = checked_program_result(
        r#"
        data Math {}

        boundary operator Math::same(left: i32, right: i32) -> bool;
        machine compare(left: i32, right: i32) -> bool { Math::same<i32>(left, right) }
        "#,
    )
    .expect_err("static argument must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("monomorphic named operator `Math::same` takes no static arguments")
    }));
}

#[test]
fn checked_boundary_first_cohort_maps_direct_open_applications_symbolically() {
    let checked = checked_program(
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
    let [application] = checked
        .facts
        .operators
        .symbolic_boundary_applications
        .as_slice()
    else {
        panic!("only the directly mapped spelled application stays symbolic")
    };
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .expect("generic producer machine");
    assert_eq!(application.machine_symbol, machine.symbol);
    let checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { .. } =
        application.site
    else {
        panic!("spelled open application keeps its expression use site")
    };
    let [
        checked_trees::CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            machine_binder_ordinal,
            machine_binder_symbol,
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one symbolic type-binder mapping")
    };
    assert_eq!(*binder_owner, application.requirement_symbol);
    assert_eq!(*binder_ordinal, 0);
    assert_eq!(
        *binder_symbol,
        checked.operator_type_parameters(&checked.operators()[0])[0].symbol
    );
    assert_eq!(*machine_binder_ordinal, 0);
    assert_eq!(
        *machine_binder_symbol,
        checked.machine_type_parameters(machine)[0].symbol
    );
}

#[test]
fn specialized_generic_operator_provider_retains_exact_closed_realization() {
    let checked = checked_program(
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
                machine.symbol == specialization.template
                    && machine.name.as_str() == "GenericProvider::identity"
            })
        })
        .expect("generic checked operator provider has one concrete specialization");
    let [realization] = specialization.operator_realizations.as_slice() else {
        panic!("specialization retains one exact operator realization")
    };
    let [
        typed_trees::operator::ClosedOperatorApplicationArgument::Type {
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
        Some(typed_trees::types::PrimitiveType::I32)
    );
    assert!(!specialization.commitment.is_zero());
    assert_eq!(
        validation::recompute_checked_machine_specialization_commitment(
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
    let typed_trees::operator::ClosedOperatorApplicationArgument::Type { binder_symbol, .. } =
        &mut tampered.typed.machine_specializations[specialization_index].operator_realizations[0]
            .arguments[0]
    else {
        panic!("fixture retained one type application")
    };
    *binder_symbol = SymbolHandle::invalid();
    assert!(
        validation::recompute_checked_machine_specialization_commitment(
            &tampered,
            tampered.typed.machine_specializations[specialization_index].instance,
        )
        .is_err(),
        "specialization replay rejects a retained binder substitution"
    );
}

#[test]
fn selected_generic_operator_provider_discovers_nested_expression_application() {
    let checked = checked_program_with_selected_generic_providers(
        r#"
        pub data GenericMath {}
        pub boundary operator == GenericMath::equal<Element>(left: Element, right: Element) -> bool;

        pub data GenericProvider {}
        pub machine GenericProvider::equal<Value>(left: Value, right: Value) -> bool
        satisfies GenericMath::equal
        { true }

        machine exercise(left: i32, right: i32) -> bool {
            !(left == right)
        }
        "#,
        &[("GenericMath", "equal", "GenericProvider::equal")],
    );

    let provider = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "GenericProvider::equal")
        .expect("generic provider remains as the authored template");
    assert_eq!(
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| specialization.template == provider.symbol)
            .count(),
        1
    );
}

fn checked_program_with_selected_generic_providers(
    source: &str,
    providers: &[(&str, &str, &str)],
) -> checked_trees::CheckedTrees {
    let typed = typed_program(source);
    let selected = providers
        .iter()
        .map(|(namespace, requirement, provider)| {
            let requirement_operator = typed
                .operators()
                .iter()
                .find(|operator| {
                    typed
                        .operator_path_members(operator.name)
                        .iter()
                        .map(|member| member.as_str())
                        .eq([*namespace, *requirement])
                })
                .expect("selected requirement exists")
                .symbol;
            let realization_machine = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == *provider)
                .expect("selected provider exists")
                .symbol;
            crate::SelectedGenericOperatorProviderSpecialization {
                requirement_operator,
                realization_machine,
            }
        })
        .collect::<Vec<_>>();
    crate::lower_typed_trees(
        typed,
        &crate::CheckingRequest::settled().with_selected_generic_operator_providers(&selected),
    )
    .expect("selected generic providers reach final-substitution closure")
}
