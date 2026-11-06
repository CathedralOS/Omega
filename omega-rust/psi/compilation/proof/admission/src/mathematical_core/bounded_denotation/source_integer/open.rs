//! Symbolic source arithmetic is a proof producer over an explicit predicate
//! telescope. Its algebraic rewrites cite fixed universal arithmetic laws;
//! source predicates and recursive hypotheses are lambda-bound evidence.

use super::super::{BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation};
use crate::mathematical_core::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Signature, Term, TermHandle, infer_sort, substitute,
};

mod laws;
mod orders;
mod sums;

use orders::OrderSearch;

#[derive(Clone, Copy)]
struct NormalInteger {
    term: TermHandle,
    equation: TermHandle,
}

impl Denotation {
    fn source_binary(
        &mut self,
        term: TermHandle,
    ) -> Result<Option<(u32, TermHandle, TermHandle)>, BoundedDenotationError> {
        let term = self.source_head(term)?;
        let Term::Apply {
            function,
            argument: right,
        } = self.arena.get(term)
        else {
            return Ok(None);
        };
        let function = self.source_head(function)?;
        let Term::Apply {
            function,
            argument: left,
        } = self.arena.get(function)
        else {
            return Ok(None);
        };
        let Term::Constant { declaration, .. } = self.arena.get(function) else {
            return Ok(None);
        };
        Ok(Some((declaration, left, right)))
    }

    fn source_normal_proposition(
        &mut self,
        term: TermHandle,
        depth: usize,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let head = self.source_head(term)?;
        let node = match self.arena.get(head) {
            Term::Apply { function, argument } => Term::Apply {
                function: self.source_normal_proposition(function, depth + 1)?,
                argument: self.source_normal_proposition(argument, depth + 1)?,
            },
            Term::Id { ty, left, right } => Term::Id {
                ty: self.source_normal_proposition(ty, depth + 1)?,
                left: self.source_normal_proposition(left, depth + 1)?,
                right: self.source_normal_proposition(right, depth + 1)?,
            },
            Term::Sigma { domain, codomain } => Term::Sigma {
                domain: self.source_normal_proposition(domain, depth + 1)?,
                codomain: self.source_normal_proposition(codomain, depth + 1)?,
            },
            Term::Pi { domain, codomain } => Term::Pi {
                domain: self.source_normal_proposition(domain, depth + 1)?,
                codomain: self.source_normal_proposition(codomain, depth + 1)?,
            },
            Term::Lambda { domain, body } => Term::Lambda {
                domain: self.source_normal_proposition(domain, depth + 1)?,
                body: self.source_normal_proposition(body, depth + 1)?,
            },
            Term::Pair { first, second } => Term::Pair {
                first: self.source_normal_proposition(first, depth + 1)?,
                second: self.source_normal_proposition(second, depth + 1)?,
            },
            Term::Fst { pair } => Term::Fst {
                pair: self.source_normal_proposition(pair, depth + 1)?,
            },
            Term::Snd { pair } => Term::Snd {
                pair: self.source_normal_proposition(pair, depth + 1)?,
            },
            _ => return Ok(head),
        };
        Ok(self.arena.insert(node))
    }

    fn source_normal_integer(
        &mut self,
        original: TermHandle,
        depth: usize,
    ) -> Result<NormalInteger, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        if let Some((_, term, equation)) = self.source_integer_value(original, depth + 1)? {
            return Ok(NormalInteger { term, equation });
        }
        let term = self.source_head(original)?;
        let integer = self.integer_constant()?;
        let Some((operation, left, right)) = self.source_binary(term)? else {
            let equation = self.arena.insert(Term::Refl {
                ty: integer,
                value: term,
            });
            return Ok(NormalInteger { term, equation });
        };
        if operation != self.add_operation()?
            && operation != self.subtract_operation()?
            && operation != self.multiply_operation()?
        {
            let equation = self.arena.insert(Term::Refl {
                ty: integer,
                value: term,
            });
            return Ok(NormalInteger { term, equation });
        }
        let l = self.source_normal_integer(left, depth + 1)?;
        let r = self.source_normal_integer(right, depth + 1)?;
        let op = self.constant(operation);
        let variable = self.arena.insert(Term::Variable(0));
        let function = self.source_apply(op, variable);
        let body = self.source_apply(function, right);
        let function = self.arena.insert(Term::Lambda {
            domain: integer,
            body,
        });
        let first = self.source_congruence(function, left, l.term, l.equation)?;
        let function = self.source_apply(op, l.term);
        let middle = self.source_apply(function, right);
        let variable = self.arena.insert(Term::Variable(0));
        let body = self.source_apply(function, variable);
        let function = self.arena.insert(Term::Lambda {
            domain: integer,
            body,
        });
        let second = self.source_congruence(function, right, r.term, r.equation)?;
        let function = self.source_apply(op, l.term);
        let normalized = self.source_apply(function, r.term);
        let equation = self.transitivity(integer, term, middle, normalized, first, second);
        if operation == self.multiply_operation()?
            && let Some((add, a, b)) = self.source_binary(r.term)?
            && add == self.add_operation()?
        {
            let expansion = self.distribute_add(l.term, a, b)?;
            let first = self.multiply_terms(l.term, a)?;
            let second = self.multiply_terms(l.term, b)?;
            let expanded = self.add_terms(first, second)?;
            let rest = self.source_normal_integer(expanded, depth + 1)?;
            let combined =
                self.transitivity(integer, term, normalized, expanded, equation, expansion);
            let equation =
                self.transitivity(integer, term, expanded, rest.term, combined, rest.equation);
            return Ok(NormalInteger {
                term: rest.term,
                equation,
            });
        }
        if operation == self.add_operation()? {
            let canonical = self.source_reduced_sum(normalized, depth + 1)?;
            let equation = self.transitivity(
                integer,
                term,
                normalized,
                canonical.term,
                equation,
                canonical.equation,
            );
            return Ok(NormalInteger {
                term: canonical.term,
                equation,
            });
        }
        Ok(NormalInteger {
            term: normalized,
            equation,
        })
    }

    fn source_flat_premises(
        &mut self,
        proposition: TermHandle,
        evidence: TermHandle,
        premises: &mut Vec<(TermHandle, TermHandle)>,
        depth: usize,
    ) -> Result<(), BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let proposition = self.source_normal_proposition(proposition, 0)?;
        if let Term::Sigma { domain, codomain } = self.arena.get(proposition) {
            // Arithmetic helpers close over value operands. A dependent pair
            // would put its projected evidence variable into those operands.
            if self.occurs_free(codomain, 0) {
                return Err(BoundedDenotationError::Unsupported(
                    "dependent source predicate premises require value abstraction",
                ));
            }
            let first = self.arena.insert(Term::Fst { pair: evidence });
            let second = self.arena.insert(Term::Snd { pair: evidence });
            self.source_flat_premises(domain, first, premises, depth + 1)?;
            let codomain = substitute(&mut self.arena, codomain, first);
            self.source_flat_premises(codomain, second, premises, depth + 1)?;
        } else {
            premises.push((proposition, evidence));
        }
        Ok(())
    }

    fn source_normal_order(
        &mut self,
        proposition: TermHandle,
        evidence: TermHandle,
    ) -> Result<Option<(TermHandle, TermHandle)>, BoundedDenotationError> {
        let Some(relation) = self.integer_relation(proposition) else {
            return Ok(None);
        };
        let (strict, left, right) = match relation {
            IntegerRelation::LessOrEqual { left, right } => (false, left, right),
            IntegerRelation::LessThan { left, right } => (true, left, right),
            _ => return Ok(None),
        };
        let l = self.source_normal_integer(left, 0)?;
        let r = self.source_normal_integer(right, 0)?;
        let first = self.integer_law_application(
            if strict {
                IntegerLaw::LessThanSubstituteLeft
            } else {
                IntegerLaw::LessOrEqualSubstituteLeft
            },
            &[left, l.term, right, l.equation, evidence],
        )?;
        let proof = self.integer_law_application(
            if strict {
                IntegerLaw::LessThanSubstituteRight
            } else {
                IntegerLaw::LessOrEqualSubstituteRight
            },
            &[l.term, right, r.term, r.equation, first],
        )?;
        Ok(Some((
            self.order_term_application(strict, l.term, r.term)?,
            proof,
        )))
    }

    fn source_symbolic_goal(
        &mut self,
        goal: TermHandle,
        search: &mut OrderSearch<'_>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if let Some(IntegerRelation::LessThan { left, right }) = self.integer_relation(goal) {
            let l = self.source_normal_integer(left, 0)?;
            let r = self.source_normal_integer(right, 0)?;
            let Some(proof) = self.source_strict_order(l.term, r.term, search)? else {
                return self.closed_source_predicate(goal, 0);
            };
            let integer = self.integer_constant()?;
            let left_equation = self.symmetry(integer, left, l.term, l.equation);
            let right_equation = self.symmetry(integer, right, r.term, r.equation);
            let proof = self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[l.term, left, r.term, left_equation, proof],
            )?;
            return self
                .integer_law_application(
                    IntegerLaw::LessThanSubstituteRight,
                    &[left, r.term, right, right_equation, proof],
                )
                .map(Some);
        }
        let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(goal) else {
            return self.closed_source_predicate(goal, 0);
        };
        let l = self.source_normal_integer(left, 0)?;
        let r = self.source_normal_integer(right, 0)?;
        let normal = self.order_term_application(false, l.term, r.term)?;
        let Some(evidence) = self.source_composed_order(normal, search)? else {
            return Ok(None);
        };
        let integer = self.integer_constant()?;
        let reverse_l = self.symmetry(integer, left, l.term, l.equation);
        let reverse_r = self.symmetry(integer, right, r.term, r.equation);
        let first = self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteLeft,
            &[l.term, left, r.term, reverse_l, evidence],
        )?;
        self.integer_law_application(
            IntegerLaw::LessOrEqualSubstituteRight,
            &[left, r.term, right, reverse_r, first],
        )
        .map(Some)
    }

    pub(super) fn source_assumption_predicate(
        &mut self,
        goal: TermHandle,
        assumptions: &[TermHandle],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        // Only closed value operands enter this producer. Predicate evidence
        // is bound below; helpers never capture an ambient free value variable.
        let context = Context::empty()
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        for proposition in assumptions.iter().copied().chain(std::iter::once(goal)) {
            infer_sort(
                &mut self.arena,
                &context,
                proposition,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .map_err(BoundedDenotationError::Kernel)?;
        }
        let goal = self.source_normal_proposition(goal, 0)?;
        let mut premises = Vec::new();
        for (position, &assumption) in assumptions.iter().enumerate() {
            let evidence = self
                .arena
                .insert(Term::Variable((assumptions.len() - 1 - position) as u32));
            self.source_flat_premises(assumption, evidence, &mut premises, 0)?;
        }
        let identities = premises
            .iter()
            .copied()
            .filter(|(proposition, _)| self.identity_parts(*proposition).is_some())
            .collect::<Vec<_>>();
        let mut orders = Vec::new();
        let mut laws = Vec::new();
        let mut rewritten_premises = Vec::new();
        for &(proposition, evidence) in &premises {
            let Some((proposition, evidence)) =
                self.source_rewritten_premise(proposition, evidence, &identities)?
            else {
                return Ok(None);
            };
            let proposition = self.source_normal_proposition(proposition, 0)?;
            rewritten_premises.push((proposition, evidence));
            if let Some(order) = self.source_normal_order(proposition, evidence)? {
                orders.push(order);
            }
            if let Some((carrier, left, right)) = self.identity_parts(proposition) {
                let integer = self.integer_constant()?;
                if self.arena.structurally_equal(carrier, integer) {
                    let forward = self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[left, right, evidence],
                    )?;
                    let relation = self.order_term_application(false, left, right)?;
                    if let Some(order) = self.source_normal_order(relation, forward)? {
                        orders.push(order);
                    }
                    let reverse = self.symmetry(integer, left, right, evidence);
                    let reverse = self.integer_law_application(
                        IntegerLaw::EqualityToLessOrEqual,
                        &[right, left, reverse],
                    )?;
                    let relation = self.order_term_application(false, right, left)?;
                    if let Some(order) = self.source_normal_order(relation, reverse)? {
                        orders.push(order);
                    }
                }
            }
            if let Some(law) = self.source_universal_law(proposition, evidence)? {
                laws.push(law);
            }
        }
        let mut search = OrderSearch::new(&orders, &laws);
        let Some(mut proof) = self.source_rewrite_proof(goal, &identities, |this, rewritten| {
            let rewritten = this.source_normal_proposition(rewritten, 0)?;
            for &(proposition, evidence) in &rewritten_premises {
                if this.arena.structurally_equal(proposition, rewritten) {
                    return Ok(Some(evidence));
                }
            }
            if let Term::Id { ty, left, right } = this.arena.get(rewritten)
                && this.arena.structurally_equal(left, right)
            {
                return Ok(Some(this.arena.insert(Term::Refl { ty, value: left })));
            }
            for law in &laws {
                let Some(application) = this.source_law_application_plan(law, rewritten)? else {
                    continue;
                };
                let mut proofs = Vec::new();
                let mut complete = true;
                for &condition in &application.conditions {
                    let Some(proof) = this.source_composed_order(condition, &mut search)? else {
                        complete = false;
                        break;
                    };
                    proofs.push(proof);
                }
                if !complete {
                    continue;
                }
                let mut evidence = application.evidence;
                for argument in application.arguments {
                    let value = match argument {
                        laws::LawArgument::Value(position) => application.values[position],
                        laws::LawArgument::Condition(position) => proofs[position],
                    };
                    evidence = this.source_apply(evidence, value);
                }
                return Ok(Some(evidence));
            }
            this.source_symbolic_goal(rewritten, &mut search)
        })?
        else {
            return Ok(None);
        };
        for &domain in assumptions.iter().rev() {
            proof = self.arena.insert(Term::Lambda {
                domain,
                body: proof,
            });
        }
        Ok(Some(proof))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mathematical_core::{Declaration, check_signature, check_type};
    use crate::{SourceIntegerTheory, TermArena};
    use numerics::bignum::BigInt;

    fn binary(
        arena: &mut TermArena,
        operation: u32,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        let function = arena.insert(Term::Constant {
            declaration: operation,
            levels: vec![],
        });
        let function = arena.insert(Term::Apply {
            function,
            argument: left,
        });
        arena.insert(Term::Apply {
            function,
            argument: right,
        })
    }

    #[test]
    fn dependent_value_premises_are_explicitly_unsupported() {
        let mut arena = TermArena::new();
        let mut declarations = crate::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let integer = arena.insert(Term::Constant {
            declaration: theory.vocabulary().carrier,
            levels: vec![],
        });
        let variable = arena.insert(Term::Variable(0));
        let predicate = arena.insert(Term::Id {
            ty: integer,
            left: variable,
            right: variable,
        });
        let premise = arena.insert(Term::Sigma {
            domain: integer,
            codomain: predicate,
        });
        let zero = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(0))
            .unwrap();
        let goal = arena.insert(Term::Id {
            ty: integer,
            left: zero,
            right: zero,
        });
        assert!(matches!(
            theory.predicate_from_assumptions(&mut arena, &mut declarations, goal, &[premise]),
            Err(BoundedDenotationError::Unsupported(_))
        ));
    }

    #[test]
    fn symbolic_source_step_abstracts_its_predicates_and_induction_hypothesis() {
        let mut arena = TermArena::new();
        let mut declarations = crate::indexed_scheme(&mut arena);
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut declarations).unwrap();
        let vocabulary = theory.vocabulary();
        let integer = arena.insert(Term::Constant {
            declaration: vocabulary.carrier,
            levels: vec![],
        });
        let mut value = || {
            let declaration = declarations.len() as u32;
            declarations.push(Declaration::assumption(0, integer));
            arena.insert(Term::Constant {
                declaration,
                levels: vec![],
            })
        };
        let length = value();
        let total = value();
        let previous = value();
        let previous_total = value();
        let head = value();
        let one = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(1))
            .unwrap();
        let ten = theory
            .literal(&mut arena, &mut declarations, BigInt::from_u64(10))
            .unwrap();
        let head_bound = binary(&mut arena, vocabulary.less_or_equal, head, ten);
        let increment = binary(&mut arena, vocabulary.add, previous, one);
        let length_equation = arena.insert(Term::Id {
            ty: integer,
            left: increment,
            right: length,
        });
        let sum = binary(&mut arena, vocabulary.add, previous_total, head);
        let total_equation = arena.insert(Term::Id {
            ty: integer,
            left: sum,
            right: total,
        });
        let prior_limit = binary(&mut arena, vocabulary.multiply, ten, previous);
        let ih = binary(
            &mut arena,
            vocabulary.less_or_equal,
            previous_total,
            prior_limit,
        );
        let limit = binary(&mut arena, vocabulary.multiply, ten, length);
        let goal = binary(&mut arena, vocabulary.less_or_equal, total, limit);
        let assumptions = [head_bound, length_equation, total_equation, ih];
        let proof = theory
            .predicate_from_assumptions(&mut arena, &mut declarations, goal, &assumptions)
            .unwrap()
            .unwrap();
        assert!(
            theory
                .predicate_from_assumptions(&mut arena, &mut declarations, goal, &assumptions[..3])
                .unwrap()
                .is_none()
        );
        for missing in 0..3 {
            let reduced = assumptions
                .iter()
                .enumerate()
                .filter_map(|(position, &assumption)| (position != missing).then_some(assumption))
                .collect::<Vec<_>>();
            assert!(
                theory
                    .predicate_from_assumptions(&mut arena, &mut declarations, goal, &reduced)
                    .unwrap()
                    .is_none(),
                "missing source premise {missing}"
            );
        }
        assert!(
            !declarations
                .iter()
                .any(|declaration| declaration.body.is_none()
                    && arena.structurally_equal(declaration.ty, goal))
        );
        let mut expected = goal;
        for &domain in assumptions.iter().rev() {
            expected = arena.insert(Term::Pi {
                domain,
                codomain: expected,
            });
        }
        let signature = check_signature(
            &mut arena,
            &declarations,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
        check_type(
            &mut arena,
            &Context::empty().with_signature(signature),
            proof,
            expected,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .unwrap();
    }
}
