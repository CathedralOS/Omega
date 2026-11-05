//! Simplify zero addends using existing cancellation and subtraction laws.
//! Neither identity below adds a trusted arithmetic law to the signature.

use super::*;
use numerics::bignum::BigInt;

impl Denotation {
    fn source_add_change_right(
        &mut self,
        left: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let add = self.add_operation()?;
        let operation = self.constant(add);
        let function = self.source_apply(operation, left);
        self.source_congruence(function, from, to, proof)
    }
    fn source_add_change_left(
        &mut self,
        right: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let add = self.add_operation()?;
        let operation = self.constant(add);
        let variable = self.arena.insert(Term::Variable(0));
        let partial = self.source_apply(operation, variable);
        let body = self.source_apply(partial, right);
        let function = self.arena.insert(Term::Lambda {
            domain: integer,
            body,
        });
        self.source_congruence(function, from, to, proof)
    }
    fn source_add_zero_right(
        &mut self,
        value: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let zero = self.math_literal(BigInt::from_i128(0))?;
        let difference = self.subtract_terms(value, zero)?;
        let original = self.add_terms(value, zero)?;
        let cancelled = self.add_terms(difference, zero)?;
        let subtraction = self.source_subtract_zero(value)?;
        let replacement = self.source_add_change_left(zero, difference, value, subtraction)?;
        let replacement = self.symmetry(integer, cancelled, original, replacement);
        let cancellation = self.source_add_cancel_subtract(value, zero)?;
        Ok(self.transitivity(
            integer,
            original,
            cancelled,
            value,
            replacement,
            cancellation,
        ))
    }
    fn source_add_zero_left(
        &mut self,
        value: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let zero = self.math_literal(BigInt::from_i128(0))?;
        let difference = self.subtract_terms(value, value)?;
        let original = self.add_terms(zero, value)?;
        let cancelled = self.add_terms(difference, value)?;
        let subtraction = self.source_subtract_self(value)?;
        let replacement = self.source_add_change_left(value, difference, zero, subtraction)?;
        let replacement = self.symmetry(integer, cancelled, original, replacement);
        let cancellation = self.source_add_cancel_subtract(value, value)?;
        Ok(self.transitivity(
            integer,
            original,
            cancelled,
            value,
            replacement,
            cancellation,
        ))
    }
    pub(super) fn source_reduced_sum(
        &mut self,
        original: TermHandle,
        depth: usize,
    ) -> Result<NormalInteger, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let add = self.add_operation()?;
        let Some((_, left, right)) = self
            .source_binary(original)?
            .filter(|(operation, _, _)| *operation == add)
        else {
            let integer = self.integer_constant()?;
            let equation = self.arena.insert(Term::Refl {
                ty: integer,
                value: original,
            });
            return Ok(NormalInteger {
                term: original,
                equation,
            });
        };
        let l = self.source_reduced_sum(left, depth + 1)?;
        let r = self.source_reduced_sum(right, depth + 1)?;
        let first = self.source_add_change_left(right, left, l.term, l.equation)?;
        let middle = self.add_terms(l.term, right)?;
        let second = self.source_add_change_right(l.term, right, r.term, r.equation)?;
        let normalized = self.add_terms(l.term, r.term)?;
        let integer = self.integer_constant()?;
        let prefix = self.transitivity(integer, original, middle, normalized, first, second);
        let reduction = if self
            .source_integer_value(r.term, 0)?
            .is_some_and(|(value, _, _)| value == BigInt::from_i128(0))
        {
            NormalInteger {
                term: l.term,
                equation: self.source_add_zero_right(l.term)?,
            }
        } else if self
            .source_integer_value(l.term, 0)?
            .is_some_and(|(value, _, _)| value == BigInt::from_i128(0))
        {
            NormalInteger {
                term: r.term,
                equation: self.source_add_zero_left(r.term)?,
            }
        } else if let Some((_, term, equation)) = self.source_integer_value(normalized, 0)? {
            NormalInteger { term, equation }
        } else {
            let equation = self.arena.insert(Term::Refl {
                ty: integer,
                value: normalized,
            });
            NormalInteger {
                term: normalized,
                equation,
            }
        };
        let equation = self.transitivity(
            integer,
            original,
            normalized,
            reduction.term,
            prefix,
            reduction.equation,
        );
        Ok(NormalInteger {
            term: reduction.term,
            equation,
        })
    }
}
