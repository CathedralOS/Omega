use super::{
    atomic_word_placement, atomic_word_placement_with_operations, atomic_word_profile,
    dormant_atomic_word, expect_exact_atomic_rejection, field_key, primitive_request_snapshot,
    provider_existing_content, stable_word_placement, stable_word_profile,
    uart_extent_with_lineage,
};
use crate::access_plans::{
    AtomicAccessOperation, AtomicPermissions, BorrowPolarity, DormantOwnedAtomicResident,
    ObservationModel, OwnedPlacementAdmission, PlacedOccurrenceId, PlacementAdmissionId,
    admit_owned_placement, adopt_owned_atomic, adopt_owned_stable,
};
use crate::extents::LoanPolarity;
use crate::extents::ResidentClaimId;
use language_core::atomic::MemoryOrdering;

#[test]
fn provider_existing_content_establishes_owned_stable_placement() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa000, 4, 92, 93);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let address_space = extent.address_space();
    let provenance = extent.provenance();
    let era = extent.era();
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(95).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let dormant =
        adopt_owned_stable(admission, content).expect("provider-evidenced Stable adoption");
    assert_eq!(dormant.admission().normalized_identity(), 95);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());
    assert_eq!(dormant.extent().base(), 0xa000);
    assert_eq!(dormant.extent().length(), 4);
    assert_eq!(dormant.extent().origin(), origin);
    assert_eq!(dormant.extent().lineage_root(), lineage);
    assert_eq!(dormant.extent().address_space(), address_space);
    assert_eq!(dormant.extent().provenance(), provenance);
    assert_eq!(dormant.extent().era(), era);
    assert_eq!(dormant.profile_receipt().normalized_identity(), 91);
    assert_eq!(dormant.resident_claim().normalized_identity(), 95);
    assert_eq!(dormant.validity_receipt().normalized_identity(), 93);
    assert_eq!(dormant.custody_receipt().normalized_identity(), 94);

    let established = dormant
        .view(PlacedOccurrenceId::from_normalized_identity(96).expect("placed occurrence"))
        .expect("owned resident-view establishment");
    assert_eq!(established.admission().normalized_identity(), 95);
    assert_eq!(established.placement_plan().identity(), plan.identity());
    assert_eq!(established.extent().base(), 0xa000);
    assert_eq!(established.extent().length(), 4);
    assert_eq!(established.resident_claim().normalized_identity(), 95);
    assert_eq!(established.occurrence().normalized_identity(), 96);
    assert_eq!(established.validity_receipt().normalized_identity(), 93);
    assert_eq!(established.custody_receipt().normalized_identity(), 94);
}

#[test]
fn provider_existing_content_establishes_and_retires_owned_atomic_placement() {
    let plan = atomic_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa400, 4, 301, 302);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let profile = {
        let loan = extent.loan(0, 4).expect("Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let admission_id = PlacementAdmissionId::from_normalized_identity(305).expect("admission");
    let admission = admit_owned_placement(admission_id, extent, &plan, &profile)
        .expect("owned Atomic admission");
    let dormant =
        adopt_owned_atomic(admission, content).expect("provider-evidenced Atomic adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    assert_eq!(dormant.admission(), admission_id);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());
    assert_eq!(dormant.profile_receipt().normalized_identity(), 155);
    assert_eq!(dormant.extent().origin(), origin);
    assert_eq!(dormant.extent().lineage_root(), lineage);
    assert_eq!(claim.normalized_identity(), 304);

    let first_occurrence =
        PlacedOccurrenceId::from_normalized_identity(306).expect("first Atomic occurrence");
    let established = dormant
        .view(first_occurrence)
        .expect("owned Atomic resident view");
    assert_eq!(established.admission(), admission_id);
    assert_eq!(established.occurrence(), first_occurrence);
    assert_eq!(established.resident_claim(), claim);
    assert_eq!(established.validity_receipt(), validity);
    assert_eq!(established.custody_receipt(), custody);

    {
        let projection = established
            .project(field_key(plan.access(), "head"))
            .expect("resident Atomic field projection");
        assert_eq!(projection.observation(), ObservationModel::Atomic);
        assert_eq!(projection.resident_claim(), Some(claim));
        assert_eq!(projection.placed_occurrence(), Some(first_occurrence));

        let request = projection
            .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
            .expect("resident Atomic single-attempt compare-exchange")
            .into_primitive_request();
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(first_occurrence));
        let atomic = request
            .into_atomic_primitive_access()
            .expect("resident Atomic specialization");
        assert_eq!(
            atomic.operation(),
            AtomicAccessOperation::CompareExchangeOnce {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }
        );
        atomic
            .validate_for_lowering()
            .expect("resident Atomic specialization replays exact custody");
        let request = atomic.into_primitive_request();
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(first_occurrence));
    }

    let dormant = established
        .retire_resident()
        .expect("resident-preserving Atomic retirement");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().origin(), origin);
    assert_eq!(dormant.extent().lineage_root(), lineage);

    let second_occurrence =
        PlacedOccurrenceId::from_normalized_identity(307).expect("second Atomic occurrence");
    let second = dormant
        .view(second_occurrence)
        .expect("re-view of the same Atomic resident");
    assert_eq!(second.resident_claim(), claim);
    assert_eq!(second.occurrence(), second_occurrence);
    assert_ne!(second.occurrence(), first_occurrence);
}

#[test]
fn atomic_adoption_is_observation_specific_and_returns_both_inputs() {
    let stable = stable_word_placement();
    let (extent, content) = provider_existing_content(&stable, 0xa480, 4, 311, 312);
    let origin = extent.origin();
    let claim = content.resident_claim();
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(315).expect("admission"),
        extent,
        &stable,
        &profile,
    )
    .expect("owned Stable admission");

    let rejection = adopt_owned_atomic(admission, content)
        .expect_err("Stable content cannot enter the Atomic adoption route");
    assert!(rejection.diagnostic().0.contains("Atomic adoption"));
    let (admission, content, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Atomic adoption"));
    assert_eq!(admission.extent().origin(), origin);
    assert_eq!(content.resident_claim(), claim);

    let dormant = adopt_owned_stable(admission, content)
        .expect("rejected inputs remain valid for their exact Stable route");
    assert_eq!(dormant.resident_claim(), claim);

    let atomic = atomic_word_placement();
    let (extent, content) = provider_existing_content(&atomic, 0xa4c0, 4, 316, 317);
    let atomic_claim = content.resident_claim();
    let profile = {
        let loan = extent.loan(0, 4).expect("Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(320).expect("Atomic admission"),
        extent,
        &atomic,
        &profile,
    )
    .expect("owned Atomic admission");
    let rejection = adopt_owned_stable(admission, content)
        .expect_err("Atomic content cannot enter the Stable adoption route");
    assert!(rejection.diagnostic().0.contains("Stable adoption"));
    let (admission, content, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Stable adoption"));
    assert_eq!(content.resident_claim(), atomic_claim);
    let dormant = adopt_owned_atomic(admission, content)
        .expect("rejected inputs remain valid for their exact Atomic route");
    assert_eq!(dormant.resident_claim(), atomic_claim);
}

#[test]
fn atomic_resident_lifecycle_returns_complete_carriers_on_authority_drift() {
    let plan = atomic_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa500, 4, 321, 322);
    let profile = {
        let loan = extent.loan(0, 4).expect("Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(325).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Atomic admission");
    let mut dormant = adopt_owned_atomic(admission, content).expect("Atomic adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();

    let coincident = uart_extent_with_lineage(0xa500, 4, 326);
    let wrong_profile = {
        let loan = coincident
            .loan(0, 4)
            .expect("coincident Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let retained_profile = std::mem::replace(&mut dormant.admission.profile, wrong_profile);
    let occurrence = PlacedOccurrenceId::from_normalized_identity(327).expect("Atomic occurrence");
    let rejection = dormant
        .view(occurrence)
        .expect_err("Atomic view must replay exact retained placement authority");
    assert!(rejection.diagnostic().0.contains("placement authority"));
    let (mut dormant, returned_occurrence, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("placement authority"));
    assert_eq!(returned_occurrence, occurrence);
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    dormant.admission.profile = retained_profile;

    let mut established = dormant
        .view(returned_occurrence)
        .expect("repaired Atomic resident view");
    let (_, replacement_content) = provider_existing_content(&plan, 0xa500, 4, 328, 329);
    let retained_content = std::mem::replace(&mut established.content, replacement_content);
    let rejection = established
        .retire_resident()
        .expect_err("Atomic retirement must replay exact provider content custody");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut established, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("provider content grant"));
    assert_eq!(established.occurrence(), occurrence);
    established.content = retained_content;

    {
        let projection = established
            .project(field_key(plan.access(), "head"))
            .expect("repaired resident Atomic projection");
        let mut request = projection
            .atomic_compare_exchange(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
            .expect("resident Atomic compare-exchange")
            .into_primitive_request();
        request.resident_claim = Some(
            ResidentClaimId::from_normalized_identity(331).expect("substituted resident claim"),
        );
        request = expect_exact_atomic_rejection(request, "resident identities");
        request.resident_claim = Some(claim);
        request.placed_occurrence = Some(
            PlacedOccurrenceId::from_normalized_identity(332)
                .expect("substituted placed occurrence"),
        );
        request = expect_exact_atomic_rejection(request, "resident identities");
        request.placed_occurrence = Some(occurrence);
        request
            .into_atomic_primitive_access()
            .expect("repaired Atomic request preserves its original authority")
            .validate_for_lowering()
            .expect("repaired Atomic specialization replays");
    }

    let dormant = established
        .retire_resident()
        .expect("repaired Atomic carrier retires without reminting custody");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
}

#[test]
fn borrowed_atomic_resident_views_preserve_lender_custody_and_exact_loan_polarity() {
    let plan = atomic_word_placement();
    let mut dormant = dormant_atomic_word(&plan, 0xa580, 340, 341, 344);
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    let admission = dormant.admission();
    let profile_receipt = dormant.profile_receipt();

    let shared_occurrence =
        PlacedOccurrenceId::from_normalized_identity(345).expect("shared Atomic occurrence");
    {
        let mut borrowed = dormant
            .borrow_view(shared_occurrence)
            .expect("shared borrowed Atomic resident view");
        assert_eq!(borrowed.base(), 0xa580);
        assert_eq!(borrowed.length(), 4);
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Shared);
        assert_eq!(borrowed.admission(), admission);
        assert_eq!(borrowed.profile_receipt(), profile_receipt);
        assert_eq!(borrowed.placement_plan(), &plan);
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.occurrence(), shared_occurrence);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);

        let projection = borrowed
            .project(field_key(plan.access(), "head"))
            .expect("shared borrowed Atomic projection");
        let request = projection
            .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
            .expect("shared single-attempt Atomic access")
            .into_primitive_request();
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(shared_occurrence));
        assert_eq!(
            primitive_request_snapshot(&request).authority_kind,
            "borrowed-atomic-resident"
        );
        request
            .into_atomic_primitive_access()
            .expect("shared borrowed Atomic specialization")
            .validate_for_lowering()
            .expect("shared borrowed Atomic authority replay");

        let projection = borrowed
            .project_mut(field_key(plan.access(), "head"))
            .expect("exclusive current projection over shared Atomic loan");
        let request = projection
            .atomic_compare_exchange(MemoryOrdering::GlobalOrder, MemoryOrdering::Receive)
            .expect("Atomic permission does not require exclusive source custody")
            .into_primitive_request();
        assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        request
            .into_atomic_primitive_access()
            .expect("decisive Atomic specialization")
            .validate_for_lowering()
            .expect("decisive shared-loan replay");

        borrowed
            .retire()
            .expect("shared borrowed Atomic retirement");
    }
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);

    let inert_occurrence =
        PlacedOccurrenceId::from_normalized_identity(347).expect("inert borrowed occurrence");
    let inert = dormant
        .borrow_view(inert_occurrence)
        .expect("borrowed Atomic view requires no operation or result carrier");
    assert_eq!(inert.resident_claim(), claim);
    assert_eq!(inert.occurrence(), inert_occurrence);
    inert
        .retire()
        .expect("projection-free view retires without an Atomic attempt or result");
    assert_eq!(dormant.resident_claim(), claim);

    let exclusive_occurrence =
        PlacedOccurrenceId::from_normalized_identity(346).expect("exclusive Atomic occurrence");
    {
        let borrowed = dormant
            .borrow_view_mut(exclusive_occurrence)
            .expect("exclusive borrowed Atomic resident view");
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Exclusive);
        let projection = borrowed
            .project(field_key(plan.access(), "head"))
            .expect("shared current projection over exclusive Atomic loan");
        let request = projection
            .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
            .expect("exclusive source loan retains admitted single-attempt operation")
            .into_primitive_request();
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(exclusive_occurrence));
        borrowed
            .retire()
            .expect("exclusive borrowed Atomic retirement");
    }

    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
}

#[test]
fn borrowed_atomic_resident_formation_rejects_non_atomic_lender_observation() {
    let stable = stable_word_placement();
    let (extent, content) = provider_existing_content(&stable, 0xa5a0, 4, 361, 362);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(365).expect("Stable admission"),
        extent,
        &stable,
        &profile,
    )
    .expect("owned Stable admission");

    // This private corruption fixture bypasses the observation-specific
    // adoption constructor. Borrowed Atomic formation must independently
    // replay the complete lender and reject the Stable plan before issuing a
    // loan-bearing carrier.
    let corrupted = DormantOwnedAtomicResident { admission, content };
    let occurrence = PlacedOccurrenceId::from_normalized_identity(366).expect("Atomic occurrence");
    let diagnostic = corrupted
        .borrow_view(occurrence)
        .expect_err("Stable resident authority cannot form an Atomic borrowed view");
    assert!(diagnostic.0.contains("Atomic"));
    assert!(diagnostic.0.contains("Stable"));
    assert_eq!(corrupted.extent().base(), 0xa5a0);
}

#[test]
fn borrowed_atomic_resident_views_add_no_permissions_and_recover_from_authority_drift() {
    let once_only = atomic_word_placement_with_operations(
        0x000a_701d,
        AtomicPermissions {
            compare_exchange_once: true,
            ..AtomicPermissions::default()
        },
    );
    let mut dormant = dormant_atomic_word(&once_only, 0xa5c0, 350, 351, 354);
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    let occurrence =
        PlacedOccurrenceId::from_normalized_identity(355).expect("borrowed Atomic occurrence");

    let (_, foreign_content) = provider_existing_content(&once_only, 0xa5c0, 4, 356, 357);
    let coincident = uart_extent_with_lineage(0xa5c0, 4, 358);
    let foreign_profile = {
        let loan = coincident.loan(0, 4).expect("foreign Atomic profile loan");
        atomic_word_profile(&loan)
    };
    let mut same_id_drift = once_only.clone();
    same_id_drift.layout.size = Some(8);
    assert_eq!(same_id_drift.identity(), once_only.identity());
    assert_ne!(same_id_drift, once_only);

    let exact_profile = std::mem::replace(&mut dormant.admission.profile, foreign_profile);
    let diagnostic = dormant
        .borrow_view_mut(occurrence)
        .expect_err("borrowed Atomic formation must replay the exact lender profile");
    assert!(diagnostic.0.contains("placement authority"));
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    dormant.admission.profile = exact_profile;

    let mut borrowed = dormant
        .borrow_view_mut(occurrence)
        .expect("exclusive once-only Atomic borrowed view");
    {
        let projection = borrowed
            .project_mut(field_key(once_only.access(), "head"))
            .expect("exclusive once-only projection");
        let diagnostic = projection
            .atomic_compare_exchange(MemoryOrdering::GlobalOrder, MemoryOrdering::Receive)
            .expect_err("exclusive polarity cannot add decisive permission");
        assert!(diagnostic.0.contains("does not permit"));
        projection
            .atomic_compare_exchange_once(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
            .expect("admitted single-attempt permission remains available");
    }

    let exact_plan = borrowed.replace_plan_for_test(same_id_drift);
    let diagnostic = borrowed
        .project(field_key(once_only.access(), "head"))
        .expect_err("same-ID full-plan drift must reject against the exact lender");
    assert!(diagnostic.0.contains("differs from the exact lender"));
    borrowed.replace_plan_for_test(exact_plan);

    let exact_admission = borrowed.replace_admission_for_test(
        PlacementAdmissionId::from_normalized_identity(360).expect("foreign admission"),
    );
    let rejection = borrowed
        .retire()
        .expect_err("admission substitution must preserve the full active carrier");
    assert!(rejection.diagnostic().0.contains("exact lender"));
    let (mut borrowed, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("exact lender"));
    assert_eq!(borrowed.resident_claim(), claim);
    assert_eq!(borrowed.occurrence(), occurrence);
    assert_eq!(borrowed.validity_receipt(), validity);
    assert_eq!(borrowed.custody_receipt(), custody);
    borrowed.replace_admission_for_test(exact_admission);

    let exact_content = borrowed.replace_content_for_test(&foreign_content);
    let diagnostic = borrowed
        .project(field_key(once_only.access(), "head"))
        .expect_err("cross-root content authority cannot substitute at projection");
    assert!(diagnostic.0.contains("resident content grant"));
    let rejection = borrowed
        .retire()
        .expect_err("cross-root content rejection must return the full active carrier");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut borrowed, _) = rejection.into_parts();
    borrowed.replace_content_for_test(exact_content);
    assert_eq!(borrowed.resident_claim(), claim);
    assert_eq!(borrowed.occurrence(), occurrence);
    assert_eq!(borrowed.validity_receipt(), validity);
    assert_eq!(borrowed.custody_receipt(), custody);
    borrowed
        .retire()
        .expect("corrected active carrier supports exact retirement retry");

    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    let owned = dormant
        .view(PlacedOccurrenceId::from_normalized_identity(359).expect("owned occurrence"))
        .expect("borrowed retirement leaves lender custody available");
    assert_eq!(owned.resident_claim(), claim);
}

#[test]
fn stable_adoption_replays_profile_and_returns_both_inputs_for_retry() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xad80, 4, 191, 192);
    let extent_origin = extent.origin();
    let extent_lineage = extent.lineage_root();
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(195).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let coincident = uart_extent_with_lineage(0xad80, 4, 196);
    let wrong_profile = stable_word_profile(&coincident);
    let OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        extent,
    } = admission;
    let corrupt = OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: wrong_profile,
        resources,
        extent,
    };

    let rejection = adopt_owned_stable(corrupt, content)
        .expect_err("Stable adoption must replay admitted profile root facts");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("replay the admitted resource profile"),
        "{}",
        rejection.diagnostic()
    );
    let (returned, content, _) = rejection.into_parts();
    assert_eq!(returned.extent().origin(), extent_origin);
    assert_eq!(returned.extent().lineage_root(), extent_lineage);
    assert_eq!(content.resident_claim().normalized_identity(), 194);
    assert_eq!(content.validity_receipt().normalized_identity(), 192);
    assert_eq!(content.custody_receipt().normalized_identity(), 193);

    let OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        extent,
    } = returned;
    let repaired = OwnedPlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile,
        resources,
        extent,
    };
    let dormant = adopt_owned_stable(repaired, content)
        .expect("returned admission and content remain valid for corrected retry");
    assert_eq!(dormant.admission().normalized_identity(), 195);
    assert_eq!(dormant.resident_claim().normalized_identity(), 194);
}

#[test]
fn owned_resident_lifecycle_replays_full_provider_content_grant() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xad90, 4, 204, 205);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(208).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();

    let (replacement_extent, _replacement_content) =
        provider_existing_content(&plan, 0xad90, 4, 209, 210);
    let replacement_profile = stable_word_profile(&replacement_extent);
    let replacement_admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(213).expect("replacement admission"),
        replacement_extent,
        &plan,
        &replacement_profile,
    )
    .expect("coincident replacement placement");
    let retained_admission = std::mem::replace(&mut dormant.admission, replacement_admission);

    let occurrence = PlacedOccurrenceId::from_normalized_identity(214).expect("placed occurrence");
    let rejection = dormant
        .view(occurrence)
        .expect_err("resident view must replay the complete provider content grant");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut dormant, returned_occurrence, _) = rejection.into_parts();
    assert_eq!(returned_occurrence, occurrence);
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    let replacement_admission = std::mem::replace(&mut dormant.admission, retained_admission);

    let mut established = dormant
        .view(returned_occurrence)
        .expect("repaired dormant carrier supports corrected view");
    let retained_admission = std::mem::replace(&mut established.admission, replacement_admission);
    let rejection = established
        .retire_resident()
        .expect_err("resident retirement must replay the complete provider content grant");
    assert!(rejection.diagnostic().0.contains("provider content grant"));
    let (mut established, _) = rejection.into_parts();
    assert_eq!(established.occurrence(), occurrence);
    assert_eq!(established.resident_claim(), claim);
    assert_eq!(established.validity_receipt(), validity);
    assert_eq!(established.custody_receipt(), custody);
    established.admission = retained_admission;

    let dormant = established
        .retire_resident()
        .expect("returned active carrier supports corrected retirement");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.admission().normalized_identity(), 208);
}

#[test]
fn owned_resident_view_and_retirement_preserve_claim_and_rotate_occurrence() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa080, 4, 97, 98);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(101).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    assert_eq!(claim.normalized_identity(), 100);

    let first_occurrence =
        PlacedOccurrenceId::from_normalized_identity(102).expect("first occurrence");
    let coincident = uart_extent_with_lineage(0xa080, 4, 199);
    dormant.admission.profile = stable_word_profile(&coincident);
    let rejection = dormant
        .view(first_occurrence)
        .expect_err("owned resident view must replay retained placement authority");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the retained placement authority"),
        "{}",
        rejection.diagnostic()
    );
    let (mut dormant, returned_occurrence, _) = rejection.into_parts();
    assert_eq!(returned_occurrence, first_occurrence);
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa080);
    dormant.admission.profile = profile;
    let mut first = dormant
        .view(returned_occurrence)
        .expect("first owned resident-view establishment");
    assert_eq!(first.resident_claim(), claim);
    assert_eq!(first.occurrence(), first_occurrence);
    {
        let projection = first
            .project(field_key(plan.access(), "word"))
            .expect("resident field projection");
        assert_eq!(projection.resident_claim(), Some(claim));
        assert_eq!(projection.placed_occurrence(), Some(first_occurrence));
        let access = projection.read().expect("resident Stable read");
        assert_eq!(access.resident_claim(), Some(claim));
        assert_eq!(access.placed_occurrence(), Some(first_occurrence));
        let request = access.into_primitive_request();
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(first_occurrence));
    }

    let retained_profile = first.admission.profile.clone();
    let coincident = uart_extent_with_lineage(0xa080, 4, 200);
    first.admission.profile = stable_word_profile(&coincident);
    let rejection = first
        .retire_resident()
        .expect_err("resident retirement must replay retained placement authority");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the retained placement authority"),
        "{}",
        rejection.diagnostic()
    );
    let (mut first, _) = rejection.into_parts();
    assert_eq!(first.resident_claim(), claim);
    assert_eq!(first.occurrence(), first_occurrence);
    assert_eq!(first.validity_receipt(), validity);
    assert_eq!(first.custody_receipt(), custody);
    first.admission.profile = retained_profile;
    let dormant = first
        .retire_resident()
        .expect("returned active resident supports corrected retirement");
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa080);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());

    let second_occurrence =
        PlacedOccurrenceId::from_normalized_identity(103).expect("second occurrence");
    let second = dormant
        .view(second_occurrence)
        .expect("second owned resident-view establishment");
    assert_eq!(second.resident_claim(), claim);
    assert_eq!(second.occurrence(), second_occurrence);
    assert_ne!(second.occurrence(), first_occurrence);
}

#[test]
fn borrowed_resident_views_retain_claim_receipts_and_exact_loan_polarity() {
    let plan = stable_word_placement();
    let (extent, content) = provider_existing_content(&plan, 0xa100, 4, 104, 105);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(108).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let mut dormant = adopt_owned_stable(admission, content).expect("provider resident adoption");
    let claim = dormant.resident_claim();
    let validity = dormant.validity_receipt();
    let custody = dormant.custody_receipt();
    let retained_profile = profile.clone();

    let shared_occurrence =
        PlacedOccurrenceId::from_normalized_identity(109).expect("shared occurrence");
    let coincident = uart_extent_with_lineage(0xa100, 4, 201);
    dormant.admission.profile = stable_word_profile(&coincident);
    let diagnostic = dormant
        .borrow_view(shared_occurrence)
        .expect_err("shared resident view must replay retained placement authority");
    assert!(diagnostic.0.contains("shared-view establishment"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa100);
    dormant.admission.profile = retained_profile.clone();
    {
        let mut borrowed = dormant
            .borrow_view(shared_occurrence)
            .expect("shared resident loan");
        assert_eq!(borrowed.base(), 0xa100);
        assert_eq!(borrowed.length(), 4);
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Shared);
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.occurrence(), shared_occurrence);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);

        let projection = borrowed
            .project(field_key(plan.access(), "word"))
            .expect("shared resident field projection");
        let request = projection
            .read()
            .expect("shared resident read")
            .into_primitive_request();
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(shared_occurrence));
        assert_eq!(
            primitive_request_snapshot(&request).authority_kind,
            "borrowed-resident"
        );
        drop(request);

        let mut projection = borrowed
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive projection borrow over shared resident loan");
        let diagnostic = projection
            .write()
            .expect_err("shared resident loan cannot authorize a write");
        assert!(diagnostic.0.contains("Shared source loan"));

        let coincident = uart_extent_with_lineage(0xa100, 4, 203);
        let wrong_profile = stable_word_profile(&coincident);
        let correct_profile = borrowed.replace_profile_for_test(wrong_profile);
        let rejection = borrowed
            .retire()
            .expect_err("shared resident retirement must replay exact loan authority");
        assert!(rejection.diagnostic().0.contains("retirement"));
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("retained placement authority")
        );
        let (mut borrowed, _) = rejection.into_parts();
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.occurrence(), shared_occurrence);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);
        borrowed.replace_profile_for_test(correct_profile);

        let (_coincident_extent, coincident_content) =
            provider_existing_content(&plan, 0xa100, 4, 215, 216);
        let correct_content = borrowed.replace_content_for_test(&coincident_content);
        let diagnostic = borrowed
            .project(field_key(plan.access(), "word"))
            .expect_err("borrowed projection must replay the exact resident content grant");
        assert!(diagnostic.0.contains("resident content grant"));
        let rejection = borrowed
            .retire()
            .expect_err("shared retirement must replay the exact borrowed content grant");
        assert!(rejection.diagnostic().0.contains("provider content grant"));
        let (mut borrowed, _) = rejection.into_parts();
        borrowed.replace_content_for_test(correct_content);
        assert_eq!(borrowed.resident_claim(), claim);
        assert_eq!(borrowed.validity_receipt(), validity);
        assert_eq!(borrowed.custody_receipt(), custody);
        borrowed
            .retire()
            .expect("returned shared resident carrier supports corrected retirement");
    }
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);

    let exclusive_occurrence =
        PlacedOccurrenceId::from_normalized_identity(110).expect("exclusive occurrence");
    let coincident = uart_extent_with_lineage(0xa100, 4, 202);
    dormant.admission.profile = stable_word_profile(&coincident);
    let diagnostic = dormant
        .borrow_view_mut(exclusive_occurrence)
        .expect_err("exclusive resident view must replay retained placement authority");
    assert!(diagnostic.0.contains("exclusive-view establishment"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.validity_receipt(), validity);
    assert_eq!(dormant.custody_receipt(), custody);
    assert_eq!(dormant.extent().base(), 0xa100);
    dormant.admission.profile = retained_profile;
    {
        let mut borrowed = dormant
            .borrow_view_mut(exclusive_occurrence)
            .expect("exclusive resident loan");
        assert_eq!(borrowed.loan_polarity(), LoanPolarity::Exclusive);
        let mut projection = borrowed
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive resident field projection");
        let request = projection
            .write()
            .expect("exclusive resident write")
            .into_primitive_request();
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.resident_claim(), Some(claim));
        assert_eq!(request.placed_occurrence(), Some(exclusive_occurrence));
        drop(request);
        borrowed
            .retire()
            .expect("exclusive resident retirement replays exact loan authority");
    }

    assert_eq!(dormant.resident_claim(), claim);
    assert_eq!(dormant.extent().base(), 0xa100);
    let owned_occurrence =
        PlacedOccurrenceId::from_normalized_identity(111).expect("owned occurrence");
    let owned = dormant
        .view(owned_occurrence)
        .expect("owned resident-view establishment");
    assert_eq!(owned.resident_claim(), claim);
    assert_eq!(owned.occurrence(), owned_occurrence);
}
