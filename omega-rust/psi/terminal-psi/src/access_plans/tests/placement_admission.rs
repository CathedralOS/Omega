use super::{
    uart_extent_with_lineage, uart_placement_plan, uart_reach, uart_resource_profile,
    uart_resource_profile_for_extent,
};
use crate::access_plans::{PlacementAdmissionId, admit_owned_placement, admit_placement};
use crate::extents::LoanPolarity;

#[test]
#[allow(
    clippy::drop_non_drop,
    reason = "the test makes loan release explicit before probing restored parent access"
)]
fn borrowed_admission_withdraws_the_exact_shared_loan() {
    let plan = uart_placement_plan();
    let mut extent = uart_extent_with_lineage(0x7200, 32, 76);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let rights = extent.rights().clone();
    let provenance = extent.provenance();
    let era = extent.era();
    let loan = extent.loan(4, 12).expect("shared placement loan");
    let profile = uart_resource_profile(&loan, &uart_reach());

    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(77).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed shared placement admission");
    let returned = admission.withdraw();

    assert_eq!(returned.base(), 0x7204);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.polarity(), LoanPolarity::Shared);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
    assert_eq!(returned.address_space(), address_space);
    assert_eq!(returned.rights(), &rights);
    assert_eq!(returned.provenance(), provenance);
    assert_eq!(returned.era(), era);

    drop(returned);
    drop(
        extent
            .loan_mut(0, 32)
            .expect("dropping the returned loan restores exclusive parent access"),
    );
}

#[test]
#[allow(
    clippy::drop_non_drop,
    reason = "the test makes loan release explicit before probing restored parent access"
)]
fn borrowed_admission_withdraws_the_exact_exclusive_loan() {
    let plan = uart_placement_plan();
    let mut extent = uart_extent_with_lineage(0x7300, 32, 78);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let rights = extent.rights().clone();
    let provenance = extent.provenance();
    let era = extent.era();
    let loan = extent.loan_mut(8, 12).expect("exclusive placement loan");
    let profile = uart_resource_profile(&loan, &uart_reach());

    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(79).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed exclusive placement admission");
    let returned = admission.withdraw();

    assert_eq!(returned.base(), 0x7308);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.polarity(), LoanPolarity::Exclusive);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
    assert_eq!(returned.address_space(), address_space);
    assert_eq!(returned.rights(), &rights);
    assert_eq!(returned.provenance(), provenance);
    assert_eq!(returned.era(), era);

    drop(returned);
    drop(
        extent
            .loan(0, 32)
            .expect("dropping the returned loan restores shared parent access"),
    );
}

#[test]
fn owned_admission_retains_and_withdraws_the_exact_extent() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7000, 12, 72);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(73).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned whole-range placement admission");
    assert_eq!(admission.identity().normalized_identity(), 73);
    assert_eq!(admission.extent().base(), 0x7000);
    assert_eq!(admission.extent().length(), 12);
    assert_eq!(admission.extent().origin(), origin);
    assert_eq!(admission.extent().lineage_root(), lineage);
    assert_eq!(admission.placement_plan().identity(), plan.identity());

    let returned = admission.withdraw();
    assert_eq!(returned.base(), 0x7000);
    assert_eq!(returned.length(), 12);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
}

#[test]
fn owned_admission_rejection_returns_the_exact_extent() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7100, 8, 74);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

    let rejection = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(75).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect_err("the complete placement must fit the owned extent");
    assert!(rejection.diagnostic().0.contains("exceeds"));
    let (returned, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("exceeds"));
    assert_eq!(returned.base(), 0x7100);
    assert_eq!(returned.length(), 8);
    assert_eq!(returned.origin(), origin);
    assert_eq!(returned.lineage_root(), lineage);
}
