use super::*;

use psi_language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};

#[test]
fn checked_named_boundary_use_retains_inferred_const_value_and_carrier() {
    let checked = checked_program_from_source(
        r#"
        data ArrayOps {}

        boundary operator ArrayOps::same_length<const N: u64>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;

        machine compare(left: [u8; 4], right: [u8; 4]) -> bool {
            ArrayOps::same_length(left, right)
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one closed const application")
    };
    let [
        psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
            binder_owner,
            binder_ordinal,
            binder_symbol,
            declared_carrier,
            value,
        },
    ] = application.arguments.as_slice()
    else {
        panic!("one const argument")
    };
    assert_eq!(*binder_owner, application.requirement_symbol);
    assert_eq!(*binder_ordinal, 0);
    assert!(binder_symbol.is_valid());
    assert_eq!(
        checked.typed.primitive_type_reference(*declared_carrier),
        Some(psi_typed_trees::types::PrimitiveType::U64)
    );
    assert_eq!(*value, CanonicalConstIdentity::integer("u64", 4));
    assert_eq!(
        value.decode_encoding(),
        Some(DecodedCanonicalConstValue::Integer {
            type_name: "u64".to_owned(),
            value: 4,
        })
    );
}

#[test]
fn checked_spelled_boundary_use_retains_inferred_const_value() {
    let checked = checked_program_from_source(
        r#"
        boundary operator == Array::equal<const N: u64>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;

        machine compare(left: [u8; 4], right: [u8; 4]) -> bool {
            left == right
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one spelled const application")
    };
    let [psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { value, .. }] =
        application.arguments.as_slice()
    else {
        panic!("one const argument")
    };
    assert_eq!(*value, CanonicalConstIdentity::integer("u64", 4));
}

#[test]
fn explicit_const_argument_must_corroborate_the_operand_value() {
    let accepted = checked_program_from_source(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::same_length<const N: u64>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;
        machine compare(left: [u8; 4], right: [u8; 4]) -> bool {
            ArrayOps::same_length<4>(left, right)
        }
        "#,
    );
    assert_eq!(accepted.facts.operators.boundary_applications.len(), 1);

    let tokens = Lexer::new(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::same_length<const N: u64>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;
        machine compare(left: [u8; 4], right: [u8; 4]) -> bool {
            ArrayOps::same_length<5>(left, right)
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("mismatched const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal the value inferred from its operands")
    }));
}

#[test]
fn repeated_const_binder_rejects_inconsistent_operand_lengths() {
    let tokens = Lexer::new(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::same_length<const N: u64>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;
        machine compare(left: [u8; 4], right: [u8; 5]) -> bool {
            ArrayOps::same_length<4>(left, right)
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("inconsistent N must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operand application remains open or unresolved")
    }));
}

#[test]
fn mixed_type_and_const_application_retains_declaration_order() {
    let checked = checked_program_from_source(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::same<const N: u64, Element>(
            left: [Element; N],
            right: [Element; N]
        ) -> bool;
        machine compare(left: [i32; 4], right: [i32; 4]) -> bool {
            ArrayOps::same<4, i32>(left, right)
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one mixed application")
    };
    assert!(matches!(
        application.arguments.as_slice(),
        [
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_ordinal: 0,
                ..
            },
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_ordinal: 1,
                ..
            }
        ]
    ));
}

#[test]
fn synthesized_generic_operand_recovers_its_const_application() {
    let checked = checked_program_from_source(
        r#"
        data Block<const N: u64> { bytes: [u8; N]; }
        data BlockOps {}
        boundary operator BlockOps::equal<const N: u64>(
            left: Block<N>,
            right: Block<N>
        ) -> bool;
        machine compare(left: Block<4>, right: Block<4>) -> bool {
            BlockOps::equal<4>(left, right)
        }
        "#,
    );

    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one reconstructed generic application")
    };
    let [psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { value, .. }] =
        application.arguments.as_slice()
    else {
        panic!("one reconstructed const argument")
    };
    assert_eq!(*value, CanonicalConstIdentity::integer("u64", 4));
}

#[test]
fn inferred_const_value_must_fit_the_declared_carrier() {
    let tokens = Lexer::new(
        r#"
        data ArrayOps {}
        boundary operator ArrayOps::same_length<const N: u8>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;
        machine compare(left: [u8; 256], right: [u8; 256]) -> bool {
            ArrayOps::same_length<256>(left, right)
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("out-of-range const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not fit exact carrier `u8`")
    }));
}

#[test]
fn spelled_const_application_reports_an_invalid_declared_carrier_value() {
    let tokens = Lexer::new(
        r#"
        boundary operator == Array::equal<const N: u8>(
            left: [u8; N],
            right: [u8; N]
        ) -> bool;
        machine compare(left: [u8; 256], right: [u8; 256]) -> bool {
            left == right
        }
        "#,
    )
    .tokenize()
    .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("out-of-range const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not fit exact carrier `u8`")
    }));
}

#[test]
fn const_binder_absent_from_operands_remains_open() {
    let mut program = psi_typed_trees::TypedTrees::default();
    let const_carrier = named_type(&mut program, "u64");
    let operand_type = named_type(&mut program, "u8");
    let mut operator =
        operator_with_spelling(SymbolHandle::from_arena_index(901), OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut operator,
        psi_typed_trees::data::TypeParameter {
            symbol: SymbolHandle::from_arena_index(902),
            name: Identifier::generated("N"),
            kind: psi_typed_trees::data::TypeParameterKind::Const {
                type_reference: const_carrier,
            },
            bounds: Default::default(),
        },
    );
    program.push_operator_parameter(
        &mut operator,
        StateParameter {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("value"),
            type_reference: operand_type,
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );

    assert!(
        psi_typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &operator,
            &[Some(operand_type)],
        )
        .is_none()
    );
}
