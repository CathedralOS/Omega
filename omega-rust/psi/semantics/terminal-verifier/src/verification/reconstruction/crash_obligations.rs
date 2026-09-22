//! Reconstructed crash obligations: the exact questions a proof bundle's
//! crash-certificate roster must answer.
//!
//! Verification reconstructs these questions from the module alone. A
//! `Site` question carries every asserted `Crash` terminator guard and every
//! independently reconstructed path axiom roster into that terminator; it is
//! discharged when each guard re-decides a supplied node under each path, or
//! the path itself is certified infeasible. A `Continuation` question carries
//! the union goals of same-cause published buckets and the complement goal
//! of the uncovered roster; it is discharged when a supplied node re-decides
//! under the caller's entry requirements.
//!
//! No question text, premise roster, or context comes from the artifact.
//! Producers compute the identical questions through
//! `reconstruct_execution_crash_obligations` and attach their answers to the
//! proof bundle; the verifier's check is `proof_admission`'s recorded-lane
//! certificate replay.

use semantic_vocabulary::{
    Proposition, PropositionContext, ScalarTerm, ScalarType, StructuralPlaceKind,
};
use terminal_psi::{
    CrashObligationOwner, CrashRouteBucket, CrashRouteGuard, OperationKind, TerminalMachine,
    TerminalModule, Terminator,
};

use crate::ModuleError;
use crate::validation::crash::{
    forwarded_formal_values, normalized_crash_routes, substitute_crash_routes,
};
use crate::validation::machine_value_context;

use super::reconstruct_validated_crash_site_facts;

/// Bound complement formation so question reconstruction stays bounded work
/// independent of roster size; it is not a proof-search budget.
const MAXIMUM_COMPLEMENT_STEPS: usize = 4096;
const MAXIMUM_COMPLEMENT_DEPTH: usize = 64;

/// One verifier-reconstructed crash question. `context` is `None` only for a
/// continuation whose caller entry context did not form; such a question is
/// retained so an undischarged obligation still rejects, and no certificate
/// can answer it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedCrashObligation {
    pub owner: CrashObligationOwner,
    pub context: Option<PropositionContext>,
    /// Invocation entry requirements certificates cite by `Assumption`
    /// position. Crash questions never add reconstructed obligations here.
    pub requirements: Vec<Proposition>,
    pub question: CrashObligationQuestion,
}

/// The goal roster one crash question asks. Producers answer each goal with
/// the lane-tagged certificate rosters in `CrashObligationEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashObligationQuestion {
    /// Asserted `site_guard` propositions in terminator order, and every
    /// reconstructed path's semantic-axiom roster in reconstruction order.
    Site {
        guards: Vec<Proposition>,
        paths: Vec<Vec<Proposition>>,
    },
    /// The formed coverage goals of same-cause published buckets and the
    /// uncovered roster's complement goal. `refutation_goal` is `None` when
    /// the uncovered roster forms no question — only coverage can then
    /// discharge it.
    Continuation {
        coverage_goals: Vec<Proposition>,
        refutation_goal: Option<Proposition>,
    },
}

/// Reconstruct every crash obligation of an already-validated module, sorted
/// by owner — the canonical roster order the proof bundle must follow.
pub(crate) fn reconstruct_validated_crash_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedCrashObligation>, ModuleError> {
    let mut obligations = Vec::new();
    let sites = reconstruct_validated_crash_site_facts(module)?;
    if !sites.is_empty() {
        for machine in &module.machines {
            for block in &machine.blocks {
                let Terminator::Crash {
                    edge, site_guard, ..
                } = &block.terminator
                else {
                    continue;
                };
                if site_guard.is_empty() {
                    continue;
                }
                let paths = sites
                    .iter()
                    .filter(|site| {
                        site.machine == machine.id && site.block == block.id && site.edge == *edge
                    })
                    .map(|site| site.semantic_axioms.clone())
                    .collect();
                obligations.push(ReconstructedCrashObligation {
                    owner: CrashObligationOwner::Site {
                        machine: machine.id,
                        block: block.id,
                        edge: *edge,
                    },
                    context: Some(machine_value_context(module, machine)?),
                    requirements: machine.contract.requires.clone(),
                    question: CrashObligationQuestion::Site {
                        guards: site_guard
                            .iter()
                            .map(|guard| guard.proposition().clone())
                            .collect(),
                        paths,
                    },
                });
            }
        }
    }
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                let continuations = match operation_continuations(module, operation)? {
                    Some(continuations) => continuations,
                    None => continue,
                };
                reconstruct_continuation_obligations(
                    machine,
                    operation.id,
                    &continuations,
                    &mut obligations,
                );
            }
        }
    }
    for contract in &module.operation_crash_contracts {
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == contract.machine)
            .expect("validated operation crash contract owner exists");
        reconstruct_continuation_obligations(
            caller,
            contract.operation,
            &contract.crash_continuations,
            &mut obligations,
        );
    }
    obligations.sort_by_key(|obligation| obligation.owner);
    Ok(obligations)
}

/// The continuation roster one operation asks the caller to cover: the stored
/// `crash_continuations` of a call operation, or the invocation-specific
/// routes a `BoundaryCall` derives by substituting its actual arguments into
/// the boundary declaration's published routes.
fn operation_continuations(
    module: &TerminalModule,
    operation: &terminal_psi::Operation,
) -> Result<Option<Vec<CrashRouteBucket>>, ModuleError> {
    match &operation.kind {
        OperationKind::Call {
            crash_continuations,
            ..
        }
        | OperationKind::CallUnit {
            crash_continuations,
            ..
        }
        | OperationKind::CallStructuralScalar {
            crash_continuations,
            ..
        }
        | OperationKind::CallDynamicScalar {
            crash_continuations,
            ..
        }
        | OperationKind::CallDynamicParameterScalar {
            crash_continuations,
            ..
        }
        | OperationKind::CallDynamicUnit {
            crash_continuations,
            ..
        }
        | OperationKind::CallDynamicParameterUnit {
            crash_continuations,
            ..
        }
        | OperationKind::CallStructural {
            crash_continuations,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            crash_continuations,
            ..
        } => Ok(Some(crash_continuations.clone())),
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            ..
        } => {
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .expect("validated boundary call target exists");
            if boundary.crash_routes.is_empty() {
                // An empty ceiling asks no coverage question; the positional
                // telescope check is skipped exactly as validation skipped it.
                return Ok(Some(Vec::new()));
            }
            let substitutions = boundary
                .scalar_contract_parameters()
                .ok_or(ModuleError::InvalidBoundaryCrashParameters(boundary.id))?
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.id,
                        ScalarTerm::value(*argument, parameter.scalar_type),
                    )
                })
                .collect();
            Ok(Some(substitute_crash_routes(
                &boundary.crash_routes,
                &substitutions,
            )))
        }
        _ => Ok(None),
    }
}

/// Emit one question per uncovered forwarded continuation bucket, exactly the
/// obligations the removed verifier-side search discharged: the caller's
/// published routes are forwarded through the formal-value map, each
/// surviving uncovered bucket asks for coverage by a same-cause published
/// goal or for refutation of its uncovered alternatives.
fn reconstruct_continuation_obligations(
    caller: &TerminalMachine,
    operation: semantic_vocabulary::OperationId,
    continuations: &[CrashRouteBucket],
    obligations: &mut Vec<ReconstructedCrashObligation>,
) {
    let published_routes = normalized_crash_routes(&caller.contract.crash_routes);
    let covered = |continuation: &CrashRouteBucket| {
        published_routes.iter().any(|published| {
            published.cause == continuation.cause
                && (published.alternatives == [CrashRouteGuard::Truth]
                    || continuation
                        .alternatives
                        .iter()
                        .all(|route| published.alternatives.contains(route)))
        })
    };
    if normalized_crash_routes(continuations).iter().all(&covered) {
        return;
    }
    // Invocation routes remain exact in the caller's actual-value namespace.
    // Only ceiling coverage may follow independently reconstructed CFG copies.
    let forwarded = forwarded_formal_values(caller);
    for continuation in normalized_crash_routes(&substitute_crash_routes(continuations, &forwarded))
    {
        if covered(&continuation) {
            continue;
        }
        let uncovered: Vec<&CrashRouteGuard> = continuation
            .alternatives
            .iter()
            .filter(|route| {
                !published_routes.iter().any(|published| {
                    published.cause == continuation.cause
                        && (published.alternatives == [CrashRouteGuard::Truth]
                            || published.alternatives.contains(route))
                })
            })
            .collect();
        let context = entry_context(caller);
        let coverage_goals = published_routes
            .iter()
            .filter(|published| published.cause == continuation.cause)
            .filter_map(coverage_goal)
            .collect();
        let refutation_goal = match context.as_ref() {
            Some(context) => match refutation_goal(context, &uncovered) {
                // No uncovered predicates remained: the question discharges
                // without a certificate, so none is emitted.
                RefutationGoal::Vacuous => continue,
                RefutationGoal::Malformed => None,
                RefutationGoal::Goal(goal) => Some(goal),
            },
            None => None,
        };
        obligations.push(ReconstructedCrashObligation {
            owner: CrashObligationOwner::Continuation {
                machine: caller.id,
                operation,
                cause: continuation.cause,
            },
            context,
            requirements: caller.contract.requires.clone(),
            question: CrashObligationQuestion::Continuation {
                coverage_goals,
                refutation_goal,
            },
        });
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
    let mut remaining = MAXIMUM_COMPLEMENT_STEPS;
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
    if depth >= MAXIMUM_COMPLEMENT_DEPTH {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    Some(())
}
