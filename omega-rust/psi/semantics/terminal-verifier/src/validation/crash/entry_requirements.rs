//! Bounded crash proofs from entry requirements and reconstructed site facts.
//! Call-ceiling coverage supplies only entry requirements; callee continuations
//! retain their independently reconstructed exact routes.

use proof_admission::{
    CheckedPredicateDenotations, PredicateDenotationError, PrimitiveJudgment, ProofNode, ProofRule,
    check_predicate_denotations, check_predicate_denotations_with_value_equalities,
};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, Proposition, PropositionContext, ScalarTerm, ScalarType,
    StructuralPlaceKind,
};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, TerminalMachine, TerminalModule, Terminator,
};

use super::super::machine_value_context;
use super::ModuleError;

mod integer_order;
mod order_chain;

const MAXIMUM_SEARCH_STEPS: usize = 4096;
const MAXIMUM_PROOF_DEPTH: usize = 64;

/// One denotation conversion the producer can search under. The lane flag in
/// `SuppliedCertificate` selects the identical conversion at check time.
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
pub(super) struct SuppliedCertificate {
    with_value_equalities: bool,
    proof: ProofNode,
}

/// The producer stage of the crash ledger: bounded proof search over the
/// denotation lanes, recording which lane each emitted node was built under.
/// An empty supply is not a rejection verdict — the consumer's check of a
/// supplied node is what grants coverage, and no supply means none passes.
pub(super) fn prove_certificates(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<SuppliedCertificate> {
    DENOTATION_LANES
        .into_iter()
        .filter_map(|(with_value_equalities, convert)| {
            prove_lane(convert, context, goal, requirements, semantic_axioms).map(|proof| {
                SuppliedCertificate {
                    with_value_equalities,
                    proof,
                }
            })
        })
        .collect()
}

/// Every node the producing search emitted for one reconstructed crash site:
/// per asserted guard, in guard order, plus the infeasibility discharge over
/// the same site axioms. A site whose supply is empty or checks to nothing
/// fails `CrashSiteGuardUnproved` at the consumer.
pub(super) struct SiteCertificates {
    guards: Vec<Vec<SuppliedCertificate>>,
    infeasible: Vec<SuppliedCertificate>,
}

impl SiteCertificates {
    /// Nodes supplied for the asserted guard at this predicate position.
    pub(super) fn guard(&self, predicate: usize) -> &[SuppliedCertificate] {
        self.guards.get(predicate).map_or(&[], Vec::as_slice)
    }

    /// Nodes supplied for the infeasibility discharge (`Falsehood` goal).
    pub(super) fn infeasible(&self) -> &[SuppliedCertificate] {
        &self.infeasible
    }
}

/// Produce certificates for one roster of reconstructed crash sites. Sites
/// arrive as plain `(machine, block, edge, axioms)` tuples — parallel to, and
/// in the same order as, `reconstruct_validated_crash_site_facts` — so this
/// producer stage needs no view into the reconstruction record type.
pub(super) fn certify_crash_sites<'a>(
    module: &TerminalModule,
    sites: impl Iterator<Item = (MachineId, BlockId, EdgeId, &'a [Proposition])>,
) -> Result<Vec<SiteCertificates>, ModuleError> {
    let mut certificates = Vec::new();
    for (machine_id, block_id, _edge_id, semantic_axioms) in sites {
        let Some(machine) = module
            .machines
            .iter()
            .find(|machine| machine.id == machine_id)
        else {
            certificates.push(SiteCertificates {
                guards: Vec::new(),
                infeasible: Vec::new(),
            });
            continue;
        };
        let context = machine_value_context(module, machine)?;
        let site_guard = machine
            .blocks
            .iter()
            .find(|block| block.id == block_id)
            .and_then(|block| match &block.terminator {
                Terminator::Crash { site_guard, .. } => Some(site_guard.as_slice()),
                _ => None,
            })
            .unwrap_or(&[]);
        let guards = site_guard
            .iter()
            .map(|guard| {
                prove_certificates(
                    &context,
                    guard.proposition(),
                    &machine.contract.requires,
                    semantic_axioms,
                )
            })
            .collect();
        let infeasible = prove_certificates(
            &context,
            &Proposition::Falsehood,
            &machine.contract.requires,
            semantic_axioms,
        );
        certificates.push(SiteCertificates { guards, infeasible });
    }
    Ok(certificates)
}

/// Check a supplied crash certificate without searching. The recorded
/// denotation lane is part of the certificate: a node produced under equality
/// transport is replayed against that conversion exactly as produced.
pub(super) fn check_supplied_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    certificate: &SuppliedCertificate,
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

/// Establish a crash predicate from invocation requirements and any exact
/// independently reconstructed site facts. Call ceilings supply no site facts.
/// Predicate conversion and the certificate are checked by the proof owner.
pub(super) fn establishes(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    // A produced node whose check fails never locks the goal into its
    // producer lane: any other supplied certificate may still check.
    prove_certificates(context, goal, requirements, semantic_axioms)
        .iter()
        .any(|certificate| {
            check_supplied_certificate(context, goal, requirements, semantic_axioms, certificate)
        })
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

fn entry_context(caller: &TerminalMachine) -> Option<PropositionContext> {
    // Body values, result pseudo-values and current storage do not belong to
    // this context. Ordinary module validation separately verifies complete
    // contract scope, including the declared structural parameter associations.
    PropositionContext::from_value_types_and_places(
        caller
            .parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
        caller.structural_places.iter().filter_map(|place| {
            matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                .then_some((place.id, place.kind))
        }),
    )
    .ok()
}

/// Disproof is a proof of the opposite predicate, never failure to prove the
/// route. Only exact invocation formals enter this context; forwarded CFG
/// parameters must already have been rejoined by the caller. Body definitions,
/// current storage and the route itself supply no assumptions.
pub(super) fn refutes<'route>(
    caller: &TerminalMachine,
    routes: impl Iterator<Item = &'route CrashRouteGuard>,
) -> bool {
    let Some(context) = entry_context(caller) else {
        return false;
    };
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    let mut goals = Vec::new();
    for route in routes {
        let CrashRouteGuard::Predicate(predicate) = route else {
            return false;
        };
        // Check the whole route before any denotation simplification. An
        // unresolved body value must not disappear in a constant branch.
        if context.validate(predicate.proposition()).is_err() {
            return false;
        }
        let Some(goal) = opposite(predicate.proposition(), &mut remaining, 0) else {
            return false;
        };
        goals.push(goal);
    }
    let goal = match goals.len() {
        0 => return true,
        1 => goals.remove(0),
        _ => Proposition::Conjunction(goals),
    };
    // All alternatives must be false. One goal bounds conversion and proof
    // search across the entire uncovered union, not separately per route.
    establishes(&context, &goal, &caller.contract.requires, &[])
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

pub(super) fn covers(caller: &TerminalMachine, published: &CrashRouteBucket) -> bool {
    let Some(context) = entry_context(caller) else {
        return false;
    };
    match published.alternatives.as_slice() {
        [] => false,
        [CrashRouteGuard::Predicate(predicate)] => establishes(
            &context,
            predicate.proposition(),
            &caller.contract.requires,
            &[],
        ),
        alternatives => {
            // A bucket publishes a union, not a chosen route. One checked
            // union goal shares the conversion and search budgets across all
            // alternatives, and preserves disjunctive entry requirements.
            let goal = Proposition::Disjunction(
                alternatives
                    .iter()
                    .map(|route| match route {
                        CrashRouteGuard::Truth => Proposition::Truth,
                        CrashRouteGuard::Predicate(predicate) => predicate.proposition().clone(),
                    })
                    .collect(),
            );
            establishes(&context, &goal, &caller.contract.requires, &[])
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        MAXIMUM_PROOF_DEPTH, MAXIMUM_SEARCH_STEPS, ProofNode, ProofRule, Proposition,
        PropositionContext, check_supplied_certificate, common_consequence, establishes, prove,
        prove_certificates,
    };
    use proof_admission::check_certificate;

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
    fn disjunction_is_not_projected_as_a_conjunction() {
        let flag = semantic_vocabulary::ScalarTerm::value(
            semantic_vocabulary::ValueId::new(1).unwrap(),
            semantic_vocabulary::ScalarType::Boolean,
        );
        let goal = Proposition::Equal(flag.clone(), semantic_vocabulary::ScalarTerm::boolean(true));
        let requirements = [Proposition::Disjunction(vec![
            goal.clone(),
            Proposition::Equal(flag, semantic_vocabulary::ScalarTerm::boolean(false)),
        ])];
        let mut remaining = MAXIMUM_SEARCH_STEPS;
        assert!(prove(&goal, &requirements, &[], &mut remaining, 0).is_none());
    }

    #[test]
    fn supplied_certificates_are_redecided_not_trusted() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let identity = ValueId::new(1).unwrap();
        let context =
            PropositionContext::from_value_types([(identity, ScalarType::Boolean)]).unwrap();
        let goal = Proposition::Equal(
            ScalarTerm::value(identity, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            goal.clone(),
        ])];
        let certificates = prove_certificates(&context, &goal, &requirements, &[]);
        assert!(!certificates.is_empty());
        // Every supplied node is replayed through the recorded denotation
        // conversion and the kernel check; supply alone grants nothing.
        assert!(certificates.iter().all(|certificate| {
            check_supplied_certificate(&context, &goal, &requirements, &[], certificate)
        }));
        // A node produced for this goal is not evidence for another question.
        let other = Proposition::Equal(
            ScalarTerm::value(identity, ScalarType::Boolean),
            ScalarTerm::boolean(false),
        );
        assert!(!certificates.iter().any(|certificate| {
            check_supplied_certificate(&context, &other, &requirements, &[], certificate)
        }));
    }

    #[test]
    fn an_empty_supply_establishes_nothing() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let goal = Proposition::Equal(
            ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        let context =
            PropositionContext::from_value_types([(ValueId::new(1).unwrap(), ScalarType::Boolean)])
                .unwrap();
        assert!(prove_certificates(&context, &goal, &[], &[]).is_empty());
        assert!(!establishes(&context, &goal, &[], &[]));
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
    fn nested_cases_discharge_local_assumptions_for_requirements_and_axioms() {
        use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
        let identifiers = [1, 2, 3].map(|index| ValueId::new(index).unwrap());
        let context = PropositionContext::from_value_types(
            identifiers.map(|identity| (identity, ScalarType::Boolean)),
        )
        .unwrap();
        let [goal, other, third] = identifiers.map(|identity| {
            Proposition::Equal(
                ScalarTerm::value(identity, ScalarType::Boolean),
                ScalarTerm::boolean(true),
            )
        });
        let cases = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![goal.clone(), other.clone()]),
            Proposition::Conjunction(vec![
                third.clone(),
                Proposition::Disjunction(vec![
                    goal.clone(),
                    Proposition::Conjunction(vec![other.clone(), goal.clone()]),
                ]),
            ]),
        ]);
        for semantic in [false, true] {
            let mut requirements = vec![other.clone()];
            let mut axioms = Vec::new();
            if semantic {
                axioms.push(cases.clone());
            } else {
                requirements.push(cases.clone());
            }
            assert!(establishes(&context, &goal, &requirements, &axioms));
            assert!(!establishes(&context, &third, &requirements, &axioms));
            let mut remaining = 3;
            let premise = ProofNode {
                conclusion: cases.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index: 0 }
                } else {
                    ProofRule::Assumption { index: 1 }
                },
            };
            assert!(
                common_consequence(
                    &goal,
                    premise.clone(),
                    requirements.len(),
                    !semantic,
                    &mut remaining,
                    0
                )
                .is_none()
            );
            assert_eq!(remaining, 0);
            let mut remaining = MAXIMUM_SEARCH_STEPS;
            let proof = common_consequence(
                &goal,
                premise,
                requirements.len(),
                !semantic,
                &mut remaining,
                0,
            )
            .unwrap();
            check_certificate(&context, &goal, &requirements, &axioms, &proof).unwrap();
        }
        let leaking_cases = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![goal.clone(), other.clone()]),
            other,
        ]);
        assert!(!establishes(&context, &goal, &[leaking_cases], &[]));
    }
}
