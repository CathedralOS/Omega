//! Exact anonymous values, independent of their eventual const landing.

use diagnostics::Diagnostic;
use numerics::bignum::BigRational;
use numerics::literals::FloatLiteral;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::expression::TableBinaryExpression;
use syntax_trees::item::Item;
use syntax_trees::operator_spelling::OperatorSpelling;
use syntax_trees::types::TypeReferenceNode;

pub(crate) struct AnonymousNumericValue {
    pub(super) value: BigRational,
    fractional: Option<(ExpressionHandle, BigRational)>,
}

/// Named values and prior landings stay with the declared integer evaluator.
/// Wholly anonymous arithmetic needs no declaration selection: it denotes an
/// exact rational (wiki/spec/language/numeric_values.md), and operator
/// spellings resolve by operand carrier, which anonymous operands lack.
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
                ExpressionNode::Float(text) => {
                    let value = BigRational::from_decimal_str(text.as_str())
                        .ok_or_else(|| "anonymous decimal literal has no exact value".to_owned())?;
                    if fractional.is_none() && value.to_integer_exact().is_none() {
                        fractional = Some((expression, value.clone()));
                    }
                    values.push(value);
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
            "anonymous arithmetic preserves the exact fractional intermediate `{fractional_value}` before landing as integer `{integer}`; type an operand if typed integer division was intended"
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

/// Keep authored operator applications intact until declaration selection.
pub(super) fn requires_const_operator_selection(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            !has_builtin_const_operator(syntax, binary)
                || requires_const_operator_selection(syntax, binary.left)
                || requires_const_operator_selection(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => requires_const_operator_selection(syntax, unary.operand),
        _ => false,
    }
}

/// Whether this pre-resolution pass may apply the builtin meaning of one
/// binary node. Operator spellings resolve by operand carrier
/// (wiki/spec/language/numeric_values.md), so a node whose operands are both
/// wholly anonymous selects no declaration: it is exact builtin arithmetic or
/// comparison, the rule checked-tree operator selection also applies.
pub(super) fn has_builtin_const_operator(
    syntax: &SyntaxTrees,
    binary: &TableBinaryExpression,
) -> bool {
    if anonymous_numeric_expression(syntax, binary.left)
        && anonymous_numeric_expression(syntax, binary.right)
    {
        return true;
    }
    let Some(spelling) = binary_operator_spelling(binary.operator) else {
        return true;
    };
    has_no_authored_spelling(syntax, spelling)
}

fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    Some(match operator {
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
        | BinaryOperator::ShiftRight => return None,
    })
}

fn has_no_authored_spelling(syntax: &SyntaxTrees, spelling: OperatorSpelling) -> bool {
    // This pre-resolution evaluator has no selected-operator authority. With a
    // typed operand, any potentially relevant authored spelling makes this
    // narrow builtin check decline; it does not authorize that declaration or certify the existing
    // const evaluator's handling of authored operator meanings. A fixed token
    // after `machine` authors the same spelling the retired `operator` head
    // did (`boundary machine % Math::remainder` is that slot's new spelling),
    // and a trait requirement may bind one too, so machine items and trait
    // member signatures join the fence alongside the two legacy lanes.
    !syntax
        .root_items()
        .any(|item| authored_spelling_parameters(syntax, item, spelling).is_some())
}

/// The declared parameter carriers of one item's authored `spelling`, or
/// `None` when the item does not author it. Each parameter yields the spelled
/// name of its type (`u64`, `Nat`, `f32`); a parameter whose carrier cannot
/// be read — a binder, a borrow, an uninspectable lane — yields `None`, which
/// may bind any operand.
fn authored_spelling_parameters(
    syntax: &SyntaxTrees,
    item: &Item,
    spelling: OperatorSpelling,
) -> Option<Vec<Option<String>>> {
    match item {
        Item::Machine(machine) if machine.spelling == Some(spelling) => {
            Some(machine_parameter_names(syntax, machine))
        }
        // Operator declarations, domain-attached operators and trait member
        // signatures author the spelling through shapes this pass does not
        // read into parameter carriers; they conservatively may bind.
        Item::Operator(operator) if operator.spelling == Some(spelling) => Some(vec![None, None]),
        Item::Domain(domain) => syntax
            .items
            .operators(domain.operators)
            .iter()
            .any(|operator| operator.spelling == Some(spelling))
            .then(|| vec![None, None]),
        Item::Trait(trait_definition) => syntax
            .items
            .state_signatures(trait_definition.machines)
            .iter()
            .any(|handle| syntax.items.state_signature(*handle).spelling == Some(spelling))
            .then(|| vec![None, None]),
        _ => None,
    }
}

/// The parameter type names a spelled machine declares on its entry state,
/// with the machine's own type binders left unread: `T` can carry any
/// operand, so it stays `None`.
fn machine_parameter_names(
    syntax: &SyntaxTrees,
    machine: &syntax_trees::item::Machine,
) -> Vec<Option<String>> {
    let binder_names: Vec<&str> = syntax
        .items
        .type_parameters(machine.type_parameters)
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();
    let Some(entry) = syntax.items.state_handles(machine.states).first() else {
        return Vec::new();
    };
    let state = syntax.items.state(*entry);
    syntax
        .items
        .state_parameters(state.parameters)
        .iter()
        .map(|handle| {
            let parameter = syntax.items.state_parameter(*handle);
            let TypeReferenceNode::Named(name) = syntax
                .type_references
                .type_reference(parameter.type_reference)
            else {
                return None;
            };
            (!binder_names.contains(&name.as_str())).then(|| name.as_str().to_string())
        })
        .collect()
}

/// Whether one authored `spelling` occurrence could bind these closed operand
/// values. `has_no_authored_spelling` answers before operand carriers are
/// known, so the core `Nat` `<=`/`-` machines make every `x <= y` fact
/// decline even where the operands could never reach a `Nat` parameter.
/// Once both operands landed, a spelled machine applies only when each
/// declared parameter accepts the operand's carrier; a fact between `u64`
/// values cannot reach `Nat` or `f32` parameters and keeps the builtin
/// meaning. Callers whose subject binding loses the carrier (domain replay
/// on a scalar `self`) must keep `has_builtin_const_operator`'s answer.
pub(super) fn authored_operator_can_bind(
    syntax: &SyntaxTrees,
    operator: BinaryOperator,
    left: &super::fact_values::ConstFactValue,
    right: &super::fact_values::ConstFactValue,
) -> bool {
    let Some(spelling) = binary_operator_spelling(operator) else {
        return false;
    };
    syntax.root_items().any(|item| {
        authored_spelling_parameters(syntax, item, spelling).is_some_and(|parameters| {
            parameters.len() != 2
                || parameter_binds(parameters[0].as_deref(), left)
                    && parameter_binds(parameters[1].as_deref(), right)
        })
    })
}

/// Whether a parameter declared with the named carrier accepts this operand.
/// A declared-integer operand binds only its own carrier (`u64` reaches `u64`
/// parameters, never `Nat`); an anonymous or bare integer binds only builtin
/// integer carriers — never a nominal `Nat`, `f32` or `Quantity`; a Boolean
/// binds `bool`.
fn parameter_binds(
    parameter_name: Option<&str>,
    operand: &super::fact_values::ConstFactValue,
) -> bool {
    let Some(name) = parameter_name else {
        return true;
    };
    use super::fact_values::ConstFactValue;
    match operand {
        ConstFactValue::DeclaredInteger { integer_type, .. } => name == integer_type.name(),
        ConstFactValue::Integer(_) | ConstFactValue::Anonymous(_) => {
            numerics::literals::LandedIntegerType::from_name(name).is_some()
        }
        ConstFactValue::Boolean(_) => name == "bool",
    }
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
            ExpressionNode::Float(text)
                if FloatLiteral::parse(text.as_str())
                    .is_some_and(|literal| literal.landing().is_none()) => {}
            // No authored spelling can select an operator here: neither
            // operand has a carrier until the completed value lands, the same
            // rule the checked-tree operator selection applies.
            ExpressionNode::Binary(binary) => {
                if !matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                ) {
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
