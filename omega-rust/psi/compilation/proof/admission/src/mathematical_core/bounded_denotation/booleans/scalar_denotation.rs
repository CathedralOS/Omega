//! Denote Boolean source values with one ordered scalar continuation.
//!
//! Pending Not/Equal operands live on the heap, while the produced terms stay
//! in the existing arena. This is the original Boolean denotation, including
//! neutral operands: no closed-value shortcut or new decision axiom. Equality's
//! shared function is published only after both original operands complete.

use super::*;

enum Pending<'source> {
    Not,
    EqualLeft { right: &'source ScalarTerm },
    EqualRight { left: TermHandle },
}

pub(in super::super) fn denote(
    denotation: &mut Denotation,
    term: &ScalarTerm,
) -> Result<TermHandle, BoundedDenotationError> {
    let mut pending = Vec::new();
    let mut current = term;
    'enter: loop {
        let mut value = match current {
            ScalarTerm::Boolean(false) => denotation.two_zero,
            ScalarTerm::Boolean(true) => denotation.two_one,
            ScalarTerm::BooleanNot { operand } => {
                pending.push(Pending::Not);
                current = operand;
                continue 'enter;
            }
            ScalarTerm::BooleanEqual { left, right } => {
                pending.push(Pending::EqualLeft { right });
                current = left;
                continue 'enter;
            }
            _ => denotation.scalar_leaf(current)?,
        };
        loop {
            match pending.pop() {
                Some(Pending::Not) => {
                    value = denotation.boolean_case(value, denotation.two_one, denotation.two_zero);
                }
                Some(Pending::EqualLeft { right }) => {
                    pending.push(Pending::EqualRight { left: value });
                    current = right;
                    continue 'enter;
                }
                Some(Pending::EqualRight { left }) => {
                    if !denotation.boolean_equality.is_valid() {
                        denotation.boolean_equality = equality_function(
                            &mut denotation.arena,
                            denotation.two,
                            denotation.two_zero,
                            denotation.two_one,
                        );
                    }
                    value = denotation
                        .arena
                        .apply_binary(denotation.boolean_equality, left, value);
                }
                None => return Ok(value),
            }
        }
    }
}
