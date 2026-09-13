//! Exact closed numeric queries shared by type identity and semantic validation.
//! The source expression and selected operation remain authoritative. Anonymous
//! arithmetic stays rational until an operand lands; each typed node checks its
//! own carrier. No parameter, field, call or flow fact becomes a static value.

use crate::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::{PrimitiveType, TypeReferenceHandle},
};
use numerics::{
    arithmetic::ArithmeticDomain,
    bignum::{BigInt, BigRational},
    literals::LandedIntegerType,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

/// Exact anonymous result and the warning occurrence retained by its traversal.
pub struct AnonymousNumericValue {
    pub value: BigRational,
    /// First fractional occurrence, even when later arithmetic cancels it.
    pub fractional_origin: ExpressionHandle,
}

struct NumericValue {
    value: BigRational,
    primitive: Option<PrimitiveType>,
    type_reference: Option<TypeReferenceHandle>,
    fractional_origin: ExpressionHandle,
}

/// A completed closed integer retains the carrier of its last typed operation.
pub struct ClosedIntegerValue {
    pub value: BigInt,
    pub primitive: Option<PrimitiveType>,
    pub type_reference: Option<TypeReferenceHandle>,
}

impl NumericValue {
    fn anonymous(value: BigRational) -> Self {
        Self {
            value,
            primitive: None,
            type_reference: None,
            fractional_origin: ExpressionHandle::invalid(),
        }
    }
}

/// Evaluate only anonymous operands and already selected Match result edges.
/// The caller owns selection/coverage and supplies each operator's authority.
pub fn evaluate_anonymous_value<const ALLOW_DECIMAL_LITERALS: bool>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<AnonymousNumericValue> {
    let value = evaluate_value::<ALLOW_DECIMAL_LITERALS>(
        program,
        expression,
        selected_arms,
        None,
        builtin,
    )?;
    Some(AnonymousNumericValue {
        value: value.value,
        fractional_origin: value.fractional_origin,
    })
}

impl TypedTrees {
    /// Exact closed integer meaning, without runtime bindings or flow facts.
    /// Context-free typed operators retain the conservative specialization veto.
    pub fn closed_integer_expression_value(&self, expression: ExpressionHandle) -> Option<BigInt> {
        self.closed_integer_expression_value_in(expression, symbols::SymbolHandle::invalid())
    }

    /// Normalize only after evaluating the authored endpoint, retaining its
    /// landing and per-operation obligations. The predecessor is proof-integer
    /// arithmetic, not a new operation in the endpoint's runtime carrier.
    pub fn closed_integer_range_endpoint(
        &self,
        expression: ExpressionHandle,
        end_inclusive: bool,
    ) -> Option<BigInt> {
        let value = self.closed_integer_expression_value(expression)?;
        Some(canonical_integer_range_endpoint(value, end_inclusive))
    }

    /// The owning machine supplies operator selection, not runtime values.
    pub fn closed_integer_expression_value_in(
        &self,
        expression: ExpressionHandle,
        machine: symbols::SymbolHandle,
    ) -> Option<BigInt> {
        Some(self.closed_integer_value_in(expression, machine)?.value)
    }

    /// Closed value and carrier for immutable bounds; no runtime lookup occurs.
    pub fn closed_integer_value_in(
        &self,
        expression: ExpressionHandle,
        machine: symbols::SymbolHandle,
    ) -> Option<ClosedIntegerValue> {
        let value =
            evaluate_value::<true>(self, expression, &[], Some(machine), &mut |expression| {
                has_anonymous_operator_meaning(self, expression)
            })?;
        Some(ClosedIntegerValue {
            value: value.value.to_integer_exact()?,
            primitive: value.primitive,
            type_reference: value.type_reference,
        })
    }
}

/// Canonical predecessor of an already validated integer endpoint. Static const
/// substitution may supply a decoded value without another source expression.
pub fn canonical_integer_range_endpoint(value: BigInt, end_inclusive: bool) -> BigInt {
    if end_inclusive {
        value
    } else {
        value.sub(&BigInt::from_u64(1))
    }
}

fn evaluate_value<const ALLOW_DECIMAL_LITERALS: bool>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    typed_owner: Option<symbols::SymbolHandle>,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<NumericValue> {
    enum Step {
        Enter(ExpressionHandle),
        Leave(ExpressionHandle),
        Binary(ExpressionHandle, BinaryOperator),
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    let mut values: Vec<NumericValue> = Vec::new();
    let mut fractional_origin = ExpressionHandle::invalid();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => {
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return None;
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Match(dispatch) => {
                        let mut selections = selected_arms
                            .iter()
                            .filter(|(owner, _)| *owner == expression);
                        let (_, selected) = selections.next()?;
                        if selections.any(|(_, other)| other != selected) {
                            return None;
                        }
                        let arms = program.expression_table.match_arms(dispatch.arms);
                        if arms.len() != dispatch.arms.len()
                            || !arms.iter().any(|arm| arm.value == *selected)
                        {
                            return None;
                        }
                        active.push(expression);
                        pending.push(Step::Leave(expression));
                        pending.push(Step::Enter(*selected));
                    }
                    ExpressionNode::Integer(literal) if literal.landing().is_none() => values.push(
                        NumericValue::anonymous(BigRational::from_integer(literal.value_bignum()?)),
                    ),
                    ExpressionNode::Float(literal)
                        if ALLOW_DECIMAL_LITERALS && literal.landing().is_none() =>
                    {
                        let value = BigRational::from_decimal_str(literal.text())?;
                        if !fractional_origin.is_valid() && value.to_integer_exact().is_none() {
                            fractional_origin = expression;
                        }
                        values.push(NumericValue::anonymous(value));
                    }
                    ExpressionNode::Integer(literal) if typed_owner.is_some() => {
                        let landing = literal.landing()?;
                        if landing.domain != ArithmeticDomain::Exact {
                            return None;
                        }
                        let (primitive, atom) = literal_carrier(landing.landed_type)?;
                        let symbol = program
                            .symbols
                            .child_handles(program.symbols.root())?
                            .find(|symbol| {
                                program.symbols.builtin_type_atom(*symbol) == Some(atom)
                            })?;
                        let type_reference = program
                            .type_reference_table
                            .find_named_type_reference(symbol)?;
                        let value = literal.value_bignum()?;
                        land_integer(&value, primitive)?;
                        values.push(NumericValue {
                            value: BigRational::from_integer(value),
                            primitive: Some(primitive),
                            type_reference: Some(type_reference),
                            fractional_origin: ExpressionHandle::invalid(),
                        });
                    }
                    ExpressionNode::Binary(binary)
                        if typed_owner.is_some() || builtin(expression) =>
                    {
                        active.push(expression);
                        pending.push(Step::Leave(expression));
                        pending.push(Step::Binary(expression, binary.operator));
                        pending.push(Step::Enter(binary.right));
                        pending.push(Step::Enter(binary.left));
                    }
                    _ => return None,
                }
            }
            Step::Leave(expression) => {
                if active.pop() != Some(expression) {
                    return None;
                }
            }
            Step::Binary(expression, operator) => {
                let right = values.pop()?;
                let left = values.pop()?;
                let mut value = apply_numeric(
                    program,
                    typed_owner,
                    expression,
                    operator,
                    left,
                    right,
                    builtin,
                )?;
                if !fractional_origin.is_valid() && value.value.to_integer_exact().is_none() {
                    fractional_origin = expression;
                }
                value.fractional_origin = fractional_origin;
                values.push(value);
            }
        }
    }
    if values.len() != 1 {
        return None;
    }
    let mut value = values.pop()?;
    value.fractional_origin = fractional_origin;
    Some(value)
}

fn apply_numeric(
    program: &TypedTrees,
    typed_owner: Option<symbols::SymbolHandle>,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: NumericValue,
    right: NumericValue,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<NumericValue> {
    if left.primitive.is_none() && right.primitive.is_none() {
        if typed_owner.is_some() && !builtin(expression) {
            return None;
        }
        return Some(NumericValue::anonymous(match operator {
            BinaryOperator::Add => left.value.add(&right.value),
            BinaryOperator::Subtract => left.value.sub(&right.value),
            BinaryOperator::Multiply => left.value.mul(&right.value),
            BinaryOperator::Divide => left.value.div(&right.value)?,
            _ => return None,
        }));
    }
    let machine = typed_owner?;
    let spelling = match operator {
        BinaryOperator::Add => language_core::OperatorSpelling::Add,
        BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
        BinaryOperator::Multiply => language_core::OperatorSpelling::Multiply,
        BinaryOperator::Divide => language_core::OperatorSpelling::Divide,
        BinaryOperator::Modulo => language_core::OperatorSpelling::Modulo,
        _ => return None,
    };
    let references = [left.type_reference, right.type_reference];
    if !machine.is_valid()
        && program
            .machine_specializations
            .iter()
            .any(|specialization| {
                !crate::operator::selected_trait_operator_meanings(
                    program,
                    specialization.instance,
                    spelling,
                    &references,
                )
                .is_empty()
            })
    {
        return None;
    }
    if !crate::operator::has_builtin_spelled_expression_meaning(
        program,
        machine,
        expression,
        spelling,
        &references,
    ) || left
        .primitive
        .zip(right.primitive)
        .is_some_and(|(left, right)| left != right)
    {
        return None;
    }
    let primitive = left.primitive.or(right.primitive)?;
    let integer = integer_carrier(primitive)?;
    let left_value = land_integer(&left.value.to_integer_exact()?, primitive)?;
    let right_value = land_integer(&right.value.to_integer_exact()?, primitive)?;
    let result = match operator {
        BinaryOperator::Add => integer.exact_add(left_value, right_value),
        BinaryOperator::Subtract => integer.exact_sub(left_value, right_value),
        BinaryOperator::Multiply => integer.exact_mul(left_value, right_value),
        BinaryOperator::Divide => integer.exact_div(left_value, right_value),
        BinaryOperator::Modulo => integer.exact_rem(left_value, right_value),
        _ => return None,
    }?;
    let result = match result {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    };
    Some(NumericValue {
        value: BigRational::from_integer(result),
        primitive: Some(primitive),
        type_reference: left.type_reference.or(right.type_reference),
        fractional_origin: ExpressionHandle::invalid(),
    })
}

/// Exact carrier admission, also used when one arithmetic operand is variable.
pub fn land_integer(value: &BigInt, primitive: PrimitiveType) -> Option<IntegerValue> {
    let value = if primitive.is_signed_integer() {
        IntegerValue::Signed(i128::from(value.to_i64()?))
    } else {
        IntegerValue::Unsigned(u128::from(value.to_u64()?))
    };
    integer_carrier(primitive)?.admits(value).then_some(value)
}

fn integer_carrier(primitive: PrimitiveType) -> Option<IntegerType> {
    let width = match primitive {
        PrimitiveType::I8 | PrimitiveType::U8 => 8,
        PrimitiveType::I16 | PrimitiveType::U16 => 16,
        PrimitiveType::I32 | PrimitiveType::U32 => 32,
        PrimitiveType::I64 | PrimitiveType::U64 => 64,
        _ => return None,
    };
    IntegerType::new(
        if primitive.is_signed_integer() {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        },
        width,
    )
    .ok()
}

fn literal_carrier(
    landing: LandedIntegerType,
) -> Option<(PrimitiveType, symbols::BuiltinTypeAtom)> {
    use symbols::BuiltinTypeAtom as Atom;
    Some(match landing {
        LandedIntegerType::I8 => (PrimitiveType::I8, Atom::I8),
        LandedIntegerType::I16 => (PrimitiveType::I16, Atom::I16),
        LandedIntegerType::I32 => (PrimitiveType::I32, Atom::I32),
        LandedIntegerType::I64 => (PrimitiveType::I64, Atom::I64),
        LandedIntegerType::U8 => (PrimitiveType::U8, Atom::U8),
        LandedIntegerType::U16 => (PrimitiveType::U16, Atom::U16),
        LandedIntegerType::U32 => (PrimitiveType::U32, Atom::U32),
        LandedIntegerType::U64 => (PrimitiveType::U64, Atom::U64),
        LandedIntegerType::Addr => return None,
    })
}
/// Check builtin anonymous arithmetic eligibility without selecting a carrier.
pub fn has_anonymous_operator_meaning(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        _ => return false,
    };
    has_builtin_anonymous_operands(program, expression, spelling)
}

/// Require anonymous operand lookup and each retained authored occurrence to
/// permit builtin meaning. The spelling alone never grants that authority.
pub fn has_builtin_anonymous_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    spelling: language_core::OperatorSpelling,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    crate::operator::resolve_spelling_for_operands(program, spelling, &[None, None]).is_empty()
        && program
            .expression_table
            .authored_selection_occurrences(expression)
            .all(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        matches!(
                            selection.target(),
                            Target::Intrinsic(Intrinsic::BuiltinOperator)
                                | Target::LateBound(LateBinding::CheckedOperator)
                        )
                    })
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::TableBinaryExpression;
    use numerics::literals::IntegerLiteral;

    fn integer(program: &mut TypedTrees, value: i64) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
    }

    fn binary(
        program: &mut TypedTrees,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
            }))
    }

    #[test]
    fn shared_anonymous_query_retains_fractional_origin_and_single_operator_visits() {
        let mut program = TypedTrees::default();
        let one = integer(&mut program, 1);
        let two = integer(&mut program, 2);
        let quotient = binary(&mut program, one, BinaryOperator::Divide, two);
        let root = binary(&mut program, quotient, BinaryOperator::Multiply, two);
        let mut visits = Vec::new();
        let result = evaluate_anonymous_value::<true>(&program, root, &[], &mut |expression| {
            visits.push(expression);
            true
        })
        .expect("exact anonymous evaluation");
        assert_eq!(result.value.to_integer_exact(), Some(BigInt::from_i64(1)));
        assert_eq!(result.fractional_origin, quotient);
        assert_eq!(visits, [root, quotient]);
        assert!(evaluate_anonymous_value::<true>(&program, root, &[], &mut |_| false).is_none());
    }

    #[test]
    fn proof_predecessor_does_not_use_the_signed_endpoint_window() {
        let mut program = TypedTrees::default();
        let minimum = integer(&mut program, i64::MIN);
        assert_eq!(
            program.closed_integer_range_endpoint(minimum, false),
            Some(BigInt::from_i64(i64::MIN).sub(&BigInt::from_i64(1)))
        );
        assert_eq!(
            program.closed_integer_range_endpoint(minimum, true),
            Some(BigInt::from_i64(i64::MIN))
        );
        let zero = integer(&mut program, 0);
        let invalid = binary(&mut program, minimum, BinaryOperator::Divide, zero);
        assert!(
            program
                .closed_integer_range_endpoint(invalid, false)
                .is_none()
        );
    }

    #[test]
    fn shared_carrier_admission_keeps_full_width_and_rejects_oversized_operands() {
        let maximum = BigInt::from_u64(u64::MAX);
        assert_eq!(
            land_integer(&maximum, PrimitiveType::U64),
            Some(IntegerValue::Unsigned(u128::from(u64::MAX)))
        );
        assert!(land_integer(&maximum.add(&BigInt::from_i64(1)), PrimitiveType::U64).is_none());
        assert!(land_integer(&BigInt::from_i64(-1), PrimitiveType::U64).is_none());
        assert!(land_integer(&BigInt::from_i64(256), PrimitiveType::U8).is_none());
    }
}
