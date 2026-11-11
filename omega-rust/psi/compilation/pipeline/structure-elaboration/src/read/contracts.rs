/*
The contract family of the reader: `requires`, `ensures` and `crashes` clauses
read into propositions over the formals, and the identity of the reserved
`result`.
*/

use semantic_vocabulary::{MachineId, Proposition, ScalarTerm, ScalarType, ValueId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::{Program, Slot};
use crate::canonical::{self, Repeated};
use crate::{Unserved, refuse};

/// What a contract clause may name: the formals, and the result pseudo-value
/// only in a normal-return guarantee of a scalar machine.
#[derive(Clone, Copy)]
pub(super) struct ClauseScope<'s> {
    pub(super) slots: &'s [Slot],
    /// The callable whose reserved `result` this clause may name (the
    /// machine's one state; see `TableNamePath::reserved_result_of`).
    pub(super) owner: SymbolHandle,
    pub(super) result: ResultTerm,
}

#[derive(Clone, Copy)]
pub(super) enum ResultTerm {
    /// The result pseudo-value's identity and type.
    Named(ValueId, ScalarType),
    /// Why this clause cannot name `result`.
    Absent(&'static str),
}

impl Program<'_> {
    // Contract clauses: a comparison of formals, literals and Booleans, or a
    // conjunction of those. Terms name formal `i` as value `i + 1`.

    /// A contract fact that is an expression; memberships and named
    /// propositions are outside the subset.
    pub(super) fn fact(&self, id: MachineId, fact: &ProofFact) -> Checked<ExpressionHandle> {
        match fact {
            ProofFact::Expression(expression) => Ok(*expression),
            ProofFact::Membership(_) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("domain membership"),
            )),
            ProofFact::Proposition(_) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("named proposition"),
            )),
        }
    }

    pub(super) fn clause(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Proposition> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing clause expression");
        }
        let ExpressionNode::Binary(binary) = typed.expression_table.expression(expression) else {
            return unserved("clause that is not a comparison or conjunction");
        };
        let operator = binary.operator;
        let comparison = match operator {
            BinaryOperator::And => {
                let mut conjuncts = Vec::new();
                for side in [binary.left, binary.right] {
                    match self.clause(id, scope, side)? {
                        Proposition::Conjunction(inner) => conjuncts.extend(inner),
                        other => conjuncts.push(other),
                    }
                }
                return canonical::clauses(conjuncts, Repeated::Refuse, id)
                    .map(Proposition::Conjunction);
            }
            BinaryOperator::Equal
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => operator,
            BinaryOperator::NotEqual => return unserved("inequality (no negated proposition)"),
            BinaryOperator::Or => return unserved("disjunction"),
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => return unserved("arithmetic clause"),
            BinaryOperator::CaseMembership => return unserved("case membership"),
        };
        // An anonymous literal lands in its partner's type.
        let ((left, left_type), (right, right_type)) = if self.anonymous_literal(binary.left) {
            let right = self.term(id, scope, binary.right, None)?;
            (self.term(id, scope, binary.left, Some(right.1))?, right)
        } else {
            let left = self.term(id, scope, binary.left, None)?;
            let right = self.term(id, scope, binary.right, Some(left.1))?;
            (left, right)
        };
        if left_type != right_type {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("clause operand types differ"),
            ));
        }
        let ordered = matches!(left_type, ScalarType::Integer(_));
        match comparison {
            BinaryOperator::Equal => canonical::equal(left, right, id),
            _ if !ordered => unserved("ordering of Booleans"),
            BinaryOperator::Less => Ok(Proposition::LessThan(left, right)),
            BinaryOperator::LessOrEqual => Ok(Proposition::LessOrEqual(left, right)),
            BinaryOperator::Greater => Ok(Proposition::LessThan(right, left)),
            BinaryOperator::GreaterOrEqual => Ok(Proposition::LessOrEqual(right, left)),
            _ => unserved("comparison"),
        }
    }

    pub(super) fn term(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<(ScalarTerm, ScalarType)> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing term");
        }
        match typed.expression_table.expression(expression) {
            // The reserved result, by the owner identity stage 02 recorded,
            // never by its spelling. calls_and_outcomes.md: "The reserved
            // result occurrence belongs to its contract owner, not any local
            // or parameter spelled `result`."
            ExpressionNode::Name(path) if path.reserved_result_of.is_valid() => {
                if path.reserved_result_of != scope.owner {
                    return unserved("result of another callable");
                }
                match scope.result {
                    ResultTerm::Named(id, scalar_type) => {
                        Ok((ScalarTerm::Value { id, scalar_type }, scalar_type))
                    }
                    ResultTerm::Absent(why) => unserved(why),
                }
            }
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, scope.slots, path)?;
                let row = &scope.slots[slot];
                let value = ValueId::new(slot as u64 + 1).ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("formal identity"),
                    )
                })?;
                Ok((
                    ScalarTerm::Value {
                        id: value,
                        scalar_type: row.scalar_type,
                    },
                    row.scalar_type,
                ))
            }
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                Ok((
                    ScalarTerm::Integer {
                        scalar_type: integer,
                        value,
                    },
                    ScalarType::Integer(integer),
                ))
            }
            ExpressionNode::Boolean(value) => {
                Ok((ScalarTerm::Boolean(*value), ScalarType::Boolean))
            }
            _ => unserved("term other than a formal or literal"),
        }
    }
}
