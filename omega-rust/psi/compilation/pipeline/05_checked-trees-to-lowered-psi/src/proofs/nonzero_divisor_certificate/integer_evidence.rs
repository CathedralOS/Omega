//! Prior-fact custody and closed-order primitives for integer certificates.

use std::cell::RefCell;
use std::rc::Rc;

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{IntegerType, IntegerValue, Proposition, ScalarTerm};

use super::scope_identity::{self, ScopeDerived};

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
    pub(super) projection: Rc<[usize]>,
    pub(super) proposition: &'a Proposition,
}

impl<'a> ProjectedFact<'a> {
    /// The projected leaf `projection` names under `citation` in this scope:
    /// the same fact `projected_facts` yields for any content-equal scope.
    pub(super) fn resolve(
        citation: Citation,
        projection: &Rc<[usize]>,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
    ) -> Self {
        let root = citation.fact(assumptions, semantic_axioms);
        let mut proposition = root;
        for &conjunct in projection.iter() {
            let Proposition::Conjunction(parts) = proposition else {
                unreachable!("projection follows retained conjunction children")
            };
            proposition = &parts[conjunct];
        }
        Self {
            citation,
            root,
            projection: Rc::clone(projection),
            proposition,
        }
    }

    pub(super) fn citation(&self) -> Citation {
        self.citation
    }

    pub(super) fn proof(&self) -> ProofNode {
        let mut proof = self.citation.proof(self.root);
        for &conjunct in self.projection.iter() {
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
///
/// The leaf roster is a pure function of the scope, and producers ask for it
/// at many goals under one scope, so each scope's citation and projection
/// list is derived once and resolved against the caller's slices.
pub(super) fn projected_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    thread_local! {
        static PROJECTIONS: RefCell<ScopeDerived<Vec<(Citation, Rc<[usize]>)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let projections = scope_identity::derived(&PROJECTIONS, assumptions, semantic_axioms, || {
        let mut projections = Vec::new();
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
                    projections.push((citation, Rc::<[usize]>::from(projection)));
                }
            }
        }
        projections
    });
    projections
        .iter()
        .map(|(citation, projection)| {
            ProjectedFact::resolve(*citation, projection, assumptions, semantic_axioms)
        })
        .collect()
}

/// `ScalarTerm::integer_value` with its leaf cases answered directly: a
/// literal is its own value, and a value, field or Boolean has none. Only a
/// compound term runs the evaluator, whose operand stacks allocate even for
/// a leaf; producers ask this of every cited endpoint at every goal.
pub(super) fn literal_value(term: &ScalarTerm) -> Option<(IntegerType, IntegerValue)> {
    match term {
        ScalarTerm::Integer { scalar_type, value } => Some((*scalar_type, *value)),
        ScalarTerm::Value { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::BooleanField { .. }
        | ScalarTerm::Boolean(_) => None,
        _ => term.integer_value(),
    }
}

pub(super) fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
    let (strict, left, right) = match &conclusion {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    let (left_type, left) = literal_value(left)?;
    let (right_type, right) = literal_value(right)?;
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
    /// The cited fact in this scope.
    pub(super) fn fact<'a>(
        self,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
    ) -> &'a Proposition {
        match self {
            Self::Assumption(index) => &assumptions[index],
            Self::SemanticAxiom(index) => &semantic_axioms[index],
        }
    }

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
        (target @ ScalarTerm::Value { .. }, literal) if literal_value(literal).is_some() => {
            Some(target)
        }
        (literal, target @ ScalarTerm::Value { .. }) if literal_value(literal).is_some() => {
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
