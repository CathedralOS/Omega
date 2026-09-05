use super::{Error, PackagePolicyRecoveryLimits, fixtures};
use crate::encoding::recovery::policy::calling_application::budgets::fixture_elements;
use crate::record::*;

#[test]
fn aggregate_byte_field_and_element_boundaries_include_every_nested_application() {
    let policy = fixtures::complete();
    let bytes = policy.canonical_bytes().unwrap();
    // The complete fixture has one availability interface argument, two
    // selected applications with one type and one trait argument each, and
    // the full calling graph (including both callback-slot applications).
    let elements = policy.declarations.len()
        + policy.producer_availability.len()
        + 1
        + 3 * policy.selected_availability.len()
        + policy.demands.len()
        + fixture_elements(&policy.demands[0].calling);
    let limits = |maximum_bytes, maximum_field_bytes, maximum_elements, maximum_owned| {
        PackagePolicyRecoveryLimits::new(
            maximum_bytes,
            maximum_field_bytes,
            maximum_elements,
            maximum_owned,
            usize::MAX,
        )
    };
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, elements, usize::MAX)
        )
        .unwrap(),
        policy
    );
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(
            &bytes,
            limits(bytes.len() - 1, usize::MAX, elements, usize::MAX)
        ),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(
            &bytes,
            limits(bytes.len(), 0, elements, usize::MAX)
        ),
        Err(Error::FieldTooLarge)
    );
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, elements - 1, usize::MAX)
        ),
        Err(Error::ElementLimitExceeded)
    );
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(
            &bytes,
            limits(bytes.len(), usize::MAX, elements, 0)
        ),
        Err(Error::AllocationLimitExceeded)
    );
}

#[test]
fn empty_and_declaration_owned_storage_include_outer_canonical_scratch() {
    let mut declarations = fixtures::empty();
    declarations.declarations = vec![fixtures::nominal("A"), fixtures::nominal("B")];
    for policy in [fixtures::empty(), declarations] {
        let bytes = policy.canonical_bytes().unwrap();
        let owned = bytes.len()
            + policy.declarations.len() * std::mem::size_of::<PackageReviewNominalIdentity>()
            + policy
                .declarations
                .iter()
                .map(|declaration| declaration.path.len())
                .sum::<usize>();
        let limits = |owned| {
            PackagePolicyRecoveryLimits::new(bytes.len(), usize::MAX, usize::MAX, owned, usize::MAX)
        };
        assert_eq!(
            PackagePolicyRepresentation::recover_canonical(&bytes, limits(owned)).unwrap(),
            policy
        );
        assert_eq!(
            PackagePolicyRepresentation::recover_canonical(&bytes, limits(owned - 1)),
            Err(Error::AllocationLimitExceeded)
        );
    }
}

#[test]
fn nested_calling_machine_contract_uses_the_outer_depth_ceiling() {
    let policy = fixtures::complete();
    let bytes = policy.canonical_bytes().unwrap();
    let limits = |depth| {
        PackagePolicyRecoveryLimits::new(bytes.len(), usize::MAX, usize::MAX, usize::MAX, depth)
    };
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(&bytes, limits(0)),
        Err(Error::NestingLimitExceeded)
    );
    assert_eq!(
        PackagePolicyRepresentation::recover_canonical(&bytes, limits(1)).unwrap(),
        policy
    );
}
