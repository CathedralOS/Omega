/*
The expression family of the reader: names as slots, literals, the scalar
operators and the short-circuit operators, and whether two read expressions
are the same computation.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};

use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind, Strict};
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
                (ExpressionKind::Slot(slot), slots[slot].scalar_type)
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(id, slots, unary.operand, expected)?;
                match (unary.operator, operand.result) {
                    (UnaryOperator::LogicalNot, Some(ScalarType::Boolean)) => {
                        (ExpressionKind::Not(Box::new(operand)), ScalarType::Boolean)
                    }
                    (UnaryOperator::BitwiseNot, Some(integer @ ScalarType::Integer(_))) => {
                        (ExpressionKind::BitwiseNot(Box::new(operand)), integer)
                    }
                    _ => {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Typing("unary operand type"),
                        ));
                    }
                }
            }
            ExpressionNode::Binary(binary) => {
                // What an operand expects from outside: an arithmetic or shift
                // result has its (value) operand's type, `&&`/`||` operands
                // are Boolean, comparison operands expect nothing. An
                // anonymous literal operand lands in its partner's type, so
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
                let (left, right) = if self.anonymous_literal(binary.left) {
                    let right = self.expression(id, slots, binary.right, hint)?;
                    let left = self.expression(id, slots, binary.left, hint.or(right.result))?;
                    (left, right)
                } else {
                    let left = self.expression(id, slots, binary.left, hint)?;
                    let right = self.expression(id, slots, binary.right, hint.or(left.result))?;
                    (left, right)
                };
                return binary_expression(id, binary.operator, left, right, source);
            }
            ExpressionNode::Call(call) => return self.call_expression(id, slots, call, source),
            ExpressionNode::TypeExpression(_) => return unserved("type expression"),
            ExpressionNode::Match(_) => return unserved("match"),
            ExpressionNode::SavedSubject(_) => return unserved("saved subject"),
            ExpressionNode::ArrayLiteral(_) => return unserved("array literal"),
            ExpressionNode::Atomic(_) => return unserved("atomic"),
            ExpressionNode::Cast(_) => return unserved("cast"),
            ExpressionNode::Float(_) => return unserved("float literal"),
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(_) => return unserved("index"),
            ExpressionNode::Member(_) => return unserved("member"),
            ExpressionNode::Borrow(_) => return unserved("borrow"),
            ExpressionNode::Range(_) => return unserved("range"),
            ExpressionNode::StructLiteral(_) => return unserved("record literal"),
            ExpressionNode::String(_) => return unserved("string literal"),
            ExpressionNode::ZeroValue(_) => return unserved("zero value"),
        };
        Ok(Expression {
            kind,
            result: Some(result),
            source,
        })
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
                result: Some(ScalarType::Boolean),
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
        result: Some(result),
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
            (
                ExpressionKind::Integer(_)
                | ExpressionKind::Boolean(_)
                | ExpressionKind::Slot(_)
                | ExpressionKind::Strict { .. }
                | ExpressionKind::Not(_)
                | ExpressionKind::BitwiseNot(_)
                | ExpressionKind::ShortCircuit { .. }
                | ExpressionKind::Call { .. },
                _,
            ) => false,
        }
}
