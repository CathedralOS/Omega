//! Exact retained proposition proof custody.

use proof_admission::{
    PrimitiveJudgment, ProofNode, ProofRule, check_value_equality_denotation, decide_primitive,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::integer_evidence::cited_facts;

/// Distinct scalar endpoints offered pairwise to the kernel's licensed
/// closed/open integer derivation once the cited chain cannot reach a goal.
/// Transport edges extend the roster by at most one expansion per endpoint,
/// so the bound covers both the cited endpoints and their expansions. The
/// bound keeps the normalization search a producer-side convenience over
/// a small endpoint roster, never an unbounded hunt.
const MAXIMUM_NORMALIZATION_ENDPOINTS: usize = 64;

/// Candidate endpoint pairs the kernel is asked to decide per failed cited
/// chain — a second refusal bound so a dense roster of compounds cannot turn
/// one missing edge into an unbounded search.
const MAXIMUM_NORMALIZATION_PAIRS: usize = 96;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    cited_facts(assumptions, semantic_axioms)
        .find(|(_, fact)| *fact == goal)
        .map(|(citation, fact)| citation.proof(fact))
        .or_else(|| equality_chain(context, goal, assumptions, semantic_axioms))
}

/// Follow explicitly cited equalities, proving every reversed edge. A machine
/// result aliases its returned value, whose defining operation supplies the
/// literal equation. No missing equation or implicit symmetry is assumed.
/// When no cited chain reaches the goal, the kernel's licensed
/// closed/open-term integer derivation may still certify an edge between two
/// endpoints — `(acc + 1) + (remaining - 1) == acc + remaining` — which then
/// composes like any cited fact; the kernel re-decides the same judgment on
/// replay, so a wrong update or a stale endpoint still cannot pass.
fn equality_chain(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::Equal(left, right) = goal else {
        return None;
    };
    if left == right {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        });
    }
    let mut facts = Vec::new();
    let mut pending_facts = cited_facts(assumptions, semantic_axioms)
        .map(|(citation, fact)| citation.proof(fact))
        .collect::<Vec<_>>();
    while let Some(proof) = pending_facts.pop() {
        match &proof.conclusion {
            Proposition::Conjunction(conjuncts) => {
                for (conjunct, conclusion) in conjuncts.iter().enumerate() {
                    pending_facts.push(ProofNode {
                        conclusion: conclusion.clone(),
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(proof.clone()),
                            conjunct,
                        },
                    });
                }
            }
            Proposition::Equal(_, _) => facts.push(proof),
            _ => {}
        }
    }
    chain_through(&facts, left, right).or_else(|| {
        // The derivation fallback only pays for goals a cited chain could
        // never close: a leaf-to-leaf goal has no hidden arithmetic for
        // transport or normalization to expose, while the bound/order
        // producers re-enter this producer thousands of times for leaf
        // endpoint substitutions that must stay cheap.
        if !compound(left) && !compound(right) {
            return None;
        }
        let mut facts = facts;
        // An update wraps cited names inside fresh compounds the cited chain
        // cannot name: `(acc + 1) + (remaining - 1)` holds the cited `acc` and
        // `remaining` only through the successor's own defining equations.
        // Transport certifies each endpoint's cited-definition expansion
        // before the kernel is asked to decide the exposed arithmetic.
        facts.extend(transport_edges(context, &facts, left, right));
        facts.extend(normalization_edges(context, &facts, left, right));
        chain_through(&facts, left, right)
    })
}

/// A term carrying structure the kernel's derivation can decide — anything
/// beyond a leaf value, literal or Boolean atom.
fn compound(term: &ScalarTerm) -> bool {
    !matches!(
        term,
        ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
    )
}

/// Distinct scalar endpoints offered by the goal and every cited equality.
fn endpoints(facts: &[ProofNode], left: &ScalarTerm, right: &ScalarTerm) -> Vec<ScalarTerm> {
    let mut endpoints = vec![left.clone(), right.clone()];
    for fact in facts {
        let Proposition::Equal(source, destination) = &fact.conclusion else {
            continue;
        };
        endpoints.push(source.clone());
        endpoints.push(destination.clone());
    }
    endpoints.sort();
    endpoints.dedup();
    endpoints
}

/// Cited `value = term` definitions let transport expose the compounds an
/// endpoint wraps around still-cited names: `v_succ` carrying
/// `v_succ == add(v, 1)` inside `add(v_succ, w_succ)` expands to
/// `add(add(v, 1), w_succ)` through `ValueEqualityTransport`, which the
/// kernel rechecks under the same bounded denotation owner. The premise is
/// reflexivity of the unexpanded endpoint, so the edge asserts exactly the
/// cited equations and nothing more.
fn transport_edges(
    context: &PropositionContext,
    facts: &[ProofNode],
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Vec<ProofNode> {
    let definitions = facts
        .iter()
        .filter(|fact| {
            matches!(
                fact.conclusion,
                Proposition::Equal(ScalarTerm::Value { .. }, _)
            )
        })
        .collect::<Vec<_>>();
    if definitions.is_empty() {
        return Vec::new();
    }
    let defined = definitions
        .iter()
        .filter_map(|definition| match &definition.conclusion {
            Proposition::Equal(ScalarTerm::Value { id, .. }, _) => Some(*id),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut edges = Vec::new();
    for endpoint in endpoints(facts, left, right) {
        // A leaf's expansion is its own cited edge; only a compound hides
        // cited names inside a constructor the chain cannot traverse.
        if !compound(&endpoint) {
            continue;
        }
        // Denotation validation walks every cited equation; an endpoint
        // holding no defined value cannot expand, so skip it before paying
        // that walk.
        if !endpoint.any_value_id(|value| defined.contains(&value)) {
            continue;
        }
        let premise = Proposition::Equal(endpoint.clone(), endpoint.clone());
        let Ok(denoted) = check_value_equality_denotation(
            context,
            &premise,
            definitions.iter().map(|definition| &definition.conclusion),
        ) else {
            continue;
        };
        let Proposition::Equal(expanded, _) = denoted else {
            continue;
        };
        if expanded == endpoint {
            continue;
        }
        edges.push(ProofNode {
            conclusion: Proposition::Equal(endpoint.clone(), expanded),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: premise,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                }),
                equalities: definitions
                    .iter()
                    .map(|definition| (*definition).clone())
                    .collect(),
            },
        });
    }
    edges
}

/// Edges `PrimitiveJudgment::ClosedIntegerRelation` itself decides between
/// the goal endpoints and every distinct cited endpoint. Each emitted edge
/// carries the primitive as its own proof, so custody stays explicit and the
/// receiver re-derives rather than trusts the producer's choice.
fn normalization_edges(
    context: &PropositionContext,
    facts: &[ProofNode],
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Vec<ProofNode> {
    // The roster includes each transported expansion; the pair bound below —
    // not this count — is what bounds kernel derivations.
    let endpoints = endpoints(facts, left, right);
    if endpoints.len() > MAXIMUM_NORMALIZATION_ENDPOINTS {
        return Vec::new();
    }
    // Only a pair holding at least one compound endpoint can be a new
    // decision: distinct atoms and literals were already closed-evaluated or
    // stay distinct, so leaf pairs never certify an edge the cited roster
    // lacked. The filter also keeps the kernel's own derivation cheap — it
    // runs once per candidate pair, not per endpoint. Compound pairs go
    // first: a transported expansion differs from its cited target only in
    // shape, while a leaf-to-leaf question is the cited chain's own job, so
    // sorted order must not let leaf pairs exhaust the candidate bound
    // before the deciding pair is asked.
    let compounds = endpoints
        .iter()
        .enumerate()
        .filter(|(_, endpoint)| compound(endpoint))
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (first, &source) in compounds.iter().enumerate() {
        for &destination in &compounds[first + 1..] {
            pairs.push((source, destination));
        }
    }
    for &source in &compounds {
        for (destination, endpoint) in endpoints.iter().enumerate() {
            if !compound(endpoint) {
                pairs.push((source, destination));
            }
        }
    }
    let mut edges = Vec::new();
    for (source, destination) in pairs.into_iter().take(MAXIMUM_NORMALIZATION_PAIRS) {
        let conclusion =
            Proposition::Equal(endpoints[source].clone(), endpoints[destination].clone());
        if decide_primitive(
            context,
            &conclusion,
            PrimitiveJudgment::ClosedIntegerRelation,
        )
        .is_ok()
        {
            edges.push(ProofNode {
                conclusion,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            });
        }
    }
    edges
}

/// Breadth-first traversal over `facts`: directed `Equal` edges, each
/// traversed in either orientation with explicit symmetry certificates, and
/// every hop composed by transitivity back to `left`.
fn chain_through(facts: &[ProofNode], left: &ScalarTerm, right: &ScalarTerm) -> Option<ProofNode> {
    let mut pending = vec![(left.clone(), None::<ProofNode>)];
    let mut index = 0;
    while index < pending.len() {
        let (current, prefix) = pending[index].clone();
        index += 1;
        for next in facts {
            let Proposition::Equal(source, destination) = &next.conclusion else {
                continue;
            };
            let (destination, next) = if source == &current {
                (destination, next.clone())
            } else if destination == &current {
                (
                    source,
                    ProofNode {
                        conclusion: Proposition::Equal(destination.clone(), source.clone()),
                        rule: ProofRule::EqualitySymmetry {
                            equality: Box::new(next.clone()),
                        },
                    },
                )
            } else {
                continue;
            };
            if pending.iter().any(|(value, _)| value == destination) {
                continue;
            }
            let proof = if let Some(prefix) = prefix.clone() {
                ProofNode {
                    conclusion: Proposition::Equal(left.clone(), destination.clone()),
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(prefix),
                        middle_equals_right: Box::new(next.clone()),
                    },
                }
            } else {
                next
            };
            if destination == right {
                return Some(proof);
            }
            pending.push((destination.clone(), Some(proof)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{PrimitiveJudgment, ProofRule, Proposition, prove};
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn context() -> PropositionContext {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap()
    }

    fn equality(left: u64, right: u64) -> Proposition {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(left).unwrap(), scalar_type),
            ScalarTerm::value(ValueId::new(right).unwrap(), scalar_type),
        )
    }

    #[test]
    fn directed_equality_chain_replays_each_exact_citation() {
        let goal = equality(1, 4);
        let assumptions = [Proposition::Conjunction(vec![
            equality(2, 3),
            equality(3, 4),
        ])];
        let semantic_axioms = [equality(1, 2)];
        let proof = prove(&context(), &goal, &assumptions, &semantic_axioms)
            .expect("three cited equalities");
        assert!(matches!(proof.rule, ProofRule::EqualityTransitivity { .. }));
        check_certificate(&context(), &goal, &assumptions, &semantic_axioms, &proof)
            .expect("kernel replays the directed chain");

        assert!(check_certificate(&context(), &goal, &assumptions, &[], &proof).is_err());
        assert!(
            check_certificate(
                &context(),
                &goal,
                &[equality(2, 4)],
                &semantic_axioms,
                &proof
            )
            .is_err(),
            "substituted hypothesis shape cannot satisfy the existing citation"
        );
    }

    #[test]
    fn missing_equality_edges_are_not_invented() {
        let goal = equality(1, 4);
        for semantic_axioms in [
            vec![equality(1, 2), equality(3, 4)],
            vec![equality(2, 1), equality(3, 4)],
            vec![equality(1, 2), equality(3, 2)],
        ] {
            assert!(
                prove(&context(), &goal, &[], &semantic_axioms).is_none(),
                "{semantic_axioms:?}"
            );
        }
    }

    #[test]
    fn reversed_edges_have_explicit_symmetry_certificates() {
        let goal = equality(1, 4);
        let axioms = [equality(2, 1), equality(2, 3), equality(4, 3)];
        let proof = prove(&context(), &goal, &[], &axioms).expect("two reversed edges");
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        let mut changed = axioms.clone();
        changed[0] = equality(1, 2);
        assert!(
            check_certificate(&context(), &goal, &[], &changed, &proof).is_err(),
            "a symmetric proposition does not replace the cited proof node"
        );
    }

    #[test]
    fn equality_cycles_terminate_without_hiding_a_reachable_target() {
        let goal = equality(1, 4);
        let mut semantic_axioms = vec![
            equality(1, 2),
            equality(2, 1),
            equality(2, 3),
            equality(3, 2),
        ];
        assert!(
            prove(&context(), &goal, &[], &semantic_axioms).is_none(),
            "cycle is not a missing exit"
        );
        semantic_axioms.push(equality(3, 4));
        let proof =
            prove(&context(), &goal, &[], &semantic_axioms).expect("reachable target after cycle");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    #[test]
    fn reflexivity_needs_no_citation_and_non_equality_is_not_a_chain() {
        let goal = equality(1, 1);
        let proof = prove(&context(), &goal, &[], &[]).expect("reflexivity");
        check_certificate(&context(), &goal, &[], &[], &proof).unwrap();
        let Proposition::Equal(left, right) = equality(1, 4) else {
            unreachable!()
        };
        assert!(
            prove(
                &context(),
                &Proposition::LessOrEqual(left, right),
                &[],
                &[equality(1, 4)]
            )
            .is_none()
        );
    }

    fn integer_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
    }

    fn add(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
        ScalarTerm::exact_integer_add(integer_type(), left, right).unwrap()
    }

    fn subtract(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
        ScalarTerm::exact_integer_subtract(integer_type(), left, right).unwrap()
    }

    fn term(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }

    fn one() -> ScalarTerm {
        ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(1)).unwrap()
    }

    /// `(acc + 1) + (remaining - 1)`: the arrival the cyclic guarantee must
    /// preserve against `acc + remaining`.
    fn updated_sum() -> ScalarTerm {
        add(add(term(1), one()), subtract(term(2), one()))
    }

    fn conserved_sum() -> ScalarTerm {
        add(term(1), term(2))
    }

    #[test]
    fn normalization_certifies_the_accumulator_recurrence_edge() {
        let goal = Proposition::Equal(updated_sum(), conserved_sum());
        let proof = prove(&context(), &goal, &[], &[]).expect("certified normalization edge");
        // Endpoint order is canonical, so the single hop is either the
        // primitive itself or its explicit symmetry certificate.
        match &proof.rule {
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation) => {}
            ProofRule::EqualitySymmetry { equality } => assert!(matches!(
                equality.rule,
                ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
            )),
            _ => panic!("normalization edge must carry the licensed primitive"),
        }
        check_certificate(&context(), &goal, &[], &[], &proof).unwrap();
    }

    #[test]
    fn normalization_edges_compose_through_cited_equalities() {
        let goal = Proposition::Equal(updated_sum(), term(3));
        let axioms = [Proposition::Equal(term(3), conserved_sum())];
        let proof = prove(&context(), &goal, &[], &axioms)
            .expect("normalization edge then reversed citation");
        assert!(matches!(proof.rule, ProofRule::EqualityTransitivity { .. }));
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        assert!(
            check_certificate(&context(), &goal, &[], &[], &proof).is_err(),
            "dropping the cited hop invalidates the chain even though the \
             normalization edge needs no citation"
        );
    }

    #[test]
    fn successor_endpoint_transports_through_cited_definitions() {
        // The latch arrival shape: the goal names only the successor values,
        // whose cited definitions hide `acc`/`remaining` inside compounds.
        // `add(v17, v15)` must reach the cited `add(v4, v3)` hypothesis:
        // transport expands `v17 == add(v7, v16)` and `v15 == sub(v6, v14)`
        // through `v7 == v4`, `v6 == v3`, `v16 == 1`, `v14 == 1`.
        let goal = Proposition::Equal(add(term(2), term(1)), add(term(17), term(15)));
        let axioms = [
            // The header invariant hypothesis over its parameters.
            Proposition::Equal(add(term(2), term(1)), add(term(4), term(3))),
            Proposition::Equal(term(6), term(3)),
            Proposition::Equal(term(7), term(4)),
            Proposition::Equal(term(14), one()),
            Proposition::Equal(term(15), subtract(term(6), term(14))),
            Proposition::Equal(term(16), one()),
            Proposition::Equal(term(17), add(term(7), term(16))),
        ];
        // context() registers values 1..=4 only; widen it for this roster.
        let scalar_type = ScalarType::Integer(integer_type());
        let context = PropositionContext::from_value_types(
            (1..=17).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let proof = prove(&context, &goal, &[], &axioms)
            .expect("transport then cited hypothesis closes the arrival");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(
            prove(&context, &goal, &[], &axioms[..axioms.len() - 1]).is_none(),
            "dropping the `v17` definition leaves an unprovable gap"
        );
    }

    #[test]
    fn a_wrong_update_is_decided_not_equal() {
        // `acc` forwarded unchanged: `acc + (remaining - 1)` is strictly less
        // than `acc + remaining`, so the kernel refuses the edge outright —
        // it is a decided non-equality, not an undecided search.
        let forwarded = add(term(1), subtract(term(2), one()));
        let goal = Proposition::Equal(forwarded, conserved_sum());
        assert!(prove(&context(), &goal, &[], &[]).is_none());
        // Off-by-one drift stays refused even when the update is a compound.
        let drifted = add(term(1), add(term(2), one()));
        assert!(
            prove(
                &context(),
                &Proposition::Equal(updated_sum(), drifted),
                &[],
                &[]
            )
            .is_none()
        );
    }
}
