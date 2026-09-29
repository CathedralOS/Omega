//! Range tests for proof obligations.

use super::{FloatRange, ProofConstraint};
use crate::obligations::IntegerRange;
use crate::obligations::constraints::{derived_binary_constraints, expression_constraints};
use crate::obligations::plan::ConstraintBuffer;
use crate::obligations::ranges::{
    constraints_prove_finite, float_binary_range, has_named_constraint, integer_binary_range,
    integer_range_from_constraints,
};
use numerics::bignum::BigInt;
use numerics::literals::{IntegerLanding, IntegerLiteral, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionNode, FloatLiteral, TableCastExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::TypeConstraintNode;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

/// A program carrying one builtin `Named` type reference; returns the
/// carrier handle constraint readers resolve their declared type through.
fn program_with_primitive(
    atom: symbols::BuiltinTypeAtom,
    name: &'static str,
) -> (TypedTrees, TypeReferenceHandle) {
    let mut builder = symbols::SymbolTableBuilder::new();
    let root = builder.insert_root(
        symbols::SymbolKind::Root,
        symbols::SymbolNameRef::Static("root"),
    );
    let builtins = symbols::SymbolTableBuilder::child_handles(
        builder.insert_children(root, symbols::builtin_type_symbols()),
    )
    .collect::<Vec<_>>();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: builtins[atom.ordinal()],
            name: Identifier::generated(name),
        });
    (program, carrier)
}

fn program_with_u8() -> (TypedTrees, TypeReferenceHandle) {
    program_with_primitive(symbols::BuiltinTypeAtom::U8, "u8")
}

#[test]
fn proof_range_rejects_invalid_integer_landing_in_float_fallback() {
    for floating_peer in [false, true] {
        for invalid_minimum in [false, true] {
            let (mut program, carrier) = program_with_u8();
            let invalid = program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(256).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U8,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                }),
            ));
            let peer = program.expression_table.insert(if floating_peer {
                ExpressionNode::Float(FloatLiteral::new(0.0))
            } else {
                ExpressionNode::Integer(IntegerLiteral::from_value(0))
            });
            let (minimum, maximum) = if invalid_minimum {
                (invalid, peer)
            } else {
                (peer, invalid)
            };
            assert_eq!(
                ProofConstraint::from_node(
                    &program,
                    carrier,
                    &TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive: true,
                    }
                ),
                None,
                "invalid integer endpoint must not mint floating evidence"
            );
        }
    }
}

#[test]
fn proof_float_range_endpoints_read_at_the_declared_carrier() {
    // The authored spelling rounds once at the declared carrier: an f32
    // range's `0.3` endpoint is the widened f32 grid point
    // 0.30000001192092896, not the f64 text read -- the same value every
    // landed f32 argument delivers.
    let (mut program, f32_carrier) = program_with_primitive(symbols::BuiltinTypeAtom::F32, "f32");
    let minimum = program
        .expression_table
        .insert(ExpressionNode::Float(FloatLiteral::parse("0.0").unwrap()));
    let maximum = program
        .expression_table
        .insert(ExpressionNode::Float(FloatLiteral::parse("0.3").unwrap()));
    assert_eq!(
        ProofConstraint::from_node(
            &program,
            f32_carrier,
            &TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: true,
            }
        ),
        Some(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(0.0),
            maximum: FloatLiteral::new(f64::from(0.3f32)),
            maximum_inclusive: true,
        })
    );

    // A wider authored landing cannot narrow through the f32 read, so an
    // f64-suffixed literal is not an f32 endpoint at all.
    let f64_maximum = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("0.3f64").unwrap(),
    ));
    assert_eq!(
        ProofConstraint::from_node(
            &program,
            f32_carrier,
            &TypeConstraintNode::Range {
                minimum,
                maximum: f64_maximum,
                end_inclusive: true,
            }
        ),
        None,
        "an f64-landed literal is not an f32 endpoint"
    );

    // A mixed endpoint pair converts its integer endpoint once into the
    // carrier: at f32 the bound is the f32 rendering (2^24 + 1 rounds to
    // 16777216.0f32), not the exact integer read the f64 window keeps.
    // (The float peer must be non-integral so the integer path declines.)
    let integer_minimum =
        program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(
                16777217,
            )));
    let float_maximum = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("16777218.5").unwrap(),
    ));
    assert_eq!(
        ProofConstraint::from_node(
            &program,
            f32_carrier,
            &TypeConstraintNode::Range {
                minimum: integer_minimum,
                maximum: float_maximum,
                end_inclusive: true,
            }
        ),
        Some(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(f64::from(16777217_i64 as f32)),
            maximum: FloatLiteral::new(f64::from(16777218.5f32)),
            maximum_inclusive: true,
        })
    );

    // An f32-landed endpoint under an f64 range keeps its widened f32
    // value (validation's `closed_float_range_endpoint` twin).
    let (mut program, f64_carrier) = program_with_primitive(symbols::BuiltinTypeAtom::F64, "f64");
    let minimum = program
        .expression_table
        .insert(ExpressionNode::Float(FloatLiteral::parse("0.0").unwrap()));
    let maximum = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("0.3f32").unwrap(),
    ));
    assert_eq!(
        ProofConstraint::from_node(
            &program,
            f64_carrier,
            &TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: true,
            }
        ),
        Some(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(0.0),
            maximum: FloatLiteral::new(f64::from(0.3f32)),
            maximum_inclusive: true,
        })
    );
}

#[test]
fn float_ranges_intersect_and_contain_with_strict_endpoints() {
    let strict = FloatRange {
        minimum: 0.0,
        maximum: 1.5,
        maximum_inclusive: false,
    };
    let inclusive = FloatRange {
        minimum: 0.0,
        maximum: 1.5,
        maximum_inclusive: true,
    };
    let below = FloatRange::closed(1.0);

    assert!(strict.contains_range(&below));
    assert!(!strict.contains_range(&inclusive));
    assert!(strict.contains_range(&strict));
    assert!(inclusive.contains_range(&strict));
    assert_eq!(strict.intersect(inclusive), strict);
}

#[test]
fn float_binary_range_refuses_operands_that_admit_infinity() {
    let admits_infinity = FloatRange {
        minimum: 0.0,
        maximum: f64::INFINITY,
        maximum_inclusive: true,
    };
    let finite = FloatRange::closed(1.0);

    // `inf + -inf`, `inf - inf`, `0 * inf`, `0 / 0`, `inf / inf` are all
    // NaN; no IEEE-membership claim is honest once an operand admits one.
    for operator in [
        BinaryOperator::Add,
        BinaryOperator::Subtract,
        BinaryOperator::Multiply,
        BinaryOperator::Divide,
    ] {
        assert_eq!(float_binary_range(operator, admits_infinity, finite), None);
        assert_eq!(float_binary_range(operator, finite, admits_infinity), None);
    }
}

#[test]
fn derived_float_range_stays_honest_through_magnitude_overflow() {
    let mut wide = ConstraintBuffer::new();
    wide.push(ProofConstraint::FloatRange {
        minimum: FloatLiteral::new(0.0),
        maximum: FloatLiteral::new(1e308),
        maximum_inclusive: true,
    });

    let constraints = derived_binary_constraints(BinaryOperator::Add, &wide, &wide);

    // `1e308 + 1e308` overflows to +inf: the derived bound reports it
    // honestly (`result <= +inf`) but must NOT also claim `finite`.
    assert!(constraints.iter().any(|constraint| matches!(
        constraint,
        ProofConstraint::FloatRange { maximum, .. } if maximum.value() == f64::INFINITY
    )));
    assert!(!has_named_constraint(&constraints, "finite"));
    assert!(!constraints_prove_finite(&constraints));
}

#[test]
fn derived_float_range_proves_finite_when_the_bound_is_finite() {
    let mut narrow = ConstraintBuffer::new();
    narrow.push(ProofConstraint::FloatRange {
        minimum: FloatLiteral::new(0.0),
        maximum: FloatLiteral::new(1.5),
        maximum_inclusive: true,
    });

    let constraints = derived_binary_constraints(BinaryOperator::Add, &narrow, &narrow);

    assert!(has_named_constraint(&constraints, "finite"));
    assert!(constraints_prove_finite(&constraints));
}

fn integer_range(minimum: i64, maximum: i64) -> IntegerRange {
    IntegerRange {
        minimum: BigInt::from_i64(minimum),
        maximum: BigInt::from_i64(maximum),
    }
}

#[test]
fn integer_binary_range_folds_or_and_xor_under_the_wider_mask() {
    // Flag words: `[0,1] | [0,1]` and `[0,1] ^ [0,1]` both fold to `[0,1]`
    // -- endpoint arithmetic would falsely claim `[0,2]`.
    for operator in [BinaryOperator::BitwiseOr, BinaryOperator::BitwiseXor] {
        assert_eq!(
            integer_binary_range(operator, integer_range(0, 1), integer_range(0, 1)),
            Some(integer_range(0, 1)),
        );
        assert_eq!(
            integer_binary_range(operator, integer_range(0, 15), integer_range(0, 1)),
            Some(integer_range(0, 15)),
        );
    }

    // OR keeps the wider operand's floor (x | y >= max(x, y)); XOR can
    // cancel every shared bit, so its honest floor stays zero.
    assert_eq!(
        integer_binary_range(
            BinaryOperator::BitwiseOr,
            integer_range(2, 3),
            integer_range(0, 1)
        ),
        Some(integer_range(2, 3)),
    );
    assert_eq!(
        integer_binary_range(
            BinaryOperator::BitwiseXor,
            integer_range(2, 3),
            integer_range(0, 1)
        ),
        Some(integer_range(0, 3)),
    );

    // Degenerate bounds: [0,0] | [0,0] has no set bit available.
    assert_eq!(
        integer_binary_range(
            BinaryOperator::BitwiseOr,
            integer_range(0, 0),
            integer_range(0, 0)
        ),
        Some(integer_range(0, 0)),
    );

    // A possibly-negative operand carries sign bits: no mask claim.
    for operator in [BinaryOperator::BitwiseOr, BinaryOperator::BitwiseXor] {
        assert_eq!(
            integer_binary_range(operator, integer_range(-8, -1), integer_range(0, 1)),
            None,
        );
        assert_eq!(
            integer_binary_range(operator, integer_range(0, 1), integer_range(-8, 7)),
            None,
        );
    }
}

#[test]
fn bool_to_integer_cast_constraints_mint_the_two_value_range() {
    // `bool as i32` produces exactly {0,1}: the conversion's constraint
    // derives a [0,1] range even when the source carries no facts.
    let (mut program, bool_carrier, i32_carrier) = {
        let mut builder = symbols::SymbolTableBuilder::new();
        let root = builder.insert_root(
            symbols::SymbolKind::Root,
            symbols::SymbolNameRef::Static("root"),
        );
        let builtins = symbols::SymbolTableBuilder::child_handles(
            builder.insert_children(root, symbols::builtin_type_symbols()),
        )
        .collect::<Vec<_>>();
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        };
        let bool_carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: builtins[symbols::BuiltinTypeAtom::Bool.ordinal()],
                name: Identifier::generated("bool"),
            });
        let i32_carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: builtins[symbols::BuiltinTypeAtom::I32.ordinal()],
                name: Identifier::generated("i32"),
            });
        (program, bool_carrier, i32_carrier)
    };
    let machine = Machine::default();
    let state = State::default();
    let source = program
        .expression_table
        .insert(ExpressionNode::ZeroValue(bool_carrier));
    let cast = program
        .expression_table
        .insert(ExpressionNode::Cast(TableCastExpression {
            value: source,
            target_type: i32_carrier,
            result_type: i32_carrier,
            target_label: arena::HandleSpan::empty(),
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: symbols::SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: language_core::cast_form::CastForm::Value,
        }));

    let constraints = expression_constraints(&program, &machine, &state, cast);

    assert_eq!(
        integer_range_from_constraints(&constraints),
        Some(integer_range(0, 1)),
    );
    assert!(has_named_constraint(&constraints, "non_negative"));

    // The folded flag-word shape the filesystem encoder stores:
    // `(bool as i32) | (bool as i32)` proves [0,1] end to end.
    let mut flag = ConstraintBuffer::new();
    flag.push(ProofConstraint::IntegerRange {
        minimum: BigInt::zero(),
        maximum: BigInt::from_i64(1),
    });
    assert_eq!(
        integer_binary_range(
            BinaryOperator::BitwiseOr,
            integer_range_from_constraints(&flag).unwrap(),
            integer_range_from_constraints(&flag).unwrap()
        ),
        Some(integer_range(0, 1)),
    );
}
