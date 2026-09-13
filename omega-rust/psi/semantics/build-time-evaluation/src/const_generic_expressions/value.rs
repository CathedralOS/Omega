//! Exact scalar evaluation after declaration selection.
//!
//! Boolean leaves and comparison results share the same value path so nested
//! equality keeps its operands' meaning. Each binary node is admitted against
//! its exact selected operator before evaluation; equal canonical results do
//! not replace the separate authored selection custody retained by the probe.
//! Equality is strict, so both operands use the existing left-to-right traversal.
//! Boolean logic defers its right operand until the left result selects it, as
//! required by the language expression schedule. The enclosing probe already
//! checked selection and retained custody across both operands: skipping execution
//! cannot hide a runtime name or unauthorized operator in the unselected branch.
//! A separate shape pass establishes types and complete anonymous-rational
//! landings across both operands before selective evaluation runs landed operations.
//! Two anonymous numeric operands instead compare as exact rationals through the
//! shared validation evaluator. Their Boolean result supplies no numeric carrier;
//! neither integer truncation nor floating rounding belongs to this comparison.
//! The static pass requires both rational values even in an unselected comparison:
//! anonymous division by zero has no value under the numeric contract.
//!
//! Match uses this same work stack: retain the subject, test patterns in order,
//! then visit only the selected result. The Match owner first checks the complete
//! scalar graph, coverage, arm compatibility, and actual landing boundaries.
//! Selected result handles let the shared rational evaluator read surrounding
//! arithmetic without cloning the program or prematurely landing inner arms.
//! All-arm bounds that need additional proofs reject instead of enumerating
//! independent branch combinations or executing skipped landed expressions.

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLiteral, LandedIntegerType},
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    types::PrimitiveType,
};

mod match_dispatch;

#[cfg(test)]
#[path = "value/match_tests.rs"]
mod match_tests;

#[derive(Clone, Copy)]
enum Shape {
    Anonymous(ExpressionHandle),
    Boolean,
    Integer(LandedIntegerType),
}

#[derive(Clone, Copy)]
enum Value {
    Anonymous(ExpressionHandle),
    Boolean(bool),
    Landed(LandedIntegerType, IntegerValue),
}

/// Only a wholly anonymous final value uses `destination`. A previously
/// landed result retains its carrier for the caller's destination check.
pub(crate) fn evaluate(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
    if !program.expression_table.expression_is_valid(expression) {
        return Err("invalid constant expression".to_owned());
    }
    enum Step {
        Enter(ExpressionHandle),
        Binary(ExpressionHandle, BinaryOperator),
        LogicalLeft(ExpressionHandle, BinaryOperator, ExpressionHandle),
        LogicalRight(ExpressionHandle),
        MatchSubject(ExpressionHandle),
        MatchNext(ExpressionHandle, usize, match_dispatch::MatchSubject),
        MatchPattern(ExpressionHandle, usize, match_dispatch::MatchSubject),
        MatchResult(ExpressionHandle),
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    let mut values = Vec::new();
    match_dispatch::validate_graph(program, expression)?;
    let (mut warnings, matches) =
        validate_shapes(program, machine, state, expression, destination)?;
    let mut selected_arms = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => {
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return Err("invalid or cyclic constant expression".into());
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Match(dispatch) => {
                        active.push(expression);
                        pending.push(Step::MatchSubject(expression));
                        pending.push(Step::Enter(dispatch.subject));
                    }
                    ExpressionNode::Boolean(value) => values.push(Value::Boolean(*value)),
                    ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                        values.push(landed_literal(literal)?);
                    }
                    ExpressionNode::Integer(_) => values.push(Value::Anonymous(expression)),
                    ExpressionNode::Float(literal) if literal.landing().is_none() => {
                        values.push(Value::Anonymous(expression));
                    }
                    ExpressionNode::Binary(binary) => {
                        if !validation::has_builtin_binary_expression_meaning(
                            program,
                            machine,
                            Some(state),
                            expression,
                        ) {
                            return Err(
                                "constant expression has no selected builtin operator meaning"
                                    .into(),
                            );
                        }
                        active.push(expression);
                        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                            pending.push(Step::LogicalLeft(
                                expression,
                                binary.operator,
                                binary.right,
                            ));
                        } else {
                            pending.push(Step::Binary(expression, binary.operator));
                            pending.push(Step::Enter(binary.right));
                        }
                        pending.push(Step::Enter(binary.left));
                    }
                    _ => return Err("unsupported node in exact integer constant expression".into()),
                }
            }
            Step::MatchSubject(expression) => {
                let plan = matches
                    .iter()
                    .find(|plan| plan.expression == expression)
                    .ok_or("missing constant Match plan")?;
                let subject = values.pop().ok_or("missing constant Match subject")?;
                let subject = match_dispatch::coerce(
                    program,
                    machine,
                    state,
                    subject,
                    plan.subject,
                    &selected_arms,
                    &mut warnings,
                )?;
                let subject = match_dispatch::MatchSubject::new(
                    program,
                    subject,
                    &selected_arms,
                    |operand| {
                        validation::has_builtin_binary_expression_meaning(
                            program,
                            machine,
                            Some(state),
                            operand,
                        )
                    },
                )?;
                pending.push(Step::MatchNext(expression, 0, subject));
            }
            Step::MatchNext(expression, ordinal, subject) => {
                let ExpressionNode::Match(dispatch) =
                    program.expression_table.expression(expression)
                else {
                    return Err("constant Match lost dispatch".into());
                };
                let arm = program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .get(ordinal)
                    .ok_or("constant Match has no selected arm")?;
                match arm.pattern {
                    typed_trees::expression::MatchPattern::Wildcard => {
                        selected_arms.push((expression, arm.value));
                        pending.push(Step::MatchResult(expression));
                        pending.push(Step::Enter(arm.value));
                    }
                    typed_trees::expression::MatchPattern::Value(pattern) => {
                        pending.push(Step::MatchPattern(expression, ordinal, subject));
                        pending.push(Step::Enter(pattern));
                    }
                }
            }
            Step::MatchPattern(expression, ordinal, subject) => {
                let plan = matches
                    .iter()
                    .find(|plan| plan.expression == expression)
                    .ok_or("missing constant Match plan")?;
                let pattern = values.pop().ok_or("missing constant Match pattern")?;
                let pattern = match_dispatch::coerce(
                    program,
                    machine,
                    state,
                    pattern,
                    plan.subject,
                    &selected_arms,
                    &mut warnings,
                )?;
                let matched = subject.matches(program, pattern, &selected_arms, |operand| {
                    validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        operand,
                    )
                })?;
                if matched {
                    let ExpressionNode::Match(dispatch) =
                        program.expression_table.expression(expression)
                    else {
                        return Err("constant Match lost dispatch".into());
                    };
                    let arm = program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .get(ordinal)
                        .ok_or("constant Match lost arm")?;
                    selected_arms.push((expression, arm.value));
                    pending.push(Step::MatchResult(expression));
                    pending.push(Step::Enter(arm.value));
                } else {
                    pending.push(Step::MatchNext(expression, ordinal + 1, subject));
                }
            }
            Step::MatchResult(expression) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant Match traversal".into());
                }
                let plan = matches
                    .iter()
                    .find(|plan| plan.expression == expression)
                    .ok_or("missing constant Match plan")?;
                let value = values.pop().ok_or("constant Match lost selected result")?;
                values.push(match_dispatch::coerce(
                    program,
                    machine,
                    state,
                    value,
                    plan.result,
                    &selected_arms,
                    &mut warnings,
                )?);
            }
            Step::LogicalLeft(expression, operator, right) => {
                let Some(Value::Boolean(left)) = values.pop() else {
                    return Err("Boolean logic requires a Boolean left operand".into());
                };
                if left == (operator == BinaryOperator::And) {
                    pending.push(Step::LogicalRight(expression));
                    pending.push(Step::Enter(right));
                } else {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    values.push(Value::Boolean(left));
                }
            }
            Step::LogicalRight(expression) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                if !matches!(values.last(), Some(Value::Boolean(_))) {
                    return Err("Boolean logic requires a Boolean right operand".into());
                }
            }
            Step::Binary(expression, operator) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                let right = values.pop().ok_or("missing right constant operand")?;
                let left = values.pop().ok_or("missing left constant operand")?;
                let value = match (left, right) {
                    (Value::Anonymous(_), Value::Anonymous(_))
                        if matches!(
                            operator,
                            BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                        ) =>
                    {
                        Value::Boolean(compare_anonymous(
                            program,
                            machine,
                            state,
                            expression,
                            &selected_arms,
                        )?)
                    }
                    (Value::Anonymous(_), Value::Anonymous(_)) => {
                        if !matches!(
                            operator,
                            BinaryOperator::Add
                                | BinaryOperator::Subtract
                                | BinaryOperator::Multiply
                                | BinaryOperator::Divide
                        ) {
                            return Err(
                                "anonymous constant operation requires a selected fixed carrier"
                                    .into(),
                            );
                        }
                        Value::Anonymous(expression)
                    }
                    (Value::Landed(carrier, left), Value::Anonymous(right)) => {
                        let right_destination = if matches!(
                            operator,
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                        ) {
                            PrimitiveType::U64
                        } else {
                            primitive(carrier)?
                        };
                        let right = land_anonymous(
                            program,
                            machine,
                            state,
                            right,
                            right_destination,
                            &selected_arms,
                            &mut warnings,
                        )?;
                        apply(operator, Value::Landed(carrier, left), right)?
                    }
                    (Value::Anonymous(left), Value::Landed(carrier, right)) => {
                        if matches!(
                            operator,
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                        ) {
                            return Err("shift value requires a selected fixed carrier".into());
                        }
                        let left = land_anonymous(
                            program,
                            machine,
                            state,
                            left,
                            primitive(carrier)?,
                            &selected_arms,
                            &mut warnings,
                        )?;
                        apply(operator, left, Value::Landed(carrier, right))?
                    }
                    (left, right) => apply(operator, left, right)?,
                };
                values.push(value);
            }
        }
    }
    if values.len() != 1 {
        return Err("constant expression did not produce one value".into());
    }
    let value = match values.pop().ok_or("missing constant expression value")? {
        Value::Anonymous(expression) => land_anonymous(
            program,
            machine,
            state,
            expression,
            destination,
            &selected_arms,
            &mut warnings,
        )?,
        value => value,
    };
    if let Value::Boolean(value) = value {
        return Ok((CanonicalConstValue::boolean(value), warnings));
    }
    let Value::Landed(carrier, value) = value else {
        return Err("constant expression has no integer landing".into());
    };
    let value = match value {
        IntegerValue::Signed(value) => value,
        IntegerValue::Unsigned(value) => i128::try_from(value)
            .map_err(|_| "constant value exceeds canonical integer encoding")?,
    };
    let identity = CanonicalConstIdentity::integer(carrier.name(), value);
    Ok((
        CanonicalConstValue::new(identity.type_name, identity.encoding, value.to_string()),
        warnings,
    ))
}

// Typed lowering and the ordinary partial operand validators do not establish
// exact scalar shapes after constant substitution. Visit all operands to retain
// static type and anonymous-rational landing obligations, even in skipped arms.
// Only anonymous trees are evaluated here: numeric_values.md requires their
// compile-time landing before a typed operation and supplies no runtime rational
// arithmetic. Landed operations are never executed by this pass, so a skipped
// divide, overflow or shift still follows the selective expression schedule.
fn validate_shapes(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
    destination: PrimitiveType,
) -> Result<(Vec<Diagnostic>, Vec<match_dispatch::MatchPlan>), String> {
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut shapes = Vec::new();
    let mut warnings = Vec::new();
    let mut matches = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return Err("invalid constant expression".into());
        }
        if !finish {
            if active.contains(&expression) {
                return Err("cyclic constant expression".into());
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    active.push(expression);
                    pending.push((expression, true));
                    for arm in program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .iter()
                        .rev()
                    {
                        pending.push((arm.value, false));
                        if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                            pending.push((pattern, false));
                        }
                    }
                    pending.push((dispatch.subject, false));
                }
                ExpressionNode::Boolean(_) => shapes.push(Shape::Boolean),
                ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                    let Value::Landed(carrier, _) = landed_literal(literal)? else {
                        unreachable!()
                    };
                    shapes.push(Shape::Integer(carrier));
                }
                ExpressionNode::Integer(_) => shapes.push(Shape::Anonymous(expression)),
                ExpressionNode::Float(literal) if literal.landing().is_none() => {
                    shapes.push(Shape::Anonymous(expression))
                }
                ExpressionNode::Binary(binary) => {
                    if !validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) {
                        return Err(
                            "constant expression has no selected builtin operator meaning".into(),
                        );
                    }
                    active.push(expression);
                    pending.push((expression, true));
                    pending.push((binary.right, false));
                    pending.push((binary.left, false));
                }
                _ => return Err("unsupported node in exact scalar constant expression".into()),
            }
            continue;
        }
        if active.pop() != Some(expression) {
            return Err("invalid constant expression traversal".into());
        }
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Match(_)
        ) {
            let plan = match_dispatch::validate_join(
                program,
                machine,
                state,
                expression,
                &mut shapes,
                &mut warnings,
            )?;
            shapes.push(plan.result);
            matches.push(plan);
            continue;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            unreachable!()
        };
        let operator = binary.operator;
        let right = shapes.pop().ok_or("missing right constant operand type")?;
        let left = shapes.pop().ok_or("missing left constant operand type")?;
        let shift = matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        );
        let (left, right) = match (left, right) {
            (Shape::Anonymous(_), Shape::Anonymous(_))
                if matches!(
                    operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) =>
            {
                match_dispatch::validate_anonymous_comparison(program, machine, state, expression)?;
                shapes.push(Shape::Boolean);
                continue;
            }
            (Shape::Anonymous(_), Shape::Anonymous(_)) => {
                if !matches!(
                    operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                ) {
                    return Err(
                        "anonymous constant operation requires a selected fixed carrier".into(),
                    );
                }
                shapes.push(Shape::Anonymous(expression));
                continue;
            }
            (Shape::Integer(carrier), Shape::Anonymous(right)) => {
                let destination = if shift {
                    PrimitiveType::U64
                } else {
                    primitive(carrier)?
                };
                let right_carrier = match_dispatch::validate_landing(
                    program,
                    machine,
                    state,
                    right,
                    destination,
                    &mut warnings,
                )?;
                (Shape::Integer(carrier), Shape::Integer(right_carrier))
            }
            (Shape::Anonymous(left), Shape::Integer(carrier)) => {
                if shift {
                    return Err("shift value requires a selected fixed carrier".into());
                }
                match_dispatch::validate_landing(
                    program,
                    machine,
                    state,
                    left,
                    primitive(carrier)?,
                    &mut warnings,
                )?;
                (Shape::Integer(carrier), Shape::Integer(carrier))
            }
            operands => operands,
        };
        let result = match (left, right) {
            (Shape::Boolean, Shape::Boolean)
                if matches!(
                    operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                ) =>
            {
                Shape::Boolean
            }
            (Shape::Integer(left), Shape::Integer(right)) => {
                if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                    return Err("Boolean logic requires Boolean operands".into());
                }
                if !shift && left != right {
                    return Err(
                        "constant operands have incompatible landed integer carriers".into(),
                    );
                }
                if matches!(
                    operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) {
                    Shape::Boolean
                } else {
                    Shape::Integer(left)
                }
            }
            _ => return Err("constant operator has incompatible operand types".into()),
        };
        shapes.push(result);
    }
    if shapes.len() != 1 {
        return Err("constant expression did not produce one type".into());
    }
    // The actual destination owns landing even when arithmetic surrounds a
    // dispatch. Checking only a bare Match would let an unselected fractional
    // or out-of-range result disappear before the selected value is published.
    if matches!(shapes[0], Shape::Anonymous(_)) {
        match_dispatch::validate_landing(
            program,
            machine,
            state,
            root,
            destination,
            &mut warnings,
        )?;
    }
    Ok((warnings, matches))
}

fn compare_anonymous(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
) -> Result<bool, String> {
    validation::evaluate_anonymous_numeric_comparison_with_selected_match_arms(
        program,
        expression,
        selected_arms,
        |operand| {
            validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                operand,
            )
        },
    )
    .ok_or_else(|| {
        "anonymous comparison requires defined exact numeric operands and selected builtin meaning"
            .into()
    })
}

fn land_anonymous(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, String> {
    let (literal, warning) =
        validation::land_anonymous_integer_expression_with_selected_match_arms(
            program,
            expression,
            destination,
            selected_arms,
            |expression| {
                validation::has_builtin_binary_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
            },
        )
        .ok_or(
            "anonymous constant expression cannot land exactly at the selected integer carrier",
        )?;
    if let Some(warning) = warning
        && !warnings.contains(&warning)
    {
        warnings.push(warning);
    }
    landed_literal(&literal)
}

fn landed_literal(literal: &IntegerLiteral) -> Result<Value, String> {
    let landing = literal.landing().ok_or("missing integer landing")?;
    if landing.domain != ArithmeticDomain::Exact {
        return Err("constant generic arithmetic requires Exact integer policy".into());
    }
    let carrier = integer_type(landing.landed_type)?;
    let value = if landing.landed_type.is_signed() {
        IntegerValue::Signed(i128::from(
            literal
                .value_i64()
                .ok_or("invalid signed constant literal")?,
        ))
    } else {
        IntegerValue::Unsigned(u128::from(
            literal
                .value_u64()
                .ok_or("invalid unsigned constant literal")?,
        ))
    };
    if !carrier.admits(value) {
        return Err("constant literal is outside its selected integer carrier".into());
    }
    Ok(Value::Landed(landing.landed_type, value))
}

fn apply(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, String> {
    if let (Value::Boolean(left), Value::Boolean(right)) = (left, right) {
        return match operator {
            BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
            BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
            _ => Err("unsupported builtin Boolean constant operator".into()),
        };
    }
    let (Value::Landed(left_carrier, left), Value::Landed(right_carrier, right)) = (left, right)
    else {
        return Err("integer operation requires landed operands".into());
    };
    let integer = integer_type(left_carrier)?;
    let shifts = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    if !shifts && left_carrier != right_carrier {
        return Err("constant operands have incompatible landed integer carriers".into());
    }
    if matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        let ordering = integer
            .compare(left, right)
            .ok_or("constant comparison operands are outside their selected integer carrier")?;
        let result = match operator {
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => ordering.is_ne(),
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => ordering.is_le(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => ordering.is_ge(),
            _ => unreachable!(),
        };
        return Ok(Value::Boolean(result));
    }
    let result = match operator {
        BinaryOperator::Add => integer.exact_add(left, right),
        BinaryOperator::Subtract => integer.exact_sub(left, right),
        BinaryOperator::Multiply => integer.exact_mul(left, right),
        BinaryOperator::Divide => integer.exact_div(left, right),
        BinaryOperator::Modulo => integer.exact_rem(left, right),
        BinaryOperator::BitwiseAnd => integer.bitwise_and(left, right),
        BinaryOperator::BitwiseOr => integer.bitwise_or(left, right),
        BinaryOperator::BitwiseXor => integer.bitwise_xor(left, right),
        BinaryOperator::ShiftLeft => integer.exact_shift_left(left, integer_type(right_carrier)?, right),
        BinaryOperator::ShiftRight => integer.exact_shift_right(left, integer_type(right_carrier)?, right),
        _ => return Err("unsupported builtin integer constant operator".into()),
    }.ok_or("Exact integer constant operation overflows, divides by zero, or has an invalid shift count")?;
    Ok(Value::Landed(left_carrier, result))
}

fn integer_type(carrier: LandedIntegerType) -> Result<IntegerType, String> {
    if carrier == LandedIntegerType::Addr {
        return Err("address constant evaluation requires target authority".into());
    }
    let sign = if carrier.is_signed() {
        IntegerSign::Signed
    } else {
        IntegerSign::Unsigned
    };
    IntegerType::new(sign, carrier.bit_width() as u16).map_err(|_| "invalid integer carrier".into())
}

fn primitive(carrier: LandedIntegerType) -> Result<PrimitiveType, String> {
    Ok(match carrier {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => {
            return Err("address constant evaluation requires target authority".into());
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{Value, apply, landed_literal};
    use numerics::{
        arithmetic::ArithmeticDomain,
        literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType},
    };
    use semantic_vocabulary::IntegerValue;
    use typed_trees::expression::BinaryOperator;

    #[test]
    fn exact_nodes_reject_overflow_before_later_cancellation() {
        let maximum = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(255));
        let one = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        assert!(apply(BinaryOperator::Add, maximum, one).is_err());
    }

    #[test]
    fn remainder_rejects_signed_minimum_divided_by_negative_one() {
        let minimum = Value::Landed(LandedIntegerType::I8, IntegerValue::Signed(-128));
        let negative_one = Value::Landed(LandedIntegerType::I8, IntegerValue::Signed(-1));
        assert!(apply(BinaryOperator::Modulo, minimum, negative_one).is_err());
    }

    #[test]
    fn shift_uses_value_width_and_independent_count_carrier() {
        let one = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        let invalid_count = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(8));
        assert!(apply(BinaryOperator::ShiftLeft, one, invalid_count).is_err());
        let count = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(7));
        assert!(matches!(
            apply(BinaryOperator::ShiftLeft, one, count),
            Ok(Value::Landed(
                LandedIntegerType::U8,
                IntegerValue::Unsigned(128)
            ))
        ));
    }

    #[test]
    fn arithmetic_never_relands_incompatible_carriers() {
        let left = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        let right = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(1));
        assert!(apply(BinaryOperator::Add, left, right).is_err());
    }

    #[test]
    fn unsigned_literal_preserves_values_above_signed_maximum() {
        let literal =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615")
                .expect("u64 maximum literal")
                .with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: ArithmeticDomain::Exact,
                });
        assert!(matches!(
            landed_literal(&literal),
            Ok(Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(value)))
                if value == u128::from(u64::MAX)
        ));
    }
}
