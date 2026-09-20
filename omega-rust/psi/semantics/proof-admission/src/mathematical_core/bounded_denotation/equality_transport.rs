//! Substitution needs identity elimination, not an assumed implication for
//! each arrangement of saved values. Expand the supplied value equations on
//! both statements, retaining each exact replacement context. Transport the
//! premise forward, then the goal backward through those same contexts.
//!
//! This is a proof producer, not another equality decision procedure. The
//! bounded relation has already checked the equations; the common kernel
//! checks every generated J. Opaque operations remain opaque, and arithmetic
//! normalization beyond the compositional denotation still needs its own law.

use super::{BoundedDenotationError, Denotation, MAX_ELABORATION_NODES};
use super::{ProofNode, Proposition, Term, TermHandle};
use crate::mathematical_core::substitution::shift;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Equation {
    carrier: TermHandle,
    from: TermHandle,
    to: TermHandle,
    evidence: TermHandle,
}

struct Replacement {
    equation: Equation,
    /// The original statement with exactly the replaced occurrences abstracted
    /// under one binder. Reversal must reuse this context: replacing every
    /// occurrence of the right side would also change unrelated occurrences.
    context: TermHandle,
}

impl Denotation {
    pub(super) fn compositional_transport(
        &mut self,
        premise: &Proposition,
        mut evidence: TermHandle,
        equalities: &[ProofNode],
        equality_evidence: &[TermHandle],
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let truth_premise = matches!(premise, Proposition::Truth);
        let premise = self.denote(premise)?;
        let goal = self.denote(conclusion)?;
        if self.arena.structurally_equal(premise, goal) {
            return Ok(Some(evidence));
        }
        let mut equations = Vec::new();
        for (equality, &proof) in equalities.iter().zip(equality_evidence) {
            let Proposition::Equal(left, _) = &equality.conclusion else {
                return Ok(None);
            };
            let reflexive = self.denote(&Proposition::Equal(left.clone(), left.clone()))?;
            let identity = self.denote(&equality.conclusion)?;
            let (Some((_, from, _)), Some((carrier, first, second))) = (
                self.identity_parts(reflexive),
                self.identity_parts(identity),
            ) else {
                return Ok(None);
            };
            let (to, proof) = if self.arena.structurally_equal(first, from) {
                (second, proof)
            } else if self.arena.structurally_equal(second, from) {
                (first, self.symmetry(carrier, first, second, proof))
            } else {
                return Ok(None);
            };
            if self.arena.structurally_equal(from, to)
                || equations.iter().any(|prior: &Equation| prior.from == from)
            {
                continue;
            }
            // Value leaves are interned, closed constants. Do not reinterpret
            // an arbitrary expression as a new rewrite variable.
            if !matches!(self.arena.get(from), Term::Constant { .. }) {
                return Ok(None);
            }
            equations.push(Equation {
                carrier,
                from,
                to,
                evidence: proof,
            });
        }
        let mut remaining = MAX_ELABORATION_NODES;
        let Some((normal_premise, forward)) =
            self.expand_equations(premise, &equations, &mut remaining)?
        else {
            return Ok(None);
        };
        let Some((normal_goal, backward)) =
            self.expand_equations(goal, &equations, &mut remaining)?
        else {
            return Ok(None);
        };
        if forward.is_empty() && backward.is_empty() {
            return Ok(None);
        }
        let reflexive_goal = self
            .identity_parts(normal_goal)
            .filter(|(_, left, right)| self.arena.structurally_equal(*left, *right));
        if truth_premise && let Some((ty, value, _)) = reflexive_goal {
            // Bounded normalization erases a reflexive Boolean equality to
            // Truth. Its mathematical proof is refl at the expanded value,
            // not transport from the unrelated Id Two zero zero.
            evidence = self.arena.insert(Term::Refl { ty, value });
        } else {
            for replacement in forward {
                evidence = self.replace_evidence(replacement, evidence, false);
            }
            if !self.arena.structurally_equal(normal_premise, normal_goal) {
                // Integer equality canonicalization can orient the two endpoints
                // differently after substitution. Its symmetry is itself J.
                let Some(oriented) = self.symmetry_evidence(normal_premise, normal_goal, evidence)
                else {
                    return Ok(None);
                };
                evidence = oriented;
            }
        }
        for replacement in backward.into_iter().rev() {
            evidence = self.replace_evidence(replacement, evidence, true);
        }
        Ok(Some(evidence))
    }

    fn expand_equations(
        &mut self,
        mut statement: TermHandle,
        equations: &[Equation],
        remaining: &mut u64,
    ) -> Result<Option<(TermHandle, Vec<Replacement>)>, BoundedDenotationError> {
        let mut replacements = Vec::new();
        // Each pass removes at least one level of an acyclic dependency chain,
        // regardless of the order of the supplied equations. The bounded
        // checker rejects reachable cycles; never turn a nonconvergent rewrite
        // into a mathematical judgment.
        for _ in 0..=equations.len() {
            let mut changed = false;
            for &equation in equations {
                let Some((next, context)) =
                    self.replacement_context(statement, equation, 0, remaining)?
                else {
                    return Ok(None);
                };
                if next != statement {
                    replacements.push(Replacement { equation, context });
                    statement = next;
                    changed = true;
                }
            }
            if !changed {
                return Ok(Some((statement, replacements)));
            }
        }
        Ok(None)
    }

    fn replace_evidence(
        &mut self,
        replacement: Replacement,
        evidence: TermHandle,
        reverse: bool,
    ) -> TermHandle {
        let Equation {
            carrier,
            from,
            to,
            evidence: equation,
        } = replacement.equation;
        let (from, to, equation) = if reverse {
            (to, from, self.symmetry(carrier, from, to, equation))
        } else {
            (from, to, equation)
        };
        let endpoint = self.arena.insert(Term::Variable(0));
        let identity = self.arena.insert(Term::Id {
            ty: carrier,
            left: from,
            right: endpoint,
        });
        let body = shift(&mut self.arena, replacement.context, 0, 1);
        let body = self.arena.insert(Term::Lambda {
            domain: identity,
            body,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: carrier,
            body,
        });
        self.arena.insert(Term::IdElim {
            motive,
            base: evidence,
            endpoint: to,
            proof: equation,
        })
    }

    /// Produce both the rewritten statement and its one-hole predicate.
    /// Only constructors emitted by proposition denotation belong here.
    /// Binders retain their own variables; the new predicate binder is outside
    /// them. All equation endpoints are closed declaration terms.
    fn replacement_context(
        &mut self,
        original: TermHandle,
        equation: Equation,
        depth: u32,
        remaining: &mut u64,
    ) -> Result<Option<(TermHandle, TermHandle)>, BoundedDenotationError> {
        // This optional producer must not turn an already supported bounded
        // certificate into a refusal merely because deriving a smaller
        // assumption closure costs more than its construction allowance.
        // Declining retains the explicit rule-instance assumption; it never
        // reports the attempted J derivation as checked.
        let Some(rest) = remaining.checked_sub(1) else {
            return Ok(None);
        };
        *remaining = rest;
        if original == equation.from {
            let variable = self.arena.insert(Term::Variable(depth));
            return Ok(Some((equation.to, variable)));
        }
        macro_rules! child {
            ($term:expr, $depth:expr) => {
                match self.replacement_context($term, equation, $depth, remaining)? {
                    Some(pair) => pair,
                    None => return Ok(None),
                }
            };
        }
        let term = self.arena.get(original);
        let (rewritten, context) = match term.clone() {
            Term::Variable(position) => {
                let shifted = if position >= depth {
                    position
                        .checked_add(1)
                        .ok_or(BoundedDenotationError::DepthLimitExceeded)?
                } else {
                    position
                };
                let context = if shifted == position {
                    original
                } else {
                    self.arena.insert(Term::Variable(shifted))
                };
                return Ok(Some((original, context)));
            }
            Term::Constant { .. } | Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne => {
                return Ok(Some((original, original)));
            }
            Term::Apply { function, argument } => {
                let (function, context_function) = child!(function, depth);
                let (argument, context_argument) = child!(argument, depth);
                (
                    Term::Apply { function, argument },
                    Term::Apply {
                        function: context_function,
                        argument: context_argument,
                    },
                )
            }
            Term::Id { ty, left, right } => {
                let (ty, context_ty) = child!(ty, depth);
                let (left, context_left) = child!(left, depth);
                let (right, context_right) = child!(right, depth);
                (
                    Term::Id { ty, left, right },
                    Term::Id {
                        ty: context_ty,
                        left: context_left,
                        right: context_right,
                    },
                )
            }
            Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                let (domain, context_domain) = child!(domain, depth);
                let nested = depth
                    .checked_add(1)
                    .ok_or(BoundedDenotationError::DepthLimitExceeded)?;
                let (codomain, context_codomain) = child!(codomain, nested);
                if matches!(term, Term::Pi { .. }) {
                    (
                        Term::Pi { domain, codomain },
                        Term::Pi {
                            domain: context_domain,
                            codomain: context_codomain,
                        },
                    )
                } else {
                    (
                        Term::Sigma { domain, codomain },
                        Term::Sigma {
                            domain: context_domain,
                            codomain: context_codomain,
                        },
                    )
                }
            }
            Term::Lambda { domain, body } => {
                let (domain, context_domain) = child!(domain, depth);
                let nested = depth
                    .checked_add(1)
                    .ok_or(BoundedDenotationError::DepthLimitExceeded)?;
                let (body, context_body) = child!(body, nested);
                (
                    Term::Lambda { domain, body },
                    Term::Lambda {
                        domain: context_domain,
                        body: context_body,
                    },
                )
            }
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                let (motive, context_motive) = child!(motive, depth);
                let (zero_branch, context_zero) = child!(zero_branch, depth);
                let (one_branch, context_one) = child!(one_branch, depth);
                let (scrutinee, context_scrutinee) = child!(scrutinee, depth);
                (
                    Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    },
                    Term::CaseTwo {
                        motive: context_motive,
                        zero_branch: context_zero,
                        one_branch: context_one,
                        scrutinee: context_scrutinee,
                    },
                )
            }
            _ => return Ok(None),
        };
        let rewritten = if rewritten == term {
            original
        } else {
            self.arena.insert(rewritten)
        };
        let context = if context == term {
            original
        } else {
            self.arena.insert(context)
        };
        Ok(Some((rewritten, context)))
    }
}
