use super::*;

#[test]
fn same_spelled_foreign_nominal_does_not_inherit_declared_property() {
    let copy_symbol = SymbolHandle::from_arena_index(201);
    let foreign_symbol = SymbolHandle::from_arena_index(202);
    let mut program = psi_typed_trees::TypedTrees::default();
    program.push_data_definition(psi_typed_trees::data::DataDefinition {
        symbol: copy_symbol,
        name: Identifier::generated("Shared"),
        properties: psi_typed_trees::data::DataProperties {
            multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
            ..Default::default()
        },
        ..Default::default()
    });
    let foreign_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: foreign_symbol,
            name: Identifier::generated("Shared"),
        });
    let mut diagnostics = Vec::new();
    let symbols = psi_validation::TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty());

    assert!(!psi_validation::type_satisfies_declared_property(
        &program,
        &symbols,
        &[],
        foreign_type,
        psi_validation::DeclaredPropertyRequirement::Copy,
    ));
}

#[test]
fn closed_type_satisfying_property_bound_retains_exact_checked_application() {
    let checked = checked_program_from_source(
        r#"
        data Math {}
        data CopyValue [copy] { value: i32; }

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: CopyValue, right: CopyValue) -> bool {
            Math::same(left, right)
        }
        "#,
    );

    let copy_value_symbol = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "CopyValue")
        .expect("CopyValue declaration")
        .symbol;
    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one closed property-bounded boundary application")
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
    let TypeReferenceNode::Named { symbol, .. } = checked
        .typed
        .type_reference_table
        .type_reference(*type_reference)
    else {
        panic!("CopyValue remains an exact nominal type application")
    };
    assert_eq!(*symbol, copy_value_symbol);
}

#[test]
fn closed_type_not_satisfying_property_bound_is_rejected_by_validation() {
    let tokens = Lexer::new(
        r#"
        data Math {}
        data LinearValue [linear] { value: i32; }

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: LinearValue, right: LinearValue) -> bool {
            Math::same(left, right)
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = psi_validation::validate_program(&typed)
        .expect_err("LinearValue must not satisfy the operator's [copy] bound");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("Value [copy]")
            && diagnostic.message.contains("LinearValue")
            && diagnostic.message.contains("does not satisfy `[copy]`")
    }));
}

#[test]
fn explicit_bounded_type_argument_must_equal_operand_inference() {
    let tokens = Lexer::new(
        r#"
        data Math {}
        data Actual [copy] { value: i32; }
        data Other [copy] { value: i32; }

        boundary operator Math::same<Value [copy]>(left: Value, right: Value) -> bool;
        machine compare(left: Actual, right: Actual) -> bool {
            Math::same<Other>(left, right)
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = psi_validation::validate_program(&typed)
        .expect_err("the explicit bounded type must agree with operand inference");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal the type inferred from its operands")
    }));
}
