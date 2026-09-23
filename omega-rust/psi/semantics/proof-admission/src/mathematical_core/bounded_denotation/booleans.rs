//! The selected Boolean primitives have the existing Two denotation.
//! Keeping their operands visible lets J transport saved computations into
//! an authored result contract. Treating each expression as an unrelated
//! constant would instead require an assumed implication for every program.
//!
//! BooleanEqual computes a Boolean; proposition equality remains proof-relevant
//! Id. These are already-selected scalar primitives, not authored operators
//! recognized by spelling. No truth-table axiom or kernel rule is added.
//!
//! Two `Id Two` propositions whose endpoints differ by open `not`/`equal`
//! compositions over Boolean atoms — the pairs the bounded checker's
//! Boolean normalization equates without a closed computation — are still
//! decidable evidence: `boolean_identity_transport` builds the conversion
//! `Π(_ : Id Two pl pr). Id Two gl gr` by `caseTwo` case analysis on each
//! neutral Boolean subterm, checking every leaf against the endpoint
//! evaluations itself. An atom's two branches get a `refl` where the goal
//! endpoints agree and a `J`-driven contradiction elimination where the
//! premise endpoints disagree; a branch whose goal fails under a holding
//! premise — or any term outside the fragment — declines to the caller's
//! explicit rule-instance fallback instead of naming the whole conversion
//! an axiom.

use super::{
    BoundedDenotationError, Denotation, MAX_ELABORATION_NODES, ScalarTerm, Term, TermArena,
    TermHandle,
};
use crate::mathematical_core::substitution::{shift, substitute};

/// One closed lambda, shared by the denotation. Supplying operands through
/// Apply keeps each operand once in the stored term: directly substituting
/// the right operand into both case branches creates exponential traversal
/// of a right-nested comparison even when its arena storage is shared.
fn equality_function(
    arena: &mut TermArena,
    two: TermHandle,
    zero: TermHandle,
    one: TermHandle,
) -> TermHandle {
    let right = arena.insert(Term::Variable(0));
    let left = arena.insert(Term::Variable(1));
    let motive = arena.insert(Term::Lambda {
        domain: two,
        body: two,
    });
    let opposite = arena.insert(Term::CaseTwo {
        motive,
        zero_branch: one,
        one_branch: zero,
        scrutinee: right,
    });
    let result = arena.insert(Term::CaseTwo {
        motive,
        zero_branch: opposite,
        one_branch: right,
        scrutinee: left,
    });
    let inner = arena.insert(Term::Lambda {
        domain: two,
        body: result,
    });
    arena.insert(Term::Lambda {
        domain: two,
        body: inner,
    })
}

impl Denotation {
    pub(super) fn boolean_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        Ok(Some(match term {
            ScalarTerm::Boolean(false) => self.two_zero,
            ScalarTerm::Boolean(true) => self.two_one,
            ScalarTerm::BooleanNot { operand } => {
                let operand = self.scalar_term(operand)?;
                self.boolean_case(operand, self.two_one, self.two_zero)
            }
            ScalarTerm::BooleanEqual { left, right } => {
                let left = self.scalar_term(left)?;
                let right = self.scalar_term(right)?;
                if !self.boolean_equality.is_valid() {
                    self.boolean_equality =
                        equality_function(&mut self.arena, self.two, self.two_zero, self.two_one);
                }
                let function = self.arena.insert(Term::Apply {
                    function: self.boolean_equality,
                    argument: left,
                });
                self.arena.insert(Term::Apply {
                    function,
                    argument: right,
                })
            }
            _ => return Ok(None),
        }))
    }

    fn boolean_case(
        &mut self,
        scrutinee: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
    ) -> TermHandle {
        let motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: self.two,
        });
        self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        })
    }

    /// `e : Id Two pl pr` re-presented as `Id Two gl gr` when the two
    /// Boolean identities agree at every valuation of their neutral
    /// Boolean atoms — decided by `caseTwo` elimination of each atom in
    /// turn rather than a rule-instance axiom. `None` keeps the explicit
    /// fallback for endpoints outside the fragment, an atom that is not
    /// `Two`-typed, or a valuation where the premise holds but the goal
    /// does not.
    pub(super) fn boolean_identity_transport(
        &mut self,
        premise: TermHandle,
        goal: TermHandle,
        evidence: TermHandle,
    ) -> Option<TermHandle> {
        let (carrier, premise_left, premise_right) = self.identity_parts(premise)?;
        let (goal_carrier, goal_left, goal_right) = self.identity_parts(goal)?;
        if !self.arena.structurally_equal(carrier, self.two)
            || !self.arena.structurally_equal(goal_carrier, self.two)
        {
            return None;
        }
        let mut remaining = MAX_ELABORATION_NODES;
        let function = self.boolean_case_function(
            premise_left,
            premise_right,
            goal_left,
            goal_right,
            &mut remaining,
        )?;
        Some(self.arena.insert(Term::Apply {
            function,
            argument: evidence,
        }))
    }

    /// `f : Π(_ : Id Two pl pr). Id Two gl gr` — the transport function
    /// one `caseTwo` case split or leaf judgment supplies. Every endpoint
    /// is evaluated in the Boolean fragment first: a literal-only leaf is
    /// `refl` or a false-premise elimination, while a neutral atom splits
    /// the conversion into its two constructor branches.
    fn boolean_case_function(
        &mut self,
        premise_left: TermHandle,
        premise_right: TermHandle,
        goal_left: TermHandle,
        goal_right: TermHandle,
        remaining: &mut u64,
    ) -> Option<TermHandle> {
        let evaluations = [premise_left, premise_right, goal_left, goal_right]
            .map(|term| self.boolean_eval(term, remaining));
        if evaluations
            .iter()
            .any(|evaluation| matches!(evaluation, BooleanEval::Unsupported))
        {
            return None;
        }
        if let Some(atom) = evaluations.iter().find_map(|evaluation| match evaluation {
            BooleanEval::Blocked(atom) => Some(*atom),
            _ => None,
        }) {
            // `M = λ(x : Two). Π(_ : Id Two pl[x] pr[x]). Id Two gl[x]
            // gr[x]` — the atom's occurrences move to the eliminator's
            // binder, so each branch re-checks the conversion at one
            // literal instantiation. The codomain sits under the branch
            // hypothesis binder, so its atom abstracts to `Variable(1)`.
            let abstracted_premise_left = self.abstract_atom(premise_left, atom, 0, remaining)?;
            let abstracted_premise_right = self.abstract_atom(premise_right, atom, 0, remaining)?;
            let abstracted_goal_left = self.abstract_atom(goal_left, atom, 1, remaining)?;
            let abstracted_goal_right = self.abstract_atom(goal_right, atom, 1, remaining)?;
            let premise_type = self.arena.insert(Term::Id {
                ty: self.two,
                left: abstracted_premise_left,
                right: abstracted_premise_right,
            });
            let goal_type = self.arena.insert(Term::Id {
                ty: self.two,
                left: abstracted_goal_left,
                right: abstracted_goal_right,
            });
            let body = self.arena.insert(Term::Pi {
                domain: premise_type,
                codomain: goal_type,
            });
            let motive = self.arena.insert(Term::Lambda {
                domain: self.two,
                body,
            });
            let zero_branch = self.instantiated_case(
                premise_left,
                premise_right,
                goal_left,
                goal_right,
                atom,
                self.two_zero,
                remaining,
            )?;
            let one_branch = self.instantiated_case(
                premise_left,
                premise_right,
                goal_left,
                goal_right,
                atom,
                self.two_one,
                remaining,
            )?;
            return Some(self.arena.insert(Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee: atom,
            }));
        }
        let (BooleanEval::Literal(premise_holds_left), BooleanEval::Literal(premise_holds_right)) =
            (evaluations[0], evaluations[1])
        else {
            return None;
        };
        let (BooleanEval::Literal(goal_holds_left), BooleanEval::Literal(goal_holds_right)) =
            (evaluations[2], evaluations[3])
        else {
            return None;
        };
        let domain = self.arena.insert(Term::Id {
            ty: self.two,
            left: premise_left,
            right: premise_right,
        });
        let body = if goal_holds_left == goal_holds_right {
            // The goal endpoints compute to the same constructor: `refl`
            // closes the branch by conversion, whatever the premise was.
            self.arena.insert(Term::Refl {
                ty: self.two,
                value: goal_left,
            })
        } else if premise_holds_left == premise_holds_right {
            // The premise holds while the goal fails at this valuation —
            // the conversion is not a Boolean identity here, so no
            // derivation exists to elaborate.
            return None;
        } else {
            self.false_identity_elim(
                premise_left,
                premise_right,
                goal_left,
                goal_right,
                premise_holds_left,
            )
        };
        Some(self.arena.insert(Term::Lambda { domain, body }))
    }

    /// The `zero`/`one` branch of the atom case split: instantiate every
    /// endpoint at the literal and recurse.
    fn instantiated_case(
        &mut self,
        premise_left: TermHandle,
        premise_right: TermHandle,
        goal_left: TermHandle,
        goal_right: TermHandle,
        atom: TermHandle,
        literal: TermHandle,
        remaining: &mut u64,
    ) -> Option<TermHandle> {
        let premise_left = self.instantiate_atom(premise_left, atom, literal, remaining)?;
        let premise_right = self.instantiate_atom(premise_right, atom, literal, remaining)?;
        let goal_left = self.instantiate_atom(goal_left, atom, literal, remaining)?;
        let goal_right = self.instantiate_atom(goal_right, atom, literal, remaining)?;
        self.boolean_case_function(
            premise_left,
            premise_right,
            goal_left,
            goal_right,
            remaining,
        )
    }

    /// `J(λ(y : Two). λ(_ : Id Two pl y). F(y), refl pl, pr, h)` for
    /// `h : Id Two pl pr` where `pl` and `pr` reduce to different
    /// constructors — the `F` discriminant picks `Id Two pl pl` at `pl`'s
    /// literal and `at_right` at `pr`'s, so the elimination lands on
    /// `at_right`. `premise_left_literal` is `pl`'s evaluated value. This
    /// is the same "no confusion" elimination `caseTwo` supplies for
    /// `Two`, lifted through `J` — no axiom.
    pub(super) fn two_identity_discriminant(
        &mut self,
        premise_left: TermHandle,
        premise_right: TermHandle,
        at_right: TermHandle,
        premise_left_literal: bool,
        proof: TermHandle,
    ) -> TermHandle {
        // The proof domain sits under the endpoint binder, so the fixed
        // endpoint `pl` shifts once; the discriminant's branches sit
        // under both motive binders, so every embedded endpoint shifts
        // twice.
        let shifted_premise_left = shift(&mut self.arena, premise_left, 0, 1);
        let bound = self.arena.insert(Term::Variable(0));
        let proof_domain = self.arena.insert(Term::Id {
            ty: self.two,
            left: shifted_premise_left,
            right: bound,
        });
        // `F(y)` discriminates on the proof's right endpoint under the
        // motive's two binders: at the branch `pl` selects it is `Id Two
        // pl pl` — closed by `refl` — and at the other it is `at_right`.
        let shifted_left = shift(&mut self.arena, premise_left, 0, 2);
        let shifted_at_right = shift(&mut self.arena, at_right, 0, 2);
        let self_identity = self.arena.insert(Term::Id {
            ty: self.two,
            left: shifted_left,
            right: shifted_left,
        });
        let discriminant_motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: self.type_zero,
        });
        let scrutinee = self.arena.insert(Term::Variable(1));
        // `pl` selects its own literal's branch: `zero` when `pl`
        // evaluated `false`, `one` when it evaluated `true`. `pr` is the
        // other constructor, so it always lands on `at_right`.
        let (zero_branch, one_branch) = if premise_left_literal {
            (shifted_at_right, self_identity)
        } else {
            (self_identity, shifted_at_right)
        };
        let discriminant = self.arena.insert(Term::CaseTwo {
            motive: discriminant_motive,
            zero_branch,
            one_branch,
            scrutinee,
        });
        let proof_lambda = self.arena.insert(Term::Lambda {
            domain: proof_domain,
            body: discriminant,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: self.two,
            body: proof_lambda,
        });
        let base = self.arena.insert(Term::Refl {
            ty: self.two,
            value: premise_left,
        });
        self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: premise_right,
            proof,
        })
    }

    /// The discriminant landing on a goal identity — `h : Id Two pl pr`
    /// re-presented as `Id Two gl gr`. The closed Falsehood crossing
    /// supplies the same discriminant with the goal type itself at
    /// `at_right`: the strict `Empty` is no `Type 0` branch, so a
    /// refuted `Id Two` lands on its goal type directly.
    fn false_identity_elim(
        &mut self,
        premise_left: TermHandle,
        premise_right: TermHandle,
        goal_left: TermHandle,
        goal_right: TermHandle,
        premise_left_literal: bool,
    ) -> TermHandle {
        let goal = self.arena.insert(Term::Id {
            ty: self.two,
            left: goal_left,
            right: goal_right,
        });
        let proof = self.arena.insert(Term::Variable(0));
        self.two_identity_discriminant(
            premise_left,
            premise_right,
            goal,
            premise_left_literal,
            proof,
        )
    }

    /// The denoted Boolean value `term` reduces to, the neutral subterm
    /// blocking reduction, or `Unsupported` outside the fragment. The
    /// reductions mirror the kernel's conversion — `caseTwo` selects on a
    /// constructor scrutinee and an applied lambda β-reduces — so a
    /// `Literal` answer is exactly what the kernel's `refl` conversion
    /// decides, and `Blocked` names a `Two`-typed neutral the case split
    /// can eliminate: an assumption constant of type `Two` or a fully
    /// applied uninterpreted operation.
    fn boolean_eval(&mut self, mut term: TermHandle, remaining: &mut u64) -> BooleanEval {
        loop {
            let Some(rest) = remaining.checked_sub(1) else {
                return BooleanEval::Unsupported;
            };
            *remaining = rest;
            match self.arena.get(term) {
                Term::TwoZero => return BooleanEval::Literal(false),
                Term::TwoOne => return BooleanEval::Literal(true),
                Term::Constant { declaration, .. } => {
                    let neutral = self
                        .declarations
                        .get(usize::try_from(declaration).unwrap_or(usize::MAX))
                        .is_some_and(|declaration| {
                            declaration.is_assumption()
                                && self.arena.structurally_equal(declaration.ty, self.two)
                        });
                    return if neutral {
                        BooleanEval::Blocked(term)
                    } else {
                        BooleanEval::Unsupported
                    };
                }
                Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } => match self.boolean_eval(scrutinee, remaining) {
                    BooleanEval::Literal(false) => term = zero_branch,
                    BooleanEval::Literal(true) => term = one_branch,
                    other => return other,
                },
                Term::Apply { function, argument } => {
                    match self.boolean_function_head(function, remaining) {
                        BooleanHead::Lambda(body) => {
                            term = substitute(&mut self.arena, body, argument);
                        }
                        BooleanHead::Neutral => return BooleanEval::Blocked(term),
                        BooleanHead::Blocked(atom) => return BooleanEval::Blocked(atom),
                        BooleanHead::Unsupported => return BooleanEval::Unsupported,
                    }
                }
                _ => return BooleanEval::Unsupported,
            }
        }
    }

    /// The weak head of a function-position term inside the Boolean
    /// fragment: a `Two`-domain lambda to β-reduce through, `Neutral` for
    /// a constant-headed spine the whole application inherits, `Blocked`
    /// for the atom a `caseTwo` function is stuck on, or `Unsupported`
    /// outside the fragment.
    fn boolean_function_head(
        &mut self,
        mut function: TermHandle,
        remaining: &mut u64,
    ) -> BooleanHead {
        loop {
            let Some(rest) = remaining.checked_sub(1) else {
                return BooleanHead::Unsupported;
            };
            *remaining = rest;
            match self.arena.get(function) {
                Term::Lambda { domain, body } => {
                    return if self.arena.structurally_equal(domain, self.two) {
                        BooleanHead::Lambda(body)
                    } else {
                        BooleanHead::Unsupported
                    };
                }
                Term::Apply {
                    function: inner,
                    argument,
                } => match self.boolean_function_head(inner, remaining) {
                    BooleanHead::Lambda(body) => {
                        function = substitute(&mut self.arena, body, argument);
                    }
                    other => return other,
                },
                Term::Constant { declaration, .. } => {
                    return if self
                        .declarations
                        .get(usize::try_from(declaration).unwrap_or(usize::MAX))
                        .is_some_and(|declaration| declaration.is_assumption())
                    {
                        BooleanHead::Neutral
                    } else {
                        BooleanHead::Unsupported
                    };
                }
                Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } => match self.boolean_eval(scrutinee, remaining) {
                    BooleanEval::Literal(false) => function = zero_branch,
                    BooleanEval::Literal(true) => function = one_branch,
                    BooleanEval::Blocked(atom) => return BooleanHead::Blocked(atom),
                    BooleanEval::Unsupported => return BooleanHead::Unsupported,
                },
                _ => return BooleanHead::Unsupported,
            }
        }
    }

    /// `term` with every subterm structurally equal to `atom` replaced by
    /// `Variable(depth)` — the binder the `caseTwo` motive introduces.
    /// Binder bodies raise `depth` so the replacement names the same
    /// binder under them; bound variables themselves never match an atom.
    fn abstract_atom(
        &mut self,
        term: TermHandle,
        atom: TermHandle,
        depth: u32,
        remaining: &mut u64,
    ) -> Option<TermHandle> {
        self.rewrite_atom(term, atom, depth, AtomRewrite::Binder, remaining)
    }

    /// `term` with every subterm structurally equal to `atom` replaced by
    /// the closed `literal`. No binder arithmetic: the literal has no
    /// free variables to shift.
    fn instantiate_atom(
        &mut self,
        term: TermHandle,
        atom: TermHandle,
        literal: TermHandle,
        remaining: &mut u64,
    ) -> Option<TermHandle> {
        self.rewrite_atom(term, atom, 0, AtomRewrite::Literal(literal), remaining)
    }

    /// The shared atom-rewrite traversal: descend `term`, substituting
    /// every subterm structurally equal to `atom`, and rebuild only along
    /// changed paths so untouched subterms keep their arena handles.
    /// `depth` counts enclosing binders — `AtomRewrite::Binder` writes
    /// `Variable(depth)` and `AtomRewrite::Literal` writes the closed
    /// literal — and `None` keeps the caller's fallback for a term shape
    /// the fragment never denotes.
    fn rewrite_atom(
        &mut self,
        term: TermHandle,
        atom: TermHandle,
        depth: u32,
        rewrite: AtomRewrite,
        remaining: &mut u64,
    ) -> Option<TermHandle> {
        *remaining = remaining.checked_sub(1)?;
        if self.arena.structurally_equal(term, atom) {
            let inserted = match rewrite {
                AtomRewrite::Binder => self.arena.insert(Term::Variable(depth)),
                AtomRewrite::Literal(literal) => literal,
            };
            return Some(inserted);
        }
        match self.arena.get(term) {
            Term::Variable(_)
            | Term::Constant { .. }
            | Term::Sort(_)
            | Term::Two
            | Term::TwoZero
            | Term::TwoOne => Some(term),
            Term::Apply { function, argument } => {
                let function = self.rewrite_atom(function, atom, depth, rewrite, remaining)?;
                let argument = self.rewrite_atom(argument, atom, depth, rewrite, remaining)?;
                Some(self.arena.insert(Term::Apply { function, argument }))
            }
            Term::Id { ty, left, right } => {
                let ty = self.rewrite_atom(ty, atom, depth, rewrite, remaining)?;
                let left = self.rewrite_atom(left, atom, depth, rewrite, remaining)?;
                let right = self.rewrite_atom(right, atom, depth, rewrite, remaining)?;
                Some(self.arena.insert(Term::Id { ty, left, right }))
            }
            Term::Pi { domain, codomain } => {
                let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                let codomain = self.rewrite_atom(codomain, atom, depth + 1, rewrite, remaining)?;
                Some(self.arena.insert(Term::Pi { domain, codomain }))
            }
            Term::Sigma { domain, codomain } => {
                let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                let codomain = self.rewrite_atom(codomain, atom, depth + 1, rewrite, remaining)?;
                Some(self.arena.insert(Term::Sigma { domain, codomain }))
            }
            Term::Lambda { domain, body } => {
                let domain = self.rewrite_atom(domain, atom, depth, rewrite, remaining)?;
                let body = self.rewrite_atom(body, atom, depth + 1, rewrite, remaining)?;
                Some(self.arena.insert(Term::Lambda { domain, body }))
            }
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                let motive = self.rewrite_atom(motive, atom, depth, rewrite, remaining)?;
                let zero_branch =
                    self.rewrite_atom(zero_branch, atom, depth, rewrite, remaining)?;
                let one_branch = self.rewrite_atom(one_branch, atom, depth, rewrite, remaining)?;
                let scrutinee = self.rewrite_atom(scrutinee, atom, depth, rewrite, remaining)?;
                Some(self.arena.insert(Term::CaseTwo {
                    motive,
                    zero_branch,
                    one_branch,
                    scrutinee,
                }))
            }
            _ => None,
        }
    }
}

/// The `Two`-typed subterm an endpoint evaluation stopped on, the
/// constructor literal it computed, or a shape outside the denoted
/// Boolean fragment.
#[derive(Clone, Copy)]
enum BooleanEval {
    Literal(bool),
    /// A `Two`-typed neutral — an assumption constant or a fully applied
    /// uninterpreted operation — that `caseTwo` can eliminate.
    Blocked(TermHandle),
    Unsupported,
}

/// The weak head of a function-position term: a `Two`-domain lambda to
/// β-reduce through, a neutral constant head, or a `caseTwo` stuck on
/// the same atom its value position reports.
enum BooleanHead {
    Lambda(TermHandle),
    Neutral,
    Blocked(TermHandle),
    Unsupported,
}

/// What an atom occurrence rewrites to: the `caseTwo` motive's binder at
/// the current depth, or the branch's closed constructor literal.
#[derive(Clone, Copy)]
enum AtomRewrite {
    Binder,
    Literal(TermHandle),
}

#[cfg(test)]
mod tests {
    use super::super::verify_bounded_certificate;
    use super::{Denotation, ScalarTerm, Term};
    use crate::{
        Budget, MathematicalCertificate, ProofNode, ProofRule, certificate_assumption_closure,
        verify_mathematical_certificate,
    };
    use semantic_vocabulary::{
        Proposition, PropositionContext, PropositionId, ScalarType, ValueId,
    };

    #[test]
    fn primitive_boolean_truth_tables_are_kernel_computations() {
        for left in [false, true] {
            let not = ScalarTerm::boolean_not(ScalarTerm::Boolean(left)).unwrap();
            check_closed(not, !left);
            for right in [false, true] {
                let equal = ScalarTerm::boolean_equal(
                    ScalarTerm::Boolean(left),
                    ScalarTerm::Boolean(right),
                )
                .unwrap();
                check_closed(equal, left == right);
            }
        }
    }

    fn check_closed(expression: ScalarTerm, expected: bool) {
        let mut denotation = Denotation::new();
        let value = denotation.scalar_term(&expression).unwrap();
        let expected_value = denotation
            .scalar_term(&ScalarTerm::Boolean(expected))
            .unwrap();
        let expected_type = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: expected_value,
        });
        let term = denotation.arena.insert(Term::Refl {
            ty: denotation.two,
            value,
        });
        let mut certificate = MathematicalCertificate {
            signature: denotation.declarations,
            level_arity: 0,
            context: Vec::new(),
            term,
            expected: expected_type,
        };
        assert!(
            certificate.signature.is_empty(),
            "truth tables need no assumptions"
        );
        verify_mathematical_certificate(
            &mut denotation.arena,
            &certificate,
            &mut Budget::default(),
        )
        .unwrap();
        let wrong = if expected {
            denotation.two_zero
        } else {
            denotation.two_one
        };
        certificate.expected = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: wrong,
        });
        assert!(
            verify_mathematical_certificate(
                &mut denotation.arena,
                &certificate,
                &mut Budget::default()
            )
            .is_err()
        );
    }

    #[test]
    fn symbolic_boolean_operands_remain_distinct_in_the_exact_closure() {
        let mut denotation = Denotation::new();
        let left = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
        let right = ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean);
        let expression = ScalarTerm::boolean_equal(left.clone(), right.clone()).unwrap();
        let value = denotation.scalar_term(&expression).unwrap();
        let left = denotation.scalar_term(&left).unwrap();
        let right = denotation.scalar_term(&right).unwrap();
        assert_ne!(left, right);
        assert_eq!(denotation.declarations.len(), 2);
        assert!(
            denotation
                .declarations
                .iter()
                .all(|declaration| declaration.ty == denotation.two)
        );
        let term = denotation.arena.insert(Term::Refl {
            ty: denotation.two,
            value,
        });
        let expected = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: value,
            right: value,
        });
        let certificate = MathematicalCertificate {
            signature: denotation.declarations,
            level_arity: 0,
            context: Vec::new(),
            term,
            expected,
        };
        assert_eq!(
            certificate_assumption_closure(&denotation.arena, &certificate),
            [0, 1].into_iter().collect()
        );
        verify_mathematical_certificate(
            &mut denotation.arena,
            &certificate,
            &mut Budget::default(),
        )
        .unwrap();
    }

    #[test]
    fn nested_boolean_equality_keeps_linear_comparison_input() {
        for depth in [4, 8, 16, 32] {
            let mut denotation = Denotation::new();
            let input = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean);
            let mut expression = input.clone();
            for _ in 0..depth {
                expression = ScalarTerm::boolean_equal(input.clone(), expression).unwrap();
            }
            let left = denotation.scalar_term(&expression).unwrap();
            let right = denotation.scalar_term(&expression).unwrap();
            assert!(denotation.arena.len() <= 6 * depth + 32);
            // Count occurrences, not distinct handles: a compact DAG can
            // still expand exponentially for equality and closure readers.
            let mut pending = vec![left];
            let mut occurrences = 0;
            while let Some(term) = pending.pop() {
                occurrences += 1;
                assert!(occurrences <= 32 * depth + 1);
                match denotation.arena.get(term) {
                    Term::Apply { function, argument } => pending.extend([function, argument]),
                    Term::Lambda { domain, body } => pending.extend([domain, body]),
                    Term::CaseTwo {
                        motive,
                        zero_branch,
                        one_branch,
                        scrutinee,
                    } => pending.extend([motive, zero_branch, one_branch, scrutinee]),
                    Term::Variable(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Constant { .. } => {}
                    other => panic!("unexpected Boolean denotation: {other:?}"),
                }
            }
            assert!(denotation.arena.structurally_equal(left, right));
        }
    }

    fn boolean(id: u64) -> (ValueId, ScalarTerm) {
        let id = ValueId::new(id).expect("value id");
        (id, ScalarTerm::value(id, ScalarType::Boolean))
    }

    /// `[b = true] ⊢ (not (not b)) = true` — the bounded checker licenses
    /// the conversion by normalization, but no closed computation decides
    /// it: the transport eliminates `b` through `caseTwo`, closing each
    /// literal branch by `refl` or by `J`-elimination of the contradictory
    /// premise, so no rule-instance axiom enters the signature.
    #[test]
    fn boolean_double_negation_identity_denotes_by_case_analysis() {
        let (b_id, b) = boolean(1);
        let context =
            PropositionContext::from_value_types([(b_id, ScalarType::Boolean)]).expect("context");
        let premise = Proposition::Equal(b.clone(), ScalarTerm::Boolean(true));
        let goal = Proposition::Equal(
            ScalarTerm::boolean_not(ScalarTerm::boolean_not(b).expect("inner not"))
                .expect("outer not"),
            ScalarTerm::Boolean(true),
        );
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut Budget::default(),
        )
        .expect("the Boolean identity verifies through case analysis");
        // Only `b : Two` is assumed — a rule-instance axiom would add
        // the whole conversion implication to the signature.
        assert_eq!(denoted.certificate.signature.len(), 1);
        let Term::Apply { function, argument } = denoted.arena.get(denoted.certificate.term) else {
            panic!("the transport applies its case-analysis function to the premise");
        };
        assert_eq!(denoted.arena.get(argument), Term::Variable(0));
        assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
    }

    /// `[b = c] ⊢ (b == c) = true` — Boolean `equal` of two atoms is the
    /// same proposition as their `Id Two` equality, decided here by two
    /// nested `caseTwo` splits whose mixed branches are `J`-eliminations
    /// of the impossible premise.
    #[test]
    fn boolean_equal_identity_denotes_by_nested_case_analysis() {
        let (b_id, b) = boolean(1);
        let (c_id, c) = boolean(2);
        let context = PropositionContext::from_value_types([
            (b_id, ScalarType::Boolean),
            (c_id, ScalarType::Boolean),
        ])
        .expect("context");
        let premise = Proposition::Equal(b.clone(), c.clone());
        let goal = Proposition::Equal(
            ScalarTerm::boolean_equal(b, c).expect("boolean equal"),
            ScalarTerm::Boolean(true),
        );
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut Budget::default(),
        )
        .expect("the two-atom Boolean identity verifies through case analysis");
        assert_eq!(denoted.certificate.signature.len(), 2);
        let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
            panic!("the transport applies its case-analysis function to the premise");
        };
        assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
    }

    /// `[not a = b] ⊢ a = not b` — the endpoints cross: both sides are
    /// open `not` compositions over both atoms, so the transport nests
    /// `caseTwo` on `a` inside `caseTwo` on `b`, with `J`-eliminations at
    /// the two diagonal valuations where neither side's equality holds.
    #[test]
    fn crossed_boolean_negations_denoate_by_nested_case_analysis() {
        let (a_id, a) = boolean(1);
        let (b_id, b) = boolean(2);
        let context = PropositionContext::from_value_types([
            (a_id, ScalarType::Boolean),
            (b_id, ScalarType::Boolean),
        ])
        .expect("context");
        let premise = Proposition::Equal(
            ScalarTerm::boolean_not(a.clone()).expect("not a"),
            b.clone(),
        );
        let goal = Proposition::Equal(a, ScalarTerm::boolean_not(b).expect("not b"));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut Budget::default(),
        )
        .expect("crossed negations verify through nested case analysis");
        assert_eq!(denoted.certificate.signature.len(), 2);
        let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
            panic!("the transport applies its case-analysis function to the premise");
        };
        assert!(matches!(denoted.arena.get(function), Term::CaseTwo { .. }));
    }

    /// The same Boolean identity inside a connective: `(not (not b) =
    /// true → P)` converts to `(b = true → P)` — the contravariant `Pi`
    /// domain coercion still reaches the `Id Two` pair, and `caseTwo`
    /// decides it.
    #[test]
    fn boolean_identity_nested_in_implication_domain_denotes_by_case_analysis() {
        let (b_id, b) = boolean(1);
        let context =
            PropositionContext::from_value_types([(b_id, ScalarType::Boolean)]).expect("context");
        let atom = Proposition::Atom(PropositionId::new(1).expect("atom"));
        let premise = Proposition::Implication {
            premise: Box::new(Proposition::Equal(
                ScalarTerm::boolean_not(ScalarTerm::boolean_not(b.clone()).expect("inner not"))
                    .expect("outer not"),
                ScalarTerm::Boolean(true),
            )),
            conclusion: Box::new(atom.clone()),
        };
        let goal = Proposition::Implication {
            premise: Box::new(Proposition::Equal(b, ScalarTerm::Boolean(true))),
            conclusion: Box::new(atom),
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut Budget::default(),
        )
        .expect("the nested Boolean identity verifies through case analysis");
        // `b : Two` and the proposition atom — no rule-instance axiom.
        assert_eq!(denoted.certificate.signature.len(), 2);
        assert!(matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Lambda { .. }
        ));
    }

    /// `x = y` under the proved `x = false` transports to `not x = not
    /// y` — the expanded denotations differ by open `not`s over `y`, so
    /// the conversion is a `caseTwo` on `y` wrapped in the equation's
    /// `J`s.
    #[test]
    fn value_equality_transport_through_boolean_endpoints_denotes_by_case_analysis() {
        let (x_id, x) = boolean(1);
        let (y_id, y) = boolean(2);
        let context = PropositionContext::from_value_types([
            (x_id, ScalarType::Boolean),
            (y_id, ScalarType::Boolean),
        ])
        .expect("context");
        let premise = Proposition::Equal(x.clone(), y.clone());
        let equation = Proposition::Equal(x.clone(), ScalarTerm::Boolean(false));
        let goal = Proposition::Equal(
            ScalarTerm::boolean_not(x).expect("not x"),
            ScalarTerm::boolean_not(y).expect("not y"),
        );
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                equalities: vec![ProofNode {
                    conclusion: equation.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }],
            },
        };
        let denoted = verify_bounded_certificate(
            &context,
            &goal,
            &[premise, equation],
            &[],
            &proof,
            &mut Budget::default(),
        )
        .expect("transport through Boolean endpoints verifies through J and case analysis");
        // The two Boolean atoms only — no substitution or conversion
        // axiom.
        assert_eq!(denoted.certificate.signature.len(), 2);
    }

    /// `b = true ⊢ not b = true` is not a Boolean identity — the `b =
    /// one` branch holds the premise while the goal fails — so the
    /// transport declines and the caller keeps its explicit rule-instance
    /// route. The contrapositive `b = false ⊢ not b = true` does close,
    /// and its derived function is itself a kernel judgment.
    #[test]
    fn boolean_identity_transport_declines_a_non_identity() {
        let mut denotation = Denotation::new();
        let b = denotation
            .scalar_term(&ScalarTerm::value(
                ValueId::new(1).unwrap(),
                ScalarType::Boolean,
            ))
            .unwrap();
        let not_b = denotation
            .scalar_term(
                &ScalarTerm::boolean_not(ScalarTerm::value(
                    ValueId::new(1).unwrap(),
                    ScalarType::Boolean,
                ))
                .unwrap(),
            )
            .unwrap();
        let one = denotation.two_one;
        let premise = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: b,
            right: one,
        });
        let goal = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: not_b,
            right: one,
        });
        let evidence = denotation.arena.insert(Term::Variable(0));
        assert_eq!(
            denotation.boolean_identity_transport(premise, goal, evidence),
            None,
            "a premise-true, goal-false valuation must decline"
        );
        let premise = denotation.arena.insert(Term::Id {
            ty: denotation.two,
            left: b,
            right: denotation.two_zero,
        });
        let term = denotation
            .boolean_identity_transport(premise, goal, evidence)
            .expect("the contrapositive identity is a case-analysis derivation");
        let certificate = MathematicalCertificate {
            signature: denotation.declarations.clone(),
            level_arity: 0,
            context: vec![premise],
            term,
            expected: goal,
        };
        verify_mathematical_certificate(
            &mut denotation.arena,
            &certificate,
            &mut Budget::default(),
        )
        .expect("the case-analysis derivation is a kernel judgment");
    }
}
