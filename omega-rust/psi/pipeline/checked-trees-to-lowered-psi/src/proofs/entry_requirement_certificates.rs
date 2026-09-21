//! Producer-side crash entry-requirement certificates.
//!
//! The verifier checks crash-site guards and call-continuation coverage from
//! certificates the producer supplies; it re-decides supplied nodes against
//! goals it reconstructs itself and never searches inside a check. This module
//! is the producer stage of that split: [`produce_entry_requirement_certificates`]
//! runs the same bounded denotation-lane search the verifier's
//! `entry_requirements` produce stage runs, records which lane each emitted
//! node was built under, and emits [`EntryRequirementCertificate`] rosters the
//! accepting side replays through [`check_entry_requirement_certificate`].
//!
//! The two stages must stay in lockstep: a certificate produced here is
//! accepted exactly when the verifier's `check_supplied_certificate` accepts
//! the identical `(with_value_equalities, proof)` pair. Attaching these
//! rosters to the proof bundle so verification can consume them instead of
//! searching is a separate leg; this module establishes the producer
//! capability and its parity contract.

use proof_admission::{
    CheckedPredicateDenotations, PredicateDenotationError, PrimitiveJudgment, ProofNode, ProofRule,
    check_predicate_denotations, check_predicate_denotations_with_value_equalities,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};

const MAXIMUM_SEARCH_STEPS: usize = 4096;
const MAXIMUM_PROOF_DEPTH: usize = 64;

/// One denotation conversion the producer can search under. The lane flag in
/// `EntryRequirementCertificate` selects the identical conversion at check
/// time.
type DenotationConversion =
    for<'input> fn(
        &'input PropositionContext,
        &'input Proposition,
        &'input [Proposition],
        &'input [Proposition],
    ) -> Result<CheckedPredicateDenotations<'input>, PredicateDenotationError>;

/// Producer lanes in search order: the smaller Boolean-only question first,
/// then the same question under contextual value-equality transport.
const DENOTATION_LANES: [(bool, DenotationConversion); 2] = [
    (false, check_predicate_denotations),
    (true, check_predicate_denotations_with_value_equalities),
];

/// A producer-supplied certificate for one crash goal: which denotation lane
/// the producing search ran under and the proof node it emitted. Consumers
/// re-run only the recorded conversion and re-decide the node; they never
/// search for a route themselves.
#[derive(Clone)]
pub struct EntryRequirementCertificate {
    with_value_equalities: bool,
    proof: ProofNode,
}

impl EntryRequirementCertificate {
    /// Whether the producing search ran under value-equality transport. The
    /// accepting side replays the identical conversion.
    pub fn with_value_equalities(&self) -> bool {
        self.with_value_equalities
    }

    /// The emitted proof node.
    pub fn proof(&self) -> &ProofNode {
        &self.proof
    }
}

/// The producer stage of the crash ledger: bounded proof search over the
/// denotation lanes, recording which lane each emitted node was built under.
/// An empty supply is not a rejection verdict — the consumer's check of a
/// supplied node is what grants coverage, and no supply means none passes.
pub fn produce_entry_requirement_certificates(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<EntryRequirementCertificate> {
    DENOTATION_LANES
        .into_iter()
        .filter_map(|(with_value_equalities, convert)| {
            prove_lane(convert, context, goal, requirements, semantic_axioms).map(|proof| {
                EntryRequirementCertificate {
                    with_value_equalities,
                    proof,
                }
            })
        })
        .collect()
}

/// Check a supplied crash certificate without searching. The recorded
/// denotation lane is part of the certificate: a node produced under equality
/// transport is replayed against that conversion exactly as produced. This is
/// the producer-side mirror of the verifier's `check_supplied_certificate`;
/// the accepting path runs the same replay over the supply it receives.
pub fn check_entry_requirement_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    certificate: &EntryRequirementCertificate,
) -> bool {
    let convert = if certificate.with_value_equalities {
        check_predicate_denotations_with_value_equalities
    } else {
        check_predicate_denotations
    };
    let Ok(denotations) = convert(context, goal, requirements, semantic_axioms) else {
        return false;
    };
    denotations
        .check_certificate(context, &certificate.proof)
        .is_ok()
}

/// Run the bounded search under exactly one denotation conversion.
fn prove_lane(
    convert: DenotationConversion,
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let denotations = convert(context, goal, requirements, semantic_axioms).ok()?;
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    prove(
        denotations.goal(),
        denotations.requirements(),
        denotations.semantic_axioms(),
        &mut remaining,
        0,
    )
}

fn prove(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    step(remaining, depth)?;
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (index, premise) in premises.iter().enumerate() {
            let mut path = Vec::new();
            let mut reversed = false;
            let found = projection(premise, goal, &mut path, remaining, depth)?;
            let found = if !found && let Proposition::Equal(left, right) = goal {
                path.clear();
                reversed = true;
                projection(
                    premise,
                    &Proposition::Equal(right.clone(), left.clone()),
                    &mut path,
                    remaining,
                    depth,
                )?
            } else {
                found
            };
            if !found {
                continue;
            }
            let mut proof = ProofNode {
                conclusion: premise.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index }
                } else {
                    ProofRule::Assumption { index }
                },
            };
            let mut current = premise;
            for conjunct in path {
                let Proposition::Conjunction(children) = current else {
                    return None;
                };
                current = children.get(conjunct)?;
                proof = ProofNode {
                    conclusion: current.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(proof),
                        conjunct,
                    },
                };
            }
            return Some(if reversed {
                ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::EqualitySymmetry {
                        equality: Box::new(proof),
                    },
                }
            } else {
                proof
            });
        }
    }
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (index, premise) in premises.iter().enumerate() {
            let premise = ProofNode {
                conclusion: premise.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index }
                } else {
                    ProofRule::Assumption { index }
                },
            };
            if let Some(proof) = common_consequence(
                goal,
                premise,
                requirements.len(),
                !semantic,
                remaining,
                depth + 1,
            ) {
                return Some(proof);
            }
        }
    }
    if let Some(proof) = order_chain::prove(goal, requirements, semantic_axioms, remaining, depth) {
        return Some(proof);
    }
    let rule = match goal {
        Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth),
        Proposition::Equal(left, right) if left == right => {
            ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality)
        }
        Proposition::Conjunction(children) => ProofRule::ConjunctionIntroduction(
            children
                .iter()
                .map(|child| prove(child, requirements, semantic_axioms, remaining, depth + 1))
                .collect::<Option<Vec<_>>>()?,
        ),
        Proposition::Disjunction(children) => {
            let (index, proof) = children.iter().enumerate().find_map(|(index, child)| {
                prove(child, requirements, semantic_axioms, remaining, depth + 1)
                    .map(|proof| (index, proof))
            })?;
            ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(proof),
                index,
            }
        }
        _ => return None,
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule,
    })
}

/// Eliminate a disjunction only when every alternative proves the same goal.
/// Branch assumptions occupy the kernel's next local slot and cannot escape
/// into siblings or the enclosing entry requirement context.
fn common_consequence(
    goal: &Proposition,
    premise: ProofNode,
    assumption_count: usize,
    invocation_entry: bool,
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    step(remaining, depth)?;
    if &premise.conclusion == goal {
        return Some(premise);
    }
    if let (Proposition::Equal(left, right), Proposition::Equal(other_left, other_right)) =
        (goal, &premise.conclusion)
        && left == other_right
        && right == other_left
    {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(premise),
            },
        });
    }
    if invocation_entry && let Some(proof) = integer_order::from_premise(goal, &premise) {
        return Some(proof);
    }
    // A branch may prove a published union by proving one of its alternatives.
    // This happens inside its local assumption scope; eliminating the source
    // disjunction below still requires a certificate for every source branch.
    if let Proposition::Disjunction(children) = goal {
        for (index, child) in children.iter().enumerate() {
            if let Some(proof) = common_consequence(
                child,
                premise.clone(),
                assumption_count,
                invocation_entry,
                remaining,
                depth + 1,
            ) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(proof),
                        index,
                    },
                });
            }
        }
    }
    match &premise.conclusion {
        Proposition::Conjunction(children) => {
            for (conjunct, child) in children.iter().enumerate() {
                let child = ProofNode {
                    conclusion: child.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(premise.clone()),
                        conjunct,
                    },
                };
                if let Some(proof) = common_consequence(
                    goal,
                    child,
                    assumption_count,
                    invocation_entry,
                    remaining,
                    depth + 1,
                ) {
                    return Some(proof);
                }
            }
            None
        }
        Proposition::Disjunction(children) => {
            let mut branches = Vec::new();
            for child in children {
                branches.push(common_consequence(
                    goal,
                    ProofNode {
                        conclusion: child.clone(),
                        rule: ProofRule::Assumption {
                            index: assumption_count,
                        },
                    },
                    assumption_count + 1,
                    invocation_entry,
                    remaining,
                    depth + 1,
                )?);
            }
            Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(premise),
                    branches,
                },
            })
        }
        _ => None,
    }
}

// The crash predicate vocabulary has no general negation constructor. Form
// the exact complement of supported scalar propositions; Boolean comparisons
// retain their operands and use the proof owner's checked denotation rules.
// No float, opaque, content or case law is inferred here.
fn opposite(proposition: &Proposition, remaining: &mut usize, depth: usize) -> Option<Proposition> {
    step(remaining, depth)?;
    Some(match proposition {
        Proposition::Truth => Proposition::Falsehood,
        Proposition::Falsehood => Proposition::Truth,
        Proposition::LessThan(left, right) => Proposition::LessOrEqual(right.clone(), left.clone()),
        Proposition::LessOrEqual(left, right) => Proposition::LessThan(right.clone(), left.clone()),
        Proposition::Equal(left, right) => {
            let comparison = match left.scalar_type() {
                ScalarType::Boolean => {
                    ScalarTerm::boolean_equal(left.clone(), right.clone()).ok()?
                }
                ScalarType::Integer(integer_type) => {
                    ScalarTerm::integer_equal(integer_type, left.clone(), right.clone()).ok()?
                }
                _ => return None,
            };
            Proposition::Equal(comparison, ScalarTerm::boolean(false))
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let alternatives = children
                .iter()
                .map(|child| opposite(child, remaining, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            if matches!(proposition, Proposition::Conjunction(_)) {
                Proposition::Disjunction(alternatives)
            } else {
                Proposition::Conjunction(alternatives)
            }
        }
        _ => return None,
    })
}

fn step(remaining: &mut usize, depth: usize) -> Option<()> {
    if depth >= MAXIMUM_PROOF_DEPTH {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    Some(())
}

fn projection(
    premise: &Proposition,
    goal: &Proposition,
    path: &mut Vec<usize>,
    remaining: &mut usize,
    depth: usize,
) -> Option<bool> {
    step(remaining, depth)?;
    if premise == goal {
        return Some(true);
    }
    if let Proposition::Conjunction(conjuncts) = premise {
        for (index, conjunct) in conjuncts.iter().enumerate() {
            path.push(index);
            if projection(conjunct, goal, path, remaining, depth + 1)? {
                return Some(true);
            }
            path.pop();
        }
    }
    Some(false)
}

/// Recover strict order from the entry contract's adjacent inclusive encoding.
/// The existing kernel rule checks the conversion; this is certificate search,
/// not an additional source of numeric facts or a normalization rule.
mod integer_order {
    use proof_admission::{ProofNode, ProofRule};
    use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

    pub(super) fn from_premise(goal: &Proposition, premise: &ProofNode) -> Option<ProofNode> {
        let Proposition::LessThan(left, right) = goal else {
            return None;
        };
        let Proposition::LessOrEqual(inclusive_left, inclusive_right) = &premise.conclusion else {
            return None;
        };
        if left.scalar_type() != right.scalar_type() {
            return None;
        }
        if (right == inclusive_right && adjacent(left, true).as_ref() == Some(inclusive_left))
            || (left == inclusive_left && adjacent(right, false).as_ref() == Some(inclusive_right))
        {
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(premise.clone()),
                },
            });
        }
        None
    }

    fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
        let (integer_type, value) = literal.integer_value()?;
        if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
            return None;
        }
        let value = match (value, increasing) {
            (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
            (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
            (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
            (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
        };
        ScalarTerm::integer(integer_type, value).ok()
    }
}

/// Search exact integer order paths and emit independently checked
/// certificates.
///
/// Short-circuit guards can establish an order through several comparisons.
/// Those comparisons remain separate ledger facts: this search neither adds a
/// transitive axiom nor changes the crash question. Only unconditional premises
/// and their conjuncts enter the graph; alternatives must never be combined.
/// Reachability distinguishes strict from nonstrict paths to the same
/// endpoint. Borrowed facts and predecessor coordinates keep search from
/// cloning a proof tree at every branch. Only the selected path becomes a
/// certificate.
mod order_chain {
    use proof_admission::{ProofNode, ProofRule};
    use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

    use super::step;

    struct OrderFact<'input> {
        root: &'input Proposition,
        relation: &'input Proposition,
        semantic: bool,
        premise_index: usize,
        conjuncts: Vec<usize>,
    }

    struct Reached<'input> {
        endpoint: &'input ScalarTerm,
        strict: bool,
        previous: Option<(usize, usize)>,
        length: usize,
    }

    pub(super) fn prove(
        goal: &Proposition,
        requirements: &[Proposition],
        semantic_axioms: &[Proposition],
        remaining: &mut usize,
        depth: usize,
    ) -> Option<ProofNode> {
        let (start, end, strict_goal) = relation(goal)?;
        let mut facts = Vec::new();
        for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
            for (premise_index, root) in premises.iter().enumerate() {
                collect(
                    OrderFact {
                        root,
                        relation: root,
                        semantic,
                        premise_index,
                        conjuncts: Vec::new(),
                    },
                    &mut facts,
                    remaining,
                    depth,
                )?;
            }
        }
        let mut reached = vec![Reached {
            endpoint: start,
            strict: false,
            previous: None,
            length: 0,
        }];
        let mut cursor = 0;
        while cursor < reached.len() {
            let length = reached[cursor].length + 1;
            for (fact_index, fact) in facts.iter().enumerate() {
                step(remaining, depth + length)?;
                let (left, right, strict) = relation(fact.relation)?;
                if left != reached[cursor].endpoint {
                    continue;
                }
                let strict = strict || reached[cursor].strict;
                let mut visited = false;
                for prior in &reached {
                    step(remaining, depth + length)?;
                    if prior.endpoint == right && prior.strict == strict {
                        visited = true;
                        break;
                    }
                }
                if visited {
                    continue;
                }
                reached.push(Reached {
                    endpoint: right,
                    strict,
                    previous: Some((cursor, fact_index)),
                    length,
                });
                if right == end && (!strict_goal || strict) {
                    return certificate(goal, &facts, &reached, remaining, depth);
                }
            }
            cursor += 1;
        }
        None
    }

    fn collect<'input>(
        fact: OrderFact<'input>,
        facts: &mut Vec<OrderFact<'input>>,
        remaining: &mut usize,
        depth: usize,
    ) -> Option<()> {
        step(remaining, depth)?;
        if relation(fact.relation).is_some() {
            facts.push(fact);
        } else if let Proposition::Conjunction(children) = fact.relation {
            for (conjunct, child) in children.iter().enumerate() {
                let mut conjuncts = fact.conjuncts.clone();
                conjuncts.push(conjunct);
                collect(
                    OrderFact {
                        relation: child,
                        conjuncts,
                        ..fact
                    },
                    facts,
                    remaining,
                    depth + 1,
                )?;
            }
        }
        Some(())
    }

    fn relation(proposition: &Proposition) -> Option<(&ScalarTerm, &ScalarTerm, bool)> {
        let (left, right, strict) = match proposition {
            Proposition::LessThan(left, right) => (left, right, true),
            Proposition::LessOrEqual(left, right) => (left, right, false),
            _ => return None,
        };
        (matches!(left.scalar_type(), ScalarType::Integer(_))
            && left.scalar_type() == right.scalar_type())
        .then_some((left, right, strict))
    }

    fn citation(fact: &OrderFact<'_>, remaining: &mut usize, depth: usize) -> Option<ProofNode> {
        step(remaining, depth)?;
        let mut proof = ProofNode {
            conclusion: fact.root.clone(),
            rule: if fact.semantic {
                ProofRule::SemanticAxiom {
                    index: fact.premise_index,
                }
            } else {
                ProofRule::Assumption {
                    index: fact.premise_index,
                }
            },
        };
        for (nesting, &conjunct) in fact.conjuncts.iter().enumerate() {
            step(remaining, depth + nesting + 1)?;
            let Proposition::Conjunction(children) = &proof.conclusion else {
                return None;
            };
            proof = ProofNode {
                conclusion: children.get(conjunct)?.clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof),
                    conjunct,
                },
            };
        }
        Some(proof)
    }

    fn certificate(
        goal: &Proposition,
        facts: &[OrderFact<'_>],
        reached: &[Reached<'_>],
        remaining: &mut usize,
        depth: usize,
    ) -> Option<ProofNode> {
        let mut path = Vec::new();
        let mut arrival = reached.last()?;
        while let Some((previous, fact)) = arrival.previous {
            step(remaining, depth + path.len())?;
            path.push(fact);
            arrival = reached.get(previous)?;
        }
        let mut path = path.into_iter().rev();
        let mut proof = citation(facts.get(path.next()?)?, remaining, depth)?;
        for (length, fact) in path.enumerate() {
            step(remaining, depth + length + 1)?;
            let next = citation(facts.get(fact)?, remaining, depth + length + 1)?;
            let (left, _, left_strict) = relation(&proof.conclusion)?;
            let (_, right, right_strict) = relation(&next.conclusion)?;
            let strict = left_strict || right_strict;
            proof = ProofNode {
                conclusion: if strict {
                    Proposition::LessThan(left.clone(), right.clone())
                } else {
                    Proposition::LessOrEqual(left.clone(), right.clone())
                },
                rule: if strict {
                    ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: Box::new(proof),
                        middle_to_right: Box::new(next),
                    }
                } else {
                    ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(proof),
                        middle_less_or_equal_right: Box::new(next),
                    }
                },
            };
        }
        if matches!(goal, Proposition::LessOrEqual(..)) && relation(&proof.conclusion)?.2 {
            step(remaining, depth)?;
            proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(proof),
                },
            };
        }
        (proof.conclusion == *goal).then_some(proof)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAXIMUM_PROOF_DEPTH, MAXIMUM_SEARCH_STEPS, check_entry_requirement_certificate,
        produce_entry_requirement_certificates, prove,
    };
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn boolean_value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
    }

    fn boolean_context(identities: &[u64]) -> PropositionContext {
        PropositionContext::from_value_types(
            identities
                .iter()
                .map(|identity| (ValueId::new(*identity).unwrap(), ScalarType::Boolean)),
        )
        .unwrap()
    }

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Signed, 32).unwrap()
    }

    fn integer_value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn integer_context(identities: &[u64]) -> PropositionContext {
        PropositionContext::from_value_types(identities.iter().map(|identity| {
            (
                ValueId::new(*identity).unwrap(),
                ScalarType::Integer(integer_type()),
            )
        }))
        .unwrap()
    }

    fn integer_order(left: u64, right: u64, strict: bool) -> Proposition {
        if strict {
            Proposition::LessThan(integer_value(left), integer_value(right))
        } else {
            Proposition::LessOrEqual(integer_value(left), integer_value(right))
        }
    }

    #[test]
    fn produced_certificates_are_redecided_not_trusted() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            goal.clone(),
        ])];
        let certificates =
            produce_entry_requirement_certificates(&context, &goal, &requirements, &[]);
        assert!(!certificates.is_empty());
        // Every supplied node is replayed through the recorded denotation
        // conversion and the kernel check; supply alone grants nothing.
        assert!(certificates.iter().all(|certificate| {
            check_entry_requirement_certificate(&context, &goal, &requirements, &[], certificate)
        }));
        // A node produced for this goal is not evidence for another question.
        let other = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(false));
        assert!(!certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(&context, &other, &requirements, &[], certificate)
        }));
    }

    #[test]
    fn an_empty_supply_establishes_nothing() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        assert!(produce_entry_requirement_certificates(&context, &goal, &[], &[]).is_empty());
    }

    #[test]
    fn integer_order_certificates_reach_through_inclusive_premises() {
        let context = integer_context(&[1, 2, 3]);
        let goal = integer_order(1, 3, true);
        let requirements = [integer_order(1, 2, false), integer_order(2, 3, false)];
        let certificates =
            produce_entry_requirement_certificates(&context, &goal, &requirements, &[]);
        assert!(certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(&context, &goal, &requirements, &[], certificate)
        }));
        // The certificate's recorded lane stays part of the check: the same
        // node replayed under a mismatched lane conversion is not accepted.
        assert!(
            certificates
                .iter()
                .all(|certificate| certificate.proof().conclusion == goal)
        );
    }

    #[test]
    fn conjunction_proof_search_exhausts_a_shared_budget() {
        let goal = Proposition::Truth;
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Falsehood,
            goal.clone(),
        ])];
        let mut remaining = 2;
        assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
        assert_eq!(remaining, 0);
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        let proof = prove(&goal, &requirements, &[], &mut remaining, 0).unwrap();
        check_certificate(
            &PropositionContext::default(),
            &goal,
            &requirements,
            &[],
            &proof,
        )
        .unwrap();
    }

    #[test]
    fn projection_depth_is_bounded_even_with_remaining_steps() {
        let goal = Proposition::Truth;
        let mut premise = goal.clone();
        for _ in 0..MAXIMUM_PROOF_DEPTH {
            premise = Proposition::Conjunction(vec![premise]);
        }
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &[premise], &[], &mut remaining, 0).is_none());
        assert!(remaining > 0);
    }

    #[test]
    fn case_analysis_requirements_produce_a_checkable_supply() {
        let context = boolean_context(&[1]);
        let goal = Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true));
        // A disjunctive requirement whose branches each carry the goal still
        // discharges it through disjunction elimination.
        let requirements = [Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![Proposition::Truth, goal.clone()]),
            goal.clone(),
        ])];
        let certificates =
            produce_entry_requirement_certificates(&context, &goal, &requirements, &[]);
        assert!(certificates.iter().any(|certificate| {
            check_entry_requirement_certificate(&context, &goal, &requirements, &[], certificate)
        }));
    }
}
