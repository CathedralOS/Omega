//! Search only positive IEEE order edges and kernel-decided literal bridges.
//! Parent links retain the actual premise recipes; only the selected path is
//! expanded into a certificate. Ordered and NaN-inclusive complements remain
//! different predicates throughout this search.

use super::{
    PrimitiveJudgment, ProofNode, ProofRule, Proposition, Prover, ScalarTerm, decide_primitive,
    node, primitive,
};
use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

struct Order<'a> {
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
    strict: bool,
}

fn order(proposition: &Proposition, format: IeeeFloatFormat) -> Option<Order<'_>> {
    let Proposition::ScalarIeeeFloatComparison {
        kind,
        format: actual,
        left,
        right,
    } = proposition
    else {
        return None;
    };
    if *actual != format {
        return None;
    }
    let strict = match kind {
        IeeeFloatComparisonKind::LessThan => true,
        IeeeFloatComparisonKind::LessOrEqual => false,
        _ => return None,
    };
    Some(Order {
        left,
        right,
        strict,
    })
}

fn relation(
    format: IeeeFloatFormat,
    left: ScalarTerm,
    right: ScalarTerm,
    strict: bool,
) -> Proposition {
    Proposition::ScalarIeeeFloatComparison {
        kind: if strict {
            IeeeFloatComparisonKind::LessThan
        } else {
            IeeeFloatComparisonKind::LessOrEqual
        },
        format,
        left,
        right,
    }
}

enum Step {
    Fact(usize),
    Closed(Proposition),
}

struct Reached {
    endpoint: ScalarTerm,
    strict: bool,
    previous: Option<usize>,
    steps: Vec<Step>,
}

impl Prover<'_> {
    /// A bridge is usable only when the kernel decides that exact literal
    /// comparison. No host float or integer ordering supplies a proof here.
    fn ieee_literal_bridge(
        &self,
        format: IeeeFloatFormat,
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Option<(Proposition, bool)> {
        if !matches!(
            (left, right),
            (
                ScalarTerm::IeeeFloatLiteral(_),
                ScalarTerm::IeeeFloatLiteral(_)
            )
        ) {
            return None;
        }
        for strict in [true, false] {
            let proposition = relation(format, left.clone(), right.clone(), strict);
            if decide_primitive(
                self.context,
                &proposition,
                PrimitiveJudgment::ClosedIeeeFloatRelation,
            )
            .is_ok()
            {
                return Some((proposition, strict));
            }
        }
        None
    }

    fn ieee_chain_proof(
        &self,
        goal: &Proposition,
        reached: &[Reached],
        mut parent: usize,
        tail: &[Step],
    ) -> ProofNode {
        let mut groups = vec![tail];
        while let Some(previous) = reached[parent].previous {
            groups.push(reached[parent].steps.as_slice());
            parent = previous;
        }
        let premises = groups
            .into_iter()
            .rev()
            .flat_map(|steps| steps.iter())
            .map(|step| match step {
                Step::Fact(index) => self.facts[*index].proof(&self.facts),
                Step::Closed(proposition) => primitive(
                    proposition.clone(),
                    PrimitiveJudgment::ClosedIeeeFloatRelation,
                ),
            })
            .collect();
        node(goal.clone(), ProofRule::IeeeFloatOrderChain(premises))
    }

    pub(super) fn ieee_float_order(&self, goal: &Proposition) -> Option<ProofNode> {
        let Proposition::ScalarIeeeFloatComparison { format, .. } = goal else {
            return None;
        };
        let desired = order(goal, *format)?;
        struct Edge {
            endpoint: ScalarTerm,
            strict: bool,
            step: Step,
        }
        let mut edges: BTreeMap<ScalarTerm, Vec<Edge>> = BTreeMap::new();
        let mut literals = BTreeSet::new();
        let mut record_literal = |term: &ScalarTerm| {
            if matches!(term, ScalarTerm::IeeeFloatLiteral(_)) {
                literals.insert(term.clone());
            }
        };
        record_literal(desired.left);
        record_literal(desired.right);
        for (index, fact) in self.facts.iter().enumerate() {
            let Some(edge) = order(&fact.proposition, *format) else {
                continue;
            };
            record_literal(edge.left);
            record_literal(edge.right);
            edges.entry(edge.left.clone()).or_default().push(Edge {
                endpoint: edge.right.clone(),
                strict: edge.strict,
                step: Step::Fact(index),
            });
        }
        // Sort only literals whose self-order the kernel decides. This excludes
        // NaNs without inventing a total IEEE order. Adjacent bridges suffice
        // for all literal bounds, avoiding a quadratic comparison table.
        let mut literals: Vec<_> = literals
            .into_iter()
            .filter(|term| self.ieee_literal_bridge(*format, term, term).is_some())
            .collect();
        literals.sort_by(
            |left, right| match self.ieee_literal_bridge(*format, left, right) {
                Some((_, true)) => std::cmp::Ordering::Less,
                Some((_, false))
                    if self
                        .ieee_literal_bridge(*format, right, left)
                        .is_some_and(|(_, strict)| strict) =>
                {
                    std::cmp::Ordering::Greater
                }
                Some((_, false)) => left.cmp(right),
                None => std::cmp::Ordering::Greater,
            },
        );
        for pair in literals.windows(2) {
            let (proposition, strict) = self.ieee_literal_bridge(*format, &pair[0], &pair[1])?;
            edges.entry(pair[0].clone()).or_default().push(Edge {
                endpoint: pair[1].clone(),
                strict,
                step: Step::Closed(proposition),
            });
            // Distinct presented zeros compare equal in either direction.
            if !strict {
                let (proposition, strict) =
                    self.ieee_literal_bridge(*format, &pair[1], &pair[0])?;
                edges.entry(pair[1].clone()).or_default().push(Edge {
                    endpoint: pair[0].clone(),
                    strict,
                    step: Step::Closed(proposition),
                });
            }
        }
        let mut reached = vec![Reached {
            endpoint: desired.left.clone(),
            strict: false,
            previous: None,
            steps: Vec::new(),
        }];
        let mut pending = VecDeque::from([0]);
        let mut seen = BTreeSet::from([(desired.left.clone(), false)]);
        while let Some(position) = pending.pop_front() {
            let Some(outgoing) = edges.get(&reached[position].endpoint) else {
                continue;
            };
            for edge in outgoing {
                let strict = reached[position].strict || edge.strict;
                let steps = vec![match &edge.step {
                    Step::Fact(index) => Step::Fact(*index),
                    Step::Closed(proposition) => Step::Closed(proposition.clone()),
                }];
                if edge.endpoint == *desired.right && (!desired.strict || strict) {
                    return Some(self.ieee_chain_proof(goal, &reached, position, &steps));
                }
                if seen.insert((edge.endpoint.clone(), strict)) {
                    let next = reached.len();
                    reached.push(Reached {
                        endpoint: edge.endpoint.clone(),
                        strict,
                        previous: Some(position),
                        steps,
                    });
                    pending.push_back(next);
                }
            }
        }
        None
    }
}
