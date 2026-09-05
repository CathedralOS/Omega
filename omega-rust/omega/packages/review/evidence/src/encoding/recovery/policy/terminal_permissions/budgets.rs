use super::{tests::fixture, *};

fn method_elements(method: &crate::record::PackagePolicyServiceMethod) -> usize {
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
            .map(|premise| premise.subject_projections.len() + premise.establishment_routes.len())
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
            .map(|premise| premise.subject_projections.len() + premise.establishment_routes.len())
            .sum::<usize>()
        + method.calling.as_ref().map_or(
            0,
            super::super::calling_application::budgets::fixture_elements,
        )
}

#[test]
fn aggregate_element_byte_field_and_depth_limits_include_full_schema() {
    for policy in [fixture(), super::generics::generic_fixture()] {
        let bytes = policy.canonical_bytes().unwrap();
        let elements = policy.services.len()
            + policy
                .services
                .iter()
                .map(|service| {
                    service.static_parameters.len()
                        + service
                            .static_parameters
                            .iter()
                            .map(|parameter| match &parameter.kind {
                                crate::record::PackageReviewTypeParameterKind::Machine(_) => 1,
                                crate::record::PackageReviewTypeParameterKind::Proposition(
                                    signature,
                                ) => signature.parameters.len(),
                                _ => 0,
                            })
                            .sum::<usize>()
                        + service.methods.len()
                        + service.methods.iter().map(method_elements).sum::<usize>()
                        + service.permissions.len()
                        + service
                            .permissions
                            .iter()
                            .map(|permission| permission.permitted.classes().len())
                            .sum::<usize>()
                })
                .sum::<usize>();
        let limits = |bytes, field, elements, depth| {
            PackagePolicyRecoveryLimits::new(bytes, field, elements, usize::MAX, depth)
        };
        assert_eq!(
            PackagePolicyTerminalPermissions::recover_canonical(
                &bytes,
                limits(bytes.len(), usize::MAX, elements, 1)
            )
            .unwrap(),
            policy
        );
        for (limits, error) in [
            (
                limits(bytes.len() - 1, usize::MAX, elements, 1),
                Error::InputTooLarge,
            ),
            (limits(bytes.len(), 0, elements, 1), Error::FieldTooLarge),
            (
                limits(bytes.len(), usize::MAX, elements - 1, 1),
                Error::ElementLimitExceeded,
            ),
            (
                limits(bytes.len(), usize::MAX, elements, 0),
                Error::NestingLimitExceeded,
            ),
        ] {
            assert_eq!(
                PackagePolicyTerminalPermissions::recover_canonical(&bytes, limits),
                Err(error)
            );
        }
    }
}

#[test]
fn permission_class_storage_shares_exact_aggregate_owned_boundary() {
    let policy = fixture();
    let bytes = policy.canonical_bytes().unwrap();
    let recover = |owned| {
        PackagePolicyTerminalPermissions::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits::new(bytes.len(), usize::MAX, usize::MAX, owned, 128),
        )
    };
    let mut lower = 0;
    let mut upper = 64 * 1024 * 1024;
    assert!(recover(upper).is_ok());
    while lower + 1 < upper {
        let middle = lower + (upper - lower) / 2;
        match recover(middle) {
            Ok(_) => upper = middle,
            Err(Error::AllocationLimitExceeded) => lower = middle,
            Err(error) => panic!("unexpected bounded recovery error: {error:?}"),
        }
    }
    assert_eq!(recover(upper).unwrap(), policy);
    assert_eq!(recover(upper - 1), Err(Error::AllocationLimitExceeded));
    assert!(upper > bytes.len());
    let mut empty = policy;
    empty.services.clear();
    let bytes = empty.canonical_bytes().unwrap();
    let owned = bytes.len() + empty.target.identity().as_str().len();
    let limits = |owned| PackagePolicyRecoveryLimits::new(bytes.len(), usize::MAX, 0, owned, 0);
    assert_eq!(
        PackagePolicyTerminalPermissions::recover_canonical(&bytes, limits(owned)).unwrap(),
        empty
    );
    assert_eq!(
        PackagePolicyTerminalPermissions::recover_canonical(&bytes, limits(owned - 1)),
        Err(Error::AllocationLimitExceeded)
    );
}
