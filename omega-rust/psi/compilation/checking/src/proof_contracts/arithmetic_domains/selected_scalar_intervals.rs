//! Closed integer intervals of selected scalar plans.
//!
//! Stage 04 selects each scalar expression into a `CheckedScalarExpression`
//! plan and evaluates it against live place facts during flow and at exits.
//! The interval arithmetic for those plans lives here, beside the arithmetic
//! walk's own `Interval`, so one domain answers "what range can this integer
//! take" for typed expressions and selected plans alike. Ends are closed
//! `BigInt`s on `facts::IntegerRange`: a `u64` snapshot keeps its exact
//! full-width ends, which the `i64` carrier cannot hold. The place facts
//! themselves stay with the client, which implements
//! `SelectedScalarIntervalSource` over its own snapshots, declared storage
//! invariants and byte literals; nothing here reads source syntax.
//!
//! Remainder and quotient keep the walk's meaning: a divisor interval must
//! exclude zero, its magnitude is exact at the signed minimum, the remainder
//! hull is the dividend hull including zero clipped to that magnitude, and a
//! truncated quotient takes its extrema at the endpoint pairs. The selected
//! Exact shift and bitwise providers are the shared carrier kernels.

use super::{admitted_integer, integer_magnitude, integer_type};
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use facts::IntegerRange;
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// The live facts a selected scalar plan reads. Every answer is the interval
/// of the value at that operand position now; `None` is "no bound", never a
/// refutation.
pub trait SelectedScalarIntervalSource {
    fn binding(&mut self, position: usize, primitive_type: PrimitiveType) -> Option<IntegerRange>;
    fn storage(
        &mut self,
        symbol: SymbolHandle,
        primitive_type: PrimitiveType,
    ) -> Option<IntegerRange>;
    fn structural_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange>;
    fn indexed_field(
        &mut self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
        index: Option<&IntegerRange>,
    ) -> Option<IntegerRange>;
    /// The live byte length of a resolved structural byte carrier. Only a
    /// completed whole-carrier snapshot supplies it; a source with no such
    /// snapshot keeps the full `u64` carrier, which bounds every length.
    fn byte_length(
        &mut self,
        _position: u32,
        _path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        carrier_integer_range(PrimitiveType::U64)
    }
}

/// The closed interval of one selected integer plan under `source`.
pub fn selected_scalar_interval(
    expression: &CheckedScalarExpression,
    source: &mut impl SelectedScalarIntervalSource,
) -> Option<IntegerRange> {
    integer(expression, source).map(|(_, bounds)| bounds)
}

/// Decide only conditions that hold for every value in the captured ranges.
/// An overlapping comparison remains unknown, so callers retain both paths.
pub fn selected_boolean_decision(
    expression: &CheckedBooleanExpression,
    source: &mut impl SelectedScalarIntervalSource,
) -> Option<bool> {
    use CheckedBooleanExpression as Boolean;
    use CheckedIntegerComparisonKind as Comparison;
    match expression {
        Boolean::Constant(value) => Some(*value),
        Boolean::Not(operand) => selected_boolean_decision(operand, source).map(|value| !value),
        Boolean::Equal { left, right } => Some(
            selected_boolean_decision(left, source)? == selected_boolean_decision(right, source)?,
        ),
        Boolean::And { left, right } => match (
            selected_boolean_decision(left, source),
            selected_boolean_decision(right, source),
        ) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Boolean::Or { left, right } => match (
            selected_boolean_decision(left, source),
            selected_boolean_decision(right, source),
        ) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Boolean::IntegerComparison { kind, left, right } => {
            let (left_type, left) = integer(left, source)?;
            let (right_type, right) = integer(right, source)?;
            if left_type != right_type {
                return None;
            }
            match kind {
                Comparison::Equal
                    if left.maximum < right.minimum || right.maximum < left.minimum =>
                {
                    Some(false)
                }
                Comparison::Equal
                    if left.minimum == left.maximum && right.minimum == right.maximum =>
                {
                    Some(left.minimum == right.minimum)
                }
                Comparison::LessThan if left.maximum < right.minimum => Some(true),
                Comparison::LessThan if left.minimum >= right.maximum => Some(false),
                Comparison::LessOrEqual if left.maximum <= right.minimum => Some(true),
                Comparison::LessOrEqual if left.minimum > right.maximum => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn integer(
    expression: &CheckedScalarExpression,
    source: &mut impl SelectedScalarIntervalSource,
) -> Option<(PrimitiveType, IntegerRange)> {
    use CheckedScalarExpression as Expression;
    let (primitive, bounds) = match expression {
        // A view local's extent has no parameter snapshot; its `u64`
        // carrier bounds every length it can store.
        Expression::StructuralSequenceLength { root, path } => (
            PrimitiveType::U64,
            match (
                root.parameter(),
                checked_trees::CheckedScalarReadProjection::static_path(path),
            ) {
                (Some(position), Some(path)) => source.byte_length(position, &path)?,
                _ => carrier_integer_range(PrimitiveType::U64)?,
            },
        ),
        Expression::Parameter {
            position,
            primitive_type,
        }
        | Expression::Local {
            position,
            primitive_type,
        } => (*primitive_type, source.binding(*position, *primitive_type)?),
        Expression::ErasedParameter { .. } => return None,
        Expression::StorageRead {
            symbol,
            primitive_type,
        } => (*primitive_type, source.storage(*symbol, *primitive_type)?),
        Expression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => (
            *primitive_type,
            source.structural_field(*parameter_position, path)?,
        ),
        Expression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            element_path,
            primitive_type,
        } => {
            let index = selected_scalar_interval(&index.expression, source);
            (
                *primitive_type,
                match root.parameter() {
                    // A record element's leaf, or an element selected again
                    // inside a nested array, carries only its leaf type.
                    Some(_) if !element_path.is_empty() => carrier_integer_range(*primitive_type)?,
                    Some(position) => source.indexed_field(position, path, index.as_ref())?,
                    // A view local's elements carry only their element type.
                    None => carrier_integer_range(*primitive_type)?,
                },
            )
        }
        Expression::IntegerLiteral { literal } => {
            let primitive =
                PrimitiveType::non_address_landed_integer(literal.landing()?.landed_type)?;
            let value = literal.value_bignum()?;
            (
                primitive,
                IntegerRange {
                    minimum: value.clone(),
                    maximum: value,
                },
            )
        }
        Expression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } => {
            let left_eval = integer(left, source);
            let right_eval = integer(right, source);
            let (left_type, left) = left_eval?;
            let (right_type, right) = right_eval?;
            // A shift count has its own carrier; value arithmetic still requires
            // matching operands. The selected Exact shift provider owns
            // count landing and width validity independently of result bounds.
            if left_type != *primitive_type
                || (right_type != *primitive_type
                    && !matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactShiftLeft
                            | CheckedIntegerBinaryKind::ExactShiftRight
                    ))
            {
                return None;
            }
            (
                *primitive_type,
                binary(*kind, *primitive_type, right_type, left, right)?,
            )
        }
        Expression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            let (source_type, bounds) = integer(operand, source)?;
            if !integer_range_contains(
                &carrier_integer_range(*primitive_type)?,
                &carrier_integer_range(source_type)?,
            ) {
                return None;
            }
            (*primitive_type, bounds)
        }
        Expression::IntegerExactCast {
            primitive_type,
            operand,
            range,
            ..
        } => {
            let (_, bounds) = integer(operand, source)?;
            // The retained occurrence fact proved the operand lands inside
            // `range`; flow bounds are only a coarser view of the same value.
            // The result is the meet of the two, not a failure when the flow
            // view is wider than the proved spelling range.
            (
                *primitive_type,
                IntegerRange {
                    minimum: bounds.minimum.max(range.minimum.clone()),
                    maximum: bounds.maximum.min(range.maximum.clone()),
                },
            )
        }
        // An equal-width integer re-view reads the same two's-complement
        // bits on the view carrier: exactly the modular image.
        Expression::RepresentationView {
            primitive_type,
            operand,
        } => {
            let (_, bounds) = integer(operand, source)?;
            let carrier = carrier_integer_range(*primitive_type)?;
            // Modular conversion is not monotone across a wrapping boundary.
            (
                *primitive_type,
                if integer_range_contains(&carrier, &bounds) {
                    bounds
                } else {
                    carrier
                },
            )
        }
        Expression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            let (_, bounds) = integer(operand, source)?;
            let carrier = integer_type(*primitive_type)?;
            let low = admitted_integer(carrier, &bounds.minimum)?;
            let high = admitted_integer(carrier, &bounds.maximum)?;
            (
                *primitive_type,
                IntegerRange {
                    minimum: integer_magnitude(carrier.bitwise_not(high)?),
                    maximum: integer_magnitude(carrier.bitwise_not(low)?),
                },
            )
        }
        Expression::Boolean(_)
        | Expression::IeeeFloatLiteral { .. }
        | Expression::ImplementationSelection { .. } => return None,
    };
    integer_range_contains(&carrier_integer_range(primitive)?, &bounds)
        .then_some((primitive, bounds))
}

pub(crate) fn binary(
    kind: CheckedIntegerBinaryKind,
    primitive: PrimitiveType,
    right_primitive: PrimitiveType,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    use CheckedIntegerBinaryKind as Kind;
    if matches!(kind, Kind::ExactShiftLeft | Kind::ExactShiftRight) {
        let carrier = integer_type(primitive)?;
        let count_type = integer_type(right_primitive)?;
        let value = (
            admitted_integer(carrier, &left.minimum)?,
            admitted_integer(carrier, &left.maximum)?,
        );
        let count = (
            admitted_integer(count_type, &right.minimum)?,
            admitted_integer(count_type, &right.maximum)?,
        );
        // Formation is separate from obtaining a finite hull. In particular,
        // exact left shifts reject an overflowing corner instead of clipping
        // it to the result carrier or borrowing right-shift totality.
        let (low, high) = match kind {
            Kind::ExactShiftLeft => carrier.exact_shift_left_bounds(value, count_type, count),
            Kind::ExactShiftRight => carrier.exact_shift_right_bounds(value, count_type, count),
            _ => None,
        }?;
        return Some(IntegerRange {
            minimum: integer_magnitude(low),
            maximum: integer_magnitude(high),
        });
    }
    if matches!(kind, Kind::BitwiseAnd | Kind::BitwiseOr | Kind::BitwiseXor) {
        let carrier = integer_type(primitive)?;
        let left = (
            admitted_integer(carrier, &left.minimum)?,
            admitted_integer(carrier, &left.maximum)?,
        );
        let right = (
            admitted_integer(carrier, &right.minimum)?,
            admitted_integer(carrier, &right.maximum)?,
        );
        let (low, high) = match kind {
            Kind::BitwiseAnd => carrier.bitwise_and_bounds(left, right),
            Kind::BitwiseOr => carrier.bitwise_or_bounds(left, right),
            Kind::BitwiseXor => carrier.bitwise_xor_bounds(left, right),
            _ => None,
        }?;
        return Some(IntegerRange {
            minimum: integer_magnitude(low),
            maximum: integer_magnitude(high),
        });
    }
    if matches!(
        kind,
        Kind::ExactRemainder | Kind::WrappingRemainder | Kind::SaturatingRemainder
    ) {
        return remainder(&left, &right);
    }
    if matches!(
        kind,
        Kind::ExactDivide | Kind::WrappingDivide | Kind::SaturatingDivide
    ) {
        return quotient(&left, &right);
    }
    let mut bounds = match kind {
        Kind::ExactAdd | Kind::WrappingAdd | Kind::SaturatingAdd => IntegerRange {
            minimum: left.minimum.add(&right.minimum),
            maximum: left.maximum.add(&right.maximum),
        },
        Kind::ExactSubtract | Kind::WrappingSubtract | Kind::SaturatingSubtract => IntegerRange {
            minimum: left.minimum.sub(&right.maximum),
            maximum: left.maximum.sub(&right.minimum),
        },
        Kind::ExactMultiply | Kind::WrappingMultiply | Kind::SaturatingMultiply => {
            let corners = [
                left.minimum.mul(&right.minimum),
                left.minimum.mul(&right.maximum),
                left.maximum.mul(&right.minimum),
                left.maximum.mul(&right.maximum),
            ];
            IntegerRange {
                minimum: corners.iter().min()?.clone(),
                maximum: corners.iter().max()?.clone(),
            }
        }
        _ => return None,
    };
    let carrier = carrier_integer_range(primitive)?;
    match kind {
        Kind::ExactAdd | Kind::ExactSubtract | Kind::ExactMultiply => {
            integer_range_contains(&carrier, &bounds).then_some(bounds)
        }
        Kind::WrappingAdd | Kind::WrappingSubtract | Kind::WrappingMultiply => {
            // Reduction is monotone only within one modular period. Shift by
            // the signed carrier minimum before selecting that period.
            let modulus = carrier
                .maximum
                .sub(&carrier.minimum)
                .add(&BigInt::from_u64(1));
            let reduce = |value: &BigInt| {
                let shifted = value.sub(&carrier.minimum);
                let (quotient, remainder) = shifted.div_rem(&modulus)?;
                Some(if remainder.is_negative() {
                    (quotient.sub(&BigInt::from_u64(1)), remainder.add(&modulus))
                } else {
                    (quotient, remainder)
                })
            };
            let (low_period, low) = reduce(&bounds.minimum)?;
            let (high_period, high) = reduce(&bounds.maximum)?;
            Some(if low_period == high_period {
                IntegerRange {
                    minimum: low.add(&carrier.minimum),
                    maximum: high.add(&carrier.minimum),
                }
            } else {
                carrier
            })
        }
        Kind::SaturatingAdd | Kind::SaturatingSubtract | Kind::SaturatingMultiply => {
            bounds.minimum = bounds
                .minimum
                .max(carrier.minimum.clone())
                .min(carrier.maximum.clone());
            bounds.maximum = bounds.maximum.max(carrier.minimum).min(carrier.maximum);
            Some(bounds)
        }
        _ => None,
    }
}

/// Whether every value of a divisor interval is nonzero.
fn excludes_zero(divisor: &IntegerRange) -> bool {
    divisor.minimum >= BigInt::from_u64(1) || divisor.maximum <= BigInt::from_i64(-1)
}

/// Select the primitive after the caller establishes builtin operator meaning.
pub fn checked_integer_binary_kind(
    operator: BinaryOperator,
    domain: ArithmeticDomain,
) -> Option<CheckedIntegerBinaryKind> {
    match (operator, domain) {
        (BinaryOperator::BitwiseAnd, _) => Some(CheckedIntegerBinaryKind::BitwiseAnd),
        (BinaryOperator::BitwiseOr, _) => Some(CheckedIntegerBinaryKind::BitwiseOr),
        (BinaryOperator::BitwiseXor, _) => Some(CheckedIntegerBinaryKind::BitwiseXor),
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingShiftRight)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftRight)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactShiftLeft)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingShiftRight)
        }
        (BinaryOperator::ShiftLeft, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingShiftLeft)
        }
        (BinaryOperator::ShiftRight, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingShiftRight)
        }
        // Trapping arithmetic selects its own runtime-checked primitive; it is
        // never rewritten into the Exact, Wrapping, or Saturating sibling.
        (BinaryOperator::Add, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingAdd)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingMultiply)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Trapping) => {
            Some(CheckedIntegerBinaryKind::TrappingRemainder)
        }
        (BinaryOperator::Add, ArithmeticDomain::Exact) => Some(CheckedIntegerBinaryKind::ExactAdd),
        (BinaryOperator::Subtract, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactMultiply)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Exact) => {
            Some(CheckedIntegerBinaryKind::ExactRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingRemainder)
        }
        (BinaryOperator::Divide, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingDivide)
        }
        (BinaryOperator::Modulo, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingRemainder)
        }
        (BinaryOperator::Add, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingAdd)
        }
        (BinaryOperator::Add, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingAdd)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingSubtract)
        }
        (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingSubtract)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => {
            Some(CheckedIntegerBinaryKind::WrappingMultiply)
        }
        (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => {
            Some(CheckedIntegerBinaryKind::SaturatingMultiply)
        }
        _ => None,
    }
}

/// `dividend % divisor` under truncated division: the remainder takes the
/// dividend's sign and its magnitude stays below the divisor's. The divisor
/// magnitude is exact at the signed minimum because the ends are `BigInt`s,
/// and the dividend hull includes zero because a remainder can be zero even
/// when the dividend cannot. This bounds only the result; the division's own
/// definedness obligation stays with the formation owner.
fn remainder(dividend: &IntegerRange, divisor: &IntegerRange) -> Option<IntegerRange> {
    if !excludes_zero(divisor) {
        return None;
    }
    let one = BigInt::from_u64(1);
    let bound = divisor.minimum.abs().max(divisor.maximum.abs()).sub(&one);
    let minimum = dividend
        .minimum
        .clone()
        .min(BigInt::zero())
        .max(bound.negate());
    let maximum = dividend.maximum.clone().max(BigInt::zero()).min(bound);
    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
}

/// Truncated division is monotone in each operand within each sign region,
/// so a divisor interval excluding zero has its quotient extrema at the
/// endpoint pairs, including when the dividend crosses zero.
fn quotient(dividend: &IntegerRange, divisor: &IntegerRange) -> Option<IntegerRange> {
    if !excludes_zero(divisor) {
        return None;
    }
    let corners = [
        dividend.minimum.div_rem(&divisor.minimum)?.0,
        dividend.minimum.div_rem(&divisor.maximum)?.0,
        dividend.maximum.div_rem(&divisor.minimum)?.0,
        dividend.maximum.div_rem(&divisor.maximum)?.0,
    ];
    Some(IntegerRange {
        minimum: corners.iter().min()?.clone(),
        maximum: corners.iter().max()?.clone(),
    })
}

/// Read the declared `Range` constraints on an exact integer reference. Those
/// constraints are storage invariants: construction and every write must
/// satisfy them, so a read can use them even when no value snapshot is live.
/// A non-Exact arithmetic domain does not range-check its stores, so such a
/// reference supplies no declared bound and the caller keeps the raw carrier.
/// An exclusive borrow can be reborrowed through a pointee type that drops
/// declared constraints, so only a frozen reference chain keeps them.
/// `None` also means "no tightening available"; it is never an error.
pub fn frozen_declared_integer_range(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    primitive_type: PrimitiveType,
) -> Option<IntegerRange> {
    let mut walked = reference;
    loop {
        match program.type_reference_table.type_reference(walked) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                if access.is_exclusive() {
                    return None;
                }
                walked = *referee;
            }
            TypeReferenceNode::Constrained { base_type, .. } => walked = *base_type,
            _ => break,
        }
    }
    let shells = crate::value_custody::type_shapes::exact_constrained_shells(program, reference)?;
    let declared = shells
        .iter()
        .flat_map(|constraints| constraints.iter())
        .filter_map(|constraint| match constraint {
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                // The same closed-endpoint evaluation as source range
                // validation; failure is no bound, never a guessed one.
                let low = crate::closed_integer_range_bound(program, *minimum)?;
                let high = crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?;
                Some(IntegerRange {
                    minimum: low,
                    maximum: high,
                })
            }
            _ => None,
        })
        .reduce(|previous, next| IntegerRange {
            minimum: previous.minimum.max(next.minimum),
            maximum: previous.maximum.min(next.maximum),
        })?;
    let carrier = carrier_integer_range(primitive_type)?;
    let minimum = declared.minimum.max(carrier.minimum);
    let maximum = declared.maximum.min(carrier.maximum);
    (minimum <= maximum).then_some(IntegerRange { minimum, maximum })
}

/// Whether `inner` is a nonempty interval inside `outer`.
pub fn integer_range_contains(outer: &IntegerRange, inner: &IntegerRange) -> bool {
    outer.minimum <= inner.minimum
        && inner.minimum <= inner.maximum
        && inner.maximum <= outer.maximum
}

/// The complete representable range of a fixed-width integer primitive, with
/// exact `u64` ends.
pub fn carrier_integer_range(primitive: PrimitiveType) -> Option<IntegerRange> {
    let (minimum, maximum) = primitive.integer_bounds()?;
    Some(IntegerRange {
        minimum: BigInt::from_i128(minimum),
        maximum: BigInt::from_i128(maximum),
    })
}
