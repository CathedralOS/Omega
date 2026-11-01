//! Substitution needs identity elimination, not an assumed implication for
//! each arrangement of saved values. Expand the supplied value equations on
//! both statements, retaining each exact replacement context. Transport the
//! premise forward, then the goal backward through those same contexts.
//!
//! This is a proof producer, not another equality decision procedure. The
//! bounded relation has already checked the equations; the common kernel
//! checks every generated J. Opaque operations remain opaque, and arithmetic
//! normalization beyond the compositional denotation still needs its own law.

use super::{
    BoundedDenotationError, Denotation, IntegerLaw, IntegerRelation, MAX_ELABORATION_NODES,
    integer_value_of,
};
use super::{ProofNode, Proposition, Term, TermHandle};
use crate::mathematical_core::substitution::shift;

mod coercion;
mod inhabitants;

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
    /// Rewrite a source goal through explicit, lambda-bound identities and
    /// transport a derived normal goal back with the same J contexts. Closed
    /// integer value parameters are rewrite variables; no proposition is
    /// installed as a signature assumption by this producer.
    pub(super) fn source_rewrite_proof(
        &mut self,
        goal: TermHandle,
        identities: &[(TermHandle, TermHandle)],
        solve: impl FnOnce(&mut Self, TermHandle) -> Result<Option<TermHandle>, BoundedDenotationError>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let mut equations = Vec::new();
        for &(statement, evidence) in identities {
            let Some((carrier, left, right)) = self.identity_parts(statement) else {
                continue;
            };
            if !self.arena.structurally_equal(carrier, integer) {
                continue;
            }
            let parameter = |term| match self.arena.get(term) {
                Term::Constant { declaration, .. } => self
                    .declarations
                    .get(declaration as usize)
                    .is_some_and(|declaration| declaration.body.is_none()),
                _ => false,
            };
            let (from, to, evidence) = if parameter(left) {
                (left, right, evidence)
            } else if parameter(right) {
                (right, left, self.symmetry(carrier, left, right, evidence))
            } else {
                continue;
            };
            if from != to
                && !equations
                    .iter()
                    .any(|equation: &Equation| equation.from == from)
            {
                equations.push(Equation {
                    carrier,
                    from,
                    to,
                    evidence,
                });
            }
        }
        let mut remaining = MAX_ELABORATION_NODES;
        let Some((normal, replacements)) =
            self.expand_equations(goal, &equations, &mut remaining)?
        else {
            return Ok(None);
        };
        let Some(mut evidence) = solve(self, normal)? else {
            return Ok(None);
        };
        for replacement in replacements.into_iter().rev() {
            evidence = self.replace_evidence(replacement, evidence, true);
        }
        Ok(Some(evidence))
    }

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
        // With no cited replacement on either side the statement pair is
        // already its normalized form — `oriented_evidence` still decides
        // whether an unused-equation or connective-normalized crossing
        // derives without an axiom.
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
                let Some(oriented) =
                    self.oriented_evidence(normal_premise, normal_goal, evidence)?
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
    /// analysis on each neutral atom. A goal whose truth is closed-decidable
    /// — a reflexive `Id`, a decided `IntLt`/`IntLe`, or the connective
    /// denotation of a Truth-normalized proposition — is inhabited outright;
    /// a premise whose falsity is closed-decidable refutes to `Empty` and
    /// eliminates into the goal, the `Id Two`/`Int` carrier crossing.
    /// A difference that is only the `connective()` normalization's
    /// sorted, deduplicated or flattened arrangement — permuted or nested
    /// `Σ` conjunctions, retagged disjunction families, dropped
    /// `Truth`/`Falsehood` children — derives through
    /// the coercion driver's connective fallback with projections, pairs and `caseTwo`
    /// eliminations rather than an axiom.
    /// `Ok(None)` keeps the explicit rule-instance route for any difference
    /// outside these shapes — a dependent codomain, an unmatched family, an
    /// open relation, or endpoints that are neither a single swap nor a
    /// checked Boolean identity.
    pub(super) fn oriented_evidence(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if self.arena.structurally_equal(premise, goal) {
            return Ok(Some(evidence));
        }
        let mut remaining = MAX_ELABORATION_NODES;
        self.oriented_coercion(premise, goal, evidence, &mut remaining)
    }

    /// An inhabitant of a denoted type whose truth is closed-decidable:
    /// `refl` for a reflexive `Id`; a decided `IntLt` by the shared
    /// numeral-order derivation; a decided `IntLe` through `lt_le` or
    /// `eq_le`; a non-dependent `Σ` pairs component inhabitants, the
    /// `Σ(t : Two). caseTwo(M, d₀, rest, t)` disjunction denotation tags
    /// whichever branch carries one, and a non-dependent `Π` abstracts
    /// it. `Ok(None)` keeps the caller's fallback for an open or
    /// non-reflexive relation and every other shape — the term is only
    /// ever built when the kernel would accept it.
    pub(super) fn closed_inhabitant(
        &mut self,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        inhabitants::construct(self, goal)
    }

    /// `goal`-typed evidence out of a denoted premise whose falsity is
    /// closed-decidable. `Id Two` between the two constructors eliminates
    /// through `J`'s discriminant — landing on the goal itself, since the
    /// strict `Empty` cannot be a `caseTwo` branch; a closed
    /// `Id Int`/`IntLt`/`IntLe` decided false composes the numeral-order
    /// derivation with the substitution and transitivity roster laws,
    /// landing on fixed strict irreflexivity and `EmptyElim`.
    /// `Ok(None)` keeps the caller's fallback for an open, true or
    /// undecided relation and every other shape. The coercion continuation
    /// owns conjunction projection search and refuses dependent codomains
    /// before invoking this atomic relation producer.
    fn closed_relation_refutation(
        &mut self,
        premise: TermHandle,
        evidence: TermHandle,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if let Some((ty, left, right)) = self.identity_parts(premise) {
            if self.arena.structurally_equal(ty, self.two) {
                // `Id Two` over distinct constructors is a false Boolean
                // identity — the `J` discriminant lands on the goal.
                let premise_left_literal = if self.arena.structurally_equal(left, self.two_one) {
                    true
                } else if self.arena.structurally_equal(left, self.two_zero) {
                    false
                } else {
                    return Ok(None);
                };
                let right_is_other = if premise_left_literal {
                    self.arena.structurally_equal(right, self.two_zero)
                } else {
                    self.arena.structurally_equal(right, self.two_one)
                };
                if !right_is_other {
                    return Ok(None);
                }
                return Ok(Some(self.two_identity_discriminant(
                    left,
                    right,
                    goal,
                    premise_left_literal,
                    evidence,
                )));
            }
            // `Id Int a b` decided false: order the numerals, substitute
            // the identity across, and strict irreflexivity lands on
            // `Empty`. An equal closed pair is a true relation — and an
            // open or non-`Int` endpoint has no decided value at all.
            let (Some(left_value), Some(right_value)) =
                (self.closed_math_value(left), self.closed_math_value(right))
            else {
                return Ok(None);
            };
            if left_value == right_value {
                return Ok(None);
            }
            let (low, high, low_value, high_value, equality) = if left_value < right_value {
                (left, right, left_value, right_value, evidence)
            } else {
                (
                    right,
                    left,
                    right_value,
                    left_value,
                    self.symmetry(ty, left, right, evidence),
                )
            };
            let (Some(low_value), Some(high_value)) =
                (integer_value_of(&low_value), integer_value_of(&high_value))
            else {
                return Ok(None);
            };
            let strict = self.literal_order(low_value, high_value)?;
            let looped = self.integer_law_application(
                IntegerLaw::LessThanSubstituteLeft,
                &[low, high, high, equality, strict],
            )?;
            let absurd = self.irreflexive(high, looped)?;
            return Ok(Some(self.arena.insert(Term::EmptyElim {
                ty: goal,
                scrutinee: absurd,
            })));
        }
        match self.integer_relation(premise) {
            Some(IntegerRelation::LessThan { left, right }) => {
                let (Some(left_value), Some(right_value)) =
                    (self.closed_math_value(left), self.closed_math_value(right))
                else {
                    return Ok(None);
                };
                if left_value < right_value {
                    return Ok(None);
                }
                let absurd = if left_value == right_value {
                    self.irreflexive(left, evidence)?
                } else {
                    let (Some(right_value), Some(left_value)) = (
                        integer_value_of(&right_value),
                        integer_value_of(&left_value),
                    ) else {
                        return Ok(None);
                    };
                    let backwards = self.literal_order(right_value, left_value)?;
                    let looped = self.integer_law_application(
                        IntegerLaw::LessThanTransitivity,
                        &[left, right, left, evidence, backwards],
                    )?;
                    self.irreflexive(left, looped)?
                };
                Ok(Some(self.arena.insert(Term::EmptyElim {
                    ty: goal,
                    scrutinee: absurd,
                })))
            }
            Some(IntegerRelation::LessOrEqual { left, right }) => {
                let (Some(left_value), Some(right_value)) =
                    (self.closed_math_value(left), self.closed_math_value(right))
                else {
                    return Ok(None);
                };
                if left_value <= right_value {
                    return Ok(None);
                }
                let (Some(right_value), Some(left_value)) = (
                    integer_value_of(&right_value),
                    integer_value_of(&left_value),
                ) else {
                    return Ok(None);
                };
                let backwards = self.literal_order(right_value, left_value)?;
                let looped = self.integer_law_application(
                    IntegerLaw::LessOrEqualLessThanTransitivity,
                    &[left, right, left, evidence, backwards],
                )?;
                let absurd = self.irreflexive(left, looped)?;
                Ok(Some(self.arena.insert(Term::EmptyElim {
                    ty: goal,
                    scrutinee: absurd,
                })))
            }
            _ => Ok(None),
        }
    }

    /// One oriented coercion question and its ordered alternatives. All
    /// recursive transitions share the same continuation owner and allowance;
    /// an exhausted allowance declines, never accepts or rejects a judgment.
    fn oriented_coercion(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
        remaining: &mut u64,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        coercion::construct(self, premise, goal, evidence, remaining)
    }

    /// The `Σ(t : Two). caseTwo(M, d₀, rest, t)` disjunction denotation's
    /// family pieces `(M, d₀, rest)`, or `None` for any other term —
    /// including a `Σ` whose first component is not `Two` (an ordinary
    /// dependent pair) and a `caseTwo` payload that names the tag binder,
    /// which could not rebind cleanly under the eliminator's motive.
    fn tagged_sum_parts(&self, term: TermHandle) -> Option<(TermHandle, TermHandle, TermHandle)> {
        let Term::Sigma { domain, codomain } = self.arena.get(term) else {
            return None;
        };
        if !self.arena.structurally_equal(domain, self.two) {
            return None;
        }
        let Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } = self.arena.get(codomain)
        else {
            return None;
        };
        if !matches!(self.arena.get(scrutinee), Term::Variable(0)) {
            return None;
        }
        if [motive, zero_branch, one_branch]
            .into_iter()
            .any(|payload| self.occurs_free(payload, 0))
        {
            return None;
        }
        Some((motive, zero_branch, one_branch))
    }

    /// Whether `Variable(index)` — measured from this term's own root —
    /// occurs free in the term; each `Pi`/`Sigma`/`Lambda` body raises the
    /// index by one. Used to decline codomains that depend on their
    /// binder, which this producer's propositional connectives never
    /// generate.
    pub(super) fn occurs_free(&self, term: TermHandle, index: u32) -> bool {
        let mut pending = vec![(term, index)];
        while let Some((term, index)) = pending.pop() {
            let node = self.arena.node(term);
            if let Term::Variable(position) = node {
                if *position == index {
                    return true;
                }
                continue;
            }
            // The term layout owns binder counts for every child. Reverse
            // push order preserves the former left-to-right short circuit;
            // an overflowing binder index remains ineligible as before.
            for (child, binders) in node.subterms().iter().rev() {
                if let Some(nested) = index.checked_add(binders) {
                    pending.push((child, nested));
                }
            }
        }
        false
    }
}
