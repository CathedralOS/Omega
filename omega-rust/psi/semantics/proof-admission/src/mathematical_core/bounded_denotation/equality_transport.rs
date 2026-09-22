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
                // differently after substitution. Its symmetry is itself J, and
                // the same reversal can sit inside a connective's nested `Id`.
                let Some(oriented) = self.oriented_evidence(normal_premise, normal_goal, evidence)
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

    /// `e : ⟦P⟧` re-presented as `⟦Q⟧` when the licensed premise and goal
    /// differ only by identity endpoint orientation, possibly nested inside
    /// connectives — each differing `Id` gets a `sym` `J` at exactly its
    /// position rather than a rule-instance axiom for the whole judgment.
    /// `Pi` domains coerce contravariantly, `Sigma` pairs covariantly, and a
    /// `Two`-indexed family selects the per-branch coercion by `caseTwo`, so
    /// conjunction, disjunction and implication positions are all covered.
    /// `Id Two` endpoints differing by open `not`/`equal` compositions are
    /// not a swap — they are a Boolean identity decided by `caseTwo` case
    /// analysis on each neutral atom. `None` keeps the explicit rule-instance
    /// route for any difference outside these shapes — a dependent codomain,
    /// an unmatched family, or endpoints that are neither a single swap nor
    /// a checked Boolean identity.
    pub(super) fn oriented_evidence(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(premise, goal) {
            return Some(evidence);
        }
        self.oriented_coercion(premise, goal, evidence)
    }

    fn oriented_coercion(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(premise, goal) {
            return Some(evidence);
        }
        let from = self.arena.get(premise);
        let to = self.arena.get(goal);
        match (from.clone(), to.clone()) {
            (
                Term::Id { ty, left, right },
                Term::Id {
                    ty: goal_ty,
                    left: goal_left,
                    right: goal_right,
                },
            ) => {
                if self.arena.structurally_equal(ty, goal_ty)
                    && self.arena.structurally_equal(goal_left, right)
                    && self.arena.structurally_equal(goal_right, left)
                {
                    return Some(self.symmetry(ty, left, right, evidence));
                }
                // Two `Id Two` identities can also differ by open
                // `not`/`equal` compositions over Boolean atoms — a
                // Boolean identity the kernel itself decides by `caseTwo`
                // case analysis on each atom rather than an instance
                // axiom for the whole judgment.
                self.boolean_identity_transport(premise, goal, evidence)
            }
            (
                Term::Pi { domain, codomain },
                Term::Pi {
                    domain: goal_domain,
                    codomain: goal_codomain,
                },
            ) => {
                // `λ(h : ⟦goal premise⟧). ↑(evidence (h : ⟦premise⟧))`:
                // the domain coerces contravariantly, the codomain
                // covariantly. A codomain that names its own binder is a
                // dependent function type — outside this producer's
                // propositional `Pi`s.
                if self.occurs_free(codomain, 0) || self.occurs_free(goal_codomain, 0) {
                    return None;
                }
                let hypothesis = self.arena.insert(Term::Variable(0));
                let argument = self.oriented_coercion(goal_domain, domain, hypothesis)?;
                let function = shift(&mut self.arena, evidence, 0, 1);
                let applied = self.arena.insert(Term::Apply { function, argument });
                let body = self.oriented_coercion(codomain, goal_codomain, applied)?;
                Some(self.arena.insert(Term::Lambda {
                    domain: goal_domain,
                    body,
                }))
            }
            (
                Term::Sigma { domain, codomain },
                Term::Sigma {
                    domain: goal_domain,
                    codomain: goal_codomain,
                },
            ) => {
                // `⟨c↑(fst e), c↑(snd e)⟩` — the pair evidence destructures
                // directly; each projection coerces covariantly toward the
                // goal's domain and codomain.
                let fst = self.arena.insert(Term::Fst { pair: evidence });
                let first = self.oriented_coercion(domain, goal_domain, fst)?;
                // The disjunction denotation is a `Two`-indexed family
                // `Σ(t : Two). caseTwo(M, d₀, rest, t)`; coercing its second
                // projections needs the eliminator that picks the branch
                // coercion under the same tag. A codomain without that
                // scrutinee is an ordinary non-dependent pair type.
                let second = match (self.arena.get(codomain), self.arena.get(goal_codomain)) {
                    (
                        Term::CaseTwo {
                            motive,
                            zero_branch: zero,
                            one_branch: one,
                            scrutinee,
                        },
                        Term::CaseTwo {
                            motive: goal_motive,
                            zero_branch: goal_zero,
                            one_branch: goal_one,
                            scrutinee: goal_scrutinee,
                        },
                    ) if self.arena.structurally_equal(motive, goal_motive)
                        && self.arena.structurally_equal(domain, goal_domain)
                        && matches!(self.arena.get(scrutinee), Term::Variable(0))
                        && matches!(self.arena.get(goal_scrutinee), Term::Variable(0))
                        // Arm payloads move to a different binder's scope;
                        // a payload naming the Σ binder itself would
                        // rebind — only binder-closed payloads carry over.
                        && [motive, zero, one, goal_zero, goal_one]
                            .into_iter()
                            .all(|payload| !self.occurs_free(payload, 0)) =>
                    {
                        self.branch_selected_coercion(
                            motive, zero, one, goal_zero, goal_one, evidence,
                        )?
                    }
                    _ => {
                        if self.occurs_free(codomain, 0) || self.occurs_free(goal_codomain, 0) {
                            return None;
                        }
                        let snd = self.arena.insert(Term::Snd { pair: evidence });
                        self.oriented_coercion(codomain, goal_codomain, snd)?
                    }
                };
                Some(self.arena.insert(Term::Pair { first, second }))
            }
            _ => None,
        }
    }

    /// `caseTwo(λ(t : Two). Π(_ : ⟦F t⟧). ⟦G t⟧, λ(h : d₀). …, λ(h :
    /// rest). …, fst p) (snd p)` — the coercion of a disjunction pair's
    /// second projection, where the two `Sigma` codomains are the tagged
    /// families `F t = caseTwo(M, d₀, rest, t)` and `G t = caseTwo(M, d₀',
    /// rest', t)` differing only by `Id` orientation inside the branch
    /// payloads. The branches' own annotations name the reduced family
    /// arms — the same proposition types the eliminator's motive yields.
    fn branch_selected_coercion(
        &mut self,
        motive: TermHandle,
        zero: TermHandle,
        one: TermHandle,
        goal_zero: TermHandle,
        goal_one: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        let tag = self.arena.insert(Term::Variable(0));
        let premise_family = self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch: zero,
            one_branch: one,
            scrutinee: tag,
        });
        // The Pi's codomain sits under one extra binder — the family
        // mention `t` and every captured branch payload shift by one.
        let shifted_tag = self.arena.insert(Term::Variable(1));
        let shifted_goal_zero = shift(&mut self.arena, goal_zero, 0, 1);
        let shifted_goal_one = shift(&mut self.arena, goal_one, 0, 1);
        let shifted_motive = shift(&mut self.arena, motive, 0, 1);
        let goal_family = self.arena.insert(Term::CaseTwo {
            motive: shifted_motive,
            zero_branch: shifted_goal_zero,
            one_branch: shifted_goal_one,
            scrutinee: shifted_tag,
        });
        let coercion_body = self.arena.insert(Term::Pi {
            domain: premise_family,
            codomain: goal_family,
        });
        let coercion_motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: coercion_body,
        });
        let hypothesis = self.arena.insert(Term::Variable(0));
        let zero_coerced = self.oriented_coercion(zero, goal_zero, hypothesis)?;
        let zero_branch = self.arena.insert(Term::Lambda {
            domain: zero,
            body: zero_coerced,
        });
        let hypothesis = self.arena.insert(Term::Variable(0));
        let one_coerced = self.oriented_coercion(one, goal_one, hypothesis)?;
        let one_branch = self.arena.insert(Term::Lambda {
            domain: one,
            body: one_coerced,
        });
        let scrutinee = self.arena.insert(Term::Fst { pair: evidence });
        let eliminator = self.arena.insert(Term::CaseTwo {
            motive: coercion_motive,
            zero_branch,
            one_branch,
            scrutinee,
        });
        let argument = self.arena.insert(Term::Snd { pair: evidence });
        Some(self.arena.insert(Term::Apply {
            function: eliminator,
            argument,
        }))
    }

    /// Whether `Variable(index)` — measured from this term's own root —
    /// occurs free in the term; each `Pi`/`Sigma`/`Lambda` body raises the
    /// index by one. Used to decline codomains that depend on their
    /// binder, which this producer's propositional connectives never
    /// generate.
    fn occurs_free(&self, term: TermHandle, index: u32) -> bool {
        match self.arena.get(term) {
            Term::Variable(position) => position == index,
            Term::Pi { domain, codomain } | Term::Sigma { domain, codomain } => {
                self.occurs_free(domain, index)
                    || index
                        .checked_add(1)
                        .is_some_and(|nested| self.occurs_free(codomain, nested))
            }
            Term::Lambda { domain, body } => {
                self.occurs_free(domain, index)
                    || index
                        .checked_add(1)
                        .is_some_and(|nested| self.occurs_free(body, nested))
            }
            Term::Apply { function, argument }
            | Term::Pair {
                first: function,
                second: argument,
            } => self.occurs_free(function, index) || self.occurs_free(argument, index),
            Term::Fst { pair } | Term::Snd { pair } => self.occurs_free(pair, index),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                self.occurs_free(motive, index)
                    || self.occurs_free(zero_branch, index)
                    || self.occurs_free(one_branch, index)
                    || self.occurs_free(scrutinee, index)
            }
            Term::Id { ty, left, right } => {
                self.occurs_free(ty, index)
                    || self.occurs_free(left, index)
                    || self.occurs_free(right, index)
            }
            Term::Refl { ty, value } => {
                self.occurs_free(ty, index) || self.occurs_free(value, index)
            }
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => {
                self.occurs_free(motive, index)
                    || self.occurs_free(base, index)
                    || self.occurs_free(endpoint, index)
                    || self.occurs_free(proof, index)
            }
            Term::W { carrier, children } => {
                self.occurs_free(carrier, index) || self.occurs_free(children, index)
            }
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => {
                self.occurs_free(carrier, index)
                    || self.occurs_free(children, index)
                    || self.occurs_free(label, index)
                    || self.occurs_free(function, index)
            }
            Term::IndW { motive, step, tree } => {
                self.occurs_free(motive, index)
                    || self.occurs_free(step, index)
                    || self.occurs_free(tree, index)
            }
            Term::EmptyElim { ty, scrutinee } => {
                self.occurs_free(ty, index) || self.occurs_free(scrutinee, index)
            }
            Term::Squash { ty } | Term::Box { ty } => self.occurs_free(ty, index),
            Term::SquashIntro { ty, value } | Term::BoxIntro { ty, value } => {
                self.occurs_free(ty, index) || self.occurs_free(value, index)
            }
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                self.occurs_free(proposition, index)
                    || self.occurs_free(function, index)
                    || self.occurs_free(scrutinee, index)
            }
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                self.occurs_free(motive, index)
                    || self.occurs_free(body, index)
                    || self.occurs_free(scrutinee, index)
            }
            Term::Dummy
            | Term::Sort(_)
            | Term::Constant { .. }
            | Term::Two
            | Term::TwoZero
            | Term::TwoOne
            | Term::Empty => false,
        }
    }
}
