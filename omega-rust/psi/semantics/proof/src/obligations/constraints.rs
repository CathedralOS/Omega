//! Deriving the constraints an expression carries: primitives, literals,
//! binary operators, builtin, extrema and range calls, and named facts.

use crate::obligations::plan::ConstraintBuffer;
use crate::obligations::program_queries::{call_expression_return_type, expression_type_reference};
use crate::obligations::ranges::{
    arithmetic_domain_from_constraints, constraints_prove_finite, float_binary_range,
    float_range_from_constraints, has_named_constraint, integer_binary_range,
    integer_constraints_are_exact, integer_constraints_are_wrapping,
    integer_range_from_constraints,
};
use crate::obligations::{IntegerRange, ProofConstraint};
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, FloatLiteral};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn expression_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> ConstraintBuffer {
    match program.expression_table.expression(expression) {
        // Arm-local constraints require a checked join; no individual arm's
        // refinement is an unconditional fact about the dispatch result.
        ExpressionNode::Match(_) => ConstraintBuffer::new(),
        ExpressionNode::Atomic(atomic) => {
            expression_constraints(program, machine, state, atomic.value)
        }
        ExpressionNode::Binary(binary) => {
            let left = expression_constraints(program, machine, state, binary.left);
            let right = expression_constraints(program, machine, state, binary.right);
            derived_binary_constraints(binary.operator, &left, &right)
        }
        ExpressionNode::Range(range) => {
            let mut constraints = ConstraintBuffer::new();
            if range.start.is_valid() {
                constraints.extend(expression_constraints(program, machine, state, range.start));
            }
            if range.end.is_valid() {
                constraints.extend(expression_constraints(program, machine, state, range.end));
            }
            constraints
        }
        ExpressionNode::Call(call) => {
            if let Some(constraints) =
                derived_builtin_call_constraints(program, machine, state, call)
            {
                return constraints;
            }

            if let Some(return_type) = call_expression_return_type(program, machine, state, call) {
                return collect_constraints_in_state(program, machine, state, return_type);
            }

            ConstraintBuffer::new()
        }
        ExpressionNode::Cast(cast) => expression_constraints(program, machine, state, cast.value),
        ExpressionNode::Unary(unary) => {
            expression_constraints(program, machine, state, unary.operand)
        }
        ExpressionNode::Float(value) => float_literal_constraints(value),
        ExpressionNode::Integer(value) => integer_literal_constraints(value),
        ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            integer_literal_constraints(&numerics::literals::IntegerLiteral::from_value(
                u32::MAX as i64,
            ))
        }
        // An INDEXED read carries its collection's ELEMENT-type constraints
        // (`cells: [i32 [0..=7]; 4]` -> `cells[rp]` reads as [0..=7]). Sound
        // because the same element type now collects a bounded-assignment
        // obligation at every indexed WRITE (the #40 rule: no read narrowing
        // without write enforcement), and ZII requires 0 in the element range.
        ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_) => expression_type_reference(program, machine, state, expression)
            .map(|type_reference| {
                collect_constraints_in_state(program, machine, state, type_reference)
            })
            .unwrap_or_default(),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => ConstraintBuffer::new(),
    }
}

pub(crate) fn collect_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> ConstraintBuffer {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => collect_constraints(program, *referee),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut derived = collect_constraints(program, *base_type);
            derived.extend_iter(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| {
                        // A VALUE of an exact-interval domain satisfies that
                        // interval, so the fact is honest evidence wherever it
                        // is read. What an obligation may DEMAND as an
                        // interval is the separate question
                        // `store_constraint_nodes_with_domain_intervals`
                        // answers.
                        ProofConstraint::from_node_with_domain_intervals(
                            program, *base_type, constraint,
                        )
                    }),
            );
            augment_constraints_with_named_facts(&mut derived);
            derived
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_constraints(program, *element_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .flat_map(|argument| collect_constraints(program, *argument))
            .collect(),
        TypeReferenceNode::Slice { element_type } => collect_constraints(program, *element_type),
        TypeReferenceNode::DynamicTrait { .. } => ConstraintBuffer::new(),
        TypeReferenceNode::Named { name, .. } => primitive_constraints(name),
        TypeReferenceNode::ConstExpression(_) => ConstraintBuffer::new(),
        TypeReferenceNode::Unit => ConstraintBuffer::new(),
    }
}

fn collect_constraints_in_state(
    program: &TypedTrees,
    _machine: &Machine,
    _state: &State,
    type_reference: TypeReferenceHandle,
) -> ConstraintBuffer {
    collect_constraints(program, type_reference)
}

fn primitive_constraints(name: &Identifier) -> ConstraintBuffer {
    // Every UNSIGNED primitive carries its type range as a proof fact -- the
    // lower half (`>= 0`) is what discharges index/bound obligations when a
    // guard supplies only the upper half (`idx < 3`). This list originally
    // held just u32 and usize (now retired); the retirement's corpus sweep
    // exposed the gap: a `u64` field carried no `>= 0` fact and
    // previously-proving programs failed their bounded-parameter checks.
    // N2 (2026-07-11): the proof domain is EXACT bignum, so u64's fact is
    // the true (0, u64::MAX) -- the old i64::MAX cap let a real u64::MAX
    // pass a containment check against a `[0..=i64::MAX]` target (probe-
    // confirmed store unsoundness).
    let mut constraints = ConstraintBuffer::new();
    let range = match name.as_str() {
        "u8" => Some((BigInt::zero(), BigInt::from_u64(u8::MAX as u64))),
        "u16" => Some((BigInt::zero(), BigInt::from_u64(u16::MAX as u64))),
        "u32" => Some((BigInt::zero(), BigInt::from_u64(u32::MAX as u64))),
        "u64" => Some((BigInt::zero(), BigInt::from_u64(u64::MAX))),
        _ => None,
    };
    if let Some((minimum, maximum)) = range {
        constraints.push(ProofConstraint::IntegerRange { minimum, maximum });
    }
    augment_constraints_with_named_facts(&mut constraints);
    constraints
}

fn integer_literal_constraints(literal: &numerics::literals::IntegerLiteral) -> ConstraintBuffer {
    let mut constraints = ConstraintBuffer::new();
    // N2: literal facts are EXACT at any magnitude (canonical text always
    // parses); the D14 width gate still owns which POSITIONS may spell an
    // oversize literal.
    let Some(value) = literal.value_bignum() else {
        return constraints;
    };
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "exact",
    )));

    if !value.is_negative() {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "non_negative",
        )));
    }

    if !value.is_negative() && !value.is_zero() {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "positive",
        )));
    }

    constraints.push(ProofConstraint::IntegerRange {
        minimum: value.clone(),
        maximum: value,
    });

    constraints
}

fn float_literal_constraints(value: &FloatLiteral) -> ConstraintBuffer {
    // The literal's riding landing is authoritative: a suffixed `0.3f32`
    // means the widened f32 value, not the f64 read of its spelling.
    let value = value.landed_f64();
    if !value.is_finite() {
        return ConstraintBuffer::new();
    }

    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "finite",
    )));
    constraints.push(ProofConstraint::FloatRange {
        minimum: FloatLiteral::new(value),
        maximum: FloatLiteral::new(value),
        maximum_inclusive: true,
    });
    constraints
}

pub(crate) fn derived_binary_constraints(
    operator: BinaryOperator,
    left_constraints: &ConstraintBuffer,
    right_constraints: &ConstraintBuffer,
) -> ConstraintBuffer {
    let mut constraints = ConstraintBuffer::new();
    let arithmetic_domain = arithmetic_domain_from_constraints(left_constraints)
        .combine(arithmetic_domain_from_constraints(right_constraints));
    if arithmetic_domain != numerics::arithmetic::ArithmeticDomain::Exact {
        constraints.push(ProofConstraint::ArithmeticDomain(arithmetic_domain));
    }

    // Saturating floats clamp magnitude overflow, so finite operands remain
    // finite for the operations whose only non-finite route is overflow.
    // Divide/modulo deliberately stay out: zero divisors and invalid
    // operations retain IEEE Inf/NaN under the settled policy.
    if arithmetic_domain == numerics::arithmetic::ArithmeticDomain::Saturating
        && constraints_prove_finite(left_constraints)
        && constraints_prove_finite(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add | BinaryOperator::Multiply | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
    }

    if integer_constraints_are_exact(left_constraints)
        && integer_constraints_are_exact(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if integer_constraints_are_wrapping(left_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "wrapping",
        )));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(left_constraints),
        integer_range_from_constraints(right_constraints),
    ) && let Some(range) = integer_binary_range(operator, left_range, right_range)
    {
        if !range.minimum.is_negative() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if !range.minimum.is_negative() && !range.minimum.is_zero() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }
        constraints.push(ProofConstraint::IntegerRange {
            minimum: range.minimum,
            maximum: range.maximum,
        });
    }

    if let (Some(left_range), Some(right_range)) = (
        float_range_from_constraints(left_constraints),
        float_range_from_constraints(right_constraints),
    ) && let Some(range) = float_binary_range(operator, left_range, right_range)
    {
        // The derived bound is honest, but `finite` is the stronger claim:
        // it holds only when the bound itself EXCLUDES the infinities an
        // overflowing operation can still reach (`1e308 + 1e308` is +inf).
        if range.proves_finite() {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "finite",
            )));
        }
        constraints.push(ProofConstraint::FloatRange {
            minimum: FloatLiteral::new(range.minimum),
            maximum: FloatLiteral::new(range.maximum),
            maximum_inclusive: range.maximum_inclusive,
        });
    }

    constraints
}

fn derived_builtin_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<ConstraintBuffer> {
    match call.target.as_str() {
        "max" => derived_extrema_call_constraints(program, machine, state, call, true),
        "min" => derived_extrema_call_constraints(program, machine, state, call, false),
        "range" => derived_range_call_constraints(program, machine, state, call),
        _ => None,
    }
}

fn derived_extrema_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &typed_trees::expression::TableCallExpression,
    is_max: bool,
) -> Option<ConstraintBuffer> {
    let [left, right] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };

    let left_constraints = expression_constraints(program, machine, state, *left);
    let right_constraints = expression_constraints(program, machine, state, *right);
    let mut constraints = ConstraintBuffer::new();

    if integer_constraints_are_exact(&left_constraints)
        && integer_constraints_are_exact(&right_constraints)
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if let Some(range) = extrema_integer_range(
        is_max,
        integer_range_from_constraints(&left_constraints),
        integer_range_from_constraints(&right_constraints),
    ) {
        constraints.push(ProofConstraint::IntegerRange {
            minimum: range.minimum,
            maximum: range.maximum,
        });
    }

    if constraints.is_empty() {
        return None;
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn extrema_integer_range(
    is_max: bool,
    left: Option<IntegerRange>,
    right: Option<IntegerRange>,
) -> Option<IntegerRange> {
    // One-sided cases FABRICATED the missing bound with an i64 sentinel --
    // a false claim once real u64-magnitude values flow (the same class as
    // the retired u64 range-fact cap). With exact bounds they are honest
    // only as "no range"; the two-sided folds are exact.
    match (is_max, left, right) {
        (true, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.max(right.minimum),
            maximum: left.maximum.max(right.maximum),
        }),
        (false, Some(left), Some(right)) => Some(IntegerRange {
            minimum: left.minimum.min(right.minimum),
            maximum: left.maximum.min(right.maximum),
        }),
        _ => None,
    }
}

fn derived_range_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<ConstraintBuffer> {
    let [_, exclusive_max] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };

    let upper_constraints = expression_constraints(program, machine, state, *exclusive_max);
    let mut constraints = ConstraintBuffer::new();
    constraints.push(ProofConstraint::Named(Identifier::generated_static(
        "exact",
    )));

    if let Some(upper_range) = integer_range_from_constraints(&upper_constraints) {
        constraints.push(ProofConstraint::IntegerRange {
            minimum: BigInt::zero(),
            maximum: upper_range.maximum,
        });
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn augment_constraints_with_named_facts(constraints: &mut ConstraintBuffer) {
    if constraints.iter().any(|constraint| {
        matches!(
            constraint,
            ProofConstraint::IntegerRange { minimum, maximum } if minimum == maximum
        ) || matches!(
            constraint,
            ProofConstraint::FloatRange {
                minimum,
                maximum,
                maximum_inclusive,
            } if minimum == maximum && *maximum_inclusive
        )
    }) && !has_named_constraint(constraints, "exact")
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "exact",
        )));
    }

    if let Some(range) = integer_range_from_constraints(constraints) {
        if !range.minimum.is_negative() && !has_named_constraint(constraints, "non_negative") {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "non_negative",
            )));
        }
        if !range.minimum.is_negative()
            && !range.minimum.is_zero()
            && !has_named_constraint(constraints, "positive")
        {
            constraints.push(ProofConstraint::Named(Identifier::generated_static(
                "positive",
            )));
        }
    }

    // A float range is `finite` evidence only when its membership excludes
    // the infinities it names: an inclusive bound that evaluates to +inf
    // admits it, while the exclusive `x < +inf` bounds below it.
    if float_range_from_constraints(constraints).is_some_and(|range| range.proves_finite())
        && !has_named_constraint(constraints, "finite")
    {
        constraints.push(ProofConstraint::Named(Identifier::generated_static(
            "finite",
        )));
    }
}
