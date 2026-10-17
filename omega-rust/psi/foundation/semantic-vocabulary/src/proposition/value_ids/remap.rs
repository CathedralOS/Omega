//! Value substitution preserves the surrounding proposition and term operators.

use super::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};

enum Pending<'a> {
    Proposition(&'a mut Proposition),
    Scalar(&'a mut ScalarTerm),
    IntegerMath(&'a mut IntegerMathTerm),
}

impl Proposition {
    /// Rewrite value coordinates, including mathematical embeddings, without
    /// changing structural or opaque subjects. The caller owns the namespace
    /// correspondence and must revalidate canonical ordering if it changes.
    pub fn remap_value_ids(&mut self, mut map: impl FnMut(ValueId) -> ValueId) {
        let mut pending = vec![Pending::Proposition(self)];
        while let Some(node) = pending.pop() {
            match node {
                Pending::Proposition(proposition) => match proposition {
                    Self::Truth
                    | Self::Falsehood
                    | Self::Atom(_)
                    | Self::IeeeFloatComparison { .. }
                    | Self::ByteSequenceEqual { .. }
                    | Self::StructuralIdentity { .. }
                    | Self::StructuralCaseMembership { .. }
                    | Self::ContentConservation(_) => {}
                    Self::Equal(left, right)
                    | Self::LessThan(left, right)
                    | Self::LessOrEqual(left, right)
                    | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                        pending.push(Pending::Scalar(right));
                        pending.push(Pending::Scalar(left));
                    }
                    Self::IntegerMathEqual(left, right)
                    | Self::IntegerMathLessThan(left, right)
                    | Self::IntegerMathLessOrEqual(left, right) => {
                        pending.push(Pending::IntegerMath(right));
                        pending.push(Pending::IntegerMath(left));
                    }
                    Self::Conjunction(parts) | Self::Disjunction(parts) => {
                        pending.extend(parts.iter_mut().rev().map(Pending::Proposition));
                    }
                    Self::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(Pending::Proposition(conclusion));
                        pending.push(Pending::Proposition(premise));
                    }
                },
                Pending::IntegerMath(term) => match term {
                    IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                    IntegerMathTerm::IntegerLiteral(_) => {}
                    IntegerMathTerm::MathValue { value, .. } => *value = map(*value),
                    term => {
                        for operand in term.operands_mut().into_iter().rev() {
                            pending.push(Pending::IntegerMath(operand));
                        }
                    }
                },
                Pending::Scalar(term) => match term {
                    ScalarTerm::Value { id, .. } => *id = map(*id),
                    crate::scalar_term_predicate_quint_pattern!(..)
                    | ScalarTerm::Boolean(_)
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::IeeeFloatLiteral(_) => {}
                    _ => {
                        for operand in term.operands_mut().into_iter().rev() {
                            pending.push(Pending::Scalar(operand));
                        }
                    }
                },
            }
        }
    }
}
