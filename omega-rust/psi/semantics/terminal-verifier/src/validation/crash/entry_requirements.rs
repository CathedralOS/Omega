//! Bounded crash proofs from entry requirements and reconstructed site facts.
//! Call-ceiling coverage supplies only entry requirements; callee continuations
//! retain their independently reconstructed exact routes.
//!
//! Both consumers of this module stage proof search behind a produce/check
//! split: `certify_crash_sites` and `certify_continuation` run the bounded
//! denotation-lane searches and emit [`SuppliedCertificate`] rosters, while the
//! accepting paths only re-decide supplied nodes against goals they reconstruct
//! themselves. No search runs inside a check.

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
#[derive(Clone)]
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

/// The producer's certificate supply for one uncovered call continuation: a
/// coverage roster per published bucket sharing its cause — paired with the
/// bucket so the consumer re-derives each union goal itself — plus the
/// refutation roster over the still-uncovered alternatives.
pub(super) struct ContinuationCertificates<'a> {
    coverage: Vec<(&'a CrashRouteBucket, Vec<SuppliedCertificate>)>,
    refutation: RefutationCertificates<'a>,
}

/// The refutation supply over one uncovered route roster.
pub(super) enum RefutationCertificates<'a> {
    /// The roster named no uncovered predicates: vacuously discharged, no
    /// certificate needed.
    Vacuous,
    /// A non-predicate guard or a context-invalid proposition formed no
    /// question; nothing supplied can discharge it.
    Unformed,
    /// The still-uncovered routes and the nodes produced for their
    /// complement-conjunction goal. The consumer re-derives the goal from
    /// `routes`; the supply never determines which question is asked.
    Supplied {
        routes: Vec<&'a CrashRouteGuard>,
        certificates: Vec<SuppliedCertificate>,
    },
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

/// Producer stage for the call-ceiling questions of one uncovered
/// continuation: a denotation-lane search per same-cause published bucket and
/// one over the uncovered roster's complement. Every search this path needs
/// runs here — [`ContinuationCertificates::discharges`] never searches.
pub(super) fn certify_continuation<'a>(
    caller: &'a TerminalMachine,
    published: impl Iterator<Item = &'a CrashRouteBucket>,
    uncovered: &[&'a CrashRouteGuard],
) -> ContinuationCertificates<'a> {
    let context = entry_context(caller);
    let coverage = published
        .map(|bucket| {
            let certificates = context
                .as_ref()
                .zip(coverage_goal(bucket))
                .map(|(context, goal)| {
                    prove_certificates(context, &goal, &caller.contract.requires, &[])
                })
                .unwrap_or_default();
            (bucket, certificates)
        })
        .collect();
    let refutation = match context.as_ref() {
        Some(context) => match refutation_goal(context, uncovered) {
            RefutationGoal::Vacuous => RefutationCertificates::Vacuous,
            RefutationGoal::Malformed => RefutationCertificates::Unformed,
            RefutationGoal::Goal(goal) => RefutationCertificates::Supplied {
                routes: uncovered.to_vec(),
                certificates: prove_certificates(context, &goal, &caller.contract.requires, &[]),
            },
        },
        None => RefutationCertificates::Unformed,
    };
    ContinuationCertificates {
        coverage,
        refutation,
    }
}

impl ContinuationCertificates<'_> {
    /// The uncovered continuation is discharged when a same-cause published
    /// bucket's reconstructed union goal or the reconstructed complement of
    /// its uncovered roster re-decides one supplied node. Every verdict runs
    /// the recorded denotation conversion and the kernel check over supplied
    /// nodes only; an empty or unformed supply grants nothing.
    pub(super) fn discharges(&self, caller: &TerminalMachine) -> bool {
        let Some(context) = entry_context(caller) else {
            return false;
        };
        let covered = self.coverage.iter().any(|(bucket, certificates)| {
            coverage_goal(bucket).is_some_and(|goal| {
                certificates.iter().any(|certificate| {
                    check_supplied_certificate(
                        &context,
                        &goal,
                        &caller.contract.requires,
                        &[],
                        certificate,
                    )
                })
            })
        });
        covered
            || match &self.refutation {
                RefutationCertificates::Vacuous => true,
                RefutationCertificates::Unformed => false,
                RefutationCertificates::Supplied {
                    routes,
                    certificates,
                } => match refutation_goal(&context, routes) {
                    RefutationGoal::Goal(goal) => certificates.iter().any(|certificate| {
                        check_supplied_certificate(
                            &context,
                            &goal,
                            &caller.contract.requires,
                            &[],
                            certificate,
                        )
                    }),
                    // The producer recorded a formed goal; re-deriving a
                    // vacuous or malformed roster here is unreachable and
                    // grants nothing.
                    _ => false,
                },
            }
    }
}

/// The coverage question one published bucket asks: its single predicate, or
/// the disjunction of its alternatives (`Truth` remains a member, never
/// collapses the union). An empty bucket forms no question.
fn coverage_goal(published: &CrashRouteBucket) -> Option<Proposition> {
    match published.alternatives.as_slice() {
        [] => None,
        [CrashRouteGuard::Predicate(predicate)] => Some(predicate.proposition().clone()),
        alternatives => Some(Proposition::Disjunction(
            alternatives
                .iter()
                .map(|route| match route {
                    CrashRouteGuard::Truth => Proposition::Truth,
                    CrashRouteGuard::Predicate(predicate) => predicate.proposition().clone(),
                })
                .collect(),
        )),
    }
}

/// The question one uncovered roster asks for its refutation.
enum RefutationGoal {
    /// No uncovered predicates remained: discharged without a certificate.
    Vacuous,
    /// A non-predicate guard, an invalid proposition, or a complement the
    /// vocabulary cannot form: no question exists and nothing is granted.
    Malformed,
    /// The complement of every predicate, conjoined into one goal.
    Goal(Proposition),
}

fn refutation_goal(context: &PropositionContext, routes: &[&CrashRouteGuard]) -> RefutationGoal {
    let mut remaining = MAXIMUM_SEARCH_STEPS;
    let mut goals = Vec::new();
    for route in routes {
        let CrashRouteGuard::Predicate(predicate) = route else {
            return RefutationGoal::Malformed;
        };
        // Check the whole route before any denotation simplification. An
        // unresolved body value must not disappear in a constant branch.
        if context.validate(predicate.proposition()).is_err() {
            return RefutationGoal::Malformed;
        }
        let Some(goal) = opposite(predicate.proposition(), &mut remaining, 0) else {
            return RefutationGoal::Malformed;
        };
        goals.push(goal);
    }
    match goals.len() {
        0 => RefutationGoal::Vacuous,
        1 => RefutationGoal::Goal(goals.remove(0)),
        // All alternatives must be false. One goal bounds conversion and
        // search across the entire uncovered union, not separately per route.
        _ => RefutationGoal::Goal(Proposition::Conjunction(goals)),
    }
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
///
/// This produce-and-check composition remains for the search unit tests; the
/// accepting paths consume supplies through `certify_crash_sites`,
/// `certify_continuation`, and `check_supplied_certificate` instead.
#[cfg(test)]
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

    #[test]
    fn continuation_supplies_discharge_only_the_reconstructed_questions() {
        use semantic_vocabulary::{
            BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
            ScalarTerm, ScalarType, ValueId,
        };
        use terminal_psi::{
            Block, CrashCause, CrashPredicateTerm, CrashRouteGuard, MachineContract,
            TerminalMachine, TerminalMachineResult, Terminator, ValueDeclaration,
        };

        fn semantic_id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
            make(raw).unwrap()
        }
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let flag = semantic_id(1, ValueId::new);
        let bound = semantic_id(2, ValueId::new);
        let boolean = |value| {
            Proposition::Equal(
                ScalarTerm::value(flag, ScalarType::Boolean),
                ScalarTerm::boolean(value),
            )
        };
        // Complement formation lands this strict-order predicate back in the
        // requirement vocabulary: `opposite(LessThan(bound, zero))` is
        // `LessOrEqual(zero, bound)`.
        let zero = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
        let bounded = Proposition::LessOrEqual(
            zero.clone(),
            ScalarTerm::value(bound, ScalarType::Integer(integer)),
        );
        let strict =
            Proposition::LessThan(ScalarTerm::value(bound, ScalarType::Integer(integer)), zero);
        let caller = TerminalMachine {
            id: semantic_id(1, MachineId::new),
            attachment: None,
            parameters: vec![
                ValueDeclaration {
                    id: flag,
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                },
                ValueDeclaration {
                    id: bound,
                    scalar_type: ScalarType::Integer(integer),
                    qualifications: Default::default(),
                },
            ],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: semantic_id(1, BlockId::new),
            blocks: vec![Block {
                id: semantic_id(1, BlockId::new),
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: semantic_id(1, EdgeId::new),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: semantic_id(1, ContractId::new),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                requires: vec![boolean(true), bounded.clone()],
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        };
        let bucket = |proposition| terminal_psi::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                proposition,
            ))],
        };
        let route = |proposition| CrashRouteGuard::Predicate(CrashPredicateTerm::new(proposition));

        // Coverage: the produced supply discharges the reconstructed bucket
        // goal, and the same roster asks nothing of a vacuous refutation.
        let covered_bucket = bucket(boolean(true));
        let covered = super::certify_continuation(&caller, [&covered_bucket].into_iter(), &[]);
        assert!(covered.discharges(&caller));

        // A supply produced for one goal grants nothing to another question:
        // pairing the same certificates with a different bucket stays
        // uncovered, and an unformed refutation grants nothing either.
        let wrong_bucket = bucket(boolean(false));
        let swapped = super::ContinuationCertificates {
            coverage: covered
                .coverage
                .iter()
                .map(|(_, certificates)| (&wrong_bucket, certificates.clone()))
                .collect(),
            refutation: super::RefutationCertificates::Unformed,
        };
        assert!(!swapped.discharges(&caller));

        // Refutation: the reconstructed complement of the uncovered route is
        // the entry requirement itself, so the produced supply discharges it.
        let strict_route = route(strict);
        let refuted = super::certify_continuation(&caller, [].into_iter(), &[&strict_route]);
        assert!(refuted.discharges(&caller));
        // A route whose complement is unprovable grants nothing.
        let bounded_route = route(bounded);
        let refuted = super::certify_continuation(&caller, [].into_iter(), &[&bounded_route]);
        assert!(!refuted.discharges(&caller));

        // A `Truth` row forms no refutation question, and an empty supply
        // grants it nothing.
        let truth_route = CrashRouteGuard::Truth;
        let unformed = super::certify_continuation(&caller, [].into_iter(), &[&truth_route]);
        assert!(!unformed.discharges(&caller));

        // An empty roster is vacuously discharged without any certificate.
        let vacuous = super::certify_continuation(&caller, [].into_iter(), &[]);
        assert!(vacuous.discharges(&caller));
    }
}
