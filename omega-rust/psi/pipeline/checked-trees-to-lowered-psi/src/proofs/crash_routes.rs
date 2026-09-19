//! Crash-route and canonical proposition lowering.
//!
//! This file lowers crash routes, frontiers, exits and route buckets.
//! `structural_members.rs` lowers structural member paths and terms,
//! `structural_arithmetic.rs` guards structural divisors and shifts,
//! `structural_route_buckets.rs` lowers structural crash route buckets,
//! `crash_predicates.rs` lowers checked crash predicates and boolean
//! propositions and `scalar_terms.rs` lowers checked scalar terms.

mod argument_prefix;
#[cfg(test)]
mod boolean_connective_tests;
mod crash_predicates;
#[cfg(test)]
mod runtime_requirement_tests;
mod scalar_terms;
mod structural_arithmetic;
#[cfg(test)]
mod structural_boolean_tests;
mod structural_members;
mod structural_route_buckets;

pub(crate) use argument_prefix::structural_crash_route_argument_prefix;
pub(crate) use crash_predicates::{checked_boolean_proposition, lower_checked_crash_predicates};
pub(crate) use scalar_terms::{
    checked_boolean_scalar_term, checked_scalar_term, lowered_direct_scalar_term,
};
pub(crate) use structural_members::{lower_structural_member_path, lower_structural_member_term};
pub(crate) use structural_route_buckets::{
    lower_structural_crash_route_buckets, substitute_structural_crash_route_roots,
};

use super::{
    BTreeSet, CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedTrees, ClaimId,
    LoweringError, PermissionClaimIdentity, ScalarType, TerminalCrashCause, ValueDeclaration,
    dense_identity, unsupported, value_id,
};
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredCrashExit;

pub(crate) fn lower_boundary_crash_routes(
    checked: &CheckedTrees,
    boundary: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    // Trait requirements own capsules; attached bodyless declarations own
    // machine contracts. Neither may borrow a selected provider's body summary.
    let buckets = checked
        .facts
        .contract_plans
        .for_machine(boundary.contract_owner)
        .map(|contract| contract.crash.published())
        .or_else(|| {
            checked
                .facts
                .contract_plans
                .crash_capsule(boundary.contract_owner, boundary.state)
                .map(|capsule| capsule.published_buckets())
        })
        .ok_or(LoweringError::Unsupported(
            "boundary crash contract owner is absent",
        ))?;
    lower_formal_crash_routes(buckets, scalar_types)
}

/// Lower published crash routes into a declaration-local formal namespace:
/// the scalar lane position `k` is formal `ValueId` `k + 1`, typed by
/// `scalar_types[k]`. Boundary declarations and operation-level crash
/// contracts share this telescope, so the verifier's positional substitution
/// reconstructs both from the same identities. Authored constant routes
/// normalize before lowering: `false` contributes nothing and `true` is the
/// unconditional guard.
pub(crate) fn lower_formal_crash_routes(
    buckets: &[checked_trees::CrashRouteBucket],
    scalar_types: &[ScalarType],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    let parameters = scalar_types
        .iter()
        .enumerate()
        .map(|(position, scalar_type)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(position)?),
                scalar_type: *scalar_type,
                qualifications: Default::default(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let normalized = buckets
        .iter()
        .filter_map(|bucket| {
            let guards = bucket
                .alternative_guards()
                .iter()
                .filter_map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        match predicate.scalar_expression() {
                            Some(CheckedBooleanExpression::Constant(false)) => None,
                            Some(CheckedBooleanExpression::Constant(true)) => {
                                Some(checked_trees::CrashRouteGuard::Truth)
                            }
                            _ => Some(guard.clone()),
                        }
                    }
                    _ => Some(guard.clone()),
                })
                .collect();
            checked_trees::CrashRouteBucket::new(bucket.cause(), guards)
        })
        .collect::<Vec<_>>();
    lower_checked_crash_route_buckets(&normalized, &parameters)
}

pub(crate) fn lower_checked_crash_frontier(
    frontier: &[PermissionClaimIdentity],
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<ClaimId>, LoweringError> {
    let mut lowered = frontier
        .iter()
        .map(|identity| {
            source_claims
                .iter()
                .find_map(|(source, claim)| (source == identity).then_some(*claim))
                .ok_or(LoweringError::CrashFrontierClaimNotLowered(*identity))
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

pub(crate) fn lower_checked_crash_exit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    source_claims: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<LoweredCrashExit, LoweringError> {
    use checked_trees::statement::{
        StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    };

    let program = &checked.typed;
    let source_state = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine && machine.is_valid())
        .and_then(|source_machine| {
            program
                .machine_states(source_machine)
                .iter()
                .find(|candidate| candidate.symbol == state && state.is_valid())
        })
        .ok_or(LoweringError::Unsupported(
            "explicit crash has no matching authored machine and state",
        ))?;
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(statement_ordinal as usize)
    else {
        return unsupported("explicit crash has no matching authored transition statement");
    };
    let TransitionExit::Crash(authored_cause) = transition.exit else {
        return unsupported("checked crash site names an ordinary authored transition");
    };
    // A nonzero stale target resolves through ZII to the dummy Terminal node.
    // Require arena membership before reading that node's semantic shape.
    if !program
        .statement_table
        .transition_target_is_valid(transition.target)
        || !matches!(
            program.statement_table.transition_target(transition.target),
            TransitionTargetNode::Terminal
        )
        || transition.continuation.is_valid()
        || transition.guard != TransitionGuardNode::Always
    {
        return unsupported(
            "explicit crash must retain an unconditional terminal target and no continuation",
        );
    }
    let Some(crash_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|contract| &contract.crash)
    else {
        return unsupported("explicit crash has no checked machine-contract plan");
    };
    let Some(checked_site) = crash_plan.checked_site_at(state, statement_ordinal) else {
        return unsupported("explicit crash has no body-derived checked crash-site row");
    };
    let authored_cause = match authored_cause {
        checked_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
        checked_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
    };
    if checked_site.cause() != authored_cause {
        return unsupported("checked crash cause disagrees with its authored transition");
    }
    let matching_contracts = crash_plan
        .covering_buckets_for_site(checked_site)
        .map(|(_, bucket)| bucket)
        .collect::<Vec<_>>();
    let [covering_bucket] = matching_contracts.as_slice() else {
        return unsupported(
            "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket",
        );
    };
    let site_identities = checked_site
        .path_guard_conjuncts()
        .iter()
        .chain(checked_site.path_guard_consequences())
        .collect::<BTreeSet<_>>();
    let site_guard = covering_bucket
        .alternative_guards()
        .iter()
        .filter_map(|guard| match guard {
            checked_trees::CrashRouteGuard::Truth => None,
            checked_trees::CrashRouteGuard::Predicate(predicate)
                if site_identities.contains(predicate) =>
            {
                Some(
                    predicate
                        .scalar_expression()
                        .cloned()
                        .ok_or(LoweringError::Unsupported(
                            "guarded crash site is outside structured scalar predicate lowering",
                        )),
                )
            }
            checked_trees::CrashRouteGuard::Predicate(_) => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !covering_bucket
        .alternative_guards()
        .contains(&checked_trees::CrashRouteGuard::Truth)
        && site_guard.is_empty()
    {
        return unsupported("guarded crash site has no structured covering predicate");
    }
    Ok(LoweredCrashExit {
        cause: match checked_site.cause() {
            checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
            checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
        },
        site_guard,
        frontier_lower_bound: lower_checked_crash_frontier(
            checked_site.frontier_lower_bound(),
            source_claims,
        )?,
    })
}

pub(crate) fn lower_checked_crash_route_buckets(
    buckets: &[checked_trees::CrashRouteBucket],
    parameters: &[ValueDeclaration],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        Ok(terminal_psi::CrashRouteGuard::Truth)
                    }
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let expression = predicate.scalar_expression().ok_or(
                            LoweringError::Unsupported(
                                "guarded crash route is outside structured scalar predicate lowering",
                            ),
                        )?;
                        Ok(terminal_psi::CrashRouteGuard::Predicate(
                            terminal_psi::CrashPredicateTerm::new(
                                checked_boolean_proposition(expression, parameters)?,
                            ),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(terminal_psi::CrashRouteBucket {
                cause: match bucket.cause() {
                    checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}
