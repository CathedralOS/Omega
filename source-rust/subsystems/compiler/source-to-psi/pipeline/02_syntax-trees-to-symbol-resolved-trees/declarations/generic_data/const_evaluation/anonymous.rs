use super::BigInt;
/* Exact anonymous values, independent of their eventual const landing. */

use super::fact_values::ConstFactValue;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use bignum::BigRational;
use diagnostics::Diagnostic;
use numeric_literals::LandedIntegerType;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::expression::TableBinaryExpression;
use syntax_trees::item::Item;
use syntax_trees::item::Machine;
use syntax_trees::operator_spelling::OperatorSpelling;

pub(crate) struct AnonymousNumericValue {
    pub(super) value: BigRational,
    fractional: Option<(ExpressionHandle, BigRational)>,
}

/*
    Named values and prior landings stay with the declared integer evaluator.
    Wholly anonymous arithmetic needs no declaration selection: it denotes an
    exact rational (wiki/spec/language/numeric_values.md), and operator
    spellings resolve by operand carrier, which anonymous operands lack.
*/
pub(super) fn evaluate_anonymous_integer_argument(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<BigInt>, String> {
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
                    let value = text
                        .literal(syntax)
                        .and_then(|literal| BigRational::from_decimal_str(literal.text()))
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
                _ => {
                    return Err("anonymous const expression changed during evaluation".to_owned());
                }
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
    ) -> Result<BigInt, String> {
        let value = self.value;
        let integer = value.to_integer_exact().ok_or_else(|| format!(
    "exact anonymous value `{value}` cannot land in an integer const argument; type an operand if typed integer division was intended"
))?;
        if let Some((origin, fractional_value)) = self.fractional {
            // Implicit const-argument landing after a fractional
            // intermediate rejects (numeric_values.md); an explicit
            // `as` operand is not an anonymous argument at this layer.
            warnings.push(Diagnostic::error(format!(
                "anonymous arithmetic with the exact fractional intermediate `{fractional_value}` cannot land implicitly as integer `{integer}`; cast the complete anonymous expression with `as` to land its exact value, or type an operand if typed integer division was intended"
            )).with_source_span(syntax.expressions.source_span(origin)));
        }
        super::values::const_integer_within_budget(integer).ok_or_else(|| {
            "integer const evaluation exceeds its canonical-value resource budget".to_owned()
        })
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

/* Keep authored operator applications intact until declaration selection. */
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

/*
    Whether this pre-resolution pass may apply the builtin meaning of one
    binary node. Operator spellings resolve by operand carrier
    (wiki/spec/language/numeric_values.md), so a node whose operands are both
    wholly anonymous selects no declaration: it is exact builtin arithmetic or
    comparison.
*/
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
    !syntax
        .root_items()
        .any(|item| authored_spelling_parameters(syntax, item, spelling).is_some())
}

/*
    The declared parameter carriers of one item's authored `spelling`, or
    `None` when the item does not author it. Each parameter yields the spelled
    name of its type (`u64`, `Nat`, `f32`); a parameter whose carrier cannot
    be read — a binder, a borrow, an uninspectable lane — yields `None`, which
    may bind any operand.
*/
fn authored_spelling_parameters(
    syntax: &SyntaxTrees,
    item: &Item,
    spelling: OperatorSpelling,
) -> Option<Vec<Option<syntax_trees::types::TypeReferenceHandle>>> {
    match item {
        Item::Machine(machine) if machine.spelling == Some(spelling) => {
            Some(machine_parameter_types(syntax, machine))
        }
        Item::Operator(operator) if operator.spelling == Some(spelling) => Some(vec![None, None]),
        Item::Domain(domain) => syntax_items(syntax)
            .operators(domain.operators)
            .iter()
            .any(|operator| operator.spelling == Some(spelling))
            .then(|| vec![None, None]),
        Item::Trait(trait_definition) => syntax_items(syntax)
            .state_signatures(trait_definition.machines)
            .iter()
            .any(|handle| syntax.items.state_signature(*handle).spelling == Some(spelling))
            .then(|| vec![None, None]),
        _ => None,
    }
}

/*
    The parameter type names a spelled machine declares on its entry state,
    with the machine's own type binders left unread: `T` can carry any
    operand, so it stays `None`.
*/
fn machine_parameter_types(
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Vec<Option<syntax_trees::types::TypeReferenceHandle>> {
    let binder_names: Vec<&str> = syntax_items(syntax)
        .type_parameters(machine.type_parameters)
        .iter()
        .map(|parameter| parameter.name.as_str(syntax))
        .collect();
    let Some(entry) = syntax.items.state_handles(machine.states).first() else {
        return Vec::new();
    };
    let state = syntax.items.state(*entry);
    syntax_items(syntax)
        .state_parameters(state.parameters)
        .iter()
        .map(|handle| {
            let parameter = syntax.items.state_parameter(*handle);
            let name = syntax_type_references(syntax)
                .type_reference(parameter.type_reference)
                .as_named()?;
            (!binder_names.contains(&name.as_str(syntax))).then_some(parameter.type_reference)
        })
        .collect()
}

/*
    Whether one authored `spelling` occurrence could bind these closed operand
    values. `has_no_authored_spelling` answers before operand carriers are
    known, so the core `Nat` `<=`/`-` machines make every `x <= y` fact
    decline even where the operands could never reach a `Nat` parameter.
    Once both operands landed, a spelled machine applies only when each
    declared parameter accepts the operand's carrier; a fact between `u64`
    values cannot reach `Nat` or `f32` parameters and keeps the builtin
    meaning. Callers whose subject binding loses the carrier (domain replay
    on a scalar `self`) must keep `has_builtin_const_operator`'s answer.
*/
pub(super) fn authored_operator_can_bind(
    syntax: &SyntaxTrees,
    operator: BinaryOperator,
    left: &ConstFactValue,
    right: &ConstFactValue,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> bool {
    let Some(spelling) = binary_operator_spelling(operator) else {
        return false;
    };
    syntax.root_items().any(|item| {
        authored_spelling_parameters(syntax, item, spelling).is_some_and(|parameters| {
            parameters.len() != 2
                || parameter_binds(syntax, parameters[0], left, selection)
                    && parameter_binds(syntax, parameters[1], right, selection)
        })
    })
}

/*
    Whether a parameter declared with the named carrier accepts this operand.
    A declared-integer operand binds only its own carrier (`u64` reaches `u64`
    parameters, never `Nat`); an anonymous or bare integer binds only builtin
    integer carriers — never a nominal `Nat`, `f32` or `Quantity`; a Boolean
    binds `bool`.
*/
fn parameter_binds(
    syntax: &SyntaxTrees,
    parameter_type: Option<syntax_trees::types::TypeReferenceHandle>,
    operand: &ConstFactValue,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> bool {
    let Some(parameter_type) = parameter_type else {
        return true;
    };
    let Some(selection) = selection else {
        return true;
    };
    let Some(name) = syntax
        .type_references
        .type_reference(parameter_type)
        .as_named()
    else {
        return true;
    };
    let Some(atom) = selection.builtin_type(syntax, name) else {
        return selection
            .type_declaration_kind(syntax, name)
            .ok()
            .flatten()
            .is_none();
    };
    use super::fact_values::ConstFactValue;
    match (operand, atom) {
        (
            ConstFactValue::DeclaredInteger { integer_type, .. },
            symbols::BuiltinTypeAtom::Integer(carrier),
        ) => *integer_type == LandedIntegerType::Fixed(carrier),
        (
            ConstFactValue::DeclaredInteger { integer_type, .. },
            symbols::BuiltinTypeAtom::Address,
        ) => *integer_type == LandedIntegerType::Addr,
        (
            ConstFactValue::Integer(_) | ConstFactValue::Anonymous(_),
            symbols::BuiltinTypeAtom::Integer(_) | symbols::BuiltinTypeAtom::Address,
        ) => true,
        (ConstFactValue::Boolean(_), symbols::BuiltinTypeAtom::Bool) => true,
        _ => false,
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
                if text
                    .literal(syntax)
                    .is_some_and(|literal| literal.landing().is_none()) => {}
            ExpressionNode::Binary(binary) => {
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
                    || !matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                    )
                {
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
