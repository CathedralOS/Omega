use super::{
    admit_uart, established_stable_word, expect_exact_stable_compound_rejection,
    expect_exact_stable_primitive_rejection, field_key, primitive_request_snapshot,
    provider_existing_content, stable_word_placement, stable_word_profile, uart_extent,
    uart_extent_with_lineage, uart_placement_plan, uart_reach, uart_resource_profile,
};
use crate::placement_authority::PlacementAuthorityRef;
use crate::{
    AccessOperation, BorrowPolarity, BoundaryReach, BoundaryServiceReachId, EffectiveSupplyKind,
    ObservationModel, PlacedOccurrenceId, PlacementAdmission, PlacementAdmissionId,
    PlacementPlanId, ResourceProfileReceiptId, SchemaCorrespondenceProviderId,
    SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant, StableDeviceInstanceId,
    StablePrimitiveOperation, admit_owned_placement, admit_placement, adopt_owned_stable,
    bind_schema_correspondence_to_placement, place,
};
use extents::LoanPolarity;
use extents::ResidentClaimId;

#[test]
fn established_owned_stable_shared_projection_seals_a_read_request() {
    let (plan, established) = established_stable_word(0xa400, 112, 113, 115);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable shared read")
        .into_primitive_request();

    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 115);
    assert_eq!(request.profile_receipt().normalized_identity(), 91);
    assert_eq!(
        request.effective_supply().kind(),
        EffectiveSupplyKind::Stable
    );
    assert_eq!(request.primitive_address(), 0xa400);
    assert_eq!(request.field(), "word");
    assert_eq!(request.observation(), ObservationModel::Stable);
    assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::Read);
}

#[test]
fn established_owned_stable_exclusive_projection_seals_a_write_request() {
    let (plan, mut established) = established_stable_word(0xa500, 116, 117, 119);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .write()
        .expect("Stable exclusive write")
        .into_primitive_request();

    assert_eq!(request.primitive_address(), 0xa500);
    assert_eq!(request.observation(), ObservationModel::Stable);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::Write);
}

#[test]
fn established_owned_stable_shared_projection_rejects_write() {
    let (plan, established) = established_stable_word(0xa600, 120, 121, 123);
    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");

    let rejection = projection
        .write()
        .expect_err("shared current borrow must not authorize Stable write");
    assert!(rejection.0.contains("Shared current borrow"));
    assert_eq!(established.validity_receipt().normalized_identity(), 121);
    assert_eq!(established.custody_receipt().normalized_identity(), 122);
}

#[test]
fn established_owned_read_specializes_for_stable_primitive_lowering() {
    let (plan, established) = established_stable_word(0xa700, 124, 125, 127);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");

    assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
    assert_eq!(stable.primitive_address(), 0xa700);
    assert_eq!(stable.transfer_width_bits(), 32);
    assert_eq!(stable.effect_footprint().address(), 0xa700);
    assert_eq!(stable.effect_footprint().length_bytes(), 4);
    assert_eq!(stable.logical_extent().fragments().len(), 1);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 127);
    assert_eq!(request.profile_receipt().normalized_identity(), 91);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn stable_primitive_lowering_replays_authority_without_consuming_retry() {
    let (plan, established) = established_stable_word(0xa740, 224, 225, 227);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("Stable read")
        .into_primitive_request();
    let mut stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");
    let expected = primitive_request_snapshot(&stable.request);

    stable.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = stable
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    stable.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

    stable.operation = StablePrimitiveOperation::Write;
    let diagnostic = stable
        .validate_for_lowering()
        .expect_err("outward preflight must reject specialization drift");
    assert!(diagnostic.0.contains("retained specialization"));
    stable.operation = StablePrimitiveOperation::Read;

    stable
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&stable.request), expected);
    assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
}

#[test]
fn provider_stable_preflight_requires_and_retains_exact_correspondence() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xa780, 4, 272);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(273).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Stable placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(274)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(275).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(276).expect("provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let word = view
        .project(field_key(plan.access(), "word"))
        .expect("Stable word projection");
    let request = word.read().expect("Stable read").into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable read specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(277)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(278).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(279)
            .expect("alternate provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = stable
        .into_corresponded_stable_access()
        .expect("provider/device Stable preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.stable_access().operation(),
        StablePrimitiveOperation::Read
    );
    assert_eq!(
        primitive_request_snapshot(corresponded.stable_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device Stable preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_stable_access().primitive_request()),
        expected
    );

    let ordinary_extent = uart_extent_with_lineage(0xa790, 4, 280);
    let ordinary_profile = stable_word_profile(&ordinary_extent);
    let ordinary_loan = ordinary_extent
        .loan(0, 4)
        .expect("ordinary shared Stable loan");
    let ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(281).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Stable placement admission"),
    )
    .expect("ordinary Stable view establishment");
    let ordinary_projection = ordinary
        .project(field_key(plan.access(), "word"))
        .expect("ordinary Stable projection");
    let ordinary_request = ordinary_projection
        .read()
        .expect("ordinary Stable read")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_stable_primitive_access()
        .expect("ordinary Stable specialization remains valid")
        .into_corresponded_stable_access()
        .expect_err("provider/device preflight rejects correspondence-free Stable storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_stable, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_stable.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized Stable request"
    );
    ordinary_stable
        .validate_for_lowering()
        .expect("returned correspondence-free Stable request remains usable elsewhere");
}

#[test]
fn established_owned_write_specializes_for_stable_primitive_lowering() {
    let (plan, mut established) = established_stable_word(0xa800, 128, 129, 131);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .write()
        .expect("Stable write")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("Stable write specialization");

    assert_eq!(stable.operation(), StablePrimitiveOperation::Write);
    assert_eq!(stable.primitive_address(), 0xa800);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 131);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
}

#[test]
fn established_owned_compound_mutation_specializes_with_exact_custody() {
    let (plan, mut established) = established_stable_word(0xad00, 160, 161, 163);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();
    let before = primitive_request_snapshot(&request);
    let compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");

    assert_eq!(compound.primitive_address(), 0xad00);
    assert_eq!(compound.transfer_width_bits(), 32);
    assert_eq!(compound.logical_extent().fragments().len(), 1);
    assert_eq!(compound.effect_footprint().address(), 0xad00);
    assert_eq!(compound.effect_footprint().length_bytes(), 4);
    let request = compound.into_primitive_request();
    assert_eq!(primitive_request_snapshot(&request), before);
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 163);
    assert_eq!(request.effective_supply().key(), request.key);
    assert_eq!(request.effective_supply().width_bits(), 32);
    assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
    assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    assert_eq!(request.operation(), AccessOperation::CompoundMutation);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 161);
    assert_eq!(established.custody_receipt().normalized_identity(), 162);
}

#[test]
fn stable_compound_lowering_replays_authority_without_consuming_retry() {
    let (plan, mut established) = established_stable_word(0xad10, 228, 229, 231);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();
    let mut compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");
    let expected = primitive_request_snapshot(&compound.request);

    compound.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let diagnostic = compound
        .validate_for_lowering()
        .expect_err("outward preflight must reject copied receipt drift");
    assert!(diagnostic.0.contains("retained placement authority"));
    compound.request.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

    compound.request.operation = AccessOperation::Write;
    let diagnostic = compound
        .validate_for_lowering()
        .expect_err("outward preflight must reject operation drift");
    assert!(diagnostic.0.contains("CompoundMutation"));
    compound.request.operation = AccessOperation::CompoundMutation;

    compound
        .validate_for_lowering()
        .expect("corrected carrier must remain valid for retry");
    assert_eq!(primitive_request_snapshot(&compound.request), expected);
}

#[test]
fn provider_stable_compound_preflight_requires_exact_correspondence() {
    let plan = stable_word_placement();
    let mut extent = uart_extent_with_lineage(0xad18, 4, 282);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(283).expect("admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("Stable placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(284)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(285).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(286).expect("provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let mut word = view
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable word projection");
    let request = word
        .compound_mutation()
        .expect("Stable compound mutation")
        .into_primitive_request();
    let expected = primitive_request_snapshot(&request);
    let compound = request
        .into_stable_compound_mutation_access()
        .expect("Stable compound specialization");

    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(287)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(288).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(289)
            .expect("alternate provider provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = compound
        .into_corresponded_stable_compound_access()
        .expect("provider/device compound preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        primitive_request_snapshot(corresponded.compound_access().primitive_request()),
        expected
    );

    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        diagnostic
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let diagnostic = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device compound preflight must replay placement authority");
    assert!(diagnostic.0.contains("copied plan"));
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("restored exact carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.into_compound_access().primitive_request()),
        expected
    );

    let mut ordinary_extent = uart_extent_with_lineage(0xad28, 4, 290);
    let ordinary_profile = stable_word_profile(&ordinary_extent);
    let ordinary_loan = ordinary_extent
        .loan_mut(0, 4)
        .expect("ordinary exclusive Stable loan");
    let mut ordinary = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(291).expect("ordinary admission"),
            ordinary_loan,
            &plan,
            &ordinary_profile,
        )
        .expect("ordinary Stable placement admission"),
    )
    .expect("ordinary Stable view establishment");
    let mut ordinary_projection = ordinary
        .project_mut(field_key(plan.access(), "word"))
        .expect("ordinary exclusive Stable projection");
    let ordinary_request = ordinary_projection
        .compound_mutation()
        .expect("ordinary Stable compound mutation")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_stable_compound_mutation_access()
        .expect("ordinary compound specialization remains valid")
        .into_corresponded_stable_compound_access()
        .expect_err("provider/device preflight rejects correspondence-free Stable storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_compound, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_compound.primitive_request()),
        ordinary_snapshot,
        "rejection returns the exact already-specialized compound request"
    );
    ordinary_compound
        .validate_for_lowering()
        .expect("returned correspondence-free compound request remains usable elsewhere");
}

#[test]
fn placed_field_authorization_replays_projection_authority_and_allows_retry() {
    let (plan, established) = established_stable_word(0xad20, 164, 165, 167);
    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");

    projection.plan.0 ^= 1;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied placement identity drift");
    assert!(diagnostic.0.contains("placed field authorization"));
    assert!(diagnostic.0.contains("retained authority"));
    projection.plan = plan.identity();

    projection.supply.offset = 4;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied supply-row drift");
    assert!(diagnostic.0.contains("replayed resource row"));
    projection.supply.offset = 0;

    projection.primitive_address += 4;
    let diagnostic = projection
        .read()
        .expect_err("authorization must reject copied primitive-address drift");
    assert!(
        diagnostic
            .0
            .contains("reproduce the projected primitive address")
    );
    projection.primitive_address -= 4;

    let request = projection
        .read()
        .expect("repaired projection remains authorizable")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired projection remains valid through specialization");
    assert_eq!(stable.primitive_address(), 0xad20);
    let request = stable.into_primitive_request();
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 167);
}

#[test]
fn stable_primitive_specialization_replays_exact_supply_row_and_returns_custody() {
    let (plan, established) = established_stable_word(0xad40, 168, 169, 171);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.effective_supply.key.slot ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "supply key and width");
    request.effective_supply.key = request.key;

    request.effective_supply.field.push_str("_drift");
    request = expect_exact_stable_primitive_rejection(request, "field identity");
    request.effective_supply.field = request.field.clone();

    request.effective_supply.width_bits = 64;
    request = expect_exact_stable_primitive_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.effective_supply.offset = 4;
    request = expect_exact_stable_primitive_rejection(request, "supply offset");
    request.effective_supply.offset = 0;

    request.effective_supply.alignment_bytes = 0;
    request = expect_exact_stable_primitive_rejection(request, "supply alignment");
    request.effective_supply.alignment_bytes = 4;

    request.primitive_address += 4;
    let request = expect_exact_stable_primitive_rejection(request, "supply offset");
    assert_eq!(request.admission().normalized_identity(), 171);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 169);
    assert_eq!(established.custody_receipt().normalized_identity(), 170);
}

#[test]
fn placed_authorization_and_specialization_replay_resident_content_grant() {
    let (plan, established) = established_stable_word(0xad50, 220, 221, 223);

    let (replacement_extent, replacement_content) =
        provider_existing_content(&plan, 0xad50, 4, 224, 225);
    let replacement_profile = stable_word_profile(&replacement_extent);
    let replacement_admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(223).expect("matching admission"),
        replacement_extent,
        &plan,
        &replacement_profile,
    )
    .expect("matching replacement placement");
    let replacement_dormant = adopt_owned_stable(replacement_admission, replacement_content)
        .expect("replacement resident adoption");
    let mut corrupt = replacement_dormant
        .view(PlacedOccurrenceId::from_normalized_identity(10_223).expect("matching occurrence"))
        .expect("replacement resident view");
    let (_unrelated_extent, unrelated_content) =
        provider_existing_content(&plan, 0xad50, 4, 228, 229);
    corrupt.content = unrelated_content;

    let mut projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    projection._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
    projection.resident_claim = Some(corrupt.resident_claim());
    projection.placed_occurrence = Some(corrupt.occurrence());
    let diagnostic = projection
        .read()
        .expect_err("authorization must replay resident content beyond copied identities");
    assert!(diagnostic.0.contains("resident content grant"));

    projection._authority = PlacementAuthorityRef::EstablishedOwned(&established);
    projection.resident_claim = Some(established.resident_claim());
    projection.placed_occurrence = Some(established.occurrence());
    let mut request = projection
        .read()
        .expect("repaired projection remains authorizable")
        .into_primitive_request();
    request._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
    request.resident_claim = Some(corrupt.resident_claim());
    request.placed_occurrence = Some(corrupt.occurrence());
    request = expect_exact_stable_primitive_rejection(request, "resident content grant");

    request._authority = PlacementAuthorityRef::EstablishedOwned(&established);
    request.resident_claim = Some(established.resident_claim());
    request.placed_occurrence = Some(established.occurrence());
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired resident content authority supports specialization");
    assert_eq!(stable.primitive_address(), 0xad50);
}

#[test]
fn stable_primitive_specialization_replays_descriptor_geometry_and_authorization() {
    let (plan, established) = established_stable_word(0xad60, 172, 173, 175);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.logical_extent.fragments[0].source_bit_offset ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "field descriptor");
    request.logical_extent = request.descriptor.logical_extent.clone();

    request.effect_footprint.address += 4;
    request = expect_exact_stable_primitive_rejection(request, "effect footprint");
    request.effect_footprint.address = request.primitive_address;

    request.effect_footprint.length_bytes = 8;
    request = expect_exact_stable_primitive_rejection(request, "effect footprint");
    request.effect_footprint.length_bytes = request.descriptor.effect_footprint.length_bytes;

    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_primitive_rejection(request, "does not permit Write");
    assert_eq!(request.admission().normalized_identity(), 175);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 173);
    assert_eq!(established.custody_receipt().normalized_identity(), 174);
}

#[test]
fn stable_primitive_specialization_replays_exact_placement_authority() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xad70, 4, 176);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(177).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");
    let view = place(admission).expect("Stable placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.plan.0 ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.plan = plan.identity();

    request.profile_receipt = ResourceProfileReceiptId::from_normalized_identity(
        request.profile_receipt.normalized_identity() ^ 1,
    )
    .expect("tampered nonzero profile receipt");
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.profile_receipt = profile.receipt();

    request.admission.0 ^= 1;
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.admission = admission_id;

    request.reach = BoundaryReach::from_services([
        BoundaryServiceReachId::from_normalized_identity(178).expect("reach"),
    ]);
    request = expect_exact_stable_primitive_rejection(request, "placement authority");
    request.reach = plan.reach().clone();

    request.source_loan = BorrowPolarity::Exclusive;
    request = expect_exact_stable_primitive_rejection(request, "source-loan");
    request.source_loan = BorrowPolarity::Shared;

    request.resident_claim =
        Some(ResidentClaimId::from_normalized_identity(179).expect("spurious resident claim"));
    request = expect_exact_stable_primitive_rejection(request, "resident identities");
    request.resident_claim = None;

    request.descriptor.field.push_str("_drift");
    request.field.push_str("_drift");
    request.effective_supply.field.push_str("_drift");
    let request = expect_exact_stable_primitive_rejection(request, "resource row");
    assert_eq!(request.admission(), admission_id);
}

#[test]
fn stable_primitive_specialization_rejects_coherent_authorization_rewrite() {
    let plan = stable_word_placement();
    let mut extent = uart_extent_with_lineage(0xad74, 4, 186);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(187).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");
    let view = place(admission).expect("Stable placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared projection over exclusive source loan");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.current_borrow = BorrowPolarity::Exclusive;
    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_primitive_rejection(request, "field authorization");
    assert_eq!(request.admission(), admission_id);
    assert_eq!(
        request.authorization.current_borrow(),
        BorrowPolarity::Shared
    );
    assert_eq!(request.authorization.operation(), AccessOperation::Read);
}

#[test]
fn borrowed_view_establishment_replays_profile_and_returns_admission_for_retry() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xad7c, 4, 188);
    let profile = stable_word_profile(&extent);
    let loan = extent.loan(0, 4).expect("shared Stable loan");
    let admission_id = PlacementAdmissionId::from_normalized_identity(189).expect("admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed Stable admission");

    let coincident = uart_extent_with_lineage(0xad7c, 4, 190);
    let wrong_profile = stable_word_profile(&coincident);
    assert_eq!(wrong_profile.receipt(), profile.receipt());
    let PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        loan,
    } = admission;
    let corrupt = PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: wrong_profile,
        resources,
        loan,
    };
    let rejection = place(corrupt)
        .expect_err("borrowed view establishment must replay admitted profile root facts");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("could not replay the admitted resource profile"),
        "{}",
        rejection.diagnostic()
    );
    let (returned, _) = rejection.into_parts();
    assert_eq!(returned.identity(), admission_id);
    assert_eq!(returned.profile_receipt(), profile.receipt());
    let PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile: _,
        resources,
        loan,
    } = returned;
    let repaired = PlacementAdmission {
        identity,
        placement_plan,
        profile_receipt,
        profile,
        resources,
        loan,
    };
    let mut view = place(repaired).expect("returned admission supports corrected retry");
    let retained_profile = view.profile.clone();
    let coincident = uart_extent_with_lineage(0xad7c, 4, 203);
    view.profile = stable_word_profile(&coincident);
    let diagnostic = view
        .project(field_key(plan.access(), "word"))
        .expect_err("field projection must replay retained placement authority");
    assert!(diagnostic.0.contains("field projection"));
    assert!(diagnostic.0.contains("retained placement authority"));
    assert_eq!(view.admission(), admission_id);
    assert_eq!(view.base(), 0xad7c);
    assert_eq!(view.length(), 4);
    view.profile = retained_profile;
    let projection = view
        .project(field_key(plan.access(), "word"))
        .expect("shared Stable projection");
    let request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();
    let stable = request
        .into_stable_primitive_access()
        .expect("repaired view remains valid through specialization");
    let request = stable.into_primitive_request();
    assert_eq!(request.admission(), admission_id);
    assert_eq!(request.profile_receipt(), profile_receipt);
}

#[test]
fn borrowed_view_retirement_replays_authority_and_returns_exact_loan() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0xad88, 12, 265);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(266).expect("placement admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let mut view = place(admission).expect("borrowed view establishment");
    let exact_resources = view.resources.clone();

    view.resources.fields[0].offset ^= 4;
    let rejection = view
        .retire()
        .expect_err("retirement must reject drifted resource compatibility");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("resource compatibility differs")
    );
    let (mut view, _) = rejection.into_parts();
    assert_eq!(view.admission(), admission_id);
    view.resources = exact_resources;

    let loan = view
        .retire()
        .expect("repaired view remains valid for retirement retry");
    assert_eq!(loan.origin(), origin);
    assert_eq!(loan.lineage_root(), lineage);
    assert_eq!(loan.base(), 0xad88);
    assert_eq!(loan.length(), 12);
    assert_eq!(loan.polarity(), LoanPolarity::Shared);
}

#[test]
fn established_primitive_specialization_replays_resident_identities() {
    let (plan, established) = established_stable_word(0xad78, 180, 181, 183);
    let projection = established
        .project(field_key(plan.access(), "word"))
        .expect("shared established projection");
    let mut request = projection
        .read()
        .expect("authorized Stable read")
        .into_primitive_request();

    request.resident_claim =
        Some(ResidentClaimId::from_normalized_identity(184).expect("drifting resident claim"));
    request = expect_exact_stable_primitive_rejection(request, "resident identities");
    request.resident_claim = Some(established.resident_claim());

    request.placed_occurrence =
        Some(PlacedOccurrenceId::from_normalized_identity(185).expect("drifting occurrence"));
    let request = expect_exact_stable_primitive_rejection(request, "resident identities");
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 181);
    assert_eq!(established.custody_receipt().normalized_identity(), 182);
}

#[test]
fn stable_compound_specialization_fails_closed_and_returns_exact_request() {
    let (plan, mut established) = established_stable_word(0xad80, 164, 165, 167);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let mut request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();

    request.observation = ObservationModel::External;
    request = expect_exact_stable_compound_rejection(request, "Stable observation");
    request.observation = ObservationModel::Stable;

    request.effective_supply.kind = EffectiveSupplyKind::External;
    request = expect_exact_stable_compound_rejection(request, "Stable supply");
    request.effective_supply.kind = EffectiveSupplyKind::Stable;

    request.key.slot ^= 1;
    request = expect_exact_stable_compound_rejection(request, "supply key and width");
    request.key = request.effective_supply.key;

    request.effective_supply.width_bits = 64;
    request = expect_exact_stable_compound_rejection(request, "supply key and width");
    request.effective_supply.width_bits = request.transfer_width_bits;

    request.current_borrow = BorrowPolarity::Shared;
    request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
    request.current_borrow = BorrowPolarity::Exclusive;

    request.source_loan = BorrowPolarity::Shared;
    request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
    request.source_loan = BorrowPolarity::Exclusive;

    request.operation = AccessOperation::Write;
    let request = expect_exact_stable_compound_rejection(request, "sealed CompoundMutation event");
    assert_eq!(request.admission().normalized_identity(), 167);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 165);
    assert_eq!(established.custody_receipt().normalized_identity(), 166);
}

#[test]
fn external_request_rejects_stable_specialization_and_returns_exact_request() {
    let plan = uart_placement_plan();
    let extent = uart_extent(0xb000, 12);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let admission = admit_uart(132, loan, &plan, &uart_reach()).expect("admitted shared UART view");
    let view = place(admission).expect("shared UART placed-view establishment");
    let projection = view
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    let request = projection
        .read()
        .expect("External status read")
        .into_primitive_request();

    let rejection = request
        .into_stable_primitive_access()
        .expect_err("External observation must not enter Stable lowering");
    assert!(rejection.diagnostic().0.contains("Stable observation"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Stable observation"));
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 132);
    assert_eq!(request.primitive_address(), 0xb000);
    assert_eq!(request.observation(), ObservationModel::External);
    assert_eq!(request.operation(), AccessOperation::Read);
    assert_eq!(request.source_loan(), BorrowPolarity::Shared);
}

#[test]
fn compound_request_rejects_stable_specialization_and_returns_custody() {
    let (plan, mut established) = established_stable_word(0xb100, 133, 134, 136);
    let mut projection = established
        .project_mut(field_key(plan.access(), "word"))
        .expect("exclusive Stable projection");
    let request = projection
        .compound_mutation()
        .expect("authorized Stable compound mutation")
        .into_primitive_request();

    let rejection = request
        .into_stable_primitive_access()
        .expect_err("compound mutation needs its distinct bounded lowering");
    assert!(rejection.diagnostic().0.contains("Read or Write"));
    let (request, diagnostic) = rejection.into_parts();
    assert!(diagnostic.0.contains("Read or Write"));
    assert_eq!(request.plan(), plan.identity());
    assert_eq!(request.admission().normalized_identity(), 136);
    assert_eq!(request.operation(), AccessOperation::CompoundMutation);
    drop(request);
    assert_eq!(established.validity_receipt().normalized_identity(), 134);
    assert_eq!(established.custody_receipt().normalized_identity(), 135);
}
