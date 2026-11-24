//! Charge the complete shape of predicates and their operands before evaluation.

use semantic_vocabulary::{
    ContentTerm, IntegerMathTerm, Proposition, ScalarTerm, scalar_term_predicate_quint_pattern,
};

/// One traversal budget shared by a group of predicates and any intermediate terms.
pub struct PredicateBudget {
    remaining: usize,
}

const NODE_LIMIT: usize = 4096;

impl Default for PredicateBudget {
    fn default() -> Self {
        Self {
            remaining: NODE_LIMIT,
        }
    }
}

impl PredicateBudget {
    pub fn step(&mut self, depth: usize) -> Result<(), PredicateSizeLimit> {
        if depth >= 64 {
            return Err(PredicateSizeLimit);
        }
        self.count(1)
    }

    /// Charge the same work as a previously checked group of predicates.
    pub fn include(&mut self, checked: &Self) -> Result<(), PredicateSizeLimit> {
        self.count(NODE_LIMIT - checked.remaining)
    }

    fn count(&mut self, count: usize) -> Result<(), PredicateSizeLimit> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or(PredicateSizeLimit)?;
        Ok(())
    }

    pub fn proposition(
        &mut self,
        value: &Proposition,
        depth: usize,
    ) -> Result<(), PredicateSizeLimit> {
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
            Proposition::DeclarationIdentity(clause) => {
                self.count(clause.values.len())?;
                for value in &clause.values {
                    match &value.operand {
                        semantic_vocabulary::ClauseOperand::Scalar(term) => self.scalar(term, depth + 1)?,
                        semantic_vocabulary::ClauseOperand::Application { arguments, .. } => self.count(arguments.len())?,
                        _ => {}
                    }
                }
            }
            Proposition::FloatIntegerConversion(clause) => {
                self.scalar(&clause.source, depth + 1)?;
                self.scalar(&clause.result, depth + 1)?;
            }
            Proposition::IeeeFloatIntegerExact { source, .. }
            | Proposition::IntegerIeeeFloatExact { source, .. } => {
                self.scalar(source, depth + 1)?
            }
            Proposition::FloatMeaningEqual(clause) => {
                self.count(clause.values.len())?;
                for value in &clause.values {
                    match value {
                        semantic_vocabulary::FloatMeaningClauseValue::Projection {
                            source, ..
                        } => self.scalar(source, depth + 1)?,
                        semantic_vocabulary::FloatMeaningClauseValue::Application {
                            operands,
                            ..
                        } => {
                            self.count(operands.len())?;
                            for operand in operands {
                                if let semantic_vocabulary::FloatMeaningClauseOperand::Integer(
                                    source,
                                ) = operand
                                {
                                    self.scalar(source, depth + 1)?;
                                }
                            }
                        }
                    }
                }
            }
            Proposition::ContentConservation(value) => {
                self.content(value.left(), depth + 1)?;
                self.content(value.right(), depth + 1)?;
            }
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        }
        Ok(())
    }

    fn content(&mut self, value: &ContentTerm, depth: usize) -> Result<(), PredicateSizeLimit> {
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
    ) -> Result<(), PredicateSizeLimit> {
        self.step(depth)?;
        if let IntegerMathTerm::Embed(term) = value {
            return self.scalar(term, depth + 1);
        }
        for operand in value.operands() {
            self.mathematical(operand, depth + 1)?;
        }
        Ok(())
    }

    pub fn scalar(&mut self, value: &ScalarTerm, depth: usize) -> Result<(), PredicateSizeLimit> {
        self.step(depth)?;
        match value {
            ScalarTerm::DeclarationApplication { arguments, .. } => {
                for argument in arguments {
                    self.scalar(argument, depth + 1)?;
                }
            }
            ScalarTerm::Application { .. }
            | ScalarTerm::Value { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
            ScalarTerm::StructuralRead { path, .. } => {
                self.count(path.len())?;
                for operand in value.operands() {
                    self.scalar(operand, depth + 1)?;
                }
            }
            scalar_term_predicate_quint_pattern!(path, ..) => self.count(path.len())?,
            semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                self.scalar(operand, depth + 1)?
            }
            semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                self.scalar(left, depth + 1)?;
                self.scalar(right, depth + 1)?;
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredicateSizeLimit;

impl std::fmt::Display for PredicateSizeLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("predicate evaluation resource limit exceeded")
    }
}

impl std::error::Error for PredicateSizeLimit {}

pub fn check_predicate_evaluation_size<'input>(
    propositions: impl IntoIterator<Item = &'input Proposition>,
) -> Result<(), PredicateSizeLimit> {
    let mut budget = PredicateBudget::default();
    for proposition in propositions {
        budget.proposition(proposition, 0)?;
    }
    Ok(())
}
