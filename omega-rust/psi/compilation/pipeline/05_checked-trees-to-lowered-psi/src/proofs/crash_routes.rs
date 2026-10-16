//! Crash-route and canonical proposition lowering.
//!
//! This file lowers crash routes, frontiers, exits and route buckets.
//! `structural_members.rs` lowers structural member paths and terms,
//! `structural_arithmetic.rs` guards structural divisors and shifts,
//! `structural_route_buckets.rs` lowers structural crash route buckets,
//! `crash_predicates.rs` lowers checked crash predicates and boolean
//! propositions and `scalar_terms.rs` lowers checked scalar terms.

mod argument_prefix;
mod crash_predicates;
mod scalar_terms;
pub(super) mod structural_arithmetic;
mod structural_members;
mod structural_route_buckets;

pub(crate) use argument_prefix::{
    captured_crash_argument, structural_crash_argument_type,
    structural_crash_route_prefix_from_type,
};
pub(crate) use crash_predicates::checked_boolean_proposition;
pub(crate) use scalar_terms::{
    checked_boolean_scalar_term, checked_scalar_term, checked_scalar_term_with_views,
    lowered_direct_scalar_term,
};
pub(crate) use structural_members::{
    lower_sequence_extent_term, lower_structural_case_membership, lower_structural_indexed_term,
    lower_structural_member_path, lower_structural_member_term,
};
pub(crate) use structural_route_buckets::{
    lower_structural_crash_predicates, lower_structural_crash_route_buckets,
    substitute_structural_crash_route_roots,
};

use super::{
    BTreeSet, CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedTrees, ClaimId,
    LoweringError, PermissionClaimIdentity, Proposition, ScalarType, TerminalCrashCause,
    ValueDeclaration, dense_identity, unsupported, value_id,
};
use crate::scalar_lowering::branch_states::prepared::LoweredCrashExit;

pub(crate) fn lower_boundary_crash_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    boundary: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
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
    // Boundary predicates observe the declaration's own structural formals.
    // The scalar-only operation contract path cannot resolve these members;
    // using provider body bindings here would change the published contract.
    lower_structural_crash_route_buckets(
        identities,
        &normalize_formal_crash_routes(buckets),
        &formal_crash_parameters(scalar_types)?,
        structural_parameters,
        structural_types,
        runtime_requirements,
    )
}

/// Lower published crash routes into a declaration-local formal namespace:
/// the scalar lane position `k` is formal `ValueId` `k + 1`, typed by
/// `scalar_types[k]`. Boundary declarations and operation-level crash
/// contracts share this telescope, so the verifier's positional substitution
/// reconstructs both from the same identities. Authored constant routes
/// normalize before lowering: `false` contributes nothing and `true` is the
/// unconditional guard.
pub(crate) fn lower_formal_crash_routes(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    buckets: &[checked_trees::CrashRouteBucket],
    scalar_types: &[ScalarType],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    lower_checked_crash_route_buckets(
        identities,
        &normalize_formal_crash_routes(buckets),
        &formal_crash_parameters(scalar_types)?,
    )
}

fn formal_crash_parameters(
    scalar_types: &[ScalarType],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    scalar_types
        .iter()
        .enumerate()
        .map(|(position, scalar_type)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(position)?),
                scalar_type: *scalar_type,
                qualifications: Default::default(),
            })
        })
        .collect()
}

fn normalize_formal_crash_routes(
    buckets: &[checked_trees::CrashRouteBucket],
) -> Vec<checked_trees::CrashRouteBucket> {
    buckets
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
        .collect()
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
                                checked_boolean_proposition(identities, expression, parameters)?,
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
