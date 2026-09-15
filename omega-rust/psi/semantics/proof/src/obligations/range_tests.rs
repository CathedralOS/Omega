//! Range tests for proof obligations.

use super::{FloatRange, ProofConstraint};
use crate::obligations::constraints::derived_binary_constraints;
use crate::obligations::plan::ConstraintBuffer;
use crate::obligations::ranges::constraints_prove_finite;
use crate::obligations::ranges::float_binary_range;
use crate::obligations::ranges::has_named_constraint;
use numerics::bignum::BigInt;
use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType};
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::FloatLiteral;
use typed_trees::name::Identifier;
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
fn proof_range_preserves_valid_mixed_float_endpoints() {
    for floating_minimum in [false, true] {
        let (mut program, carrier) = program_with_u8();
        let integer = program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_value(2).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            }),
        ));
        let floating = program
            .expression_table
            .insert(ExpressionNode::Float(FloatLiteral::new(
                if floating_minimum { 0.5 } else { 3.5 },
            )));
        let (minimum, maximum, expected_minimum, expected_maximum) = if floating_minimum {
            (floating, integer, 0.5, 2.0)
        } else {
            (integer, floating, 2.0, 3.5)
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
            Some(ProofConstraint::FloatRange {
                minimum: FloatLiteral::new(expected_minimum),
                maximum: FloatLiteral::new(expected_maximum),
                maximum_inclusive: true,
            })
        );
    }
}

#[test]
fn proof_range_keeps_exclusive_float_endpoint_verbatim() {
    let (mut program, carrier) = program_with_u8();
    let minimum = program
        .expression_table
        .insert(ExpressionNode::Float(FloatLiteral::new(0.0)));
    let maximum = program
        .expression_table
        .insert(ExpressionNode::Float(FloatLiteral::new(1.5)));

    assert_eq!(
        ProofConstraint::from_node(
            &program,
            carrier,
            &TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: false,
            }
        ),
        Some(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(0.0),
            maximum: FloatLiteral::new(1.5),
            maximum_inclusive: false,
        })
    );
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
fn proof_range_normalizes_exclusive_bounds_without_carrier_arithmetic() {
    for (minimum, maximum, expected_maximum) in [(0, 8, 7), (0, 0, -1), (-128, -128, -129)] {
        let (mut program, carrier) = program_with_u8();
        let minimum_handle = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(minimum)));
        let maximum_handle = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(maximum)));
        assert_eq!(
            ProofConstraint::from_node(
                &program,
                carrier,
                &TypeConstraintNode::Range {
                    minimum: minimum_handle,
                    maximum: maximum_handle,
                    end_inclusive: false,
                }
            ),
            Some(ProofConstraint::IntegerRange {
                minimum: BigInt::from_i64(minimum),
                maximum: BigInt::from_i64(expected_maximum),
            }),
        );
    }
}

#[test]
fn proof_range_retains_full_width_unsigned_exclusive_maximum() {
    let (mut program, carrier) = program_with_u8();
    let minimum = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let maximum = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551616").unwrap(),
    ));
    assert_eq!(
        ProofConstraint::from_node(
            &program,
            carrier,
            &TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: false,
            }
        ),
        Some(ProofConstraint::IntegerRange {
            minimum: BigInt::zero(),
            maximum: BigInt::from_u128(u128::from(u64::MAX)),
        }),
    );
}

#[test]
fn float_range_proves_finite_only_when_infinities_are_excluded() {
    let inclusive = FloatRange {
        minimum: 0.0,
        maximum: 1.5,
        maximum_inclusive: true,
    };
    let strict = FloatRange {
        minimum: 0.0,
        maximum: 1.5,
        maximum_inclusive: false,
    };
    let inclusive_supremum = FloatRange {
        minimum: 0.0,
        maximum: f64::INFINITY,
        maximum_inclusive: true,
    };
    let exclusive_supremum = FloatRange {
        minimum: 0.0,
        maximum: f64::INFINITY,
        maximum_inclusive: false,
    };
    let infinite_floor = FloatRange {
        minimum: f64::NEG_INFINITY,
        maximum: 1.0,
        maximum_inclusive: true,
    };

    assert!(inclusive.proves_finite());
    assert!(strict.proves_finite());
    // `x <= +inf` admits +inf; `x < +inf` and NaN reject it outright.
    assert!(!inclusive_supremum.proves_finite());
    assert!(exclusive_supremum.proves_finite());
    assert!(!infinite_floor.proves_finite());
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
