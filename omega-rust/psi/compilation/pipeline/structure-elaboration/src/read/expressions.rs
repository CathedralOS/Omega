/*
The expression family of the reader: names as slots, literals, the scalar
operators and the short-circuit operators, and whether two read expressions
are the same computation. Record literals and member paths are read in
`places`.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};

use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind, Strict, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl<'t> Program<'t> {
    pub(super) fn slot_of(
        &self,
        id: MachineId,
        slots: &[Slot],
        path: &TableNamePath,
    ) -> Checked<usize> {
        let typed = self.typed;
        if path.reserved_result_of.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("reserved result outside a contract term"),
            ));
        }
        if !path.symbol.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Spelling("name without a symbol"),
            ));
        }
        if typed.expression_table.name_path_members(path.members).len() != 1 {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("projected name path"),
            ));
        }
        // ponytail: linear over the body's slots; bodies in the subset hold a
        // handful of locals.
        slots
            .iter()
            .rposition(|slot| slot.symbol == path.symbol)
            .ok_or_else(|| {
                let kind = typed.symbols.get(path.symbol).kind;
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Expression(symbol_kind(kind)),
                )
            })
    }

    pub(super) fn node(
        &self,
        id: MachineId,
        expression: ExpressionHandle,
    ) -> Checked<&'t ExpressionNode> {
        if !expression.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("missing expression"),
            ));
        }
        Ok(self.typed.expression_table.expression(expression))
    }

    /// Reads one expression. `expected` is the type its context expects; only
    /// an anonymous integer literal uses it, and every other node's type is
    /// checked against its context by the caller.
    pub(super) fn expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<Expression> {
        let typed = self.typed;
        let node = self.node(id, handle)?;
        let source = SourceOrigin::of(typed.expression_table.source_span(handle));
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let (kind, result) = match node {
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                return Ok(Expression {
                    kind: ExpressionKind::Slot(slot),
                    result: Some(slots[slot].value_type),
                    source,
                });
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(id, slots, unary.operand, expected)?;
                match (unary.operator, operand.result) {
                    (UnaryOperator::LogicalNot, Some(ValueType::Scalar(ScalarType::Boolean))) => {
                        (ExpressionKind::Not(Box::new(operand)), ScalarType::Boolean)
                    }
                    (
                        UnaryOperator::BitwiseNot,
                        Some(ValueType::Scalar(integer @ ScalarType::Integer(_))),
                    ) => (ExpressionKind::BitwiseNot(Box::new(operand)), integer),
                    _ => {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Typing("unary operand type"),
                        ));
                    }
                }
            }
            ExpressionNode::Binary(_) if self.wholly_anonymous(handle) => {
                let (value, integer) = self.anonymous_value(id, handle, expected)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Binary(binary) => {
                // What an operand expects from outside: an arithmetic or shift
                // result has its (value) operand's type, `&&`/`||` operands
                // are Boolean, comparison operands expect nothing. An
                // wholly anonymous operand lands in its partner's type, so
                // its partner is read first; reading order is not evaluation
                // order, which lowering takes from the operand positions.
                let hint = match binary.operator {
                    BinaryOperator::And | BinaryOperator::Or => Some(ScalarType::Boolean),
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => expected,
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::CaseMembership => None,
                };
                // `subject in S::C`. Stage 01 writes the test as
                // `CaseMembership` in a guard and as `Equal` in value
                // position; in both the right operand names a case, which is
                // what makes it a case test. `k == Kind::A` on a
                // payload-free sum is the same test (data_and_literals.md:
                // "Primitive and payload-free-sum equality is intrinsic").
                if matches!(
                    binary.operator,
                    BinaryOperator::CaseMembership | BinaryOperator::Equal
                ) && let Some(case) = self.named_case(binary.right)
                {
                    return self.case_test(id, slots, binary.left, case, source);
                }
                let (left, right) = if self.wholly_anonymous(binary.left) {
                    let right = self.expression(id, slots, binary.right, hint)?;
                    let partner = right.result.and_then(ValueType::scalar);
                    let left = self.expression(id, slots, binary.left, hint.or(partner))?;
                    (left, right)
                } else {
                    let left = self.expression(id, slots, binary.left, hint)?;
                    let partner = left.result.and_then(ValueType::scalar);
                    let right = self.expression(id, slots, binary.right, hint.or(partner))?;
                    (left, right)
                };
                return binary_expression(id, binary.operator, left, right, source);
            }
            ExpressionNode::Call(call) => return self.call_expression(id, slots, call, source),
            ExpressionNode::TypeExpression(_) => return unserved("type expression"),
            ExpressionNode::Match(matched) => {
                return self.match_expression(id, slots, matched, expected, source);
            }
            // The subject of the enclosing `match`, named by the capture
            // identity stage 03 gave both; any other saved subject is
            // outside the subset.
            ExpressionNode::SavedSubject(saved) => {
                match saved.component {
                    language_core::PatternComponent::Fields => {}
                    language_core::PatternComponent::Case => {
                        return unserved("saved subject case");
                    }
                }
                return match self
                    .captures
                    .iter()
                    .rev()
                    .find(|(capture, _)| *capture == saved.capture)
                {
                    Some((_, subject)) => Ok(subject.clone()),
                    None => unserved("saved subject"),
                };
            }
            ExpressionNode::ArrayLiteral(_) => return unserved("array literal"),
            ExpressionNode::Atomic(_) => return unserved("atomic"),
            ExpressionNode::Cast(_) => return unserved("cast"),
            ExpressionNode::Float(_) => return unserved("float literal"),
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(_) => return unserved("index"),
            ExpressionNode::Member(member) => return self.member(id, slots, member, source),
            ExpressionNode::Borrow(_) => return unserved("borrow"),
            ExpressionNode::Range(_) => return unserved("range"),
            ExpressionNode::StructLiteral(literal) => {
                return self.record_literal(id, slots, literal, source);
            }
            ExpressionNode::String(_) => return unserved("string literal"),
            ExpressionNode::ZeroValue(_) => return unserved("zero value"),
        };
        Ok(Expression {
            kind,
            result: Some(ValueType::Scalar(result)),
            source,
        })
    }

    /// Value-position `match` over a sum with scalar arms. The subject must
    /// name storage (a slot or a path beneath one), so observing it in the
    /// dispatch and again in each arm's payload members evaluates nothing
    /// twice. Each arm is a case pattern or the wildcard, which must be
    /// last; an arm for a case an earlier arm selects can never run and is
    /// refused. Coverage is not decided here: lowering builds the
    /// terminator from the declaration's case table and reports a missing
    /// case there.
    fn match_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        expected: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let subject = self.expression(id, slots, matched.subject, None)?;
        let Some(ValueType::Record(record)) = subject.result else {
            return Err(unserved("match on a scalar subject"));
        };
        if !names_storage(&subject) {
            return Err(unserved("match on a subject that is not a place"));
        }
        self.captures.push((matched.capture, subject.clone()));
        let read = self.match_arms(id, slots, matched, record, &subject, expected);
        self.captures.pop();
        let (arms, default, result) = read?;
        Ok(Expression {
            kind: ExpressionKind::Match {
                subject: Box::new(subject),
                arms,
                default,
            },
            result: Some(ValueType::Scalar(result)),
            source,
        })
    }

    #[allow(clippy::type_complexity)]
    fn match_arms(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        record: semantic_vocabulary::StructuralTypeId,
        subject: &Expression,
        expected: Option<ScalarType>,
    ) -> Checked<(
        Vec<(usize, Expression)>,
        Option<Box<Expression>>,
        ScalarType,
    )> {
        use typed_trees::expression::MatchPattern;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let mut arms: Vec<(usize, Expression)> = Vec::new();
        let mut default = None;
        let mut result = expected;
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            if default.is_some() {
                return Err(unserved("match arm after the wildcard"));
            }
            let case = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Value(_) => return Err(unserved("match arm on a value pattern")),
                MatchPattern::Destructure(pattern) => {
                    let case = self.case_pattern(id, record, pattern)?;
                    // Stage 03 keeps the arm's test as the pattern's
                    // condition, `subject in case && true`; it must read to
                    // exactly that, or the arm tests something else.
                    let condition =
                        self.expression(id, slots, pattern.condition, Some(ScalarType::Boolean))?;
                    if !same_computation(&condition, &case_condition(subject, case)) {
                        return Err(unserved("match arm whose condition is not its case test"));
                    }
                    if arms.iter().any(|(earlier, _)| *earlier == case) {
                        return Err(unserved("match arm for a case an earlier arm selects"));
                    }
                    Some(case)
                }
            };
            let value = self.expression(id, slots, arm.value, result)?;
            let value_type = match value.result {
                Some(ValueType::Scalar(scalar_type)) => scalar_type,
                Some(ValueType::Record(_)) => return Err(unserved("match with a data result")),
                Some(ValueType::Rigid(_)) => {
                    return Err(unserved("match with a result of a rigid type"));
                }
                None => return Err(unserved("match with a Unit arm")),
            };
            if result.is_some_and(|result| result != value_type) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("match arm value type"),
                ));
            }
            result = Some(value_type);
            match case {
                Some(case) => arms.push((case, value)),
                None => default = Some(Box::new(value)),
            }
        }
        let result = result.ok_or_else(|| unserved("match without an arm"))?;
        Ok((arms, default, result))
    }
}

/// Whether a data-typed expression names storage that exists before it is
/// read: a slot, or a field or payload path beneath one. Reading it again
/// names the same place and evaluates nothing.
pub(super) fn names_storage(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Slot(_) => true,
        ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
            names_storage(base)
        }
        ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Record { .. }
        | ExpressionKind::Case { .. }
        | ExpressionKind::IsCase { .. }
        | ExpressionKind::Match { .. }
        | ExpressionKind::Strict { .. }
        | ExpressionKind::Not(_)
        | ExpressionKind::BitwiseNot(_)
        | ExpressionKind::ShortCircuit { .. }
        | ExpressionKind::Call { .. } => false,
    }
}

/// `subject in case`, the test of a case arm, as the reader reads it.
pub(super) fn case_test_of(subject: &Expression, case: usize) -> Expression {
    Expression {
        kind: ExpressionKind::IsCase {
            subject: Box::new(subject.clone()),
            case,
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean)),
        source: subject.source,
    }
}

/// `subject in case && true`: the condition stage 03 records on a case
/// pattern with no guard of its own.
pub(super) fn case_condition(subject: &Expression, case: usize) -> Expression {
    Expression {
        kind: ExpressionKind::ShortCircuit {
            conjunction: true,
            left: Box::new(case_test_of(subject, case)),
            right: Box::new(Expression {
                kind: ExpressionKind::Boolean(true),
                result: Some(ValueType::Scalar(ScalarType::Boolean)),
                source: subject.source,
            }),
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean)),
        source: subject.source,
    }
}

/// Builtin operator selection: which operation a binary operator names, and
/// its operand and result types. Builtin meaning is all the subset has: an
/// operator bound to a declared machine reaches the typed trees as a call.
fn binary_expression(
    id: MachineId,
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
    source: SourceOrigin,
) -> Checked<Expression> {
    enum Operands {
        /// Same integer type; the result has it.
        Arithmetic,
        /// Integer value and integer count, each its own type; the result
        /// has the value's type.
        Shift,
        /// Same integer or Boolean type; the result is Boolean.
        Equality,
        /// Same integer type; the result is Boolean.
        Ordering,
    }
    let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
    let (Some(left_type), Some(right_type)) = (left.result, right.result) else {
        return Err(typing("Unit operand"));
    };
    // Builtin operators are over scalars. Equality of two records needs
    // synthesized structural comparison evidence
    // (`wiki/spec/terminal-psi/structural_predicates.md`) and is refused.
    let (ValueType::Scalar(left_type), ValueType::Scalar(right_type)) = (left_type, right_type)
    else {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Expression("operator on a record"),
        ));
    };
    let (operation, operands) = match operator {
        BinaryOperator::And | BinaryOperator::Or => {
            if left_type != ScalarType::Boolean || right_type != ScalarType::Boolean {
                return Err(typing("short-circuit operand is not bool"));
            }
            return Ok(Expression {
                kind: ExpressionKind::ShortCircuit {
                    conjunction: operator == BinaryOperator::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                result: Some(ValueType::Scalar(ScalarType::Boolean)),
                source,
            });
        }
        BinaryOperator::CaseMembership => {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("case membership"),
            ));
        }
        BinaryOperator::Add => (Strict::Add, Operands::Arithmetic),
        BinaryOperator::Subtract => (Strict::Subtract, Operands::Arithmetic),
        BinaryOperator::Multiply => (Strict::Multiply, Operands::Arithmetic),
        BinaryOperator::Divide => (Strict::Divide, Operands::Arithmetic),
        BinaryOperator::Modulo => (Strict::Remainder, Operands::Arithmetic),
        BinaryOperator::BitwiseAnd => (Strict::BitwiseAnd, Operands::Arithmetic),
        BinaryOperator::BitwiseOr => (Strict::BitwiseOr, Operands::Arithmetic),
        BinaryOperator::BitwiseXor => (Strict::BitwiseXor, Operands::Arithmetic),
        BinaryOperator::ShiftLeft => (Strict::ShiftLeft, Operands::Shift),
        BinaryOperator::ShiftRight => (Strict::ShiftRight, Operands::Shift),
        BinaryOperator::Equal => (Strict::Equal, Operands::Equality),
        BinaryOperator::NotEqual => (Strict::NotEqual, Operands::Equality),
        BinaryOperator::Less => (Strict::Less, Operands::Ordering),
        BinaryOperator::LessOrEqual => (Strict::LessOrEqual, Operands::Ordering),
        BinaryOperator::Greater => (Strict::Greater, Operands::Ordering),
        BinaryOperator::GreaterOrEqual => (Strict::GreaterOrEqual, Operands::Ordering),
    };
    let integer = |scalar_type| matches!(scalar_type, ScalarType::Integer(_));
    let result = match operands {
        Operands::Arithmetic if integer(left_type) && left_type == right_type => left_type,
        Operands::Shift if integer(left_type) && integer(right_type) => left_type,
        Operands::Equality
            if left_type == right_type
                && (integer(left_type) || left_type == ScalarType::Boolean) =>
        {
            ScalarType::Boolean
        }
        Operands::Ordering if integer(left_type) && left_type == right_type => ScalarType::Boolean,
        Operands::Arithmetic | Operands::Shift | Operands::Equality | Operands::Ordering => {
            return Err(typing("operand types"));
        }
    };
    Ok(Expression {
        kind: ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        result: Some(ValueType::Scalar(result)),
        source,
    })
}

/// Whether two read expressions compute the same thing: equal kinds over
/// equal slots, literal values, operators and callee identities, ignoring
/// only where each came from. Nothing here is text.
pub(super) fn same_computation(left: &Expression, right: &Expression) -> bool {
    let both = |a: &Expression, b: &Expression| same_computation(a, b);
    left.result == right.result
        && match (&left.kind, &right.kind) {
            (ExpressionKind::Integer(a), ExpressionKind::Integer(b)) => a == b,
            (ExpressionKind::Boolean(a), ExpressionKind::Boolean(b)) => a == b,
            (ExpressionKind::Slot(a), ExpressionKind::Slot(b)) => a == b,
            (
                ExpressionKind::Strict {
                    operation: a,
                    left: a_left,
                    right: a_right,
                },
                ExpressionKind::Strict {
                    operation: b,
                    left: b_left,
                    right: b_right,
                },
            ) => a == b && both(a_left, b_left) && both(a_right, b_right),
            (ExpressionKind::Not(a), ExpressionKind::Not(b))
            | (ExpressionKind::BitwiseNot(a), ExpressionKind::BitwiseNot(b)) => both(a, b),
            (
                ExpressionKind::ShortCircuit {
                    conjunction: a,
                    left: a_left,
                    right: a_right,
                },
                ExpressionKind::ShortCircuit {
                    conjunction: b,
                    left: b_left,
                    right: b_right,
                },
            ) => a == b && both(a_left, b_left) && both(a_right, b_right),
            (
                ExpressionKind::Call {
                    callee: a,
                    arguments: a_arguments,
                },
                ExpressionKind::Call {
                    callee: b,
                    arguments: b_arguments,
                },
            ) => {
                a == b
                    && a_arguments.len() == b_arguments.len()
                    && a_arguments.iter().zip(b_arguments).all(|(a, b)| both(a, b))
            }
            (ExpressionKind::Record { fields: a }, ExpressionKind::Record { fields: b }) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((a_field, a), (b_field, b))| a_field == b_field && both(a, b))
            }
            (
                ExpressionKind::Field {
                    base: a,
                    field: a_field,
                },
                ExpressionKind::Field {
                    base: b,
                    field: b_field,
                },
            ) => a_field == b_field && both(a, b),
            (
                ExpressionKind::Case {
                    case: a_case,
                    fields: a,
                },
                ExpressionKind::Case {
                    case: b_case,
                    fields: b,
                },
            ) => {
                a_case == b_case
                    && a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((a_field, a), (b_field, b))| a_field == b_field && both(a, b))
            }
            (
                ExpressionKind::Payload {
                    base: a,
                    case: a_case,
                    field: a_field,
                },
                ExpressionKind::Payload {
                    base: b,
                    case: b_case,
                    field: b_field,
                },
            ) => a_case == b_case && a_field == b_field && both(a, b),
            (
                ExpressionKind::IsCase {
                    subject: a,
                    case: a_case,
                },
                ExpressionKind::IsCase {
                    subject: b,
                    case: b_case,
                },
            ) => a_case == b_case && both(a, b),
            (
                ExpressionKind::Match {
                    subject: a,
                    arms: a_arms,
                    default: a_default,
                },
                ExpressionKind::Match {
                    subject: b,
                    arms: b_arms,
                    default: b_default,
                },
            ) => {
                both(a, b)
                    && a_arms.len() == b_arms.len()
                    && a_arms
                        .iter()
                        .zip(b_arms)
                        .all(|((a_case, a), (b_case, b))| a_case == b_case && both(a, b))
                    && match (a_default, b_default) {
                        (Some(a), Some(b)) => both(a, b),
                        (None, None) => true,
                        (Some(_), None) | (None, Some(_)) => false,
                    }
            }
            (
                ExpressionKind::Integer(_)
                | ExpressionKind::Boolean(_)
                | ExpressionKind::Slot(_)
                | ExpressionKind::Record { .. }
                | ExpressionKind::Case { .. }
                | ExpressionKind::Payload { .. }
                | ExpressionKind::IsCase { .. }
                | ExpressionKind::Match { .. }
                | ExpressionKind::Field { .. }
                | ExpressionKind::Strict { .. }
                | ExpressionKind::Not(_)
                | ExpressionKind::BitwiseNot(_)
                | ExpressionKind::ShortCircuit { .. }
                | ExpressionKind::Call { .. },
                _,
            ) => false,
        }
}
