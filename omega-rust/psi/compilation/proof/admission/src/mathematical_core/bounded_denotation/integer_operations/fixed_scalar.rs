//! One ordered continuation for fixed-integer scalar operands.
//!
//! Closed values, source-value identities and opaque leaves retain their old
//! denotations. Exact ring operations keep their existing functions, while
//! other operations keep their complete carrier-indexed opaque identities.
//! Children complete left then right, before publishing their parent function.

use semantic_vocabulary::{IntegerMathTerm, ScalarType};

use super::{
    BoundedDenotationError, Denotation, IntegerOperation, ScalarTerm, TermHandle, application,
};

#[derive(Clone, Copy)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Primitive(IntegerOperation),
}

enum Pending<'source> {
    Unary(IntegerOperation),
    Left {
        operation: BinaryOperation,
        right: &'source ScalarTerm,
    },
    Right {
        operation: BinaryOperation,
        left: TermHandle,
    },
}

pub(in super::super) fn denote(
    denotation: &mut Denotation,
    term: &ScalarTerm,
) -> Result<TermHandle, BoundedDenotationError> {
    let mut pending = Vec::new();
    let mut current = term;
    'enter: loop {
        let mut value = if let Some((_, value)) = current.integer_value() {
            denotation.math_term(&IntegerMathTerm::literal(value))?
        } else {
            match current {
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(source_type),
                } => denotation.math_term(&IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                })?,
                ScalarTerm::ExactIntegerAdd { left, right, .. }
                | ScalarTerm::ExactIntegerSubtract { left, right, .. }
                | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                    let operation = match current {
                        ScalarTerm::ExactIntegerAdd { .. } => BinaryOperation::Add,
                        ScalarTerm::ExactIntegerSubtract { .. } => BinaryOperation::Subtract,
                        _ => BinaryOperation::Multiply,
                    };
                    pending.push(Pending::Left { operation, right });
                    current = left;
                    continue 'enter;
                }
                _ => {
                    if let Some(application) = application(current) {
                        match application.right {
                            Some(right) => pending.push(Pending::Left {
                                operation: BinaryOperation::Primitive(application.operation),
                                right,
                            }),
                            None => pending.push(Pending::Unary(application.operation)),
                        }
                        current = application.left;
                        continue 'enter;
                    }
                    let position = denotation.interned_keyed_assumption(
                        |this| &mut this.scalar_integer_terms,
                        current.clone(),
                        |this| this.integer_constant(),
                    )?;
                    denotation.constant(position)
                }
            }
        };
        loop {
            match pending.pop() {
                Some(Pending::Unary(operation)) => {
                    value = denotation.apply_integer_operation(operation, value, None)?;
                }
                Some(Pending::Left { operation, right }) => {
                    pending.push(Pending::Right {
                        operation,
                        left: value,
                    });
                    current = right;
                    continue 'enter;
                }
                Some(Pending::Right { operation, left }) => {
                    value = match operation {
                        BinaryOperation::Add => denotation.add_terms(left, value)?,
                        BinaryOperation::Subtract => denotation.subtract_terms(left, value)?,
                        BinaryOperation::Multiply => denotation.multiply_terms(left, value)?,
                        BinaryOperation::Primitive(operation) => {
                            denotation.apply_integer_operation(operation, left, Some(value))?
                        }
                    };
                }
                None => return Ok(value),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Budget, MathematicalCertificate, Term, verify_mathematical_certificate};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ValueId,
    };

    #[test]
    fn policy_and_carrier_distinctions_are_not_definitional_equalities() {
        let integer = |width| IntegerType::new(IntegerSign::Unsigned, width).unwrap();
        let u8 = integer(8);
        let u16 = integer(16);
        let u32 = integer(32);
        let id = ValueId::new(1).unwrap();
        let value = ScalarTerm::value(id, ScalarType::Integer(u8));
        let one = |ty| ScalarTerm::integer(ty, IntegerValue::Unsigned(1)).unwrap();
        let context =
            PropositionContext::from_value_types([(id, ScalarType::Integer(u8))]).unwrap();
        let pairs = [
            (
                ScalarTerm::WrappingIntegerAdd {
                    scalar_type: u8,
                    left: Box::new(value.clone()),
                    right: Box::new(one(u8)),
                },
                ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: u8,
                    left: Box::new(value.clone()),
                    right: Box::new(one(u8)),
                },
            ),
            (
                ScalarTerm::IntegerWiden {
                    source_type: u8,
                    target_type: u16,
                    operand: Box::new(value.clone()),
                },
                ScalarTerm::IntegerWiden {
                    source_type: u8,
                    target_type: u32,
                    operand: Box::new(value.clone()),
                },
            ),
            (
                ScalarTerm::WrappingIntegerShiftLeft {
                    value_type: u8,
                    count_type: u8,
                    value: Box::new(value.clone()),
                    count: Box::new(one(u8)),
                },
                ScalarTerm::WrappingIntegerShiftLeft {
                    value_type: u8,
                    count_type: u16,
                    value: Box::new(value),
                    count: Box::new(one(u16)),
                },
            ),
        ];
        for (left, right) in pairs {
            for source in [&left, &right] {
                context
                    .validate(&Proposition::Equal(source.clone(), source.clone()))
                    .unwrap();
            }
            let mut denotation = Denotation::new();
            let left = denote(&mut denotation, &left).unwrap();
            let right = denote(&mut denotation, &right).unwrap();
            let ty = denotation.integer_constant().unwrap();
            // Independently form and judge both exact self-identities first.
            // Refusing the crossed question cannot hide a malformed endpoint.
            for value in [left, right] {
                let term = denotation.arena.insert(Term::Refl { ty, value });
                let expected = denotation.arena.insert(Term::Id {
                    ty,
                    left: value,
                    right: value,
                });
                let certificate = MathematicalCertificate::with_signature(
                    denotation.declarations.clone(),
                    Vec::new(),
                    term,
                    expected,
                );
                verify_mathematical_certificate(
                    &mut denotation.arena,
                    &certificate,
                    &mut Budget::default(),
                )
                .unwrap();
            }
            let term = denotation.arena.insert(Term::Refl { ty, value: left });
            let expected = denotation.arena.insert(Term::Id { ty, left, right });
            let certificate = MathematicalCertificate::with_signature(
                denotation.declarations,
                Vec::new(),
                term,
                expected,
            );
            assert!(
                verify_mathematical_certificate(
                    &mut denotation.arena,
                    &certificate,
                    &mut Budget::default()
                )
                .is_err(),
                "different policies, conversion carriers and shift-count carriers need their own evidence"
            );
        }
    }
}
