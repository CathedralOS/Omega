//! Prior-fact custody and closed-order primitives for integer certificates.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

#[derive(Clone, Copy)]
pub(super) enum Citation {
    Assumption(usize),
    SemanticAxiom(usize),
}

pub(super) fn cited_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition)> {
    assumptions
        .iter()
        .enumerate()
        .map(|(index, fact)| (Citation::Assumption(index), fact))
        .chain(
            semantic_axioms
                .iter()
                .enumerate()
                .map(|(index, fact)| (Citation::SemanticAxiom(index), fact)),
        )
}

#[derive(Clone)]
pub(super) struct ProjectedFact<'a> {
    citation: Citation,
    root: &'a Proposition,
    pub(super) projection: Vec<usize>,
    pub(super) proposition: &'a Proposition,
}

impl ProjectedFact<'_> {
    pub(super) fn proof(&self) -> ProofNode {
        let mut proof = self.citation.proof(self.root);
        for &conjunct in &self.projection {
            let Proposition::Conjunction(parts) = &proof.conclusion else {
                unreachable!("projection follows retained conjunction children")
            };
            proof = ProofNode {
                conclusion: parts[conjunct].clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof),
                    conjunct,
                },
            };
        }
        proof
    }
}

/// Project unconditional conjunction leaves without copying their proof trees.
/// Disjunction branches and implication premises never become ambient facts.
pub(super) fn projected_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    let mut facts = Vec::new();
    for (citation, root) in cited_facts(assumptions, semantic_axioms) {
        let mut pending = vec![(root, Vec::new())];
        while let Some((proposition, projection)) = pending.pop() {
            if let Proposition::Conjunction(parts) = proposition {
                for (conjunct, part) in parts.iter().enumerate().rev() {
                    let mut child_projection = projection.clone();
                    child_projection.push(conjunct);
                    pending.push((part, child_projection));
                }
            } else {
                facts.push(ProjectedFact {
                    citation,
                    root,
                    projection,
                    proposition,
                });
            }
        }
    }
    facts
}

/// Disjunction leaves a branch's own pushed assumption exposes through its
/// conjunction children. A case split only assumes the selected alternative;
/// alternatives nested inside it are facts of that branch alone, so the caller
/// scopes each returned case to the assumption at `index` rather than the
/// ambient roster.
pub(super) fn nested_case_facts<'a>(index: usize, root: &'a Proposition) -> Vec<ProjectedFact<'a>> {
    let mut facts = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((proposition, projection)) = pending.pop() {
        match proposition {
            Proposition::Conjunction(parts) => {
                for (conjunct, part) in parts.iter().enumerate().rev() {
                    let mut child_projection = projection.clone();
                    child_projection.push(conjunct);
                    pending.push((part, child_projection));
                }
            }
            // Alternatives nested inside another disjunction's branches stay
            // conditional on that branch's own selection; only conjunction
            // descent is an unconditional fact here.
            Proposition::Disjunction(_)
                if !facts
                    .iter()
                    .any(|fact: &ProjectedFact<'_>| fact.proposition == proposition) =>
            {
                facts.push(ProjectedFact {
                    citation: Citation::Assumption(index),
                    root,
                    projection,
                    proposition,
                });
            }
            _ => {}
        }
    }
    facts
}

pub(super) fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
    let (strict, left, right) = match &conclusion {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    (left_type == right_type
        && left_type.compare(left, right).is_some_and(|order| {
            if strict {
                order.is_lt()
            } else {
                !order.is_gt()
            }
        }))
    .then_some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

pub(super) fn integer_carrier_bound(
    context: &semantic_vocabulary::PropositionContext,
    goal: &Proposition,
) -> Option<ProofNode> {
    proof_admission::decide_primitive(context, goal, PrimitiveJudgment::IntegerCarrierBound)
        .is_ok()
        .then(|| ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
        })
}

impl Citation {
    pub(super) fn proof(self, conclusion: &Proposition) -> ProofNode {
        ProofNode {
            conclusion: conclusion.clone(),
            rule: match self {
                Self::Assumption(index) => ProofRule::Assumption { index },
                Self::SemanticAxiom(index) => ProofRule::SemanticAxiom { index },
            },
        }
    }
}

/// The value a literal-bounded `LessOrEqual` goal is about: whichever side is
/// a value when the other side is an integer literal.
pub(super) fn goal_target(goal: &Proposition) -> Option<&ScalarTerm> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    match (left, right) {
        (target @ ScalarTerm::Value { .. }, literal) if literal.integer_value().is_some() => {
            Some(target)
        }
        (literal, target @ ScalarTerm::Value { .. }) if literal.integer_value().is_some() => {
            Some(target)
        }
        _ => None,
    }
}

/// Relax a proved `LessOrEqual` that shares one side with the goal into the
/// goal itself: the other side closes by a closed integer relation, and
/// transitivity joins the two.
pub(super) fn relax(goal: &Proposition, mapped: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::LessOrEqual(goal_left, goal_right),
        Proposition::LessOrEqual(mapped_left, mapped_right),
    ) = (goal, &mapped.conclusion)
    else {
        return None;
    };
    if goal_left == mapped_left {
        let tail = closed_integer_relation(Proposition::LessOrEqual(
            mapped_right.clone(),
            goal_right.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(mapped),
                middle_less_or_equal_right: Box::new(tail),
            },
        });
    }
    if goal_right == mapped_right {
        let head = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            mapped_left.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(head),
                middle_less_or_equal_right: Box::new(mapped),
            },
        });
    }
    None
}
