//! One scoped continuation owner for bounded proof elaboration.
//! Child evidence is consumed in its exact source order. No result is reused
//! across another node, occurrence, or premise roster.

use super::*;

pub(super) struct ChildEvidence<'proof> {
    values: std::vec::IntoIter<ChildTerm<'proof>>,
    premises: Option<Vec<Proposition>>,
}

struct ChildTerm<'proof> {
    source: &'proof ProofNode,
    term: TermHandle,
}

impl ChildEvidence<'_> {
    pub(super) fn take(&mut self, expected: &ProofNode) -> TermHandle {
        let child = self
            .values
            .next()
            .expect("one elaborated child in source order");
        assert!(
            std::ptr::eq(child.source, expected),
            "child evidence belongs to the exact scheduled premise"
        );
        child.term
    }

    pub(super) fn take_prepared_premises(&mut self) -> Vec<Proposition> {
        self.premises
            .take()
            .expect("transport premise statements were prepared before their children")
    }

    fn finish(mut self) {
        assert!(
            self.values.next().is_none(),
            "all scheduled child evidence was consumed"
        );
        assert!(
            self.premises.is_none(),
            "all prepared transport statements were consumed"
        );
    }
}

struct Children<'proof> {
    fixed: [Option<&'proof ProofNode>; 3],
    remaining: &'proof [ProofNode],
}

impl<'proof> Children<'proof> {
    fn fixed(nodes: [Option<&'proof ProofNode>; 3]) -> Self {
        Self {
            fixed: nodes,
            remaining: &[],
        }
    }

    fn slice(nodes: &'proof [ProofNode]) -> Self {
        Self {
            fixed: [None; 3],
            remaining: nodes,
        }
    }

    fn len(&self) -> usize {
        self.fixed.iter().filter(|node| node.is_some()).count() + self.remaining.len()
    }

    fn next(&mut self) -> Option<&'proof ProofNode> {
        for node in &mut self.fixed {
            if let Some(node) = node.take() {
                return Some(node);
            }
        }
        let (next, remaining) = self.remaining.split_first()?;
        self.remaining = remaining;
        Some(next)
    }
}

struct PartialNode<'proof> {
    proof: &'proof ProofNode,
    children: Children<'proof>,
    awaiting: Option<&'proof ProofNode>,
    evidence: Vec<ChildTerm<'proof>>,
    premises: Option<Vec<Proposition>>,
}

impl<'proof> PartialNode<'proof> {
    fn new(proof: &'proof ProofNode) -> Result<Self, BoundedDenotationError> {
        use ProofRule::*;
        let children = match &proof.rule {
            Primitive(_)
            | Assumption { index: _ }
            | SemanticAxiom { index: _ }
            | IntegerCorrelatedForbiddenRoots { witness: _ } => Children::fixed([None; 3]),
            ConjunctionIntroduction(children) => {
                let Proposition::Conjunction(expected) = &proof.conclusion else {
                    return Err(certificate!(ProofError::RuleConclusionMismatch(
                        "conjunction introduction"
                    )));
                };
                if children.len() != expected.len() {
                    return Err(certificate!(ProofError::ConjunctionArityMismatch));
                }
                Children::slice(children)
            }
            ConjunctionElimination {
                conjunction,
                conjunct: _,
            } => Children::fixed([Some(conjunction), None, None]),
            DisjunctionIntroduction { disjunct, index } => {
                let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                    return Err(certificate!(ProofError::RuleConclusionMismatch(
                        "disjunction introduction"
                    )));
                };
                if *index >= disjuncts.len() {
                    return Err(certificate!(ProofError::UnknownDisjunct(*index)));
                }
                Children::fixed([Some(disjunct), None, None])
            }
            ImplicationElimination {
                implication,
                premise,
            } => Children::fixed([Some(implication), Some(premise), None]),
            EqualitySymmetry { equality } => Children::fixed([Some(equality), None, None]),
            EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => Children::fixed([Some(left_equals_middle), Some(middle_equals_right), None]),
            PredicateDenotation { premise } => Children::fixed([Some(premise), None, None]),
            ValueEqualityTransport {
                premise,
                equalities,
            } => Children {
                fixed: [Some(premise), None, None],
                remaining: equalities,
            },
            IntegerSubtractOrder {
                difference,
                positive,
            } => Children::fixed([Some(difference), Some(positive), None]),
            IntegerAddOrder { sum, positive } => Children::fixed([Some(sum), Some(positive), None]),
            IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => Children::fixed([Some(smaller), Some(larger), Some(order)]),
            IntegerOrderDiscreteness { relation } | IntegerOrderWeakening { relation } => {
                Children::fixed([Some(relation), None, None])
            }
            IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => Children::fixed([
                Some(left_less_or_equal_middle),
                Some(middle_less_or_equal_right),
                None,
            ]),
            IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => Children::fixed([Some(left_to_middle), Some(middle_to_right), None]),
            IntegerOrderAntisymmetry { forward, backward } => {
                Children::fixed([Some(forward), Some(backward), None])
            }
            IntegerMathOrderTranslation {
                relation,
                addend: _,
            } => Children::fixed([Some(relation), None, None]),
            IntegerOrderSubstitution {
                relation,
                equality,
                endpoint: _,
            } => Children::fixed([Some(relation), Some(equality), None]),
            IntegerAffineBound {
                root_bound,
                witness: _,
            }
            | IntegerCastBound {
                root_bound,
                witness: _,
            } => Children::fixed([Some(root_bound), None, None]),
            IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom: _,
            }
            | IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom: _,
            } => Children::fixed([Some(left_bound), Some(right_bound), None]),
            ImplicationIntroduction { body: _ }
            | DisjunctionElimination {
                disjunction: _,
                branches: _,
            } => {
                unreachable!("scoped rules have their own continuation frames")
            }
        };
        let count = children.len();
        let premises =
            matches!(&proof.rule, ValueEqualityTransport { .. }).then(|| Vec::with_capacity(count));
        Ok(Self {
            proof,
            children,
            awaiting: None,
            evidence: Vec::with_capacity(count),
            premises,
        })
    }

    fn next_child(&mut self) -> Option<&'proof ProofNode> {
        let child = self.children.next()?;
        if let Some(premises) = &mut self.premises {
            // This copy preceded the corresponding child in the recursive rule.
            premises.push(child.conclusion.clone());
        }
        self.awaiting = Some(child);
        Some(child)
    }

    fn finish(
        self,
        elaboration: &mut Elaboration<'_>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let mut operands = ChildEvidence {
            values: self.evidence.into_iter(),
            premises: self.premises,
        };
        let term = elaboration.finish_regular(self.proof, &mut operands)?;
        operands.finish();
        Ok(term)
    }
}

#[derive(Clone, Copy)]
struct Scope {
    depth: u32,
    roster: usize,
    positions: usize,
}

impl Scope {
    fn current(elaboration: &Elaboration<'_>) -> Self {
        Self {
            depth: elaboration.depth,
            roster: elaboration.roster.len(),
            positions: elaboration.positions.len(),
        }
    }

    fn restore(self, elaboration: &mut Elaboration<'_>) {
        elaboration.depth = self.depth;
        elaboration.positions.truncate(self.positions);
        elaboration.roster.truncate(self.roster);
    }

    fn close_hypothesis(self, elaboration: &mut Elaboration<'_>) {
        assert_eq!(elaboration.depth, self.depth + 1);
        assert_eq!(elaboration.roster.len(), self.roster + 1);
        assert_eq!(elaboration.positions.len(), self.positions + 1);
        self.restore(elaboration);
    }

    fn close_intermediate(self, elaboration: &mut Elaboration<'_>) {
        assert_eq!(elaboration.depth, self.depth + 1);
        assert_eq!(elaboration.roster.len(), self.roster);
        assert_eq!(elaboration.positions.len(), self.positions);
        self.restore(elaboration);
    }

    fn assert_current(self, elaboration: &Elaboration<'_>) {
        assert_eq!(elaboration.depth, self.depth);
        assert_eq!(elaboration.roster.len(), self.roster);
        assert_eq!(elaboration.positions.len(), self.positions);
    }
}

fn hypothesis(elaboration: &mut Elaboration<'_>, proposition: &Proposition) -> Scope {
    let prefix = Scope::current(elaboration);
    elaboration.roster.push(proposition.clone());
    elaboration.positions.push(elaboration.depth);
    elaboration.depth += 1;
    prefix
}

#[derive(Clone, Copy)]
struct Case<'proof> {
    disjuncts: &'proof [Proposition],
    branches: &'proof [ProofNode],
    scrutinee: TermHandle,
    conclusion: TermHandle,
}

#[derive(Clone, Copy)]
struct CaseParts<'proof> {
    case: Case<'proof>,
    first: TermHandle,
    rest: TermHandle,
    zero: TermHandle,
}

enum Work<'proof> {
    Node(&'proof ProofNode),
    Case(Case<'proof>),
}

enum Pending<'proof> {
    Children(PartialNode<'proof>),
    Implication {
        proof: &'proof ProofNode,
        body: &'proof ProofNode,
        domain: TermHandle,
        scope: Scope,
    },
    Disjunction {
        proof: &'proof ProofNode,
        disjunction: &'proof ProofNode,
        branches: &'proof [ProofNode],
    },
    DisjunctionDone {
        proof: &'proof ProofNode,
        branches: &'proof [ProofNode],
    },
    CaseZero {
        case: Case<'proof>,
        first: TermHandle,
        scope: Scope,
    },
    CaseOne {
        parts: CaseParts<'proof>,
        scope: Scope,
    },
    CaseNested {
        parts: CaseParts<'proof>,
        scope: Scope,
    },
}

pub(super) fn elaborate(
    elaboration: &mut Elaboration<'_>,
    proof: &ProofNode,
) -> Result<TermHandle, BoundedDenotationError> {
    let root = Scope::current(elaboration);
    let result = drive(elaboration, proof);
    match &result {
        Ok(_) => root.assert_current(elaboration),
        Err(_) => root.restore(elaboration),
    }
    result
}

fn enter(
    elaboration: &mut Elaboration<'_>,
    proof: &ProofNode,
) -> Result<(), BoundedDenotationError> {
    elaboration.steps += 1;
    if elaboration.steps > MAX_ELABORATION_NODES {
        return Err(BoundedDenotationError::DepthLimitExceeded);
    }
    elaboration
        .context
        .validate(&proof.conclusion)
        .map_err(|error| certificate!(ProofError::MalformedProposition(error)))
}

fn drive(
    elaboration: &mut Elaboration<'_>,
    proof: &ProofNode,
) -> Result<TermHandle, BoundedDenotationError> {
    let mut pending = Vec::new();
    let mut current = Work::Node(proof);
    'drive: loop {
        let mut value = match current {
            Work::Node(proof) => {
                enter(elaboration, proof)?;
                match &proof.rule {
                    ProofRule::ImplicationIntroduction { body } => {
                        let Proposition::Implication {
                            premise,
                            conclusion: _,
                        } = &proof.conclusion
                        else {
                            return Err(certificate!(ProofError::RuleConclusionMismatch(
                                "implication introduction"
                            )));
                        };
                        let domain = elaboration.denotation.denote(premise)?;
                        let scope = hypothesis(elaboration, premise);
                        pending.push(Pending::Implication {
                            proof,
                            body,
                            domain,
                            scope,
                        });
                        current = Work::Node(body);
                        continue 'drive;
                    }
                    ProofRule::DisjunctionElimination {
                        disjunction,
                        branches,
                    } => {
                        pending.push(Pending::Disjunction {
                            proof,
                            disjunction,
                            branches,
                        });
                        current = Work::Node(disjunction);
                        continue 'drive;
                    }
                    _ => {
                        let mut node = PartialNode::new(proof)?;
                        if let Some(child) = node.next_child() {
                            pending.push(Pending::Children(node));
                            current = Work::Node(child);
                            continue 'drive;
                        }
                        node.finish(elaboration)?
                    }
                }
            }
            Work::Case(case) => {
                assert!(case.disjuncts.len() >= 2);
                assert_eq!(case.disjuncts.len(), case.branches.len());
                let first = elaboration.denotation.denote(&case.disjuncts[0])?;
                let scope = hypothesis(elaboration, &case.disjuncts[0]);
                pending.push(Pending::CaseZero { case, first, scope });
                current = Work::Node(&case.branches[0]);
                continue 'drive;
            }
        };
        loop {
            match pending.pop() {
                Some(Pending::Children(mut node)) => {
                    let source = node.awaiting.take().expect("one scheduled child completed");
                    node.evidence.push(ChildTerm {
                        source,
                        term: value,
                    });
                    if let Some(child) = node.next_child() {
                        pending.push(Pending::Children(node));
                        current = Work::Node(child);
                        continue 'drive;
                    }
                    value = node.finish(elaboration)?;
                }
                Some(Pending::Implication {
                    proof,
                    body,
                    domain,
                    scope,
                }) => {
                    scope.close_hypothesis(elaboration);
                    let term = elaboration.denotation.arena.insert(Term::Lambda {
                        domain,
                        body: value,
                    });
                    let Proposition::Implication {
                        premise: _,
                        conclusion,
                    } = &proof.conclusion
                    else {
                        unreachable!("the implication shape was admitted before the child")
                    };
                    if &body.conclusion != conclusion.as_ref() {
                        return Err(certificate!(ProofError::ImplicationConclusionMismatch));
                    }
                    elaboration
                        .rules
                        .insert(AcceptedProofRule::ImplicationIntroduction);
                    value = term;
                }
                Some(Pending::Disjunction {
                    proof,
                    disjunction,
                    branches,
                }) => {
                    let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
                        return Err(certificate!(ProofError::RulePremiseMismatch(
                            "disjunction elimination"
                        )));
                    };
                    if branches.len() != disjuncts.len() {
                        return Err(certificate!(ProofError::DisjunctionArityMismatch));
                    }
                    let conclusion = elaboration.denotation.denote(&proof.conclusion)?;
                    pending.push(Pending::DisjunctionDone { proof, branches });
                    current = Work::Case(Case {
                        disjuncts,
                        branches,
                        scrutinee: value,
                        conclusion,
                    });
                    continue 'drive;
                }
                Some(Pending::DisjunctionDone { proof, branches }) => {
                    for branch in branches {
                        if branch.conclusion != proof.conclusion {
                            return Err(certificate!(
                                ProofError::DisjunctionBranchConclusionMismatch
                            ));
                        }
                    }
                    elaboration
                        .rules
                        .insert(AcceptedProofRule::DisjunctionElimination);
                }
                Some(Pending::CaseZero { case, first, scope }) => {
                    scope.close_hypothesis(elaboration);
                    let zero = elaboration.denotation.arena.insert(Term::Lambda {
                        domain: first,
                        body: value,
                    });
                    let rest = if case.disjuncts.len() == 2 {
                        elaboration.denotation.denote(&case.disjuncts[1])?
                    } else {
                        elaboration.denotation.disjunction(&case.disjuncts[1..])?
                    };
                    let parts = CaseParts {
                        case,
                        first,
                        rest,
                        zero,
                    };
                    if case.disjuncts.len() == 2 {
                        let scope = hypothesis(elaboration, &case.disjuncts[1]);
                        pending.push(Pending::CaseOne { parts, scope });
                        current = Work::Node(&case.branches[1]);
                    } else {
                        let scope = Scope::current(elaboration);
                        elaboration.depth += 1;
                        let bound = elaboration.denotation.arena.insert(Term::Variable(0));
                        pending.push(Pending::CaseNested { parts, scope });
                        current = Work::Case(Case {
                            disjuncts: &case.disjuncts[1..],
                            branches: &case.branches[1..],
                            scrutinee: bound,
                            conclusion: case.conclusion,
                        });
                    }
                    continue 'drive;
                }
                Some(Pending::CaseOne { parts, scope }) => {
                    scope.close_hypothesis(elaboration);
                    let one = elaboration.denotation.arena.insert(Term::Lambda {
                        domain: parts.rest,
                        body: value,
                    });
                    value = case_term(elaboration, parts, one);
                }
                Some(Pending::CaseNested { parts, scope }) => {
                    scope.close_intermediate(elaboration);
                    let one = elaboration.denotation.arena.insert(Term::Lambda {
                        domain: parts.rest,
                        body: value,
                    });
                    value = case_term(elaboration, parts, one);
                }
                None => return Ok(value),
            }
        }
    }
}

fn case_term(
    elaboration: &mut Elaboration<'_>,
    parts: CaseParts<'_>,
    one: TermHandle,
) -> TermHandle {
    let inner_motive = elaboration.denotation.arena.insert(Term::Lambda {
        domain: elaboration.denotation.two,
        body: elaboration.denotation.type_zero,
    });
    let bound = elaboration.denotation.arena.insert(Term::Variable(0));
    let family = elaboration.denotation.arena.insert(Term::CaseTwo {
        motive: inner_motive,
        zero_branch: parts.first,
        one_branch: parts.rest,
        scrutinee: bound,
    });
    let body = elaboration.denotation.arena.insert(Term::Pi {
        domain: family,
        codomain: parts.case.conclusion,
    });
    let motive = elaboration.denotation.arena.insert(Term::Lambda {
        domain: elaboration.denotation.two,
        body,
    });
    let tag = elaboration.denotation.arena.insert(Term::Fst {
        pair: parts.case.scrutinee,
    });
    let elimination = elaboration.denotation.arena.insert(Term::CaseTwo {
        motive,
        zero_branch: parts.zero,
        one_branch: one,
        scrutinee: tag,
    });
    let payload = elaboration.denotation.arena.insert(Term::Snd {
        pair: parts.case.scrutinee,
    });
    elaboration.denotation.arena.insert(Term::Apply {
        function: elimination,
        argument: payload,
    })
}
