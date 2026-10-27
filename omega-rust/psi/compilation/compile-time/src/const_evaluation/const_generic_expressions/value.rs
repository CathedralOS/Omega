//! Policy-preserving scalar evaluation after declaration selection.
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
//! Unary operators are fixed, non-overloadable meanings. Their operands still
//! participate in the complete shape pass before execution: Boolean negation
//! needs bool, and complement needs an already selected integer width. The
//! shared integer operation preserves that width, including signed complement.
//! Ordinary total integer casts introduce a landing at the conversion itself;
//! they cannot widen earlier arithmetic retroactively. Landed operands require
//! total carrier inclusion, while anonymous operands use the existing rational
//! landing check. Partial conversions need evidence this evaluator does not own.
//! Landed shapes and values retain Exact/Wrapping/Saturating at every node;
//! anonymous values first land exactly even at a policy-qualified destination.
//! Call signatures supply the policy before carrier/payload snapshots cross
//! invocation boundaries. Their encoding alone cannot recover it. Existing
//! declaration/index entrypoints still require Exact, while endpoint owners
//! pass the selected destination policy explicitly. Trapping requires separate
//! invocation-failure evidence and remains outside this evaluator.
//!
//! Match uses this same work stack: retain the subject, test patterns in order,
//! then visit only the selected result. The Match owner first checks the complete
//! scalar graph, coverage, arm compatibility, and actual landing boundaries.
//! Selected result handles let the shared rational evaluator read surrounding
//! arithmetic without cloning the program or prematurely landing inner arms.
//! All-arm bounds that need additional proofs reject instead of enumerating
//! independent branch combinations or executing skipped landed expressions.
//!
//! Floating intermediates need meaning, not necessarily representation bits.
//! Calls and arithmetic retain a payloadless NaN until a comparison or other
//! semantic consumer produces its result. Declaration receipts carry that meaning;
//! a separate representation demand rejects NaN rather than publishing the
//! evaluator's arbitrary payload. Format, operator selection and argument
//! admission remain attached to every operation throughout evaluation.

use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use numerics::{
    arithmetic::ArithmeticDomain,
    float_semantics::{FloatFormat as SemanticFloatFormat, FloatMeaning, FloatSemantics},
    literals::{FloatFormat, IntegerLiteral, LandedIntegerType},
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
    machine::Machine,
    state::State,
    types::PrimitiveType,
};

mod context;
mod match_dispatch;
use context::EvaluationContext;

/// The invocation owner validates all arguments without executing landed
/// expressions, then evaluates arguments and the admitted machine only on demand.
pub(crate) trait ConstantCalls {
    /// Exact provider rows already validated before private execution preparation.
    /// They identify rewritten source occurrences; they grant no invocation.
    fn selected_provider_bodies(&self) -> &[crate::SelectedBuildTimeProviderBody] {
        &[]
    }

    /// A retained result type belongs to the same program passed to the scalar
    /// evaluator. Invalid means this caller carries no additional type shell.
    fn result_type(
        &self,
        _expression: ExpressionHandle,
    ) -> typed_trees::types::TypeReferenceHandle {
        typed_trees::types::TypeReferenceHandle::invalid()
    }

    fn validate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(PrimitiveType, Vec<Diagnostic>), String>;

    fn evaluate_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String>;

    /// Declaration values may retain floating bits without becoming index atoms.
    fn evaluate_scalar_call(
        &self,
        expression: ExpressionHandle,
    ) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
        self.evaluate_call(expression)
            .map(|(value, warnings)| (ScalarValue::Index(value), warnings))
    }
}

#[derive(Clone, Copy)]
struct ScalarDestination {
    primitive: PrimitiveType,
    policy: ArithmeticDomain,
}

#[derive(Clone, Copy)]
enum Shape {
    Anonymous(ExpressionHandle),
    Boolean,
    Float(FloatFormat),
    Integer(
        LandedIntegerType,
        typed_trees::types::TypeReferenceHandle,
        ArithmeticDomain,
    ),
}

#[derive(Clone, Copy)]
enum Value {
    Anonymous(ExpressionHandle),
    Boolean(bool),
    Float(FloatFormat, u64),
    UndeterminedNaN(FloatFormat),
    Landed(LandedIntegerType, IntegerValue, ArithmeticDomain),
}

/// Semantic intermediates, materialized declarations and generic indices have
/// different eligibility. A payloadless NaN can be consumed without encoding;
/// determined floating bits can be emitted but are never canonical index atoms.
pub(crate) enum ScalarValue {
    Index(CanonicalConstValue),
    Float {
        format: FloatFormat,
        bits: u64,
    },
    /// Semantic evaluation knows the class and format, but no payload bits.
    UndeterminedNaN(FloatFormat),
}

impl ScalarValue {
    /// Declaration receipts retain meaning even when no runtime representation exists.
    pub(crate) fn declaration_value(&self) -> language_core::constant_value::EvaluatedConstant {
        use language_core::constant_value::EvaluatedConstant;
        match self {
            Self::UndeterminedNaN(format) => EvaluatedConstant::UndeterminedNaN(*format),
            Self::Index(value) => EvaluatedConstant::Encoded(value.encoding.clone()),
            Self::Float {
                format: FloatFormat::F32,
                bits,
            } => EvaluatedConstant::Encoded(format!("float:f32:{bits:08x}")),
            Self::Float {
                format: FloatFormat::F64,
                bits,
            } => EvaluatedConstant::Encoded(format!("float:f64:{bits:016x}")),
        }
    }

    pub(crate) fn type_name(&self) -> &str {
        match self {
            Self::Index(value) => &value.type_name,
            Self::Float { format, .. } | Self::UndeterminedNaN(format) => format.name(),
        }
    }

    pub(crate) fn encoding(&self) -> Result<String, String> {
        Ok(match self {
            Self::Index(value) => value.encoding.clone(),
            Self::Float {
                format: FloatFormat::F32,
                bits,
            } => format!("float:f32:{bits:08x}"),
            Self::Float {
                format: FloatFormat::F64,
                bits,
            } => format!("float:f64:{bits:016x}"),
            Self::UndeterminedNaN(_) => {
                return Err("constant result is NaN without an exact raw-NaN realization".into());
            }
        })
    }

    pub(crate) fn into_index(self) -> Result<CanonicalConstValue, String> {
        match self {
            Self::Index(value) => Ok(value),
            Self::Float { .. } | Self::UndeterminedNaN(_) => {
                Err("floating declaration values are not canonical indices".into())
            }
        }
    }
}

/// Validate every operand and landing boundary without invoking landed calls.
pub(crate) fn validate(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    calls: Option<&dyn ConstantCalls>,
) -> Result<Vec<Diagnostic>, String> {
    match_dispatch::validate_graph(program, expression)?;
    Ok(validate_shapes(
        program,
        EvaluationContext::Machine(machine, state),
        expression,
        Some(ScalarDestination {
            primitive: destination,
            policy: ArithmeticDomain::Exact,
        }),
        calls,
    )?
    .warnings)
}

pub(crate) fn evaluate_scalar(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    calls: Option<&dyn ConstantCalls>,
) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
    evaluate_scalar_in(
        program,
        EvaluationContext::Machine(machine, state),
        expression,
        destination,
        ArithmeticDomain::Exact,
        calls,
    )
}

pub(crate) fn evaluate_scalar_with_policy(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    policy: ArithmeticDomain,
    calls: Option<&dyn ConstantCalls>,
) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
    evaluate_scalar_in(
        program,
        EvaluationContext::Machine(machine, state),
        expression,
        destination,
        policy,
        calls,
    )
}

fn evaluate_scalar_in(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    policy: ArithmeticDomain,
    calls: Option<&dyn ConstantCalls>,
) -> Result<(ScalarValue, Vec<Diagnostic>), String> {
    let (value, selected_arms, mut warnings) = evaluate_expression(
        program,
        context,
        expression,
        Some(ScalarDestination {
            primitive: destination,
            policy,
        }),
        calls,
    )?;
    let value = match value {
        Value::Anonymous(expression)
            if matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) =>
        {
            let exact = validation::evaluate_anonymous_numeric_expression(
                program,
                expression,
                &selected_arms,
                |operand| context.has_builtin(program, operand),
            )
            .ok_or("floating constant requires a defined exact anonymous value")?;
            // Round the completed rational directly at its declared format;
            // an intermediate f64 would double-round some f32 destinations.
            let value = match destination {
                PrimitiveType::F32 => ScalarValue::Float {
                    format: FloatFormat::F32,
                    bits: u64::from(exact.to_f32().to_bits()),
                },
                _ => ScalarValue::Float {
                    format: FloatFormat::F64,
                    bits: exact.to_f64().to_bits(),
                },
            };
            return Ok((value, warnings));
        }
        Value::Anonymous(expression) => land_anonymous(
            program,
            context,
            expression,
            destination,
            &selected_arms,
            &mut warnings,
        )?,
        value => value,
    };
    if let Value::Float(format, bits) = value {
        return Ok((ScalarValue::Float { format, bits }, warnings));
    }
    if let Value::UndeterminedNaN(format) = value {
        return Ok((ScalarValue::UndeterminedNaN(format), warnings));
    }
    if let Value::Boolean(value) = value {
        return Ok((
            ScalarValue::Index(CanonicalConstValue::boolean(value)),
            warnings,
        ));
    }
    let Value::Landed(carrier, value, _) = value else {
        return Err("constant expression has no integer landing".into());
    };
    let value = match value {
        IntegerValue::Signed(value) => value,
        IntegerValue::Unsigned(value) => i128::try_from(value)
            .map_err(|_| "constant value exceeds canonical integer encoding")?,
    };
    let identity = CanonicalConstIdentity::integer(carrier.name(), value);
    Ok((
        ScalarValue::Index(CanonicalConstValue::new(
            identity.type_name,
            identity.encoding,
            value.to_string(),
        )),
        warnings,
    ))
}

/// Reuse the checked closed-value projection join, including exact carrier,
/// field identities and all sibling literal checks. A projected scalar is
/// already landed at its declared field; offering its literal as anonymous
/// syntax would lose that carrier when numeric validation revisits the root.
/// This removes the separate adjacent-literal/name-based projection route.
/// The shared join currently admits Exact arithmetic only; its policy fence
/// remains authoritative here as it is for ordinary projection consumers.
fn member_field_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Result<(ExpressionHandle, PrimitiveType), String> {
    validation::closed_record_scalar_projection(program, expression).ok_or_else(|| {
        let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
            return "constant projection has no member selection".to_owned();
        };
        format!(
            "constant member read cannot select closed record field `{}` in its exact carrier",
            member.member
        )
    })
}

fn evaluate_expression(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    expression: ExpressionHandle,
    destination: Option<ScalarDestination>,
    calls: Option<&dyn ConstantCalls>,
) -> Result<
    (
        Value,
        Vec<(ExpressionHandle, ExpressionHandle)>,
        Vec<Diagnostic>,
    ),
    String,
> {
    if !program.expression_table.expression_is_valid(expression) {
        return Err("invalid constant expression".to_owned());
    }
    enum Step {
        Enter(ExpressionHandle),
        Cast(ExpressionHandle, LandedIntegerType),
        Projection(PrimitiveType),
        Unary(ExpressionHandle, UnaryOperator),
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
    let ValidatedShapes {
        mut warnings,
        matches,
        call_shapes,
        ..
    } = validate_shapes(program, context, expression, destination, calls)?;
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
                    ExpressionNode::Call(_) => {
                        let shape = call_shapes
                            .iter()
                            .find(|(source, _)| *source == expression)
                            .map(|(_, shape)| *shape)
                            .ok_or("constant call has no validated scalar carrier")?;
                        let (value, call_warnings) = calls
                            .ok_or("machine calls require constant invocation admission")?
                            .evaluate_scalar_call(expression)?;
                        values.push(match value {
                            ScalarValue::Index(value) => call_value(&value, shape)?,
                            ScalarValue::Float { format, bits }
                                if matches!(shape, Shape::Float(expected) if expected == format) =>
                            {
                                Value::Float(format, bits)
                            }
                            ScalarValue::UndeterminedNaN(format)
                                if matches!(shape, Shape::Float(expected) if expected == format) =>
                            {
                                Value::UndeterminedNaN(format)
                            }
                            _ => return Err("constant call result differs from its validated scalar carrier".into()),
                        });
                        for warning in call_warnings {
                            if !warnings.contains(&warning) {
                                warnings.push(warning);
                            }
                        }
                    }
                    ExpressionNode::Match(dispatch) => {
                        active.push(expression);
                        pending.push(Step::MatchSubject(expression));
                        pending.push(Step::Enter(dispatch.subject));
                    }
                    ExpressionNode::Boolean(value) => values.push(Value::Boolean(*value)),
                    ExpressionNode::UndeterminedNaN(format) => {
                        values.push(Value::UndeterminedNaN(*format))
                    }
                    ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                        values.push(landed_literal(literal)?);
                    }
                    ExpressionNode::Integer(_) => values.push(Value::Anonymous(expression)),
                    ExpressionNode::Float(literal) if literal.landing().is_none() => {
                        values.push(Value::Anonymous(expression));
                    }
                    ExpressionNode::Float(literal) => {
                        let format = literal
                            .landing()
                            .ok_or("floating literal lost its format")?;
                        values.push(Value::Float(
                            format,
                            match format {
                                FloatFormat::F32 => u64::from(literal.f32_bits()),
                                FloatFormat::F64 => literal.landed_f64().to_bits(),
                            },
                        ));
                    }
                    ExpressionNode::Unary(unary) => {
                        active.push(expression);
                        pending.push(Step::Unary(expression, unary.operator));
                        pending.push(Step::Enter(unary.operand));
                    }
                    ExpressionNode::Cast(cast) => {
                        active.push(expression);
                        pending.push(Step::Cast(expression, cast_integer_carrier(program, cast)?));
                        pending.push(Step::Enter(cast.value));
                    }
                    ExpressionNode::Binary(binary) => {
                        if !context.has_builtin(program, expression) {
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
                    ExpressionNode::Member(_) => {
                        let (field, primitive) = member_field_value(program, expression)?;
                        pending.push(Step::Projection(primitive));
                        pending.push(Step::Enter(field));
                    }
                    _ => return Err("unsupported node in exact integer constant expression".into()),
                }
            }
            Step::Projection(primitive) => {
                let value = values
                    .pop()
                    .ok_or("constant projection lost its scalar value")?;
                let value = match value {
                    Value::Anonymous(field)
                        if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) =>
                    {
                        let format = match primitive {
                            PrimitiveType::F32 => FloatFormat::F32,
                            _ => FloatFormat::F64,
                        };
                        Value::Float(
                            format,
                            land_anonymous_float(program, context, field, format, &selected_arms)?,
                        )
                    }
                    Value::Anonymous(field) => land_anonymous(
                        program,
                        context,
                        field,
                        primitive,
                        &selected_arms,
                        &mut warnings,
                    )?,
                    value => value,
                };
                let matches = match (value, scalar_shape(primitive)?) {
                    (Value::Boolean(_), Shape::Boolean) => true,
                    (
                        Value::Float(actual, _) | Value::UndeterminedNaN(actual),
                        Shape::Float(expected),
                    ) => actual == expected,
                    (
                        Value::Landed(actual, _, policy),
                        Shape::Integer(expected, _, expected_policy),
                    ) => actual == expected && policy == expected_policy,
                    _ => false,
                };
                if !matches {
                    return Err(
                        "constant projected value differs from its selected field carrier".into(),
                    );
                }
                values.push(value);
            }
            Step::MatchSubject(expression) => {
                let plan = matches
                    .iter()
                    .find(|plan| plan.expression == expression)
                    .ok_or("missing constant Match plan")?;
                let subject = values.pop().ok_or("missing constant Match subject")?;
                let subject = match_dispatch::coerce(
                    program,
                    context,
                    subject,
                    plan.subject,
                    &selected_arms,
                    &mut warnings,
                )?;
                let subject = match_dispatch::MatchSubject::new(
                    program,
                    subject,
                    &selected_arms,
                    |operand| context.has_builtin(program, operand),
                )?;
                pending.push(Step::MatchNext(expression, 0, subject));
            }
            Step::MatchNext(expression, ordinal, subject) => {
                let Some(dispatch) = program.expression_table.match_node(expression) else {
                    return Err("constant Match lost dispatch".into());
                };
                let arm = program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .get(ordinal)
                    .ok_or("constant Match has no selected arm")?;
                match arm.pattern {
                    typed_trees::expression::MatchPattern::Destructure(_) => {
                        return Err(
                            "structural constant Match requires saved-subject value interpretation"
                                .into(),
                        );
                    }
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
                    context,
                    pattern,
                    plan.subject,
                    &selected_arms,
                    &mut warnings,
                )?;
                let matched = subject.matches(program, pattern, &selected_arms, |operand| {
                    context.has_builtin(program, operand)
                })?;
                if matched {
                    let Some(dispatch) = program.expression_table.match_node(expression) else {
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
                    context,
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
            Step::Cast(expression, target) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                let operand = values.pop().ok_or("missing constant cast operand")?;
                let result = match operand {
                    Value::Anonymous(operand) => land_anonymous(
                        program,
                        context,
                        operand,
                        primitive(target)?,
                        &selected_arms,
                        &mut warnings,
                    )?,
                    Value::Landed(source, value, _) => Value::Landed(
                        target,
                        integer_type(source)?
                            .exact_cast_value_to(integer_type(target)?, value)
                            .ok_or("constant cast value does not fit its target carrier")?,
                        ArithmeticDomain::Exact,
                    ),
                    _ => return Err("constant cast requires an integer operand".into()),
                };
                let Some(cast) = program.expression_table.cast_node(expression) else {
                    return Err("constant cast lost its expression".into());
                };
                values.push(retag_landed(result, cast.domain));
            }
            Step::Unary(expression, operator) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                let operand = values.pop().ok_or("missing unary constant operand")?;
                let result = match (operator, operand) {
                    (UnaryOperator::LogicalNot, Value::Boolean(value)) => Value::Boolean(!value),
                    (UnaryOperator::BitwiseNot, Value::Landed(carrier, value, policy)) => {
                        let value = integer_type(carrier)?
                            .bitwise_not(value)
                            .ok_or("integer complement operand does not inhabit its carrier")?;
                        Value::Landed(carrier, value, policy)
                    }
                    _ => return Err("unary constant operator has incompatible operand type".into()),
                };
                values.push(result);
            }
            Step::Binary(expression, operator) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                let right = values.pop().ok_or("missing right constant operand")?;
                let left = values.pop().ok_or("missing left constant operand")?;
                let value = match (left, right) {
                    (Value::Anonymous(_), Value::Anonymous(_)) if comparison_operator(operator) => {
                        Value::Boolean(compare_anonymous(
                            program,
                            context,
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
                    (Value::Landed(carrier, left, policy), Value::Anonymous(right)) => {
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
                            context,
                            right,
                            right_destination,
                            &selected_arms,
                            &mut warnings,
                        )?;
                        apply(
                            operator,
                            Value::Landed(carrier, left, policy),
                            retag_landed(
                                right,
                                if matches!(
                                    operator,
                                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                                ) {
                                    ArithmeticDomain::Exact
                                } else {
                                    policy
                                },
                            ),
                        )?
                    }
                    (Value::Anonymous(left), Value::Landed(carrier, right, policy)) => {
                        if matches!(
                            operator,
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                        ) {
                            return Err("shift value requires a selected fixed carrier".into());
                        }
                        let left = land_anonymous(
                            program,
                            context,
                            left,
                            primitive(carrier)?,
                            &selected_arms,
                            &mut warnings,
                        )?;
                        apply(
                            operator,
                            retag_landed(left, policy),
                            Value::Landed(carrier, right, policy),
                        )?
                    }
                    (
                        left @ (Value::Float(format, _) | Value::UndeterminedNaN(format)),
                        Value::Anonymous(right),
                    ) => {
                        let right =
                            land_anonymous_float(program, context, right, format, &selected_arms)?;
                        apply(operator, left, Value::Float(format, right))?
                    }
                    (
                        Value::Anonymous(left),
                        right @ (Value::Float(format, _) | Value::UndeterminedNaN(format)),
                    ) => {
                        let left =
                            land_anonymous_float(program, context, left, format, &selected_arms)?;
                        apply(operator, Value::Float(format, left), right)?
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
    Ok((
        values.pop().ok_or("missing constant expression value")?,
        selected_arms,
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
struct ValidatedShapes {
    warnings: Vec<Diagnostic>,
    matches: Vec<match_dispatch::MatchPlan>,
    call_shapes: Vec<(ExpressionHandle, Shape)>,
}

fn validate_shapes(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    root: ExpressionHandle,
    destination: Option<ScalarDestination>,
    calls: Option<&dyn ConstantCalls>,
) -> Result<ValidatedShapes, String> {
    let destination_policy = destination.map(|destination| destination.policy);
    if let Some(policy) = destination_policy {
        require_supported_policy(policy)?;
    }
    let destination = destination.map(|destination| destination.primitive);
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut shapes = Vec::new();
    let mut warnings = Vec::new();
    let mut matches = Vec::new();
    let mut call_shapes = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return Err("invalid constant expression".into());
        }
        if !finish {
            if active.contains(&expression) {
                return Err("cyclic constant expression".into());
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(_) => {
                    let (primitive, call_warnings) = calls
                        .ok_or("machine calls require constant invocation admission")?
                        .validate_call(expression)?;
                    let mut shape = scalar_shape(primitive)?;
                    if let Shape::Integer(_, reference, policy) = &mut shape {
                        *reference = calls.expect("validated call owner").result_type(expression);
                        *policy = reference_policy(program, *reference)?;
                    }
                    shapes.push(shape);
                    call_shapes.push((expression, shape));
                    for warning in call_warnings {
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                    }
                }
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
                ExpressionNode::UndeterminedNaN(format) => shapes.push(Shape::Float(*format)),
                ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                    let Value::Landed(carrier, _, policy) = landed_literal(literal)? else {
                        unreachable!()
                    };
                    shapes.push(Shape::Integer(
                        carrier,
                        typed_trees::types::TypeReferenceHandle::invalid(),
                        policy,
                    ));
                }
                ExpressionNode::Integer(_) => shapes.push(Shape::Anonymous(expression)),
                ExpressionNode::Float(literal) if literal.landing().is_none() => {
                    shapes.push(Shape::Anonymous(expression))
                }
                ExpressionNode::Float(literal) => {
                    shapes.push(Shape::Float(
                        literal
                            .landing()
                            .ok_or("floating literal lost its format")?,
                    ));
                }
                ExpressionNode::Unary(unary) => {
                    active.push(expression);
                    pending.push((expression, true));
                    pending.push((unary.operand, false));
                }
                ExpressionNode::Cast(cast) => {
                    active.push(expression);
                    pending.push((expression, true));
                    pending.push((cast.value, false));
                }
                ExpressionNode::Binary(binary) => {
                    if !context.has_builtin(program, expression) {
                        return Err(
                            "constant expression has no selected builtin operator meaning".into(),
                        );
                    }
                    active.push(expression);
                    pending.push((expression, true));
                    pending.push((binary.right, false));
                    pending.push((binary.left, false));
                }
                ExpressionNode::Member(_) => {
                    let (_, primitive) = member_field_value(program, expression)?;
                    shapes.push(scalar_shape(primitive)?);
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
                context,
                expression,
                &mut shapes,
                &mut warnings,
            )?;
            shapes.push(plan.result);
            matches.push(plan);
            continue;
        }
        if let Some(unary) = program.expression_table.unary_node(expression) {
            let operand = shapes.pop().ok_or("missing unary constant operand type")?;
            let result = match (unary.operator, operand) {
                (UnaryOperator::LogicalNot, Shape::Boolean) => Shape::Boolean,
                (UnaryOperator::BitwiseNot, Shape::Integer(_, _, _)) => {
                    context.arithmetic_result(program, operand)?
                }
                _ => return Err("unary constant operator has incompatible operand type".into()),
            };
            shapes.push(result);
            continue;
        }
        if let Some(cast) = program.expression_table.cast_node(expression) {
            let target = cast_integer_carrier(program, cast)?;
            let source = shapes.pop().ok_or("missing constant cast operand type")?;
            // Conversion owns a new landing; it never retags arithmetic below
            // it. Anonymous operands owe their full all-arm landing here. An
            // already-landed operand must prove total carrier inclusion without
            // executing a skipped call to discover a convenient result. Partial
            // conversions still need their independent representability evidence.
            match source {
                Shape::Anonymous(operand) => {
                    match_dispatch::validate_landing(
                        program, context, operand, primitive(target)?, &mut warnings,
                    )?;
                }
                Shape::Integer(source, _, _) if source == target
                    || integer_type(source)?.can_widen_to(integer_type(target)?) => {}
                _ => return Err("constant cast needs a total integer conversion or checked partial-conversion evidence".into()),
            }
            shapes.push(Shape::Integer(target, cast.target_type, cast.domain));
            continue;
        }
        let Some(binary) = program.expression_table.binary_node(expression) else {
            unreachable!()
        };
        let operator = binary.operator;
        let right = shapes.pop().ok_or("missing right constant operand type")?;
        let left = shapes.pop().ok_or("missing left constant operand type")?;
        context.require_operator(program, expression, left, right)?;
        let shift = matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        );
        let (left, right) = match (left, right) {
            (Shape::Anonymous(_), Shape::Anonymous(_)) if comparison_operator(operator) => {
                match_dispatch::validate_anonymous_comparison(program, context, expression)?;
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
            (Shape::Integer(carrier, reference, policy), Shape::Anonymous(right)) => {
                let destination = if shift {
                    PrimitiveType::U64
                } else {
                    primitive(carrier)?
                };
                let right_carrier = match_dispatch::validate_landing(
                    program,
                    context,
                    right,
                    destination,
                    &mut warnings,
                )?;
                (
                    Shape::Integer(carrier, reference, policy),
                    Shape::Integer(
                        right_carrier,
                        typed_trees::types::TypeReferenceHandle::invalid(),
                        if shift {
                            ArithmeticDomain::Exact
                        } else {
                            policy
                        },
                    ),
                )
            }
            (Shape::Anonymous(left), Shape::Integer(carrier, reference, policy)) => {
                if shift {
                    return Err("shift value requires a selected fixed carrier".into());
                }
                match_dispatch::validate_landing(
                    program,
                    context,
                    left,
                    primitive(carrier)?,
                    &mut warnings,
                )?;
                (
                    Shape::Integer(
                        carrier,
                        typed_trees::types::TypeReferenceHandle::invalid(),
                        policy,
                    ),
                    Shape::Integer(carrier, reference, policy),
                )
            }
            // An already-landed float operand lands its anonymous peer at its
            // own format for this operation. The peer owes only a defined
            // exact value; the format accepts every rational magnitude.
            (Shape::Float(format), Shape::Anonymous(right)) => {
                match_dispatch::validate_anonymous_fragments(program, context, right)?;
                (Shape::Float(format), Shape::Float(format))
            }
            (Shape::Anonymous(left), Shape::Float(format)) => {
                match_dispatch::validate_anonymous_fragments(program, context, left)?;
                (Shape::Float(format), Shape::Float(format))
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
            (
                Shape::Integer(left, left_reference, left_policy),
                Shape::Integer(right, right_reference, right_policy),
            ) => {
                if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                    return Err("Boolean logic requires Boolean operands".into());
                }
                if !shift
                    && !comparison_operator(operator)
                    && (left != right || left_policy != right_policy)
                {
                    return Err(
                        "constant operands have incompatible landed integer carriers".into(),
                    );
                }
                if comparison_operator(operator) {
                    Shape::Boolean
                } else {
                    context.arithmetic_result(
                        program,
                        Shape::Integer(right, right_reference, right_policy),
                    )?;
                    context.arithmetic_result(
                        program,
                        Shape::Integer(left, left_reference, left_policy),
                    )?
                }
            }
            (Shape::Float(left), Shape::Float(right)) => {
                if left != right {
                    return Err("constant float operands have incompatible landed formats".into());
                }
                if comparison_operator(operator) {
                    Shape::Boolean
                } else if matches!(
                    operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                ) {
                    Shape::Float(left)
                } else {
                    return Err("constant operator has incompatible operand types".into());
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
    match shapes[0] {
        Shape::Anonymous(_)
            if matches!(
                destination,
                None | Some(PrimitiveType::F32 | PrimitiveType::F64)
            ) =>
        {
            match_dispatch::validate_anonymous_fragments(program, context, root)?;
        }
        Shape::Anonymous(_) => {
            match_dispatch::validate_landing(
                program,
                context,
                root,
                destination.expect("anonymous integer destination"),
                &mut warnings,
            )?;
        }
        Shape::Boolean if destination.is_none() => {
            return Err("range endpoint position requires an exact builtin integer carrier".into());
        }
        Shape::Boolean if calls.is_none() || destination == Some(PrimitiveType::Bool) => {}
        Shape::Float(format) if destination == Some(float_primitive(format)) => {}
        Shape::Integer(carrier, _, _)
            if calls.is_none()
                || destination
                    .is_none_or(|destination| primitive(carrier).ok() == Some(destination)) => {}
        _ => return Err("constant expression differs from its destination carrier".into()),
    }
    if let (Some(expected), Shape::Integer(_, _, actual)) = (destination_policy, shapes[0])
        && expected != actual
    {
        return Err("constant expression cannot implicitly change its arithmetic policy".into());
    }
    Ok(ValidatedShapes {
        warnings,
        matches,
        call_shapes,
    })
}

fn scalar_shape(primitive: PrimitiveType) -> Result<Shape, String> {
    Ok(Shape::Integer(
        match primitive {
            PrimitiveType::Bool => return Ok(Shape::Boolean),
            PrimitiveType::F32 => return Ok(Shape::Float(FloatFormat::F32)),
            PrimitiveType::F64 => return Ok(Shape::Float(FloatFormat::F64)),
            primitive => primitive
                .landed_landing_fixed()
                .ok_or("constant call requires an exact integer or Boolean carrier")?,
        },
        typed_trees::types::TypeReferenceHandle::invalid(),
        ArithmeticDomain::Exact,
    ))
}

fn cast_integer_carrier(
    program: &TypedTrees,
    cast: &typed_trees::expression::TableCastExpression,
) -> Result<LandedIntegerType, String> {
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return Err("constant cast requires an ordinary exact integer coercion".into());
    }
    let target = super::exact_probe_destination(program, cast.target_type)
        .ok_or("constant cast target requires an unqualified fixed integer carrier")?;
    let Shape::Integer(carrier, _, ArithmeticDomain::Exact) = scalar_shape(target)? else {
        return Err("constant cast target requires a fixed integer carrier".into());
    };
    Ok(carrier)
}

fn float_primitive(format: FloatFormat) -> PrimitiveType {
    match format {
        FloatFormat::F32 => PrimitiveType::F32,
        FloatFormat::F64 => PrimitiveType::F64,
    }
}

fn call_value(value: &CanonicalConstValue, shape: Shape) -> Result<Value, String> {
    match (shape, value.decode_encoding()) {
        (Shape::Boolean, Some(DecodedCanonicalConstValue::Boolean(result)))
            if value.identity() == CanonicalConstValue::boolean(result).identity() =>
        {
            Ok(Value::Boolean(result))
        }
        (
            Shape::Integer(carrier, _, policy),
            Some(DecodedCanonicalConstValue::Integer { value: result, .. }),
        ) if value.identity() == CanonicalConstIdentity::integer(carrier.name(), result) => {
            let result = if carrier.is_signed() {
                IntegerValue::Signed(result)
            } else {
                IntegerValue::Unsigned(
                    u128::try_from(result).map_err(|_| "negative unsigned constant call result")?,
                )
            };
            if !integer_type(carrier)?.admits(result) {
                return Err("constant call result exceeds its declared carrier".into());
            }
            Ok(Value::Landed(carrier, result, policy))
        }
        _ => Err("constant call result differs from its validated scalar carrier".into()),
    }
}

fn compare_anonymous(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    expression: ExpressionHandle,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
) -> Result<bool, String> {
    validation::evaluate_anonymous_numeric_comparison(
        program,
        expression,
        selected_arms,
        |operand| context.has_builtin(program, operand),
    )
    .ok_or_else(|| {
        "anonymous comparison requires defined exact numeric operands and selected builtin meaning"
            .into()
    })
}

fn land_anonymous(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, String> {
    let (literal, warning) = validation::land_anonymous_integer_expression(
        program,
        expression,
        destination,
        selected_arms,
        |expression| context.has_builtin(program, expression),
    )
    .ok_or_else(|| {
        if let Some(value) = validation::evaluate_anonymous_numeric_expression(
            program,
            expression,
            selected_arms,
            |operand| context.has_builtin(program, operand),
        ) && value.to_integer_exact().is_none()
        {
            return format!(
                "exact anonymous value `{value}` cannot land in an integer const argument"
            );
        }
        "anonymous constant expression cannot land exactly at the selected integer carrier"
            .to_owned()
    })?;
    if let Some(warning) = warning
        && !warnings.contains(&warning)
    {
        warnings.push(warning);
    }
    landed_literal(&literal)
}

fn landed_literal(literal: &IntegerLiteral) -> Result<Value, String> {
    let landing = literal.landing().ok_or("missing integer landing")?;
    require_supported_policy(landing.domain)?;
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
    Ok(Value::Landed(landing.landed_type, value, landing.domain))
}

/// An anonymous operand mixing into a landed float operation lands at that
/// operation's format first, matching the runtime landing boundary. Landing is
/// the single rounding of the completed rational at the selected format.
fn land_anonymous_float(
    program: &TypedTrees,
    context: EvaluationContext<'_>,
    expression: ExpressionHandle,
    format: FloatFormat,
    selected_arms: &[(ExpressionHandle, ExpressionHandle)],
) -> Result<u64, String> {
    let exact = validation::evaluate_anonymous_numeric_expression(
        program,
        expression,
        selected_arms,
        |operand| context.has_builtin(program, operand),
    )
    .ok_or("floating operand requires a defined exact anonymous value")?;
    Ok(match format {
        FloatFormat::F32 => u64::from(exact.to_f32().to_bits()),
        FloatFormat::F64 => exact.to_f64().to_bits(),
    })
}

/// Landed operands decode to exact meanings and re-encode through the shared
/// `FloatSemantics` provider so constant and runtime arithmetic keep one
/// definition. A NaN result retains its meaning without inventing payload bits;
/// only the later declaration encoding/materialization boundary rejects it.
fn float_apply(
    operator: BinaryOperator,
    left_format: FloatFormat,
    left: FloatMeaning,
    right_format: FloatFormat,
    right: FloatMeaning,
) -> Result<Value, String> {
    if left_format != right_format {
        return Err("constant float operands have incompatible landed formats".into());
    }
    let semantic_format = match left_format {
        FloatFormat::F32 => SemanticFloatFormat::BINARY32,
        FloatFormat::F64 => SemanticFloatFormat::BINARY64,
    };
    match operator {
        BinaryOperator::Equal => {
            return Ok(Value::Boolean(FloatSemantics::equal(&left, &right)));
        }
        BinaryOperator::NotEqual => {
            return Ok(Value::Boolean(FloatSemantics::not_equal(&left, &right)));
        }
        BinaryOperator::Less => {
            return Ok(Value::Boolean(FloatSemantics::less(&left, &right)));
        }
        BinaryOperator::LessOrEqual => {
            return Ok(Value::Boolean(FloatSemantics::less_or_equal(&left, &right)));
        }
        BinaryOperator::Greater => {
            return Ok(Value::Boolean(FloatSemantics::greater(&left, &right)));
        }
        BinaryOperator::GreaterOrEqual => {
            return Ok(Value::Boolean(FloatSemantics::greater_or_equal(
                &left, &right,
            )));
        }
        _ => {}
    }
    let meaning = match operator {
        BinaryOperator::Add => FloatSemantics::add(semantic_format, &left, &right),
        BinaryOperator::Subtract => FloatSemantics::subtract(semantic_format, &left, &right),
        BinaryOperator::Multiply => FloatSemantics::multiply(semantic_format, &left, &right),
        BinaryOperator::Divide => FloatSemantics::divide(semantic_format, &left, &right),
        _ => return Err("unsupported builtin floating constant operator".into()),
    };
    if meaning.is_nan() {
        return Ok(Value::UndeterminedNaN(left_format));
    }
    Ok(Value::Float(left_format, float_bits(left_format, &meaning)))
}

fn float_meaning(format: FloatFormat, bits: u64) -> FloatMeaning {
    match format {
        FloatFormat::F32 => FloatMeaning::from_f32(f32::from_bits(bits as u32)),
        FloatFormat::F64 => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

fn floating_meaning(value: Value) -> Option<(FloatFormat, FloatMeaning)> {
    match value {
        Value::Float(format, bits) => Some((format, float_meaning(format, bits))),
        Value::UndeterminedNaN(format) => Some((format, FloatMeaning::NaN)),
        _ => None,
    }
}

fn float_bits(format: FloatFormat, meaning: &FloatMeaning) -> u64 {
    match format {
        FloatFormat::F32 => u64::from(meaning.to_f32().to_bits()),
        FloatFormat::F64 => meaning.to_f64().to_bits(),
    }
}

fn comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    )
}

fn apply(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, String> {
    if let (Value::Boolean(left), Value::Boolean(right)) = (left, right) {
        return match operator {
            BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
            BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
            _ => Err("unsupported builtin Boolean constant operator".into()),
        };
    }
    if let (Some((left_format, left)), Some((right_format, right))) =
        (floating_meaning(left), floating_meaning(right))
    {
        return float_apply(operator, left_format, left, right_format, right);
    }
    let (
        Value::Landed(left_carrier, left, policy),
        Value::Landed(right_carrier, right, right_policy),
    ) = (left, right)
    else {
        return Err("integer operation requires landed operands".into());
    };
    let integer = integer_type(left_carrier)?;
    let shifts = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    if !shifts
        && !comparison_operator(operator)
        && (left_carrier != right_carrier || policy != right_policy)
    {
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
            .compare_with(left, integer_type(right_carrier)?, right)
            .ok_or("constant comparison operands are outside their selected integer carriers")?;
        let result = match operator {
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => ordering.is_ne(),
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => ordering.is_le(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => ordering.is_ge(),
            _ => return Err("constant comparison lost its operator".into()),
        };
        return Ok(Value::Boolean(result));
    }
    require_supported_policy(policy)?;
    use ArithmeticDomain::{Exact, Saturating, Wrapping};
    let result = match (operator, policy) {
        (BinaryOperator::Add, Exact) => integer.exact_add(left, right),
        (BinaryOperator::Add, Wrapping) => integer.wrapping_add(left, right),
        (BinaryOperator::Add, Saturating) => integer.saturating_add(left, right),
        (BinaryOperator::Subtract, Exact) => integer.exact_sub(left, right),
        (BinaryOperator::Subtract, Wrapping) => integer.wrapping_sub(left, right),
        (BinaryOperator::Subtract, Saturating) => integer.saturating_sub(left, right),
        (BinaryOperator::Multiply, Exact) => integer.exact_mul(left, right),
        (BinaryOperator::Multiply, Wrapping) => integer.wrapping_mul(left, right),
        (BinaryOperator::Multiply, Saturating) => integer.saturating_mul(left, right),
        (BinaryOperator::Divide, Exact) => integer.exact_div(left, right),
        (BinaryOperator::Divide, Wrapping) => integer.wrapping_div(left, right),
        (BinaryOperator::Divide, Saturating) => integer.saturating_div(left, right),
        (BinaryOperator::Modulo, Exact) => integer.exact_rem(left, right),
        (BinaryOperator::Modulo, Wrapping) => integer.wrapping_rem(left, right),
        (BinaryOperator::Modulo, Saturating) => integer.saturating_rem(left, right),
        (BinaryOperator::BitwiseAnd, _) => integer.bitwise_and(left, right),
        (BinaryOperator::BitwiseOr, _) => integer.bitwise_or(left, right),
        (BinaryOperator::BitwiseXor, _) => integer.bitwise_xor(left, right),
        (BinaryOperator::ShiftLeft, Exact) => {
            integer.exact_shift_left(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftLeft, Wrapping) => {
            integer.wrapping_shift_left(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftRight, Exact | Saturating) => {
            integer.exact_shift_right(left, integer_type(right_carrier)?, right)
        }
        (BinaryOperator::ShiftRight, Wrapping) => {
            integer.wrapping_shift_right(left, integer_type(right_carrier)?, right)
        }
        _ => return Err("unsupported builtin integer constant operation policy".into()),
    }
    .ok_or(format!(
        "{} integer constant operation overflows, divides by zero, or has an invalid shift count",
        policy.name()
    ))?;
    Ok(Value::Landed(left_carrier, result, policy))
}

fn require_supported_policy(policy: ArithmeticDomain) -> Result<(), String> {
    if matches!(
        policy,
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
    ) {
        Ok(())
    } else {
        Err("constant scalar evaluation requires a nontrapping integer policy".into())
    }
}

/// Region/domain identity stays in the retained type; this reads only its one
/// arithmetic policy, rejecting ambiguity before a call snapshot can erase it.
fn reference_policy(
    program: &TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Result<ArithmeticDomain, String> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    if !reference.is_valid() {
        return Ok(ArithmeticDomain::Exact);
    }
    let mut seen = Vec::new();
    let mut policy = None;
    loop {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || seen.contains(&reference)
        {
            return Err("constant scalar type is invalid or cyclic".into());
        }
        seen.push(reference);
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        else {
            break;
        };
        let constraints = program
            .type_reference_table
            .constraint_span(*constraints)
            .ok_or("constant scalar type has invalid constraints")?;
        for constraint in constraints {
            if let TypeConstraintNode::ArithmeticDomain(selected) = constraint {
                require_supported_policy(*selected)?;
                if policy.replace(*selected).is_some() {
                    return Err("constant scalar type has multiple arithmetic policies".into());
                }
            }
        }
        reference = *base_type;
    }
    Ok(policy.unwrap_or(ArithmeticDomain::Exact))
}

/// Only anonymous landing or an authored cast calls this operation. A landed
/// destination check must compare policies instead of using it to retag values.
fn retag_landed(value: Value, policy: ArithmeticDomain) -> Value {
    match value {
        Value::Landed(carrier, value, _) => Value::Landed(carrier, value, policy),
        value => value,
    }
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
    match carrier {
        LandedIntegerType::Addr => {
            Err("address constant evaluation requires target authority".into())
        }
        carrier => Ok(PrimitiveType::from_landed_integer(carrier)),
    }
}

/// Anonymous values land exactly; already-landed values retain their policy.
pub(crate) fn validate_closed_scalar_with_policy(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    policy: ArithmeticDomain,
    calls: &dyn ConstantCalls,
) -> Result<Vec<Diagnostic>, String> {
    match_dispatch::validate_graph(program, expression)?;
    Ok(validate_shapes(
        program,
        EvaluationContext::Closed,
        expression,
        Some(ScalarDestination {
            primitive: destination,
            policy,
        }),
        Some(calls),
    )?
    .warnings)
}

/// The caller retains its checked destination type while the canonical value
/// crosses the invocation boundary as carrier and payload.
pub(crate) fn evaluate_closed_scalar_with_policy(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    policy: ArithmeticDomain,
    calls: &dyn ConstantCalls,
) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
    let (value, warnings) = evaluate_scalar_in(
        program,
        EvaluationContext::Closed,
        expression,
        destination,
        policy,
        Some(calls),
    )?;
    Ok((value.into_index()?, warnings))
}

pub(crate) fn evaluate_integer_endpoint(
    program: &TypedTrees,
    expression: ExpressionHandle,
    calls: &dyn ConstantCalls,
) -> Result<(IntegerLiteral, Vec<Diagnostic>), String> {
    use numerics::literals::{IntegerLanding, IntegerRadix};
    let (value, selected_arms, warnings) = evaluate_expression(
        program,
        EvaluationContext::Closed,
        expression,
        None,
        Some(calls),
    )?;
    let (integer, landing) = match value {
        Value::Anonymous(expression) => {
            let exact = validation::evaluate_anonymous_numeric_expression(
                program,
                expression,
                &selected_arms,
                |_| true,
            )
            .and_then(|value| value.to_integer_exact())
            .ok_or("range endpoint needs a defined exact integer")?;
            (exact, None)
        }
        Value::Landed(carrier, value, policy) => {
            let integer = match value {
                IntegerValue::Signed(value) => numerics::bignum::BigInt::from_i128(value),
                IntegerValue::Unsigned(value) => numerics::bignum::BigInt::from_u128(value),
            };
            (
                integer,
                Some(IntegerLanding {
                    landed_type: carrier,
                    domain: policy,
                }),
            )
        }
        Value::Boolean(_) | Value::Float(_, _) | Value::UndeterminedNaN(_) => {
            return Err("range endpoint position requires an exact builtin integer carrier".into());
        }
    };
    let mut literal = IntegerLiteral::from_parts(
        integer.is_negative(),
        IntegerRadix::Decimal,
        &integer.abs().to_string(),
    )
    .map_err(|error| error.to_string())?;
    if let Some(landing) = landing {
        literal = literal.with_landing(landing);
    }
    Ok((literal, warnings))
}

/// Evaluate a context-free const argument against its declared exact carrier.
/// Deferred const-generic applications fold through this entry once Omega
/// supplies their selected provider bodies; the canonical index carries the
/// spelling a `Named` type-reference node records.
pub(crate) fn evaluate_closed_const_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    calls: &dyn ConstantCalls,
) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
    let (value, warnings) = evaluate_scalar_in(
        program,
        EvaluationContext::Closed,
        expression,
        destination,
        ArithmeticDomain::Exact,
        Some(calls),
    )?;
    match value {
        ScalarValue::Index(canonical) => Ok((canonical, warnings)),
        ScalarValue::Float { .. } | ScalarValue::UndeterminedNaN(_) => {
            Err("const application needs an exact integer or Boolean carrier".into())
        }
    }
}
