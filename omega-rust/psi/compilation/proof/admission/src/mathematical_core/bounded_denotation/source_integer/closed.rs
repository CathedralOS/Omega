//! Closed arithmetic evidence uses the existing numeral equations and fixed
//! order laws. Evaluation chooses a proof to construct; J, numeral laws and
//! ordinary kernel checking establish the original, unreduced proposition.

use super::super::{
    BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation, integer_value_of,
};
use crate::mathematical_core::{
    Budget, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle, substitute,
    weak_head_normalize_retaining,
};
use numerics::bignum::BigInt;

impl Denotation {
    pub(super) fn source_head(
        &mut self,
        term: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let signature = Signature::from_declarations(self.declarations.clone());
        let values = &self.closed_math_values;
        weak_head_normalize_retaining(
            &mut self.arena,
            &signature,
            term,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            &|position| values.contains_key(&position),
        )
        .map_err(BoundedDenotationError::Kernel)
    }

    pub(super) fn source_apply(
        &mut self,
        function: TermHandle,
        argument: TermHandle,
    ) -> TermHandle {
        self.arena.insert(Term::Apply { function, argument })
    }

    /// Congruence of a closed Int function, derived by J. This helper is only
    /// called after both operands have closed numeral derivations.
    pub(super) fn source_congruence(
        &mut self,
        function: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let start = self.source_apply(function, from);
        let y = self.arena.insert(Term::Variable(0));
        let identity = self.arena.insert(Term::Id {
            ty: integer,
            left: from,
            right: y,
        });
        let y = self.arena.insert(Term::Variable(1));
        let end = self.source_apply(function, y);
        let result = self.arena.insert(Term::Id {
            ty: integer,
            left: start,
            right: end,
        });
        let inner = self.arena.insert(Term::Lambda {
            domain: identity,
            body: result,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: integer,
            body: inner,
        });
        let base = self.arena.insert(Term::Refl {
            ty: integer,
            value: start,
        });
        Ok(self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: to,
            proof,
        }))
    }

    pub(super) fn source_integer_value(
        &mut self,
        original: TermHandle,
        depth: usize,
    ) -> Result<Option<(BigInt, TermHandle, TermHandle)>, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let term = self.source_head(original)?;
        let integer = self.integer_constant()?;
        if let Some(value) = self.closed_math_value(term) {
            let proof = self.arena.insert(Term::Refl {
                ty: integer,
                value: term,
            });
            return Ok(Some((value, term, proof)));
        }
        let Term::Apply {
            function,
            argument: right,
        } = self.arena.get(term)
        else {
            return Ok(None);
        };
        let function = self.source_head(function)?;
        let Term::Apply {
            function: operation,
            argument: left,
        } = self.arena.get(function)
        else {
            return Ok(None);
        };
        let Term::Constant { declaration, .. } = self.arena.get(operation) else {
            return Ok(None);
        };
        let kind = if declaration == self.add_operation()? {
            0
        } else if declaration == self.subtract_operation()? {
            1
        } else if declaration == self.multiply_operation()? {
            2
        } else {
            return Ok(None);
        };
        let Some((left_value, left_term, left_proof)) =
            self.source_integer_value(left, depth + 1)?
        else {
            return Ok(None);
        };
        let Some((right_value, right_term, right_proof)) =
            self.source_integer_value(right, depth + 1)?
        else {
            return Ok(None);
        };
        let value = match kind {
            0 => left_value.add(&right_value),
            1 => left_value.sub(&right_value),
            _ => left_value.mul(&right_value),
        };
        let (Some(l), Some(r), Some(v)) = (
            integer_value_of(&left_value),
            integer_value_of(&right_value),
            integer_value_of(&value),
        ) else {
            return Ok(None);
        };
        let result = self.math_literal(value.clone())?;
        let applied = self.source_apply(operation, left_term);
        let normalized = self.source_apply(applied, right_term);
        let equation = match kind {
            0 => self.numeral_sum(l, r, v, left_term, right_term, result)?,
            1 => self.numeral_difference(l, r, v, left_term, right_term, result)?,
            _ => self.numeral_product(l, r, v, left_term, right_term, result)?,
        };
        let Some(equation) = equation else {
            return Ok(None);
        };
        let variable = self.arena.insert(Term::Variable(0));
        let applied = self.source_apply(operation, variable);
        let body = self.source_apply(applied, right);
        let function = self.arena.insert(Term::Lambda {
            domain: integer,
            body,
        });
        let first = self.source_congruence(function, left, left_term, left_proof)?;
        let applied = self.source_apply(operation, left_term);
        let middle = self.source_apply(applied, right);
        let variable = self.arena.insert(Term::Variable(0));
        let body = self.source_apply(applied, variable);
        let function = self.arena.insert(Term::Lambda {
            domain: integer,
            body,
        });
        let second = self.source_congruence(function, right, right_term, right_proof)?;
        let combined = self.transitivity(integer, term, middle, normalized, first, second);
        let proof = self.transitivity(integer, term, normalized, result, combined, equation);
        Ok(Some((value, result, proof)))
    }

    pub(in super::super) fn closed_source_predicate(
        &mut self,
        original: TermHandle,
        depth: usize,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let goal = self.source_head(original)?;
        if let Some((ty, left, right)) = self.identity_parts(goal) {
            if self.arena.structurally_equal(left, right) {
                return Ok(Some(self.arena.insert(Term::Refl { ty, value: left })));
            }
            let Some((lv, lc, lp)) = self.source_integer_value(left, depth + 1)? else {
                return Ok(None);
            };
            let Some((rv, rc, rp)) = self.source_integer_value(right, depth + 1)? else {
                return Ok(None);
            };
            if lv != rv {
                return Ok(None);
            }
            let integer = self.integer_constant()?;
            let reverse = self.symmetry(integer, right, rc, rp);
            return Ok(Some(
                self.transitivity(integer, left, lc, right, lp, reverse),
            ));
        }
        if let Some(relation) = self.integer_relation(goal) {
            let (strict, left, right) = match relation {
                IntegerRelation::LessThan { left, right } => (true, left, right),
                IntegerRelation::LessOrEqual { left, right } => (false, left, right),
                _ => return Ok(None),
            };
            let Some((_, lc, lp)) = self.source_integer_value(left, depth + 1)? else {
                return Ok(None);
            };
            let Some((_, rc, rp)) = self.source_integer_value(right, depth + 1)? else {
                return Ok(None);
            };
            let normalized = self.order_term_application(strict, lc, rc)?;
            let Some(evidence) = self.closed_inhabitant(normalized)? else {
                return Ok(None);
            };
            let integer = self.integer_constant()?;
            let left_reverse = self.symmetry(integer, left, lc, lp);
            let right_reverse = self.symmetry(integer, right, rc, rp);
            let first = self.integer_law_application(
                if strict {
                    IntegerLaw::LessThanSubstituteLeft
                } else {
                    IntegerLaw::LessOrEqualSubstituteLeft
                },
                &[lc, left, rc, left_reverse, evidence],
            )?;
            return self
                .integer_law_application(
                    if strict {
                        IntegerLaw::LessThanSubstituteRight
                    } else {
                        IntegerLaw::LessOrEqualSubstituteRight
                    },
                    &[left, rc, right, right_reverse, first],
                )
                .map(Some);
        }
        match self.arena.get(goal) {
            Term::Sigma { domain, codomain } => {
                let Some(first) = self.closed_source_predicate(domain, depth + 1)? else {
                    return Ok(None);
                };
                let rest = substitute(&mut self.arena, codomain, first);
                let Some(second) = self.closed_source_predicate(rest, depth + 1)? else {
                    return Ok(None);
                };
                Ok(Some(self.arena.insert(Term::Pair { first, second })))
            }
            Term::Box { ty } => {
                let Term::Pi { domain, codomain } = self.arena.get(ty) else {
                    return Ok(None);
                };
                if !matches!(self.arena.get(domain), Term::Empty)
                    || !matches!(self.arena.get(codomain), Term::Empty)
                {
                    return Ok(None);
                }
                let body = self.arena.insert(Term::Variable(0));
                let value = self.arena.insert(Term::Lambda { domain, body });
                Ok(Some(self.arena.insert(Term::BoxIntro { ty, value })))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceIntegerTheory;
    use crate::mathematical_core::{Context, Level, Sort, check_signature, check_type};

    #[test]
    fn source_integer_closed_arithmetic_is_evidence_for_the_exact_goal() {
        let mut arena = crate::TermArena::new();
        let mut declarations = crate::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let vocabulary = theory.vocabulary();
        let seven = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(7))
            .unwrap();
        // Source declarations may append between uses of the shared denoter.
        let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        declarations.push(crate::Declaration::assumption(0, type_zero));
        let three = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(3))
            .unwrap();
        let ten = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(10))
            .unwrap();
        let add = arena.insert(Term::Constant {
            declaration: vocabulary.add,
            levels: vec![],
        });
        let applied = arena.insert(Term::Apply {
            function: add,
            argument: seven,
        });
        let sum = arena.insert(Term::Apply {
            function: applied,
            argument: three,
        });
        let int = arena.insert(Term::Constant {
            declaration: vocabulary.carrier,
            levels: vec![],
        });
        let goal = arena.insert(Term::Id {
            ty: int,
            left: sum,
            right: ten,
        });
        let evidence = theory
            .closed_predicate(&mut arena, &mut declarations, goal)
            .unwrap()
            .unwrap();
        let wrong = arena.insert(Term::Id {
            ty: int,
            left: sum,
            right: seven,
        });
        assert!(
            theory
                .closed_predicate(&mut arena, &mut declarations, wrong)
                .unwrap()
                .is_none()
        );
        let le = arena.insert(Term::Constant {
            declaration: vocabulary.less_or_equal,
            levels: vec![],
        });
        let function = arena.insert(Term::Apply {
            function: le,
            argument: sum,
        });
        let bound = arena.insert(Term::Apply {
            function,
            argument: ten,
        });
        let order = theory
            .closed_predicate(&mut arena, &mut declarations, bound)
            .unwrap()
            .unwrap();
        let signature = check_signature(
            &mut arena,
            &declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        let context = Context::empty().with_signature(signature);
        check_type(
            &mut arena,
            &context,
            evidence,
            goal,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        check_type(
            &mut arena,
            &context,
            order,
            bound,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        assert!(
            check_type(
                &mut arena,
                &context,
                evidence,
                wrong,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS)
            )
            .is_err()
        );
    }
}
