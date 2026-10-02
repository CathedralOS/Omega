//! Proof-only integer contract terms. Finite observations retain their own
//! carrier and arithmetic policy; the surrounding operators are unbounded.
//! Terms use one local arena so source replay can validate every child identity
//! without giving mathematical expressions an executable scalar carrier.

use super::CheckedScalarExpression;
use arena::{Arena, Handle};
use semantic_vocabulary::{IntegerMathLiteral, IntegerValue};
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedIntegerMathExpression {
    pub terms: Arena<CheckedIntegerMathTerm>,
    pub root: Handle<CheckedIntegerMathTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedIntegerMathTerm {
    Embed {
        primitive_type: PrimitiveType,
        operand: CheckedScalarExpression,
    },
    Literal(IntegerMathLiteral),
    Add {
        left: Handle<Self>,
        right: Handle<Self>,
    },
    Subtract {
        left: Handle<Self>,
        right: Handle<Self>,
    },
    Multiply {
        left: Handle<Self>,
        right: Handle<Self>,
    },
}

impl Default for CheckedIntegerMathTerm {
    fn default() -> Self {
        Self::Literal(IntegerMathLiteral::from_integer_value(
            IntegerValue::Unsigned(0),
        ))
    }
}

impl CheckedIntegerMathExpression {
    /// Invalid or cyclic terms do not become an executable-safe condition.
    pub(super) fn contains_trapping_operation(&self) -> bool {
        let mut pending = vec![self.root];
        let mut remaining = 4096usize;
        while let Some(handle) = pending.pop() {
            if remaining == 0 || !self.terms.is_valid(handle) {
                return true;
            }
            remaining -= 1;
            match self.terms.get(handle) {
                CheckedIntegerMathTerm::Embed { operand, .. } => {
                    if operand.contains_trapping_operation() {
                        return true;
                    }
                }
                CheckedIntegerMathTerm::Literal(_) => {}
                CheckedIntegerMathTerm::Add { left, right }
                | CheckedIntegerMathTerm::Subtract { left, right }
                | CheckedIntegerMathTerm::Multiply { left, right } => {
                    pending.push(*right);
                    pending.push(*left);
                }
            }
        }
        false
    }
}
