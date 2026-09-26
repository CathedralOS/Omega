use super::{Identifier, OperatorSpelling, StateParameter, SymbolHandle};
use crate::tests::front_end::{checked_program, checked_program_result};
use crate::tests::operators::named_type;
use crate::tests::operators::operator_with_spelling;

use language_semantics::const_value::CanonicalConstIdentity;

#[test]
fn checked_spelled_boundary_use_retains_inferred_const_value() {
    let checked = checked_program(
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
    let [checked_trees::CheckedBoundaryOperatorApplicationArgument::Const { value, .. }] =
        application.arguments.as_slice()
    else {
        panic!("one const argument")
    };
    assert_eq!(*value, CanonicalConstIdentity::integer("u64", 4));
}

#[test]
fn explicit_const_argument_must_corroborate_the_operand_value() {
    let accepted = checked_program(
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

    let diagnostics = checked_program_result(
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
    .expect_err("mismatched const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal the value inferred from its operands")
    }));
}

#[test]
fn mixed_type_and_const_application_retains_declaration_order() {
    let checked = checked_program(
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
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_ordinal: 0,
                ..
            },
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_ordinal: 1,
                ..
            }
        ]
    ));
}

#[test]
fn spelled_const_application_reports_an_invalid_declared_carrier_value() {
    let diagnostics = checked_program_result(
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
    .expect_err("out-of-range const must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not fit exact carrier `u8`")
    }));
}

#[test]
fn const_binder_absent_from_operands_remains_open() {
    let mut program = typed_trees::TypedTrees::default();
    let const_carrier = named_type(&mut program, "u64");
    let operand_type = named_type(&mut program, "u8");
    let mut operator =
        operator_with_spelling(SymbolHandle::from_arena_index(901), OperatorSpelling::Add);
    program.push_operator_type_parameter(
        &mut operator,
        typed_trees::data::TypeParameter {
            symbol: SymbolHandle::from_arena_index(902),
            name: Identifier::generated("N"),
            kind: typed_trees::data::TypeParameterKind::Const {
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
            relevance: language_core::BindingRelevance::Relevant,
        },
    );

    assert!(
        typed_trees::operator::closed_operator_application_for_operands(
            &program,
            &operator,
            &[Some(operand_type)],
        )
        .is_none()
    );
}
