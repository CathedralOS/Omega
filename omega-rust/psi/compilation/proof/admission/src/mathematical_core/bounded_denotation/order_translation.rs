//! A proved mathematical order translated through the existing addition law.
//! No producer-selected rule-instance assumption is introduced.

use super::{BoundedDenotationError, Denotation, IntegerLaw, TermHandle, integer_value_of};
use semantic_vocabulary::{IntegerMathTerm, Proposition};

impl Denotation {
    pub(super) fn mathematical_order_translation(
        &mut self,
        relation: &Proposition,
        addend: &IntegerMathTerm,
        evidence: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Proposition::IntegerMathLessOrEqual(left, right) = relation else {
            return Err(BoundedDenotationError::Certificate(
                crate::ProofError::RulePremiseMismatch("mathematical integer order translation"),
            ));
        };
        let left_term = self.math_term(left)?;
        let right_term = self.math_term(right)?;
        let addend_term = self.math_term(addend)?;
        let mut result =
            self.add_order_translation(left_term, right_term, addend_term, evidence)?;
        let left_sum = self.add_terms(left_term, addend_term)?;
        let right_sum = self.add_terms(right_term, addend_term)?;
        let left_result = self.math_term(&IntegerMathTerm::Add(
            Box::new(left.clone()),
            Box::new(addend.clone()),
        ))?;
        let right_result = self.math_term(&IntegerMathTerm::Add(
            Box::new(right.clone()),
            Box::new(addend.clone()),
        ))?;
        // Closed sums denote their checked numeral rather than an application.
        // Reuse the existing numeral equation and endpoint substitution laws.
        if !self.arena.structurally_equal(left_sum, left_result) {
            let equality = self.translated_numeral_equality(left_term, addend_term, left_result)?;
            result = self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[left_sum, left_result, right_sum, equality, result],
            )?;
        }
        if !self.arena.structurally_equal(right_sum, right_result) {
            let equality =
                self.translated_numeral_equality(right_term, addend_term, right_result)?;
            result = self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[left_result, right_sum, right_result, equality, result],
            )?;
        }
        Ok(result)
    }

    fn translated_numeral_equality(
        &mut self,
        left: TermHandle,
        right: TermHandle,
        result: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let value = |term| {
            self.closed_math_value(term)
                .as_ref()
                .and_then(integer_value_of)
        };
        let (Some(left_value), Some(right_value), Some(result_value)) =
            (value(left), value(right), value(result))
        else {
            return Err(BoundedDenotationError::Unsupported(
                "translated numeral exceeds existing numeral evidence",
            ));
        };
        self.numeral_sum(left_value, right_value, result_value, left, right, result)?
            .ok_or(BoundedDenotationError::Unsupported(
                "translated numeral lacks exact addition evidence",
            ))
    }
}

#[cfg(test)]
mod tests;
