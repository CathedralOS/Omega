use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundSide {
    Upper,
    Lower,
}

/// Bounds that hold at every read, independently of arrival and prefix effects.
fn standing_upper_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    name: ExpressionHandle,
) -> Option<i64> {
    let declared =
        crate::value_custody::places::declared_place_type_raw(program, machine, Some(source), name)
            .and_then(|raw| {
                let interval =
                    crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                        program, raw,
                    )?;
                interval.high()
            });
    declared.or_else(|| standing_place_high(program, machine, source, name))
}

/// The place's standing interval: its declared range, an immutable
/// parameter's `requires`, or its data's `where` facts. A declared type range
/// is read directly above because it is the tightest and cheapest source;
/// this covers the invariants that hold at every program point without being
/// spelled on the type, which is where `data Main where k <= 3` lives now
/// that a data `where` is the canonical spelling for a field relationship.
fn standing_place_high(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    name: ExpressionHandle,
) -> Option<i64> {
    crate::proof_contracts::arithmetic_domains::standing_integer_interval(
        program, machine, source, name,
    )?
    .high()
}

fn standing_place_low(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    name: ExpressionHandle,
) -> Option<i64> {
    crate::proof_contracts::arithmetic_domains::standing_integer_interval(
        program, machine, source, name,
    )?
    .low()
}

/// Standing lower bound, including an exact unsigned carrier's zero floor.
fn standing_lower_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    name: ExpressionHandle,
) -> Option<i64> {
    let declared =
        crate::value_custody::places::declared_place_type_raw(program, machine, Some(source), name)
            .and_then(|raw| {
                let interval =
                    crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                        program, raw,
                    )?;
                interval.low()
            });
    declared
        .or_else(|| standing_place_low(program, machine, source, name))
        .or_else(|| unsigned_place_floor(program, machine, source, name))
}

/// An EXACT unsigned place's floor is 0 by its carrier, with no range,
/// `requires` or `where` needed. The composite walk asks for a floor before it
/// will multiply or subtract, so without this a `u32` field with a stated
/// ceiling -- `data Main where k <= 3` -- bounds above and still fails the
/// nonnegative-operand test that keeps `k * 2` from wrapping below its bound.
///
/// The exactness is the whole of the soundness. A `u32 in Wrapping` place has
/// the same carrier and no such floor: its arithmetic may wrap, which is
/// exactly what those operand-floor tests exist to exclude, and handing one a
/// floor of 0 admits the drifted stride walk that
/// `a_drifted_stride_witness_refuses_the_dependent_view` pins as refused.
fn unsigned_place_floor(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    name: ExpressionHandle,
) -> Option<i64> {
    use typed_trees::types::PrimitiveType;
    let raw = crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(source),
        name,
    )?;
    if program.arithmetic_domain_for_type_reference(raw)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    matches!(
        program.primitive_type_reference(raw)?,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    )
    .then_some(0)
}

fn standing_expression_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    source: &typed_trees::state::State,
    expression: ExpressionHandle,
    side: BoundSide,
) -> Option<i64> {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        return literal.value_i64();
    }
    match side {
        BoundSide::Upper => standing_upper_bound(program, machine, source, expression),
        BoundSide::Lower => standing_lower_bound(program, machine, source, expression),
    }
}

/// One operand's inclusive bound: a literal at its value, a place's
/// standing declaration facts, or (for the upper side) recursively composed
/// arithmetic. Arrival facts belong to the exact-occurrence query instead.
fn offset_operand_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    operand: ExpressionHandle,
    side: BoundSide,
) -> Option<i64> {
    // Composite operands bound through the structural walk FIRST: a
    // declared type range on the arithmetic result is far looser than the
    // composed operand bounds.
    if side == BoundSide::Upper
        && matches!(
            program.expression_table.expression(operand),
            ExpressionNode::Binary(_)
        )
        && let Some(composite) = composite_offset_upper_bound(program, machine, state, operand)
    {
        return Some(composite);
    }
    standing_expression_bound(program, machine, state, operand, side)
}

/// An inclusive UPPER bound for a COMPUTED offset expression (`k * 2`,
/// `k * stride + base`) composed from standing declaration bounds. The
/// composite is sound only where it cannot wrap below the claimed bound:
/// Multiply and ShiftLeft require nonnegative operand floors (checked ops
/// keep an overflowing result unproven), and Subtract additionally requires
/// the left floor to dominate the right ceiling so wrapping underflow
/// cannot produce a value above the bound. Anything else stays unproven.
pub(super) fn composite_offset_upper_bound(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<i64> {
    use typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return offset_operand_bound(program, machine, state, expression, BoundSide::Upper);
    };
    match binary.operator {
        BinaryOperator::Add => {
            offset_operand_bound(program, machine, state, binary.left, BoundSide::Upper)?
                .checked_add(offset_operand_bound(
                    program,
                    machine,
                    state,
                    binary.right,
                    BoundSide::Upper,
                )?)
        }
        BinaryOperator::Subtract => {
            let left_floor =
                offset_operand_bound(program, machine, state, binary.left, BoundSide::Lower)?;
            let right_ceiling =
                offset_operand_bound(program, machine, state, binary.right, BoundSide::Upper)?;
            if left_floor < right_ceiling {
                return None;
            }
            offset_operand_bound(program, machine, state, binary.left, BoundSide::Upper)?
                .checked_sub(offset_operand_bound(
                    program,
                    machine,
                    state,
                    binary.right,
                    BoundSide::Lower,
                )?)
        }
        BinaryOperator::Multiply => {
            let left_floor =
                offset_operand_bound(program, machine, state, binary.left, BoundSide::Lower)?;
            let right_floor =
                offset_operand_bound(program, machine, state, binary.right, BoundSide::Lower)?;
            if left_floor < 0 || right_floor < 0 {
                return None;
            }
            offset_operand_bound(program, machine, state, binary.left, BoundSide::Upper)?
                .checked_mul(offset_operand_bound(
                    program,
                    machine,
                    state,
                    binary.right,
                    BoundSide::Upper,
                )?)
        }
        BinaryOperator::ShiftLeft => {
            let left_floor =
                offset_operand_bound(program, machine, state, binary.left, BoundSide::Lower)?;
            if left_floor < 0 {
                return None;
            }
            let ExpressionNode::Integer(shift) = program.expression_table.expression(binary.right)
            else {
                return None;
            };
            let factor = 1i64.checked_shl(shift.value_i64()?.try_into().ok()?)?;
            offset_operand_bound(program, machine, state, binary.left, BoundSide::Upper)?
                .checked_mul(factor)
        }
        _ => None,
    }
}

/// Prove `expression ≡ 0 (mod modulus)` for a runtime offset expression --
/// the congruence half of the interior-slice tiling proof. Residues compose
/// structurally: a literal multiple; a product with one zero-residue
/// factor; a sum or difference of two zero-residue operands; a left shift
/// by enough bits that `2^n ≡ 0`; a place whose declared interval
/// collapses to a single multiple. Anything else stays unproven.
///
/// Caller contract: this is only consulted after the offset's proven upper
/// bound lands below the source region, so the composed value is small
/// enough that no operand can wrap past it -- the residue arithmetic is
/// exact, not modular.
pub(super) fn offset_expression_is_multiple_of(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    modulus: usize,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    if modulus < 2 {
        return false;
    }
    let modulus = modulus as i64;
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal
            .value_i64()
            .is_some_and(|value| value % modulus == 0),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Multiply => {
                offset_expression_is_multiple_of(
                    program,
                    machine,
                    state,
                    binary.left,
                    modulus as usize,
                ) || offset_expression_is_multiple_of(
                    program,
                    machine,
                    state,
                    binary.right,
                    modulus as usize,
                )
            }
            BinaryOperator::Add | BinaryOperator::Subtract => {
                offset_expression_is_multiple_of(
                    program,
                    machine,
                    state,
                    binary.left,
                    modulus as usize,
                ) && offset_expression_is_multiple_of(
                    program,
                    machine,
                    state,
                    binary.right,
                    modulus as usize,
                )
            }
            BinaryOperator::ShiftLeft => {
                let Some(shift) = ({
                    let ExpressionNode::Integer(shift) =
                        program.expression_table.expression(binary.right)
                    else {
                        return false;
                    };
                    shift.value_i64()
                }) else {
                    return false;
                };
                (0..=60).contains(&shift) && (1i64 << shift) % modulus == 0
            }
            _ => false,
        },
        _ => {
            // A place whose declared interval collapses to one value is a
            // constant for congruence purposes.
            let Some(raw) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            ) else {
                return false;
            };
            let Some(interval) =
                crate::proof_contracts::arithmetic_domains::range_constraint_interval(program, raw)
            else {
                return false;
            };
            matches!(
                (interval.low(), interval.high()),
                (Some(low), Some(high)) if low == high && low % modulus == 0
            )
        }
    }
}
