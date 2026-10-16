use semantic_vocabulary::{ContentTerm, IntegerMathTerm, Proposition, ScalarTerm};

use super::PredicateDenotationError;

pub(super) struct Budget {
    remaining: usize,
}

impl Budget {
    pub(super) fn new() -> Self {
        Self { remaining: 4096 }
    }

    pub(super) fn step(&mut self, depth: usize) -> Result<(), PredicateDenotationError> {
        if depth >= 64 {
            return Err(PredicateDenotationError::ResourceLimitExceeded);
        }
        self.count(1)
    }

    fn count(&mut self, count: usize) -> Result<(), PredicateDenotationError> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or(PredicateDenotationError::ResourceLimitExceeded)?;
        Ok(())
    }

    pub(super) fn proposition(
        &mut self,
        value: &Proposition,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                self.scalar(left, depth + 1)?;
                self.scalar(right, depth + 1)?;
            }
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                self.mathematical(left, depth + 1)?;
                self.mathematical(right, depth + 1)?;
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    self.proposition(child, depth + 1)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.proposition(premise, depth + 1)?;
                self.proposition(conclusion, depth + 1)?;
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                self.count(left.path().len())?;
                self.count(right.path().len())?;
            }
            Proposition::ByteSequenceEqual { left, right } => {
                self.count(left.path().len())?;
                self.count(right.path().len())?;
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                self.count(subject.path().len())?
            }
            Proposition::StructuralIdentity { left, right } => {
                self.count(left.path().len())?;
                self.count(right.path().len())?;
            }
            Proposition::ContentConservation(value) => {
                self.content(value.left(), depth + 1)?;
                self.content(value.right(), depth + 1)?;
            }
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        }
        Ok(())
    }

    fn content(
        &mut self,
        value: &ContentTerm,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            ContentTerm::Projection { subject, .. } => self.count(subject.segments.len())?,
            ContentTerm::Separate(children) => {
                for child in children {
                    self.content(child, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn mathematical(
        &mut self,
        value: &IntegerMathTerm,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        for operand in value.operands() {
            self.mathematical(operand, depth + 1)?;
        }
        Ok(())
    }

    pub(super) fn scalar(
        &mut self,
        value: &ScalarTerm,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            ScalarTerm::Value { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
            ScalarTerm::StructuralRead { path, .. } => {
                self.count(path.len())?;
                for operand in value.operands() {
                    self.scalar(operand, depth + 1)?;
                }
            }
            semantic_vocabulary::scalar_term_predicate_quint_pattern!(path, ..) => {
                self.count(path.len())?
            }
            super::scalar_term_unary_operand!(operand) => self.scalar(operand, depth + 1)?,
            super::scalar_term_binary_operands!(left, right) => {
                self.scalar(left, depth + 1)?;
                self.scalar(right, depth + 1)?;
            }
        }
        Ok(())
    }
}
