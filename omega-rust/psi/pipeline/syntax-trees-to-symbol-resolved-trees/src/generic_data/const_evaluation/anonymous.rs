//! Exact anonymous values, independent of their eventual const landing.

use diagnostics::Diagnostic;
use numerics::bignum::BigRational;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::Item;
use syntax_trees::operator_spelling::OperatorSpelling;

pub(in crate::generic_data) struct AnonymousNumericValue {
    pub(super) value: BigRational,
    fractional: Option<(ExpressionHandle, BigRational)>,
}

/// Named values and prior landings stay with the declared integer evaluator.
/// This pass has no declaration-selection authority: the shared anonymous
/// classifier declines when an authored spelling could supply the meaning.
pub(super) fn evaluate_anonymous_integer_argument(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<i128>, String> {
    evaluate_anonymous_numeric_expression(syntax, expression)?
        .map(|value| value.into_integer(syntax, warnings))
        .transpose()
}

pub(super) fn evaluate_anonymous_numeric_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<Option<AnonymousNumericValue>, String> {
    if !anonymous_numeric_expression(syntax, expression) {
        return Ok(None);
    }
    enum Step {
        Enter(ExpressionHandle),
        Binary(ExpressionHandle, BinaryOperator),
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut values: Vec<BigRational> = Vec::new();
    let mut fractional = None;
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => match syntax.expressions.expression(expression) {
                ExpressionNode::Integer(literal) => {
                    let value = literal
                        .value_bignum()
                        .ok_or_else(|| "anonymous integer literal has no exact value".to_owned())?;
                    values.push(BigRational::from_integer(value));
                }
                ExpressionNode::Binary(binary) => {
                    pending.push(Step::Binary(expression, binary.operator));
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                _ => return Err("anonymous const expression changed during evaluation".to_owned()),
            },
            Step::Binary(expression, operator) => {
                let right = values
                    .pop()
                    .ok_or_else(|| "missing right const operand".to_owned())?;
                let left = values
                    .pop()
                    .ok_or_else(|| "missing left const operand".to_owned())?;
                let value = match operator {
                    BinaryOperator::Add => left.add(&right),
                    BinaryOperator::Subtract => left.sub(&right),
                    BinaryOperator::Multiply => left.mul(&right),
                    BinaryOperator::Divide => left
                        .div(&right)
                        .ok_or_else(|| "division by zero is invalid".to_owned())?,
                    _ => return Err("operator has no anonymous const meaning".to_owned()),
                };
                if fractional.is_none() && value.to_integer_exact().is_none() {
                    fractional = Some((expression, value.clone()));
                }
                values.push(value);
            }
        }
    }
    if values.len() != 1 {
        return Err("anonymous const expression has no single value".to_owned());
    }
    let value = values
        .pop()
        .ok_or_else(|| "missing anonymous const value".to_owned())?;
    Ok(Some(AnonymousNumericValue { value, fractional }))
}

impl AnonymousNumericValue {
    pub(super) fn into_integer(
        self,
        syntax: &SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<i128, String> {
        let value = self.value;
        let integer = value.to_integer_exact().ok_or_else(|| format!(
        "exact anonymous value `{value}` cannot land in an integer const argument; type an operand if typed integer division was intended"
    ))?;
        let value = integer
            .to_i64()
            .map(i128::from)
            .or_else(|| integer.to_u64().map(i128::from))
            .ok_or_else(|| {
                "integer const argument exceeds the signed/unsigned 64-bit envelope".to_owned()
            })?;
        if let Some((origin, fractional_value)) = self.fractional {
            warnings.push(Diagnostic::warning(format!(
            "anonymous division preserves the exact fractional intermediate `{fractional_value}` before landing as integer `{integer}`; type an operand if typed integer division was intended"
        )).with_source_span(syntax.expressions.source_span(origin)));
        }
        Ok(value)
    }

    pub(super) fn binary(
        self,
        right: Self,
        operator: BinaryOperator,
        expression: ExpressionHandle,
    ) -> Result<Self, String> {
        let value = match operator {
            BinaryOperator::Add => self.value.add(&right.value),
            BinaryOperator::Subtract => self.value.sub(&right.value),
            BinaryOperator::Multiply => self.value.mul(&right.value),
            BinaryOperator::Divide => self
                .value
                .div(&right.value)
                .ok_or_else(|| "division by zero is invalid".to_owned())?,
            _ => return Err("operator has no anonymous const meaning".to_owned()),
        };
        let fractional = self.fractional.or(right.fractional).or_else(|| {
            value
                .to_integer_exact()
                .is_none()
                .then(|| (expression, value.clone()))
        });
        Ok(Self { value, fractional })
    }
}

pub(super) fn has_builtin_const_operator(syntax: &SyntaxTrees, operator: BinaryOperator) -> bool {
    let spelling = match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        // These tokens have no authored operator spelling.
        BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return true,
    };
    has_no_authored_spelling(syntax, spelling)
}

pub(super) fn has_no_authored_spelling(syntax: &SyntaxTrees, spelling: OperatorSpelling) -> bool {
    // This pre-resolution evaluator has no selected-operator authority. Any
    // potentially relevant authored spelling makes this narrow builtin check
    // decline; it does not authorize that declaration or certify the existing
    // const evaluator's handling of authored operator meanings.
    !syntax.root_items().any(|item| match item {
        Item::Operator(operator) => operator.spelling == Some(spelling),
        Item::Domain(domain) => syntax
            .items
            .operators(domain.operators)
            .iter()
            .any(|operator| operator.spelling == Some(spelling)),
        _ => false,
    })
}

pub(super) fn anonymous_numeric_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> bool {
    enum Step {
        Enter(ExpressionHandle),
        Leave,
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    while let Some(step) = pending.pop() {
        let Step::Enter(expression) = step else {
            active.pop();
            continue;
        };
        if !expression.is_valid() || active.contains(&expression) {
            return false;
        }
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
            ExpressionNode::Binary(binary) => {
                let spelling = match binary.operator {
                    BinaryOperator::Add => OperatorSpelling::Add,
                    BinaryOperator::Subtract => OperatorSpelling::Subtract,
                    BinaryOperator::Multiply => OperatorSpelling::Multiply,
                    BinaryOperator::Divide => OperatorSpelling::Divide,
                    _ => return false,
                };
                if !has_no_authored_spelling(syntax, spelling) {
                    return false;
                }
                active.push(expression);
                pending.push(Step::Leave);
                pending.push(Step::Enter(binary.right));
                pending.push(Step::Enter(binary.left));
            }
            _ => return false,
        }
    }
    true
}
