use super::PrimitiveType;
use crate::execution::terminal_unit::shared_convergence::shared_integer_runtime_parameter_positions_for_test;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{CheckedIntegerBinaryKind, CheckedScalarExpression};

fn parameter(position: usize) -> CheckedScalarExpression {
    CheckedScalarExpression::Parameter {
        position,
        primitive_type: PrimitiveType::U8,
    }
}

fn binary(
    kind: CheckedIntegerBinaryKind,
    left: CheckedScalarExpression,
    right: CheckedScalarExpression,
) -> CheckedScalarExpression {
    CheckedScalarExpression::IntegerBinary {
        kind,
        primitive_type: PrimitiveType::U8,
        left: Box::new(left),
        right: Box::new(right),
    }
}

#[test]
fn shared_integer_inputs_compose_operations_without_association_or_shell_catalogues() {
    let mut expression = parameter(0);
    for kind in [
        CheckedIntegerBinaryKind::BitwiseAnd,
        CheckedIntegerBinaryKind::BitwiseOr,
        CheckedIntegerBinaryKind::BitwiseXor,
        CheckedIntegerBinaryKind::WrappingShiftLeft,
        CheckedIntegerBinaryKind::WrappingShiftRight,
        CheckedIntegerBinaryKind::WrappingAdd,
        CheckedIntegerBinaryKind::SaturatingAdd,
        CheckedIntegerBinaryKind::WrappingSubtract,
        CheckedIntegerBinaryKind::SaturatingSubtract,
        CheckedIntegerBinaryKind::WrappingMultiply,
        CheckedIntegerBinaryKind::SaturatingMultiply,
        CheckedIntegerBinaryKind::ExactAdd,
        CheckedIntegerBinaryKind::ExactSubtract,
        CheckedIntegerBinaryKind::ExactMultiply,
        CheckedIntegerBinaryKind::ExactDivide,
        CheckedIntegerBinaryKind::ExactRemainder,
        CheckedIntegerBinaryKind::ExactShiftLeft,
        CheckedIntegerBinaryKind::ExactShiftRight,
    ] {
        // Right-associated computed operands and runtime siblings need no
        // extra producer family. This is input custody, not a safety proof.
        expression = binary(kind, parameter(1), expression);
        assert_eq!(
            shared_integer_runtime_parameter_positions_for_test(&expression, 2),
            Some(vec![0, 1])
        );
    }
    for _ in 0..4 {
        expression = CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type: PrimitiveType::U8,
            operand: Box::new(expression),
        };
    }
    let widened = CheckedScalarExpression::IntegerWiden {
        primitive_type: PrimitiveType::U16,
        operand: Box::new(expression),
    };
    let narrowed = CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(widened),
        range: checked_trees::CheckedIntegerRange {
            minimum: numerics::bignum::BigInt::from_i64(0),
            maximum: numerics::bignum::BigInt::from_i64(255),
        },
    };
    assert_eq!(
        shared_integer_runtime_parameter_positions_for_test(&narrowed, 2),
        Some(vec![0, 1])
    );
}

#[test]
fn shared_integer_inputs_reject_missing_coordinates_and_invalid_carrier_custody() {
    let independently_typed_count = binary(
        CheckedIntegerBinaryKind::ExactShiftRight,
        parameter(0),
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U16,
        },
    );
    assert_eq!(
        shared_integer_runtime_parameter_positions_for_test(&independently_typed_count, 2),
        Some(vec![0, 1]),
        "a fixed shift count has its own carrier, unlike an arithmetic operand",
    );
    let local = CheckedScalarExpression::Local {
        position: 0,
        primitive_type: PrimitiveType::U8,
    };
    let boolean = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::Bool,
    };
    let address = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::Addr,
    };
    let invalid_widen = CheckedScalarExpression::IntegerWiden {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(parameter(0)),
    };
    let wrong_carrier = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    for rejected in [
        local,
        boolean,
        address,
        invalid_widen,
        wrong_carrier,
        parameter(2),
    ] {
        assert_eq!(
            shared_integer_runtime_parameter_positions_for_test(
                &binary(CheckedIntegerBinaryKind::ExactAdd, parameter(0), rejected),
                2,
            ),
            None
        );
    }
}

#[test]
fn shared_exact_cast_custody_distinguishes_operations_from_partial_conversion_words() {
    let cast = |operand| CheckedScalarExpression::IntegerExactCast {
        primitive_type: PrimitiveType::U8,
        operand: Box::new(operand),
        range: checked_trees::CheckedIntegerRange {
            minimum: numerics::bignum::BigInt::from_i64(0),
            maximum: numerics::bignum::BigInt::from_i64(255),
        },
    };
    assert_eq!(
        shared_integer_runtime_parameter_positions_for_test(&cast(parameter(0)), 1),
        Some(vec![0]),
        "an identity exact-cast operation does not need a partial conversion word",
    );
    for primitive_type in [PrimitiveType::Addr, PrimitiveType::Bool] {
        assert!(
            shared_integer_runtime_parameter_positions_for_test(
                &cast(CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type
                }),
                1,
            )
            .is_none()
        );
    }
}

#[test]
fn nominal_shared_convergence_preserves_cleanup_and_member_input_custody() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Root {}
        machine Root::computed(token: Token, value: u8, flag: bool) -> bool {
            let staged: bool = (((value ^ 1u8) + 0u8) == 0u8) || flag;
            staged
        }
        machine Root::member(token: Token, flag: bool) -> bool {
            let staged: bool = token.observed && flag;
            staged
        }
        machine Root::repeated_member(token: Token, flag: bool) -> bool {
            let staged: bool = token.observed && (flag || token.observed);
            staged
        }
        machine Root::member_only(token: Token) -> bool {
            let staged: bool = token.observed && true;
            staged
        }
        machine Root::two_members(token: Token) -> bool {
            let staged: bool = token.observed && token.other;
            staged
        }
        machine Root::member_and_integer(token: Token, value: u8) -> bool {
            let staged: bool = token.observed && (value == 0u8);
            staged
        }
        machine Root::multiple_bindings(token: Token, flag: bool) -> bool {
            let first: bool = flag && true;
            let staged: bool = first || false;
            staged
        }
        machine Root::runtime_boolean_equality(
            token: Token, left: bool, right: bool
        ) -> bool {
            let staged: bool = (left == right) && true;
            staged
        }
        "#,
    );
    for name in ["computed", "member", "repeated_member"] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, name))
            .expect("one source-owned shared continuation");
        assert!(plan.shared_boolean_convergence.is_some(), "{name}");
        assert_eq!(plan.cleanup_actions.len(), 1, "{name}");
    }
    for name in ["member_only", "two_members"] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, name))
            .expect("source-distributed member cleanup remains supported");
        assert!(plan.shared_boolean_convergence.is_none(), "{name}");
        assert_eq!(plan.cleanup_actions.len(), 1, "{name}");
    }
    for name in [
        "member_and_integer",
        "multiple_bindings",
        "runtime_boolean_equality",
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, name));
        assert!(
            plan.is_none_or(|plan| plan.shared_boolean_convergence.is_none()),
            "{name} must not bypass the shared input/cleanup contract",
        );
    }
}

#[test]
fn nominal_cleanup_keeps_finite_continuations_and_effect_boundaries() {
    let checked = checked(
        r#"
        data Token {}
        machine Token::drop(&mut self) {}
        data Helper {}
        machine Helper::value() -> u64 { 1u64 }
        machine Helper::touch() {}
        data Root {}
        machine Root::return_expression(token: Token) -> bool {
            let staged: bool = true && false;
            !staged
        }
        machine Root::reused_return(token: Token) -> bool {
            let staged: bool = true && false;
            staged == staged
        }
        machine Root::continuation_chain(token: Token) -> bool {
            let staged: bool = true && false;
            let inverted: bool = !staged;
            let restored: bool = !inverted;
            let final_value: bool = !restored;
            final_value
        }
        machine Root::repeated_short_circuit(token: Token) -> bool {
            let first: bool = true && false;
            let second: bool = first || true;
            second
        }
        machine Root::nested_short_circuit_return(token: Token) -> bool {
            true && (false || true)
        }
        machine Root::repeated_short_circuit_return(token: Token) -> bool {
            (true && false) || true
        }
        machine Root::nested_short_circuit_locals(token: Token) -> bool {
            let staged: bool = true && (false || true);
            let repeated: bool = staged || (true && false);
            repeated
        }
        machine Root::mutable_local(token: Token) -> u64 {
            let mut staged: u64 = 1u64;
            staged
        }
        machine Root::call_local(token: Token) -> u64 {
            let staged: u64 = Helper::value();
            staged
        }
        machine Root::effect_before_return(token: Token) -> u64 {
            Helper::touch();
            1u64
        }
    "#,
    );
    for (name, binding_count) in [
        ("return_expression", 1),
        ("reused_return", 1),
        ("continuation_chain", 4),
        ("repeated_short_circuit", 2),
        ("nested_short_circuit_return", 0),
        ("repeated_short_circuit_return", 0),
        ("nested_short_circuit_locals", 2),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(machine_named(&checked, name))
            .expect("finite continuation cleanup");
        assert_eq!(plan.bindings.len(), binding_count, "{name}");
        assert_eq!(
            plan.return_statement_ordinal as usize, binding_count,
            "{name}"
        );
        assert_eq!(plan.cleanup_actions.len(), 1, "{name}");
    }
    for name in ["mutable_local", "call_local", "effect_before_return"] {
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine_named(&checked, name))
                .is_none(),
            "{name}"
        );
    }
}
