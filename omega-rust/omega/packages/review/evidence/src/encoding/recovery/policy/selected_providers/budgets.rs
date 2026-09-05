use super::{Error, PackagePolicyRecoveryLimits, fixtures};
use crate::encoding::recovery::policy::calling_application::budgets::fixture_elements;
use crate::record::*;

fn elements(policy: &PackagePolicySelectedProviders) -> usize {
    policy.plans.len()
        + policy.families.len()
        + policy
            .families
            .iter()
            .map(|family| family.coordinates.len())
            .sum::<usize>()
        + policy
            .plans
            .iter()
            .map(|plan| {
                plan.methods.len()
                    + plan.rows.len()
                    + plan.grants.len()
                    + plan
                        .methods
                        .iter()
                        .map(|method| {
                            method.signature.schema_arguments.len()
                                + method.signature.requirement_arguments.len()
                                + method.signature.requirement_lifetime_arguments.len()
                                + 2 * method.signature.static_parameters.len()
                                + method.signature.parameters.len()
                                + method.authority.service_reach.len()
                                + method.authority.synchronous_invocations.len()
                                + method.authority.progress_premises.len()
                                + method
                                    .authority
                                    .progress_premises
                                    .iter()
                                    .map(|premise| {
                                        premise.subject_projections.len()
                                            + premise.establishment_routes.len()
                                    })
                                    .sum::<usize>()
                                + method.parameter_type_identities.len()
                                + method.entry_claims.len()
                                + method.result_claims.len()
                                + method.service_reach.len()
                                + method.synchronous_invocations.len()
                                + method.termination_premises.len()
                                + method
                                    .termination_premises
                                    .iter()
                                    .map(|premise| {
                                        premise.subject_projections.len()
                                            + premise.establishment_routes.len()
                                    })
                                    .sum::<usize>()
                                + method.calling.as_ref().map_or(0, fixture_elements)
                        })
                        .sum::<usize>()
                    + plan
                        .rows
                        .iter()
                        .map(|row| {
                            row.requirement_lifetime_partition.len()
                                + row.installation_reach.as_ref().map_or(0, |reach| {
                                    reach.upper_bound.len() + reach.resolved.len()
                                })
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}

#[test]
fn aggregate_byte_element_field_and_depth_ceilings_cover_nested_calling() {
    let policy = fixtures::complete();
    let bytes = policy.canonical_bytes().unwrap();
    let limits = |bytes, fields, elements, depth| {
        PackagePolicyRecoveryLimits::new(bytes, fields, elements, usize::MAX, depth)
    };
    let count = elements(&policy);
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, count, 1)
        )
        .unwrap(),
        policy
    );
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(
            &bytes,
            limits(bytes.len() - 1, usize::MAX, count, 1)
        ),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(&bytes, limits(bytes.len(), 0, count, 1)),
        Err(Error::FieldTooLarge)
    );
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, count - 1, 1)
        ),
        Err(Error::ElementLimitExceeded)
    );
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, count, 0)
        ),
        Err(Error::NestingLimitExceeded)
    );
}

#[test]
fn empty_provider_aggregate_accounts_target_string_and_canonical_scratch() {
    let policy = fixtures::empty();
    let bytes = policy.canonical_bytes().unwrap();
    let owned = policy.target.identity().as_str().len() + bytes.len();
    let limits = |owned| PackagePolicyRecoveryLimits::new(bytes.len(), usize::MAX, 0, owned, 0);
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(&bytes, limits(owned)).unwrap(),
        policy
    );
    assert_eq!(
        PackagePolicySelectedProviders::recover_canonical(&bytes, limits(owned - 1)),
        Err(Error::AllocationLimitExceeded)
    );
}
