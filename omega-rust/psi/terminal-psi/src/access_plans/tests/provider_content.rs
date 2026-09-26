use super::{
    extent_id, provider_existing_content, stable_word_placement, stable_word_profile,
    uart_extent_with_lineage, uart_placement_plan, uart_reach, uart_resource_profile_for_extent,
    uart_root_grant, uart_root_grant_with_mapping,
};
use crate::access_plans::{
    AccessPlan, BoundaryReach, PlacementAdmissionId, PlacementPlan, admit_owned_placement,
    adopt_owned_stable, validate_placement_plan,
};
use crate::extents::ResidentClaimId;
use crate::extents::{ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId};

#[test]
fn provider_existing_content_cannot_replay_across_extent_roots() {
    let plan = stable_word_placement();
    let (_source_extent, content) = provider_existing_content(&plan, 0xa100, 4, 96, 97);
    let coincident = uart_extent_with_lineage(0xa100, 4, 98);
    let returned_origin = coincident.origin();
    let returned_lineage = coincident.lineage_root();
    let profile = stable_word_profile(&coincident);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(99).expect("admission"),
        coincident,
        &plan,
        &profile,
    )
    .expect("coincident root admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("existing-content authority must not replay across roots");
    assert!(rejection.diagnostic().0.contains("lineage"));
    let (admission, content, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("lineage"));
    assert_eq!(content.lineage_root().normalized_identity(), 96);
    assert_eq!(content.resident_claim().normalized_identity(), 99);
    let returned = admission.withdraw();
    assert_eq!(returned.origin(), returned_origin);
    assert_eq!(returned.lineage_root(), returned_lineage);
}

#[test]
fn provider_existing_content_cannot_replay_after_mapping_era_drift() {
    let plan = stable_word_placement();
    let (_source_extent, content) = provider_existing_content(&plan, 0xa180, 4, 108, 109);
    let drifted = uart_root_grant_with_mapping(1, 108, 5, 110)
        .mint(0xa180, 4)
        .expect("same-root geometry in a later mapping era");
    assert_eq!(content.origin(), drifted.origin());
    assert_eq!(content.lineage_root(), drifted.lineage_root());
    assert_eq!(
        (content.base(), content.length()),
        (drifted.base(), drifted.length())
    );
    assert_eq!(content.address_space(), drifted.address_space());
    assert_eq!(content.provenance(), drifted.provenance());
    assert_ne!(content.era(), drifted.era());
    let profile = stable_word_profile(&drifted);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(111).expect("admission"),
        drifted,
        &plan,
        &profile,
    )
    .expect("drifted mapping admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("existing-content authority must not replay after mapping-era drift");
    assert!(rejection.diagnostic().0.contains("mapping era"));
    let (admission, content, _) = rejection.into_parts();
    assert_eq!(content.era().normalized_identity(), 6);
    assert_eq!(admission.withdraw().era().normalized_identity(), 110);
}

#[test]
fn provider_existing_content_must_name_the_actual_placement() {
    let plan = stable_word_placement();
    let (extent, content) = uart_root_grant(1, 100)
        .mint_provider_existing_content(
            0xa200,
            4,
            crate::extents::ExtentContentInterpretation::from_sha256_commitment(
                extent_id(
                    plan.identity().compatibility_fingerprint() + 1,
                    crate::extents::ExtentContentInterpretationId::from_normalized_identity,
                ),
                plan.content_interpretation().commitment(),
            ),
            extent_id(104, ResidentClaimId::from_normalized_identity),
            extent_id(
                101,
                ExtentContentValidityReceiptId::from_normalized_identity,
            ),
            extent_id(102, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect("provider existing-content extent");
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(103).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("provider interpretation must match the actual admitted placement");
    assert!(rejection.diagnostic().0.contains("interpretation"));
}

#[test]
fn provider_content_rejects_compact_equal_structural_substitution() {
    let original = stable_word_placement();
    let layout = original.layout().clone();
    let mut substituted = validate_placement_plan(PlacementPlan {
        access: AccessPlan::inaccessible(&layout).expect("inaccessible alternate access policy"),
        layout,
        reach: BoundaryReach::default(),
    })
    .expect("structurally distinct alternate placement");
    assert_ne!(
        original.content_interpretation().commitment(),
        substituted.content_interpretation().commitment(),
        "the authoritative commitment must cover the complete access policy"
    );
    let substituted_commitment = substituted.content_interpretation().commitment();
    substituted.identity = original.identity;
    substituted.content_interpretation =
        crate::extents::ExtentContentInterpretation::from_sha256_commitment(
            original
                .content_interpretation()
                .compatibility_fingerprint(),
            substituted_commitment,
        );
    assert_eq!(
        original.identity(),
        substituted.identity(),
        "the adversarial fixture deliberately forces equal compact coordinates"
    );
    assert_eq!(
        original
            .content_interpretation()
            .compatibility_fingerprint(),
        substituted
            .content_interpretation()
            .compatibility_fingerprint(),
        "the adversarial fixture deliberately forces equal report fingerprints"
    );

    let (extent, content) = provider_existing_content(&original, 0xa240, 4, 108, 109);
    let profile = stable_word_profile(&extent);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(112).expect("admission"),
        extent,
        &substituted,
        &profile,
    )
    .expect("the alternate inaccessible placement is resource-compatible");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("compact-equal placement substitution must not inherit content authority");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("interpretation commitment"),
        "{}",
        rejection.diagnostic()
    );
}

#[test]
fn provider_content_does_not_turn_external_placement_into_stable_adoption() {
    let plan = uart_placement_plan();
    let (extent, content) = provider_existing_content(&plan, 0xa300, 12, 104, 105);
    let profile = uart_resource_profile_for_extent(&extent, &uart_reach());
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(107).expect("admission"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned External admission");

    let rejection = adopt_owned_stable(admission, content)
        .expect_err("External observation needs its distinct adopt route");
    assert!(
        rejection.diagnostic().0.contains("External")
            && rejection.diagnostic().0.contains("Stable adoption")
    );
}
